// copyright (C) 2022-2023 Ula Shipman <ula.hello@mailbox.org>
// licensed under GPL-3.0-or-later

use anyhow::Context;
use serde_json::ser;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;
use tokio::task;
use tracing::{field, span, Level};

use alloc::sync::Arc;
use std::io::{stdout, BufWriter, StdoutLock, Write};

use crate::blocks::Blocks;
use crate::config::Config;
use crate::protocol::Header;

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
    config: Config,
    blocks: Blocks,

    rx: mpsc::Receiver<BarMsg>,
    tx: mpsc::Sender<BarMsg>,

    stdout: BufWriter<StdoutLock<'static>>,

    signal_handles_created: bool,
}

impl Bar {
    /* arbitrary, but not too high. this is only 1KiB of bar messages. */
    const CHANNEL_SIZE: usize = 1024;

    pub fn new(config: Config) -> (Self, mpsc::Sender<BarMsg>) {
        let (tx, rx) = mpsc::channel(Self::CHANNEL_SIZE);

        let mut blocks = Blocks::new(tx.clone());
        // TODO: avoid cloning
        blocks.add_all(
            config.toml.blocks.iter().cloned(),
            Arc::new(config.toml.body.clone()),
            Arc::new(config.command_dir.clone()),
        );

        let stdout = BufWriter::new(stdout().lock());

        (
            Self {
                config,
                blocks,
                rx,
                tx: tx.clone(),
                stdout,
                signal_handles_created: true,
            },
            tx,
        )
    }

    /// Send the configured [`Header`](crate::protocol::Header) through standard
    /// output.
    ///
    /// # Errors
    ///
    /// Writing to standard output may fail.
    pub fn write_header(&mut self) -> anyhow::Result<()> {
        let header = self.config.toml.header;
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

        ser::to_writer(&mut self.stdout, &self.config.toml.header)?;
        write!(self.stdout, "\n[")?;
        self.stdout.flush()?;

        tracing::trace!("sent header");

        Ok(())
    }

    pub async fn reload(&mut self) -> anyhow::Result<()> {
        let new_config =
            Config::read_from_path(&self.config.path).context("failed to load config")?;
        self.blocks.remove_all().await;
        self.config = new_config;
        // TODO: avoid cloning
        self.blocks.add_all(
            self.config.toml.blocks.iter().cloned(),
            Arc::new(self.config.toml.body.clone()),
            Arc::new(self.config.command_dir.clone()),
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
            let header = self.config.toml.header;
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
                    if let Ok(mut sig) = signal(sig_kind) {
                        {
                            let _enter = span.enter();
                            tracing::trace!("signal is valid, listening");
                        }

                        while let Some(()) = sig.recv().await {
                            let _enter = span.enter();
                            tracing::trace!("received signal, sending {action:?} to bar");
                            tx.send(action)
                                .await
                                .expect("signal handles must outlive Bar");
                        }
                    } else {
                        let _enter = span.enter();
                        // TODO: give more info
                        tracing::warn!("failed to register signal listener");
                    }
                });
                handles.push(handle);
            }
            handles
        })
    }
}
