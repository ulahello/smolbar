use log::{info, trace};
use serde_json::ser;
use tokio::sync::{broadcast, mpsc};
use tokio::task;

use std::io::{stdout, BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::block::Block;
use crate::config::{Config, TomlBlock};
use crate::protocol::Body;
use crate::Error;

pub struct Smolbar {
    config: Config,
    blocks: Arc<Mutex<Option<Vec<Block>>>>,

    // sender is somewhere outside this struct. to safely drop the bar, we tell
    // that sender to halt through cont_stop_send_halt.
    cont_recv: Option<mpsc::Receiver<bool>>,
    stop_recv: Option<mpsc::Receiver<bool>>,
    cont_stop_send_halt: broadcast::Sender<()>,

    // the receiver is kept alive as long as it receives true.
    refresh_recv: mpsc::Receiver<bool>,
    refresh_send: mpsc::Sender<bool>,
}

impl Smolbar {
    pub fn new(
        config: Config,
        cont_recv: Option<mpsc::Receiver<bool>>,
        stop_recv: Option<mpsc::Receiver<bool>>,
        cont_stop_send_halt: broadcast::Sender<()>,
    ) -> Result<Self, Error> {
        // initialize bar without any blocks
        let (refresh_send, refresh_recv) = mpsc::channel(1);
        let blocks = Arc::new(Mutex::new(Some(Vec::with_capacity(
            config.toml.blocks.len(),
        ))));
        let bar = Self {
            config,
            blocks,
            cont_recv,
            stop_recv,
            cont_stop_send_halt,
            refresh_recv,
            refresh_send,
        };

        // add blocks
        for block in &bar.config.toml.blocks {
            Self::push_block(
                bar.blocks.clone(),
                bar.refresh_send.clone(),
                bar.config.command_dir.clone(),
                block.clone(),
                bar.config.toml.body.clone(),
            )
            .unwrap();
        }

        Ok(bar)
    }

    pub fn init(&self) -> Result<(), Error> {
        ser::to_writer(stdout(), &self.config.toml.header)?;
        write!(stdout(), "\n[")?;

        trace!("sent header: {:?}", self.config.toml.header);

        Ok(())
    }

    pub async fn run(mut self) -> Result<(), Error> {
        /* listen for cont */
        let blocks_c = self.blocks.clone();
        let refresh_send_c = self.refresh_send.clone();
        let mut cont = None;
        if let Some(mut cont_recv) = self.cont_recv {
            cont = Some(task::spawn(async move {
                while cont_recv.recv().await.unwrap() {
                    /* reload configuration */
                    trace!("bar received cont");
                    info!("reloading config");

                    // stop all blocks
                    let mut blocks = blocks_c.lock().unwrap().take().unwrap();
                    for block in blocks.drain(..) {
                        block.stop().await;
                    }

                    trace!("stopped all blocks");

                    // read configuration
                    self.config = Config::read_from_path(self.config.path)?;

                    // reuse now-empty block vector
                    *blocks_c.lock().unwrap() = Some(blocks);

                    // add new blocks
                    for block in self.config.toml.blocks {
                        Self::push_block(
                            blocks_c.clone(),
                            refresh_send_c.clone(),
                            self.config.command_dir.clone(),
                            block,
                            self.config.toml.body.clone(),
                        )
                        .unwrap();
                    }

                    // dont hang on to unused capacity
                    if let Some(ref mut blocks) = *blocks_c.lock().unwrap() {
                        blocks.shrink_to_fit();
                    }
                }

                Ok::<(), Error>(())
            }));
        }

        /* listen for refresh */
        let blocks_c = self.blocks.clone();
        let refresh = task::spawn(async move {
            let mut stdout = BufWriter::new(stdout());

            // continue as long as receive true
            while self.refresh_recv.recv().await.unwrap() {
                if let Some(blocks) = &*blocks_c.lock().unwrap() {
                    // write each json block
                    write!(stdout, "[")?;

                    for (i, block) in blocks.iter().enumerate() {
                        write!(stdout, "{}", ser::to_string_pretty(&*block.read())?)?;

                        // last block doesn't have comma after it
                        if i != blocks.len() - 1 {
                            writeln!(stdout, ",")?;
                        }
                    }

                    writeln!(stdout, "],")?;

                    stdout.flush()?;

                    trace!("bar sent {} block(s)", blocks.len());
                }
            }

            Ok::<(), Error>(())
        });

        /* listen for stop */
        if let Some(mut stop_recv) = self.stop_recv {
            // we await this because we want to return (and end program) when we
            // recieve stop
            task::spawn(async move {
                stop_recv.recv().await.unwrap();

                /* we received stop signal */
                trace!("bar received stop");

                // stop each block. we do this first because blocks expect
                // self.refresh_recv to be alive.
                let blocks = self.blocks.lock().unwrap().take().unwrap();
                for block in blocks {
                    block.stop().await;
                }

                trace!("stopped all blocks");

                // tell `refresh` to halt, now that all blocks are dropped
                self.refresh_send.send(false).await.unwrap();
                refresh.await.unwrap().unwrap();

                // tell the senders attached to cont/stop recv to halt
                self.cont_stop_send_halt.send(()).unwrap();

                // wait for cont to stop before returning and dropping the bar
                if let Some(task) = cont {
                    task.await.unwrap().unwrap();
                }
            })
            .await
            .unwrap();
        } else {
            // if there is no valid stop signal to listen for, await the refresh
            // loop (loop not expected to break)
            refresh.await.unwrap().unwrap();
        }

        Ok(())
    }

    #[must_use]
    fn push_block(
        blocks: Arc<Mutex<Option<Vec<Block>>>>,
        refresh_send: mpsc::Sender<bool>,
        cmd_dir: PathBuf,
        block: TomlBlock,
        global: Body,
    ) -> Option<()> {
        if let Some(vec) = &mut *blocks.lock().unwrap() {
            trace!("pushed block with command `{}`", block.command);
            vec.push(Block::new(block, global, refresh_send, cmd_dir));
            Some(())
        } else {
            trace!("unable to push block with command `{}`", block.command);
            None
        }
    }
}
