//! Defines a runtime bar.

use log::{error, info, trace};
use serde_json::ser;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::{select, task};

use std::io::{stdout, BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;

use crate::block::Block;
use crate::config::{Config, TomlBlock};
use crate::protocol::Body;
use crate::Error;

/// Configured bar at runtime.
pub struct Smolbar {
    config: Config,
    blocks: Arc<Mutex<Option<Vec<Block>>>>,

    // sender is somewhere outside this struct. to safely drop the bar, we tell
    // that sender to halt through cont_stop_send_halt.
    cont_stop_recv: mpsc::Receiver<ContOrStop>,
    cont_stop_send_halt: broadcast::Sender<()>,

    // the receiver is kept alive as long as it receives true.
    refresh_recv: mpsc::Receiver<bool>,
    refresh_send: mpsc::Sender<bool>,
}

impl Smolbar {
    /// Constructs a new, inactive [`Smolbar`], with the given configuration.
    ///
    /// When [run](Self::run), `cont_stop_recv` will listen for [`ContOrStop`]
    /// messages. The caller should listen for continue and stop signals and
    /// send either [`Cont`](ContOrStop::Cont) or [`Stop`](ContOrStop::Stop)
    /// accordingly.
    pub async fn new(
        config: Config,
        cont_stop_recv: mpsc::Receiver<ContOrStop>,
        cont_stop_send_halt: broadcast::Sender<()>,
    ) -> Self {
        // initialize bar without any blocks
        let (refresh_send, refresh_recv) = mpsc::channel(1);
        let blocks = Arc::new(Mutex::new(Some(Vec::with_capacity(
            config.toml.blocks.len(),
        ))));
        let bar = Self {
            config,
            blocks,
            cont_stop_recv,
            cont_stop_send_halt,
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
            .await
            .unwrap();
        }

        bar
    }

    /// Send the configured [`Header`](crate::protocol::Header) through standard output.
    ///
    /// # Errors
    ///
    /// Writing to standard output may fail.
    pub fn init(&self) -> Result<(), Error> {
        ser::to_writer(stdout(), &self.config.toml.header)?;
        write!(stdout(), "\n[")?;

        trace!("sent header: {:?}", self.config.toml.header);

        Ok(())
    }

    /// Activate and run the bar until completion.
    pub async fn run(mut self) {
        /* listen for refresh */
        let blocks_c = self.blocks.clone();
        let refresh = task::spawn(async move {
            loop {
                let blocks_c = blocks_c.clone();
                let inner = task::spawn(async move {
                    let mut stdout = BufWriter::new(stdout());

                    // continue as long as receive true
                    while self.refresh_recv.recv().await.unwrap() {
                        select!(
                            // we may receive a halt msg while waiting to take
                            // the lock
                            msg = self.refresh_recv.recv() => {
                                if !msg.unwrap() {
                                    break;
                                }
                            }

                            guard = blocks_c.lock() => {
                                if let Some(ref blocks) = *guard {
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
                                            ser::to_string_pretty(&*block.read())
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

                                    trace!("bar sent {} block(s)", blocks.len());
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
                        error!("bar refresh writer: {}", error);
                        self.refresh_recv = recv;
                    }
                }
            }
        });

        /* listen for cont and stop */
        loop {
            match self.cont_stop_recv.recv().await.unwrap() {
                ContOrStop::Cont => {
                    /* reload configuration */
                    trace!("bar received cont");
                    info!("reloading config");

                    // read configuration
                    match Config::read_from_path(self.config.path.clone()) {
                        Ok(config) => {
                            self.config = config;

                            {
                                // stop all blocks
                                let mut guard = self.blocks.lock().await;
                                let blocks = guard.take().unwrap();
                                for block in blocks {
                                    block.stop().await;
                                }

                                // after taking, put back Some before releasing the guard
                                *guard = Some(Vec::with_capacity(self.config.toml.blocks.len()));

                                trace!("cont: stopped all blocks");
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
                                .await
                                .unwrap();
                            }

                            trace!("done reloading");
                        }

                        Err(error) => {
                            error!(
                                "reading config from '{}' failed: {}",
                                self.config.path.display(),
                                error
                            );
                            info!("keeping current configuration");
                        }
                    }
                }

                ContOrStop::Stop => {
                    /* we received stop signal */
                    trace!("bar received stop");

                    // stop each block. we do this first because blocks expect
                    // self.refresh_recv to be alive.
                    let mut guard = self.blocks.lock().await;
                    let blocks = guard.take().unwrap();
                    for block in blocks {
                        block.stop().await;
                    }

                    trace!("stop: stopped all blocks");

                    // tell `refresh` to halt, now that all blocks are dropped
                    trace!("stop: sending halt to refresh loop");
                    self.refresh_send.send(false).await.unwrap();
                    trace!("stop: waiting for refresh loop to halt");
                    refresh.await.unwrap();

                    // tell the senders attached to cont/stop recv to halt
                    self.cont_stop_send_halt.send(()).unwrap();

                    break;
                }
            }
        }
    }

    async fn push_block(
        blocks: &Arc<Mutex<Option<Vec<Block>>>>,
        refresh_send: mpsc::Sender<bool>,
        cmd_dir: PathBuf,
        block: TomlBlock,
        global: Body,
    ) -> Option<()> {
        if let Some(vec) = &mut *blocks.lock().await {
            trace!("pushed block with command `{}`", block.command);
            vec.push(Block::new(block, global, refresh_send, cmd_dir));
            Some(())
        } else {
            trace!("unable to push block with command `{}`", block.command);
            None
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
