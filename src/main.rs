use dirs::config_dir;
use serde_derive::{Deserialize, Serialize};
use serde_json::ser;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;
use tokio::task::{self, JoinHandle};
use tokio::time;
use toml::Value;

use std::env::{self, VarError};
use std::fs;
use std::io::{stderr, stdout, Error, Write};
use std::path::PathBuf;
use std::process;
use std::process::Command;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use smolbar::protocol::{Body, Header};

const CONFIG_VAR: &str = "CONFIG_PATH";

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(err) = try_main().await {
        eprintln!("fatal: {}", err);
        process::exit(1);
    }
}

async fn try_main() -> Result<(), Error> {
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

    /* prepare to send continue and stop msgs to bar */
    // NOTE: signals may be forbidden, so a channel may not always be possible (hence option)
    let mut cont_recv = None;
    let mut stop_recv = None;

    let header = Header::default();
    for (sig, recv) in [
        (header.cont_signal, &mut cont_recv),
        (header.stop_signal, &mut stop_recv),
    ] {
        let sig = SignalKind::from_raw(sig);

        if let Ok(mut stream) = signal(sig) {
            // stream is ok, so make channel
            let channel = mpsc::channel(1);
            let send = channel.0;
            *recv = Some(channel.1);

            task::spawn(async move {
                loop {
                    stream.recv().await;
                    send.send(()).await.unwrap();
                }
            });
        }
    }

    /* read bar from config */
    let bar = Smolbar::new(path, header, cont_recv, stop_recv)?;

    /* start printing and updating blocks */
    bar.init()?;
    bar.run().await?;

    Ok(())
}

pub struct Smolbar {
    config_path: PathBuf,
    cmd_dir: PathBuf,
    header: Header,
    blocks: Vec<Block>,
    cont_recv: Option<mpsc::Receiver<()>>,
    stop_recv: Option<mpsc::Receiver<()>>,
    refresh_recv: mpsc::Receiver<()>,
    refresh_send: mpsc::Sender<()>,
}

impl Smolbar {
    pub fn new(
        config: PathBuf,
        header: Header,
        cont_recv: Option<mpsc::Receiver<()>>,
        stop_recv: Option<mpsc::Receiver<()>>,
    ) -> Result<Self, Error> {
        // initialize bar without any blocks
        let (refresh_send, refresh_recv) = mpsc::channel(1);
        let (toml_blocks, cmd_dir) = Self::read_config(config.clone())?;
        let mut bar = Self {
            config_path: config,
            cmd_dir,
            header,
            blocks: Vec::with_capacity(toml_blocks.len()),
            cont_recv,
            stop_recv,
            refresh_recv,
            refresh_send,
        };

        // add blocks
        for block in toml_blocks {
            bar.push_block(block);
        }

        Ok(bar)
    }

    pub fn init(&self) -> Result<(), Error> {
        ser::to_writer(stdout(), &self.header)?;
        write!(stdout(), "\n[")?;

        Ok(())
    }

