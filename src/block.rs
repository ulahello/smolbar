// copyright (C) 2022  Ula Shipman <ula.hello@mailbox.org>
// licensed under GPL-3.0-or-later

//! Defines a runtime block.

use log::{error, warn};
use tokio::select;
use tokio::signal;
use tokio::signal::unix::SignalKind;
use tokio::sync::{mpsc, RwLock, RwLockReadGuard};
use tokio::task::{self, JoinHandle};
use tokio::time::{self, Instant};

use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use crate::config::TomlBlock;
use crate::protocol::Body;

/// Configured block at runtime, which communicates to a parent
/// [bar](crate::bar::Smolbar).
#[derive(Debug)]
#[must_use]
pub struct Block {
    body: Arc<RwLock<Body>>,

    // `cmd` is responsible for sending refresh msgs to the bar. it continues as
    // long as it receives `true`, then it halts. `cmd` expects `bar_refresh` to
    // be alive.
    cmd: (mpsc::Sender<bool>, JoinHandle<()>),

    // `interval` sends a refresh to `cmd` on an interval. if it receives `()`, it
    // halts.
    interval: (mpsc::Sender<()>, JoinHandle<()>),

    // `signal` sends a refresh to `cmd` any time it receives a certain os
    // signal. if it receives `()`, it halts.
    signal: (mpsc::Sender<()>, JoinHandle<()>),
}

