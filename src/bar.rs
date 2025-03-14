// copyright (C) 2022-2023 Ula Shipman <ula.hello@mailbox.org>
// licensed under GPL-3.0-or-later

use anyhow::Context;
use serde_json::ser;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;
use tokio::task;
use tracing::{field, span, Level};

use alloc::sync::Arc;
use core::hash::{Hash as HashTrait, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::io::{stdout, BufWriter, StdoutLock, Write};
use std::path::PathBuf;

use crate::blocks::Blocks;
use crate::config::Config;
use crate::protocol::Header;
use crate::Hash;

#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum BarMsg {
    Reload,
    ShutDown,
    RefreshBlocks,
}

#[derive(Debug)]
pub struct Bar {
    header: Header,
    config_path: PathBuf,
    blocks: Blocks,

    latest_blocks_hash: Option<Hash>,
    first_header_hash: Option<Hash>,

    rx: mpsc::Receiver<BarMsg>,
    tx: mpsc::Sender<BarMsg>,

    stdout: BufWriter<StdoutLock<'static>>,

    signal_handles_created: bool,
}

impl Bar {
    /* arbitrary, but not too high. this is only 1KiB of bar messages. */
    const CHANNEL_SIZE: usize = 1024;

    pub fn new(config: Config) -> Self {
        let (tx, rx) = mpsc::channel(Self::CHANNEL_SIZE);

        let mut blocks = Blocks::new(tx.clone());
        blocks.add_all(
            config.toml.blocks.into_iter(),
            Arc::new(config.toml.body),
            Arc::new(config.command_dir),
        );

        let stdout = BufWriter::new(stdout().lock());

        Self {
            header: config.toml.header,
            config_path: config.path,
            blocks,
            latest_blocks_hash: None,
            first_header_hash: None,
            rx,
            tx: tx.clone(),
            stdout,
            signal_handles_created: true,
        }
    }

    /// Send the configured [`Header`] through standard
    /// output.
    ///
    /// # Errors
    ///
    /// Writing to standard output may fail.
    pub fn write_header(&mut self) -> anyhow::Result<()> {
        let header = self.header;
        let span = span!(
            Level::INFO,
            "bar_send_header",
            header.version = header.version,
            header.click_events = header.click_events,
            header.cont_signal = field::Empty,
            header.stop_signal = field::Empty,
        );
        for (signal, field) in [
            (header.cont_signal, "header.cont_signal"),
            (header.stop_signal, "header.stop_signal"),
        ] {
            if let Some(signal) = signal {
                span.record(field, format_args!("{signal}"));
            }
        }
        let _enter = span.enter();

        ser::to_writer(&mut self.stdout, &self.header)?;
        write!(self.stdout, "\n[")?;
        self.stdout.flush()?;

        tracing::trace!("sent header");

        if self.first_header_hash.is_none() {
            let hash = Hash::new(&self.header);
            self.first_header_hash = Some(hash);
        }

        Ok(())
    }

    pub async fn reload(&mut self) -> anyhow::Result<()> {
        let new_config =
            Config::read_from_path(&self.config_path).context("failed to reload config")?;

        if let Some(old) = self.first_header_hash {
            let new = Hash::new(&new_config.toml.header);
            if old != new {
                tracing::warn!(
                    "changes to the header will not take effect until smolbar is restarted"
                );
            }
        }

        self.blocks.remove_all().await;
        self.config_path = new_config.path;
        self.blocks.add_all(
            new_config.toml.blocks.into_iter(),
            Arc::new(new_config.toml.body),
            Arc::new(new_config.command_dir),
        );
        Ok(())
    }

    async fn shut_down(&mut self, sig_handles: &mut Vec<task::JoinHandle<()>>) {
        let span = span!(Level::INFO, "bar_shut_down");
        let _enter = span.enter();

        self.blocks.remove_all().await;

        for handle in sig_handles.drain(..) {
            handle.abort();
            crate::await_cancellable(handle).await;
        }

        tracing::trace!("shutdown complete");
    }

