// copyright (C) 2022-2023 Ula Shipman <ula.hello@mailbox.org>
// licensed under GPL-3.0-or-later

//! Defines a runtime bar.

use anyhow::Error;
use serde_json::ser;
use tokio::sync::{mpsc, Mutex};
use tokio::{select, task};
use tracing::{error, info, span, trace, Level};

use std::io::{stdout, BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;

use crate::block::Block;
use crate::config::{Config, TomlBlock};
use crate::protocol::Body;

/// Configured bar at runtime.
#[derive(Debug)]
pub struct Smolbar {
    config: Config,

    // blocks must never be both unlocked and None
    blocks: Arc<Mutex<Option<Vec<Block>>>>,

    cont_stop_recv: mpsc::Receiver<ContOrStop>,

    // the receiver is kept alive as long as it receives true.
    refresh_recv: mpsc::Receiver<bool>,
    refresh_send: mpsc::Sender<bool>,
}

impl Smolbar {
    /// Constructs a new, inactive [`Smolbar`], with the given configuration. If
    /// a bar has already been constructed previously, this returns [`None`].
    ///
    /// When [run](Self::run), `cont_stop_recv` will listen for [`ContOrStop`]
    /// messages. The caller should listen for continue and stop signals and
    /// send either [`Cont`](ContOrStop::Cont) or [`Stop`](ContOrStop::Stop)
    /// accordingly.
    #[allow(clippy::missing_panics_doc)]
    pub async fn new(config: Config, cont_stop_recv: mpsc::Receiver<ContOrStop>) -> Self {
        let span = span!(Level::TRACE, "bar_new");
        let _enter = span.enter();

        // initialize bar without any blocks
        let (refresh_send, refresh_recv) = mpsc::channel(1);
        let blocks = Arc::new(Mutex::new(Some(Vec::with_capacity(
            config.toml.blocks.len(),
        ))));
        let bar = Self {
            config,
            blocks,
            cont_stop_recv,
            refresh_recv,
            refresh_send,
        };

        // add blocks
        for block in &bar.config.toml.blocks {
            Self::push_block(
                &bar.blocks,
                bar.refresh_send.clone(),
                bar.config.command_dir.clone(),
                block.clone(),
                bar.config.toml.body.clone(),
            )
            .await;
        }

        bar
    }

    /// Send the configured [`Header`](crate::protocol::Header) through standard
    /// output.
    ///
    /// # Errors
    ///
    /// Writing to standard output may fail.
    fn init(&self) -> anyhow::Result<()> {
        let span = span!(
            Level::TRACE,
            "bar_init",
            header.version = self.config.toml.header.version,
            header.click_events = self.config.toml.header.click_events,
            header.cont_signal = self.config.toml.header.cont_signal,
            header.stop_signal = self.config.toml.header.stop_signal,
        );
        let _enter = span.enter();

        ser::to_writer(stdout(), &self.config.toml.header)?;
        write!(stdout(), "\n[")?;

        trace!("sent header");

        Ok(())
    }

    /// Activate and run the bar until completion. This also sends the
    /// [`Header`](crate::protocol::Header).
    ///
    /// # Errors
    ///
    /// Writing the header to `stdout` may fail.
    #[allow(clippy::missing_panics_doc)]
    pub async fn run(mut self) -> Result<(), Error> {
        // initialize header
        self.init()?;

        /* listen for refresh */
        let blocks_c = Arc::clone(&self.blocks);
        let refresh = task::spawn(async move {
            loop {
                let blocks_c = Arc::clone(&blocks_c);
                let inner = task::spawn(async move {
                    let mut stdout = BufWriter::new(stdout());

                    // continue as long as we receive true. self.refresh_send
                    // must not be dropped until it sends a halt msg.
                    while self.refresh_recv.recv().await.unwrap() {
                        select!(
                            // we may receive a halt msg while waiting to take
                            // the lock
                            msg = self.refresh_recv.recv() => {
                                // refresh sender must not be dropped until it
                                // sends a halt msg.
                                if !msg.unwrap() {
                                    break;
                                }
                            }

                            guard = blocks_c.lock() => {
                                if let Some(ref blocks) = *guard {
                                    let span = span!(
                                        Level::TRACE,
                                        "bar_refresh_loop_write",
                                        num_blocks = blocks.len(),
                                    );
                                    let _enter = span.enter();

                                    // write each json block
                                    match write!(stdout, "[") {
                                        Ok(()) => (),
                                        Err(error) => {
                                            return Err((self.refresh_recv, error.into()));
                                        }
                                    }

                                    for (i, block) in blocks.iter().enumerate() {
                                        match write!(
                                            stdout,
                                            "{}",
                                            ser::to_string_pretty(&*block.read().await)
                                                .expect("invalid body json. this is a bug.")
                                        ) {
                                            Ok(()) => (),
                                            Err(error) => {
                                                return Err((self.refresh_recv, error.into()));
                                            }
                                        }

                                        // last block doesn't have comma after it
                                        if i != blocks.len() - 1 {
                                            match writeln!(stdout, ",") {
                                                Ok(()) => (),
                                                Err(error) => {
                                                    return Err((self.refresh_recv, error.into()));
                                                }
                                            }
                                        }
                                    }

                                    match writeln!(stdout, "],") {
                                        Ok(()) => (),
                                        Err(error) => {
                                            return Err((self.refresh_recv, error.into()));
                                        }
                                    }

                                    match stdout.flush() {
                                        Ok(()) => (),
                                        Err(error) => {
                                            return Err((self.refresh_recv, error.into()));
                                        }
                                    }

                                    trace!("sent block(s)");
                                } else {
                                    unreachable!("refresh loop expects that guard is held while blocks are taken");
                                }
                            }
                        );
                    }

                    Ok::<(), (_, Error)>(())
                });

                match inner.await.unwrap() {
                    Ok(()) => {
                        break;
                    }
                    Err((recv, error)) => {
                        error!("refresh writer: {}", error);
                        self.refresh_recv = recv;
                    }
                }
            }
        });

        /* listen for cont and stop */
        loop {
            // cont_stop sender must not be dropped until it sends
            // ContOrStop::Stop.
            match self.cont_stop_recv.recv().await.unwrap() {
                ContOrStop::Cont => {
                    let span = span!(Level::TRACE, "bar_recv_cont");
                    let _enter = span.enter();

                    /* reload configuration */
                    trace!("received msg");
                    info!("reloading config");

                    // read configuration
                    match Config::read_from_path(&self.config.path) {
                        Ok(config) => {
                            self.config = config;

                            {
                                // halt all blocks
                                let mut guard = self.blocks.lock().await;
                                // self.blocks must never be both unlocked and None
                                let blocks = guard.take().unwrap();
                                for block in blocks {
                                    block.halt().await;
                                }

                                // after taking, put back Some before releasing
                                // the guard. this way, its None state is
                                // inaccessible.
                                *guard = Some(Vec::with_capacity(self.config.toml.blocks.len()));

                                trace!("halted all blocks");
                            }

                            // add new blocks
                            for block in self.config.toml.blocks {
                                Self::push_block(
                                    &self.blocks,
                                    self.refresh_send.clone(),
                                    self.config.command_dir.clone(),
                                    block,
                                    self.config.toml.body.clone(),
                                )
                                .await;
                            }

                            trace!("done reloading");
                        }

                        Err(error) => {
                            error!(
                                "reading config from '{}' failed: {}",
                                self.config.path.display(),
                                error,
                            );
                            info!("keeping current configuration");
                        }
                    }
                }

                ContOrStop::Stop => {
                    let span = span!(Level::TRACE, "bar_recv_stop");
                    let _enter = span.enter();

                    /* we received stop signal */
                    trace!("received msg");

                    // halt each block. we do this first because blocks expect
                    // self.refresh_recv to be alive.
                    let mut guard = self.blocks.lock().await;
                    // blocks must never be both unlocked and None.
                    let blocks = guard.take().unwrap();
                    for block in blocks {
                        block.halt().await;
                    }

                    trace!("halted all blocks");

                    // tell `refresh` to halt, now that all blocks are dropped
                    trace!("sending halt to refresh loop");
                    // refresh loop must not halt until it receives halt msg
                    self.refresh_send.send(false).await.unwrap();
                    trace!("waiting for refresh loop to halt");
                    // refresh loop must not panic
                    refresh.await.unwrap();

                    break;
                }
            }
        }

        Ok(())
    }

    async fn push_block(
        blocks: &Arc<Mutex<Option<Vec<Block>>>>,
        refresh_send: mpsc::Sender<bool>,
        cmd_dir: PathBuf,
        block: TomlBlock,
        global: Body,
    ) {
        let span = span!(Level::TRACE, "bar_push_block");
        let _enter = span.enter();
        if let Some(ref mut vec) = *blocks.lock().await {
            let id = vec.len() + 1;
            trace!(id, "pushed block");
            vec.push(Block::new(block, global, refresh_send, cmd_dir, id).await);
        } else {
            unreachable!("blocks must not be pushed while the inner block vector is taken");
        }
    }
}

/// Either a continue or stop signal.
///
/// See [`Smolbar::new`] for more details on how this is used.
#[derive(Debug, Clone, Copy)]
pub enum ContOrStop {
    /// Continue signal (see
    /// [`Header::cont_signal`](crate::protocol::Header::cont_signal))
    Cont,
    /// Stop signal (see
    /// [`Header::stop_signal`](crate::protocol::Header::stop_signal))
    Stop,
}