impl Block {
    // TODO: Smolbar refresh loop is private
    /// Initializes a new [`Block`].
    ///
    /// `bar_refresh` is connected to a [`Smolbar`](crate::bar::Smolbar)'s
    /// refresh loop.
    #[allow(clippy::missing_panics_doc)]
    pub async fn new(
        toml: TomlBlock,
        global: Body,
        bar_refresh: mpsc::Sender<bool>,
        cmd_dir: PathBuf,
        id: usize,
    ) -> Self {
        let toml = Arc::new(toml);
        let body = Arc::new(RwLock::new(Body::new()));

        let (sig_send, sig_recv) = mpsc::channel(1);
        let (interval_send, interval_recv) = mpsc::channel(1);
        let (cmd_send, cmd_recv) = mpsc::channel(1);

        /* listen for body refresh msgs and fulfill them */
        let cmd = (
            cmd_send.clone(),
            Self::task_cmd(
                Arc::clone(&toml),
                bar_refresh,
                global,
                Arc::clone(&body),
                cmd_recv,
                cmd_dir,
                id,
            )
            .await,
        );

        /* refresh on an interval */
        let interval = (
            interval_send,
            Self::task_interval(Arc::clone(&toml), interval_recv, cmd_send.clone(), id),
        );

        /* refresh on a signal */
        let signal = (
            sig_send,
            Self::task_signal(sig_recv, cmd_send.clone(), toml.signal),
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

    #[allow(clippy::items_after_statements)]
    async fn task_cmd(
        toml: Arc<TomlBlock>,
        bar_refresh: mpsc::Sender<bool>,
        global: Body,
        body: Arc<RwLock<Body>>,
        mut cmd_recv: mpsc::Receiver<bool>,
        cmd_dir: PathBuf,
        id: usize,
    ) -> JoinHandle<()> {
        async fn apply_scopes(
            mut lines: std::str::Lines<'_>,
            global: &Body,
            toml: &Arc<TomlBlock>,
            body: &Arc<RwLock<Body>>,
        ) {
            let mut body = body.write().await;

            fn update<T: Clone + FromStr>(
                field: &mut Option<T>,
                value: Option<&str>,
                local: &Option<T>,
                global: &Option<T>,
            ) {
                *field = match value {
                    Some(val) => match val.parse() {
                        Ok(new) => Some(new),
                        Err(_) => None,
                    },
                    None => None,
                }
                .or_else(|| local.clone())
                .or_else(|| global.clone());
            }

            update(
                &mut body.full_text,
                lines.next(),
                &toml.body.full_text,
                &global.full_text,
            );
            update(
                &mut body.short_text,
                lines.next(),
                &toml.body.short_text,
                &global.short_text,
            );
            update(
                &mut body.color,
                lines.next(),
                &toml.body.color,
                &global.color,
            );
            update(
                &mut body.background,
                lines.next(),
                &toml.body.background,
                &global.background,
            );
            update(
                &mut body.border,
                lines.next(),
                &toml.body.border,
                &global.border,
            );
            update(
                &mut body.border_top,
                lines.next(),
                &toml.body.border_top,
                &global.border_top,
            );
            update(
                &mut body.border_bottom,
                lines.next(),
                &toml.body.border_bottom,
                &global.border_bottom,
            );
            update(
                &mut body.border_left,
                lines.next(),
                &toml.body.border_left,
                &global.border_left,
            );
            update(
                &mut body.border_right,
                lines.next(),
                &toml.body.border_right,
                &global.border_right,
            );
            update(
                &mut body.min_width,
                lines.next(),
                &toml.body.min_width,
                &global.min_width,
            );
            update(
                &mut body.align,
                lines.next(),
                &toml.body.align,
                &global.align,
            );
            update(&mut body.name, lines.next(), &toml.body.name, &global.name);
            update(
                &mut body.instance,
                lines.next(),
                &toml.body.instance,
                &global.instance,
            );
            update(
                &mut body.urgent,
                lines.next(),
                &toml.body.urgent,
                &global.urgent,
            );
            update(
                &mut body.separator,
                lines.next(),
                &toml.body.separator,
                &global.separator,
            );
            update(
                &mut body.separator_block_width,
                lines.next(),
                &toml.body.separator_block_width,
                &global.separator_block_width,
            );
            update(
                &mut body.markup,
                lines.next(),
                &toml.body.markup,
                &global.markup,
            );

            // full text is prefixed by `prefix`,
            // postfixed by `postfix` field in toml
            if let Some(ref prefix) = toml.prefix {
                if let Some(full_text) = &body.full_text {
                    let mut prefix = prefix.to_string();
                    prefix.push_str(full_text);
                    body.full_text = Some(prefix);
                }
            }

            if let Some(ref mut full_text) = body.full_text {
                if let Some(postfix) = &toml.postfix {
                    full_text.push_str(postfix);
                }
            };
        }

        // initialize block body according to local and global scope
        apply_scopes("".lines(), &global, &toml, &body).await;

        // respond to refresh requests
        task::spawn(async move {
            while cmd_recv.recv().await.unwrap() {
                if let Some(ref toml_command) = toml.command {
                    let mut command = Command::new(toml_command);
                    command.current_dir(&cmd_dir);

                    // run command and capture output for Body
                    select!(
                        // we may receive halt while the command is running
                        // (which could take arbitrary time)
                        maybe_halt = cmd_recv.recv() => {
                            if !maybe_halt.unwrap() {
                                // halt!
                                break;
                            }
                        }

                        // refresh block body
                        Ok(try_output) = task::spawn_blocking(move || command.output()) => {
                            let immediate = match try_output {
                                Ok(output) => if let Ok(utf8) = String::from_utf8(output.stdout) {
                                    Some(utf8)
                                } else {
                                    error!(
                                        "block {}: command `{}` produced invalid utf8",
                                        id,
                                        toml_command
                                    );
                                    None
                                }
                                Err(error) => {
                                    error!("block {}: command `{}`: {}", id, toml_command, error);
                                    None
                                }
                            }.unwrap_or_else(|| "".into());

                            // if command fails (and immedate == ""), this
                            // iterator will only yield None
                            let lines = immediate.lines();

                            // update body with scopes regardless of whether
                            // command succeeded
                            apply_scopes(lines, &global, &toml, &body).await;

                            // ping parent bar to let know we are refreshed
                            bar_refresh.send(true).await.unwrap();
                        }
                    );
                } else {
                    // no command is set, the body cant change until config
                    // changes
                }
            }
        })
    }

    fn task_interval(
        toml: Arc<TomlBlock>,
        mut interval_recv: mpsc::Receiver<()>,
        cmd_send: mpsc::Sender<bool>,
        id: usize,
    ) -> JoinHandle<()> {
        task::spawn(async move {
            let mut yes_actually_exit = false;
            let mut deadline = Instant::now();

            match toml.interval.map(Duration::try_from_secs_f32) {
                Some(Ok(mut timeout)) => {
                    if timeout == Duration::ZERO {
                        warn!("block {} has zero timeout", id);
                    } else {
                        if timeout < Duration::from_millis(1) {
                            timeout = Duration::from_millis(1);
                            warn!(
                                "block {} timeout was really small and clamped to a millisecond",
                                id
                            );
                        }

                        loop {
                            // NOTE: if an iteration is faster than `timeout`,
                            // deadline < Instant::now
                            let now = Instant::now();
                            while deadline < now {
                                deadline += timeout;
                            }

                            match time::timeout_at(deadline, interval_recv.recv()).await {
                                Ok(halt) => {
                                    halt.unwrap();
                                    // we received halt msg
                                    yes_actually_exit = true;
                                    break;
                                }
                                Err(_) => {
                                    // receiving halt msg timed out, so we refresh
                                    // the body. this creates the behavior of
                                    // refreshing the body at a specific interval
                                    // until halting
                                    cmd_send.send(true).await.unwrap();
                                }
                            }
                        }
                    }
                }

                Some(Err(error)) => warn!("block {} has invalid timeout: {}", id, error),
                _ => (),
            }

            if !yes_actually_exit {
                // wait for halt msg
                interval_recv.recv().await.unwrap();
            }
        })
    }

    fn task_signal(
        mut sig_recv: mpsc::Receiver<()>,
        cmd_send: mpsc::Sender<bool>,
        signal: Option<i32>,
    ) -> JoinHandle<()> {
        task::spawn(async move {
            if let Some(sig) = signal {
                let sig = SignalKind::from_raw(sig);
                if let Ok(mut stream) = signal::unix::signal(sig) {
                    loop {
                        select!(
                            halt = sig_recv.recv() => {
                                halt.unwrap();
                                // we received halt msg
                                break;
                            }

                            sig = stream.recv() => {
                                sig.unwrap();
                                // refresh the body on receiving signal
                                cmd_send.send(true).await.unwrap();
                            }
                        );
                    }
                }
            } else {
                // wait for halt msg
                sig_recv.recv().await.unwrap();
            }
        })
    }

    /// Lock the body and return a guard to it.
    pub async fn read(&self) -> RwLockReadGuard<Body> {
        self.body.read().await
    }

    /// Gracefully halt the block, consuming it.
    #[allow(clippy::missing_panics_doc)]
    pub async fn halt(self) {
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