    pub async fn refresh_blocks(&mut self) -> anyhow::Result<()> {
        let span = span!(
            Level::INFO,
            "bar_refresh_blocks",
            num_blocks = self.blocks.len(),
        );
        let _enter = span.enter();

        // make sure we're not sending the same sequence of blocks
        let new_hash = {
            let mut hasher = DefaultHasher::new();
            for (_handle, _block_tx, body) in self.blocks.iter() {
                body.read().await.hash(&mut hasher);
            }
            Hash(hasher.finish())
        };
        if let Some(old_hash) = self.latest_blocks_hash {
            if old_hash == new_hash {
                tracing::trace!("blocks unchanged, suppressing refresh");
                return Ok(());
            }
        }

        write!(self.stdout, "[")?;
        for (idx, (_handle, _block_tx, body)) in self.blocks.iter().enumerate() {
            ser::to_writer_pretty(&mut self.stdout, &*body.read().await)?;

            // all but last block have comma
            if idx != self.blocks.len() - 1 {
                writeln!(self.stdout, ",")?;
            }
        }
        writeln!(self.stdout, "],")?;

        self.stdout.flush()?;
        tracing::trace!("sent block(s)");

        self.latest_blocks_hash = Some(new_hash);

        Ok(())
    }

    pub async fn listen(mut self) -> anyhow::Result<()> {
        async fn inner(
            span: impl Fn() -> tracing::Span,
            bar: &mut Bar,
            sig_handles: &mut Vec<task::JoinHandle<()>>,
        ) -> anyhow::Result<()> {
            while let Some(msg) = bar.rx.recv().await {
                let span = span();
                let _enter = span.enter();
                span.record("msg", format_args!("{msg:?}"));

                tracing::trace!("received message");

                match msg {
                    BarMsg::Reload => {
                        tracing::info!("reloading configuration");
                        bar.reload().await?;
                    }

                    BarMsg::ShutDown => {
                        tracing::info!("shutting down");
                        bar.shut_down(sig_handles).await;
                        break;
                    }

                    BarMsg::RefreshBlocks => {
                        tracing::trace!("refreshing blocks");
                        bar.refresh_blocks().await?;
                    }
                }
            }
            Ok(())
        }

        let span = || span!(Level::INFO, "bar_listen", msg = field::Empty);

        let mut sig_handles = self
            .signal_handles()
            .expect("signal handles must not yet be created");

        let result = inner(span, &mut self, &mut sig_handles).await;
        match result {
            Ok(()) => {}
            Err(ref err) => {
                let span = span();
                let _enter = span.enter();
                tracing::error!(
                    err = format_args!("{err}"),
                    "fatal error has occurred, shutting down"
                );
                self.shut_down(&mut sig_handles).await;
            }
        }
        result
    }
}

impl Bar {
    fn signal_handles(&mut self) -> Option<Vec<task::JoinHandle<()>>> {
        self.signal_handles_created.then(|| {
            self.signal_handles_created = true;
            let mut handles = Vec::with_capacity(2);
            let header = self.header;
            for (signum, action, signame) in [
                (
                    header.cont_signal.unwrap_or(Header::DEFAULT_CONT_SIG),
                    BarMsg::Reload,
                    "continue",
                ),
                (
                    header.stop_signal.unwrap_or(Header::DEFAULT_STOP_SIG),
                    BarMsg::ShutDown,
                    "stop",
                ),
            ] {
                let tx = self.tx.clone();
                let handle = task::spawn(async move {
                    let span = span!(
                        Level::INFO,
                        "sig_listener",
                        signum = format_args!("{signum}"),
                        signame = format_args!("{signame}")
                    );

                    let sig_kind = SignalKind::from_raw(signum.as_raw());
                    match signal(sig_kind) {
                        Ok(mut sig) => {
                            {
                                let _enter = span.enter();
                                tracing::trace!("signal is valid, listening");
                            }

                            while let Some(()) = sig.recv().await {
                                let _enter = span.enter();
                                tracing::trace!("received signal, sending {action:?} to Bar");
                                tx.send(action)
                                    .await
                                    .expect("signal handles must outlive Bar");
                            }
                        }
                        Err(err) => {
                            let _enter = span.enter();
                            if signal_hook_registry::FORBIDDEN.contains(&signum.as_raw()) {
                                tracing::warn!("signal is invalid");
                            } else {
                                tracing::error!("failed to register signal listener: {err}");
                            }
                        }
                    }
                });
                handles.push(handle);
            }
            handles
        })
    }
}
