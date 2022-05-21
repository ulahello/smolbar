use crossbeam_channel::{bounded, RecvTimeoutError, Sender};
use dirs::config_dir;
use serde_derive::{Deserialize, Serialize};
use serde_json::ser;
use std::env::{self, VarError};
use std::fs;
use std::io::{stderr, stdout, Error, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use toml::Value;

use smolbar::bar::Bar;
use smolbar::protocol::{Body, Header};
use smolbar::runtime;

const CONFIG_VAR: &str = "CONFIG_PATH";

fn main() {
    if let Err(err) = try_main() {
        eprintln!("fatal: {}", err);
        process::exit(1);
    }
}

fn try_main() -> Result<(), Error> {
    /* get configuration file */
    let path = match env::var(CONFIG_VAR) {
        Ok(val) => {
            writeln!(stderr(), "info: set config path to \"{}\"", val)?;
            val.into()
        }
        Err(err) => {
            let env_status = match err {
                VarError::NotPresent => "defined",
                VarError::NotUnicode(_) => "unicode",
            };

            if let Some(mut fallback) = config_dir() {
                fallback.push("smolbar");
                fallback.push("config.toml");

                writeln!(
                    stderr(),
                    "info: environment variable {} not {}, fallback to \"{}\"",
                    CONFIG_VAR,
                    env_status,
                    fallback.display()
                )?;

                fallback
            } else {
                return writeln!(
                    stderr(),
                    "info: environment variable {} not {}, fallback not available",
                    CONFIG_VAR,
                    env_status
                );
            }
        }
    };

    /* read bar from config */
    let bar = Smolbar::new(path, Header::default())?;

    // NOTE: bar is expected to be active before passed to runtime
    runtime::start(Box::new(bar))?;

    Ok(())
}

#[derive(Debug)]
pub struct Smolbar {
    config: PathBuf,
    header: Header,
    blocks: Arc<Mutex<Vec<Block>>>,
    listen: (Sender<bool>, JoinHandle<Result<(), Error>>),
}

impl Smolbar {
    pub fn new(config: PathBuf, header: Header) -> Result<Self, Error> {
        /* start writing json */
        ser::to_writer(stdout(), &header)?;
        write!(stdout(), "\n[")?;

        /* read config */
        let (sender, receiver) = bounded(1);
        let blocks = Arc::new(Mutex::new(Self::read_blocks(&config, sender.clone())?));
        let blocks_c = blocks.clone();

        /* initialize with listener */
        Ok(Self {
            config,
            header,
            blocks: blocks,
            listen: (
                sender,
                thread::spawn(move || {
                    Ok(loop {
                        // wait for refresh ping
                        if receiver.recv().unwrap() {
                            // write each json block
                            write!(stdout(), "[")?;

                            let blocks = blocks_c.lock().unwrap();
                            for (i, block) in blocks.iter().enumerate() {
                                ser::to_writer_pretty(stdout(), &block.read())?;

                                // last block doesn't have comma after it
                                if i != blocks.len() - 1 {
                                    writeln!(stdout(), ",")?;
                                }
                            }

                            writeln!(stdout(), "],")?;
                        } else {
			    // we received the shutdown signal
                            break;
                        }
                    })
                }),
            ),
        })
    }

    pub fn push(&mut self, block: TomlBlock) {
        self.blocks
            .lock()
            .unwrap()
            .push(Block::new(block, self.listen.0.clone()));
    }

    fn read_blocks(path: &Path, bar_refresh: Sender<bool>) -> Result<Vec<Block>, Error> {
        let config = fs::read_to_string(path)?;
        let toml: Value = toml::from_str(&config)?;
        let mut blocks = Vec::new();

        if let Some(items) = toml.as_table() {
            for (name, item) in items {
                // TODO: clone?
                if let Ok(block) = item.clone().try_into() {
                    blocks.push(Block::new(block, bar_refresh.clone()));
                }
            }
        }

        Ok(blocks)
    }
}

impl Bar for Smolbar {
    fn header(&self) -> Header {
        self.header
    }

    fn cont(&mut self) {
        // reload the config file
        if let Ok(blocks) = Self::read_blocks(&self.config, self.listen.0.clone()) {
            self.blocks = Arc::new(Mutex::new(blocks));
        }
    }
}

impl Drop for Smolbar {
    fn drop(&mut self) {
        self.listen.0.send(false).unwrap();
    }
}

#[derive(Debug)]
pub struct Block {
    body: Arc<Mutex<Body>>,
    cmd: (Sender<bool>, JoinHandle<()>),
    pulse: (Sender<()>, JoinHandle<()>),
}

impl Block {
    pub fn new(toml: TomlBlock, bar_refresh: Sender<bool>) -> Self {
        let (cmd_send, cmd_recv) = bounded(1);
        let (pulse_send, pulse_recv) = bounded(1);
        let pulse_send_cmd = cmd_send.clone();
        let body = Arc::new(Mutex::new(Body::new()));
        let body_c = body.clone();
        Self {
            body,
            cmd: (
                cmd_send,
                thread::spawn(move || {
                    let mut command = Command::new(toml.command);

                    // continue until instructed to shut down
                    while cmd_recv.recv().unwrap() {
                        // run command and capture output for Body
			// TODO: run output in the config path
                        if let Ok(output) = command.output() {
                            let mut body = body_c.lock().unwrap();

                            // refresh block body
                            if let Ok(utf8) = String::from_utf8(output.stdout) {
                                let mut lines = utf8.lines();

                                body.full_text = lines.next().map(|s| s.to_string());
                                body.short_text = lines.next().map(|s| s.to_string());
                                body.color = lines.next().map(|s| s.to_string());
                                body.background = lines.next().map(|s| s.to_string());
                                body.border = lines.next().map(|s| s.to_string());
                                //body.border_top = lines.next().map(|s| s.to_string());
                                //body.border_bottom = lines.next().map(|s| s.to_string());
                                //body.border_left = lines.next().map(|s| s.to_string());
                                //body.border_right = lines.next().map(|s| s.to_string());
                                body.min_width = lines.next().map(|s| s.to_string());
                                //body.align = lines.next().map(|s| s.to_string());
                                body.name = lines.next().map(|s| s.to_string());
                                body.instance = lines.next().map(|s| s.to_string());
                                //body.urgent = lines.next().map(|s| s.to_string());
                                //body.separator = lines.next().map(|s| s.to_string());
                                //body.separator_block_width = lines.next().map(|s| s.to_string());
                                //body.markup = lines.next().map(|s| s.to_string());
                            }

                            // ping parent bar to let know we are refreshed
                            bar_refresh.send(true).unwrap();
                        }
                    }
                }),
            ),
            pulse: (
                pulse_send,
                thread::spawn(move || {
		    // TODO: update on signal
                    if let Some(interval) = toml.interval {
                        let interval = Duration::from_secs(interval.into());
                        // update the body at the interval
                        loop {
                            if let Err(stay_alive) = pulse_recv.recv_timeout(interval) {
                                match stay_alive {
                                    RecvTimeoutError::Timeout => pulse_send_cmd.send(true).unwrap(),
                                    RecvTimeoutError::Disconnected => todo!("good error message"),
                                }
                            } else {
                                // we received a signal to shut down
                                break;
                            }
                        }
                    } else {
                        // only update the body once
                        pulse_send_cmd.send(true).unwrap();
                    }
                }),
            ),
        }
    }

    fn read(&self) -> Body {
        // TODO: could this return a mutexguard?
        self.body.lock().unwrap().clone()
    }

    fn stop(&mut self) {
	// TODO: race conditions
        self.cmd.0.send(false).unwrap();
        self.pulse.0.send(()).unwrap();
    }
}

impl Drop for Block {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TomlBlock {
    command: String,
    interval: Option<u32>,
    signal: Option<i32>,
}
