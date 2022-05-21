use crossbeam_channel::{bounded, RecvTimeoutError, Sender};
use dirs::config_dir;
use serde_derive::{Deserialize, Serialize};
use serde_json::ser;
use signal_hook::consts;
use signal_hook::iterator::Signals;
use toml::Value;

use std::env::{self, VarError};
use std::fs;
use std::io::{stderr, stdout, Error, Write};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

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
    let bar = Smolbar::new(path, Header::default(), true)?;

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
    pub fn new(config: PathBuf, header: Header, send_header: bool) -> Result<Self, Error> {
        /* start writing json */
        if send_header {
            ser::to_writer(stdout(), &header)?;
            write!(stdout(), "\n[")?;
        }

        /* read config */
        let (sender, receiver) = bounded(1);
        let blocks = Arc::new(Mutex::new(Self::read_blocks(
            config.clone(),
            sender.clone(),
        )?));
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
                                ser::to_writer_pretty(stdout(), block.read().deref())?;

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

    pub fn push(&mut self, config: PathBuf, block: TomlBlock) {
        self.blocks
            .lock()
            .unwrap()
            .push(Block::new(block, config, self.listen.0.clone()));
    }

    fn read_blocks(path: PathBuf, bar_refresh: Sender<bool>) -> Result<Vec<Block>, Error> {
        let config = fs::read_to_string(path.clone())?;
        let toml: Value = toml::from_str(&config)?;
        let mut blocks = Vec::new();

        match toml {
            Value::Table(items) => {
                for (name, item) in items {
                    if let Ok(block) = item.try_into() {
                        blocks.push(Block::new(block, path.clone(), bar_refresh.clone()));
                    }
                }
            }
            _ => (),
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
        if let Ok(new) = Self::new(self.config.clone(), self.header, false) {
            while let Some(block) = self.blocks.lock().unwrap().pop() {
                block.stop();
            }

            *self = new;
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
    signal: (Sender<()>, JoinHandle<Result<(), Error>>),
}

impl Block {
    pub fn new(toml: TomlBlock, cmd_dir: PathBuf, bar_refresh: Sender<bool>) -> Self {
        let (cmd_send, cmd_recv) = bounded(1);
        let (pulse_send, pulse_recv) = bounded(1);
        let (signal_send, signal_recv) = bounded(1);

        let pulse_send_cmd = cmd_send.clone();
        let signal_send_cmd = cmd_send.clone();

        let body = Arc::new(Mutex::new(Body::new()));
        let body_c = body.clone();

        Self {
            body,
            cmd: (
                cmd_send,
                // NOTE: this thread expects the main thread to be alive
                thread::spawn(move || {
                    let mut command = Command::new(toml.command);
                    command.current_dir(cmd_dir);

                    // continue until instructed to shut down
                    while cmd_recv.recv().unwrap() {
                        // run command and capture output for Body
                        // TODO: parse command string to pass args
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
                // NOTE: this thread expects the main thread and command thread to be alive
                thread::spawn(move || {
                    if let Some(interval) = toml.interval {
                        let interval = Duration::from_secs(interval.into());
                        // update the body at the interval
                        loop {
                            if let Err(stay_alive) = pulse_recv.recv_timeout(interval) {
                                match stay_alive {
                                    RecvTimeoutError::Timeout => pulse_send_cmd.send(true).unwrap(),
                                    RecvTimeoutError::Disconnected => {
                                        panic!("pulse shutdown channel disconnected");
                                    }
                                }
                            } else {
                                // we received a signal to shut down
                                break;
                            }
                        }
                    } else {
                        // only update the body once
                        pulse_send_cmd.send(true).unwrap();

                        // wait for the shutdown signal to shut down
                        pulse_recv.recv().unwrap();
                    }
                }),
            ),

            signal: (
                signal_send,
                // NOTE: this thread expects the main thread and command thread to be alive
                thread::spawn(move || {
                    // TODO: is there a way to listen for a signal on a timeout?
                    const SHUTDOWN_CHECK: Duration = Duration::from_millis(50);

                    // if signal is set, wait on it
                    if let Some(signal) = toml.signal {
                        if !consts::FORBIDDEN.contains(&signal) {
                            let mut signals = Signals::new(&[signal])?;

                            loop {
                                // check for shutdown signal
                                if let Err(stay_alive) = signal_recv.recv_timeout(SHUTDOWN_CHECK) {
                                    match stay_alive {
                                        RecvTimeoutError::Timeout => (),
                                        RecvTimeoutError::Disconnected => {
                                            panic!("signal shutdown channel disconnected");
                                        }
                                    }
                                } else {
                                    // we received the signal to shut down
                                    break;
                                }

                                // only iterates when there are received signals
                                for incoming in signals.pending() {
                                    let incoming = incoming as i32;
                                    if incoming == signal {
                                        // refresh the block
                                        signal_send_cmd.send(true).unwrap();
                                    } else {
                                        unreachable!();
                                    }
                                }
                            }
                        }
                    } else {
                        // otherwise, wait for the shutdown signal
                        signal_recv.recv().unwrap();
                    }

                    Ok(())
                }),
            ),
        }
    }

    fn read(&self) -> MutexGuard<Body> {
        self.body.lock().unwrap()
    }

    fn stop(self) {
        self.pulse.0.send(()).unwrap();
        self.signal.0.send(()).unwrap();

        self.pulse.1.join().unwrap();
        self.signal.1.join().unwrap().unwrap();

        // only shut down command thread once pulse and signal are done
        self.cmd.0.send(false).unwrap();
        self.cmd.1.join().unwrap();
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TomlBlock {
    command: String,
    interval: Option<u32>,
    signal: Option<i32>,
}