    pub async fn run(mut self) -> Result<(), Error> {
        /* listen for cont */
        if let Some(mut cont_recv) = self.cont_recv {
            task::spawn(async move {
                loop {
                    cont_recv.recv().await.unwrap();
                    todo!("cont");
                }
            });
        }

        /* listen for refresh */
        let refresh = task::spawn(async move {
            loop {
                // wait for refresh signal
                self.refresh_recv.recv().await.unwrap();

                // write each json block
                write!(stdout(), "[")?;

                for (i, block) in self.blocks.iter().enumerate() {
                    ser::to_writer_pretty(stdout(), &*block.read())?;

                    // last block doesn't have comma after it
                    if i != self.blocks.len() - 1 {
                        writeln!(stdout(), ",")?;
                    }
                }

                writeln!(stdout(), "],")?;
            }

            Ok::<(), Error>(())
        });

        /* listen for stop */
        if let Some(mut stop_recv) = self.stop_recv {
            // we await this because we want to return (and end program) when we
            // recieve stop
            task::spawn(async move {
                loop {
                    stop_recv.recv().await.unwrap();
                    todo!("stop");
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

    fn read_config(path: PathBuf) -> Result<(Vec<TomlBlock>, PathBuf), Error> {
        let config = fs::read_to_string(path.clone())?;
        let toml: Value = toml::from_str(&config)?;
        let mut blocks = Vec::new();

        // cmd_dir is either the config's parent path or whatever is specified
        // in toml
        let mut cmd_dir: PathBuf = path.parent().unwrap_or(&path).to_path_buf();
        if let Some(val) = toml.get("command_dir") {
            if let Value::String(dir) = val {
                // if the toml cmd_dir is relative, its appended to the config
                // path parent
                cmd_dir.push(PathBuf::from(dir));
            }
        }

        if let Value::Table(items) = toml {
            for (name, item) in items {
                if let Ok(block) = item.try_into() {
                    blocks.push(block);
                }
            }
        }

        Ok((blocks, cmd_dir))
    }

    fn push_block(&mut self, block: TomlBlock) {
        self.blocks.push(Block::new(
            block,
            self.refresh_send.clone(),
            // TODO: way to not clone path?
            self.cmd_dir.clone(),
        ));
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TomlBlock {
    command: String,
    interval: Option<u32>,
    signal: Option<i32>,

    prefix: Option<String>,
    color: Option<String>,
    separator: Option<bool>,
}

pub struct Block {
    body: Arc<Mutex<Body>>,

    // `cmd` is responsible for sending refresh msgs to the bar.
    // it continues as long as it receives `true`, then it halts.
    cmd: (mpsc::Sender<bool>, JoinHandle<()>),

    // `interval` sends a refresh to `cmd` on an interval. if it receives `()`, it
    // halts.
    interval: (mpsc::Sender<()>, JoinHandle<()>),

    // `signal` sends a refresh to `cmd` any time it receives a certain os
    // signal. if it receives `()`, it halts.
    signal: (mpsc::Sender<()>, JoinHandle<()>),
}

impl Block {
    pub fn new(toml: TomlBlock, bar_refresh: mpsc::Sender<()>, cmd_dir: PathBuf) -> Self {
        let body = Arc::new(Mutex::new(Body::new()));

        let (sig_send, mut sig_recv) = mpsc::channel(1);
        let (interval_send, mut interval_recv) = mpsc::channel(1);
        let (cmd_send, mut cmd_recv) = mpsc::channel(1);

        /* listen for body refresh msgs and fulfill them */
        let color = toml.color;
        let body_c = body.clone();
        let cmd = (
            cmd_send.clone(),
            task::spawn(async move {
                while cmd_recv.recv().await.unwrap() {
                    let mut command = Command::new(&toml.command);
                    // TODO: this is breaking PATH
                    command.current_dir(&cmd_dir);

                    // run command and capture output for Body
                    if let Ok(output) = task::spawn_blocking(move || command.output())
                        .await
                        .unwrap()
                    {
                        // refresh block body
                        if let Ok(utf8) = String::from_utf8(output.stdout) {
                            let mut lines = utf8.lines();

                            {
                                let mut body = body_c.lock().unwrap();

                                /* configure according to command output */
                                body.full_text = lines.next().map(|s| {
                                    format!("{}{}", toml.prefix.clone().unwrap_or("".into()), s)
                                });
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
                                body.separator = match lines.next() {
                                    Some(s) => match s.to_lowercase().as_ref() {
                                        "true" => Some(true),
                                        "false" => Some(false),
                                        _ => None,
                                    },
                                    None => None,
                                };
                                //body.separator_block_width = lines.next().map(|s| s.to_string());
                                //body.markup = lines.next().map(|s| s.to_string());

                                /* fall back to block's toml config */
                                if body.color.is_none() {
                                    body.color = color.clone();
                                }

                                if body.separator.is_none() {
                                    body.separator = toml.separator;
                                }
                            }

                            // ping parent bar to let know we are refreshed
                            bar_refresh.send(()).await.unwrap();
                        }
                    }
                }
            }),
        );

        /* refresh on an interval */
        let cmd_send_c = cmd_send.clone();
        let toml_interval = toml.interval;
        let interval = (
            interval_send,
            task::spawn(async move {
                if let Some(timeout) = toml_interval {
                    let timeout = Duration::from_secs(timeout.into());

                    loop {
                        match time::timeout(timeout, interval_recv.recv()).await {
                            Ok(halt) => {
                                halt.unwrap();
                                // we received halt msg
                                break;
                            }
                            Err(_) => {
                                // receiving halt msg timed out, so we refresh
                                // the body. this creates the behavior of
                                // refreshing the body at a specific interval
                                // until halting
                                cmd_send_c.send(true).await.unwrap();
                            }
                        }
                    }
                }
            }),
        );

        /* update body on a signal */
        let cmd_send_c = cmd_send.clone();
        let signal = (
            sig_send,
            task::spawn(async move {
                if let Some(sig) = toml.signal {
                    let sig = SignalKind::from_raw(sig);
                    if let Ok(mut stream) = signal(sig) {
                        loop {
                            select!(
                                halt = sig_recv.recv() => {
                                    halt.unwrap();
                                    break;
                                }
                                sig = stream.recv() => {
                                    sig.unwrap();
                                    cmd_send_c.send(true).await.unwrap();
                                }
                            );
                        }
                    }
                }
            }),
        );

        /* initialize block */
        task::spawn(async move {
            cmd_send.send(true).await.unwrap();
        });

        Self {
            body,
            cmd,
            interval,
            signal,
        }
    }

    pub fn read(&self) -> MutexGuard<Body> {
        self.body.lock().unwrap()
    }

    pub async fn stop(mut self) {
        // halt interval and signal tasks
        task::spawn(async move { self.interval.0.send(()).await.unwrap() });
        task::spawn(async move { self.signal.0.send(()).await.unwrap() });

        self.interval.1.await.unwrap();
        self.signal.1.await.unwrap();

        // halt cmd channel, after interval/signal tasks are done
        // NOTE: if `cmd` halts while `interval` or `signal` are alive, they will
        // fail to send a refresh to `cmd`
        self.cmd.0.send(false).await.unwrap();
        self.cmd.1.await.unwrap();
    }
}
