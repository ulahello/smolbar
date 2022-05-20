use crossbeam_channel::{bounded, RecvTimeoutError, Sender};
use dirs::config_dir;
use serde_derive::{Deserialize, Serialize};
use std::env::{self, VarError};
use std::fs;
use std::io::{stderr, Error, Write};
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
    let config = fs::read_to_string(path)?;
    let toml: Value = toml::from_str(&config)?;
    let mut bar = Smolbar {
        header: Header::default(),
        blocks: Vec::new(),
    };

    if let Some(items) = toml.as_table() {
        for (name, item) in items {
            if let Ok(block) = item.clone().try_into() {
                bar.push(block);
            }
        }
    }

    // NOTE: bar is expected to be active before passed to runtime
    runtime::start(Box::new(bar))?;

    Ok(())
}

#[derive(Debug)]
pub struct Smolbar {
    header: Header,
    blocks: Vec<Block>,
}

impl Smolbar {
    pub fn push(&mut self, block: TomlBlock) {
        self.blocks.push(Block::new(block));
    }
}

impl Bar for Smolbar {
    fn header(&self) -> Header {
        self.header
    }

    // TODO: does this need to do anything?
    fn cont(&mut self) {}
}

#[derive(Debug)]
pub struct Block {
    body: Arc<Mutex<Body>>,
    cmd: (Sender<bool>, JoinHandle<()>),
    pulse: (Sender<()>, JoinHandle<()>),
}

impl Block {
    pub fn new(toml: TomlBlock) -> Self {
        let (cmd_send, cmd_recv) = bounded(1);
        let (pulse_send, pulse_recv) = bounded(1);
        let pulse_send_cmd = cmd_send.clone();
        let body = Arc::new(Mutex::new(Body::default()));
        let body_c = body.clone();
        Self {
            body,
            cmd: (
                cmd_send,
                thread::spawn(move || {
                    let mut command = Command::new(toml.command);

                    // update the body until we are instructed to shut down
                    while cmd_recv.recv().unwrap() {
                        if let Ok(output) = command.output() {
                            let mut body = body_c.lock().unwrap();
                            dbg!(output);
                            //*body = todo!("parse command stdout and update block body");
                        }
                    }
                }),
            ),
            pulse: (
                pulse_send,
                thread::spawn(move || {
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
