// copyright (C) 2022-2023 Ula Shipman <ula.hello@mailbox.org>
// licensed under GPL-3.0-or-later

use tokio::process::Command;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tokio::{task, time};
use tokio_util::sync::CancellationToken;
use tracing::{field, span, Level};

use alloc::sync::Arc;
use core::str::{self, FromStr, Lines};
use core::time::Duration;
use std::path::PathBuf;
use std::process::Stdio;

use crate::bar::BarMsg;
use crate::config::TomlBlock;
use crate::protocol::Body;
use crate::Hash;

#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Copy, Debug)]
pub struct RegenBody {
    init: bool,
}

#[derive(Debug)]
pub struct Block {
    body: Arc<RwLock<Body>>,
    global_body: Arc<Body>,
    toml: TomlBlock,
    command_dir: Arc<PathBuf>,

    id: usize,

    rx: mpsc::Receiver<RegenBody>,
    tx: mpsc::Sender<RegenBody>,
    bar_tx: mpsc::Sender<BarMsg>,
    cancel: CancellationToken,

    interval_handle_created: bool,
    signal_handle_created: bool,
}

impl Block {
    pub fn new(
        toml: TomlBlock,
        global_body: Arc<Body>,
        command_dir: Arc<PathBuf>,
        bar_tx: mpsc::Sender<BarMsg>,
        id: usize,
        num_blocks: usize,
    ) -> (Self, CancellationToken) {
        let body = Arc::new(RwLock::new(Body::new()));
        let (tx, rx) = mpsc::channel(
            /* kinda arbitrary. this number tries to prevent hanging if a lot of
             * blocks send a refresh request. */
            num_blocks.saturating_mul(2),
        );
        let cancel_parent = CancellationToken::new();
        let cancel_child = cancel_parent.child_token();
        (
            Self {
                body,
                global_body,
                toml,
                command_dir,
                id,
                rx,
                tx,
                bar_tx,
                cancel: cancel_child,
                interval_handle_created: false,
                signal_handle_created: false,
            },
            cancel_parent,
        )
    }

    pub fn body(&self) -> Arc<RwLock<Body>> {
        Arc::clone(&self.body)
    }

    pub async fn listen(mut self) {
        let interval_handle = self
            .interval_handle()
            .expect("interval handle must not yet be created");
        let signal_handle = self
            .signal_handle()
            .expect("signal handle must not yet be created");

        // generate body for the first time
        let tx = self.tx.clone();
        task::spawn(async move {
            let span = span!(Level::INFO, "block_init", id = self.id);
            let _enter = span.enter();
            tracing::trace!("performing body initialization");
            tx.send(RegenBody { init: true }).await.unwrap();
        });

        'listen_loop: loop {
            let span = span!(
                Level::INFO,
                "block_listen",
                id = self.id,
                command = self.toml.command,
                msg = field::Empty
            );
            tokio::select!(
                () = self.cancel.cancelled() => {
                    let _enter = span.enter();
                    tracing::trace!("shutting down");
                    for handle in [interval_handle, signal_handle] {
                        handle.abort();
                        crate::await_cancellable(handle).await;
                    }
                    break 'listen_loop;
                }

                Some(msg) = self.rx.recv() => {
                    {
                        let _enter = span.enter();
                        span.record("msg", format_args!("{msg:?}"));
                        tracing::trace!("regenerating body");
                    }
                    self.regenerate_body(msg.init).await;
                }
            );
        }
    }
}

impl Block {
    #[allow(clippy::too_many_lines)]
    async fn update_body(
        immediate: Lines<'_>,
        global: &Body,
        local: &TomlBlock,
        body: &mut Body,
        bar_tx: mpsc::Sender<BarMsg>,
    ) {
        fn update<T: Clone + FromStr>(
            field: &mut Option<T>,
            immediate: Option<&str>,
            local: Option<&T>,
            global: Option<&T>,
        ) {
            *field = match immediate {
                Some(val) => match val.parse() {
                    Ok(new) => Some(new),
                    Err(_) => None,
                },
                None => None,
            }
            .or_else(|| local.cloned())
            .or_else(|| global.cloned());
        }

        // compute hash of old body to later compare with new body
        let old_body_hash = crate::Hash::new(body);

        let mut lines = immediate;
        let toml = local;

        update(
            &mut body.full_text,
            lines.next(),
            toml.body.full_text.as_ref(),
            global.full_text.as_ref(),
        );
        update(
            &mut body.short_text,
            lines.next(),
            toml.body.short_text.as_ref(),
            global.short_text.as_ref(),
        );
        update(
            &mut body.color,
            lines.next(),
            toml.body.color.as_ref(),
            global.color.as_ref(),
        );
        update(
            &mut body.background,
            lines.next(),
            toml.body.background.as_ref(),
            global.background.as_ref(),
        );
        update(
            &mut body.border,
            lines.next(),
            toml.body.border.as_ref(),
            global.border.as_ref(),
        );
        update(
            &mut body.border_top,
            lines.next(),
            toml.body.border_top.as_ref(),
            global.border_top.as_ref(),
        );
        update(
            &mut body.border_bottom,
            lines.next(),
            toml.body.border_bottom.as_ref(),
            global.border_bottom.as_ref(),
        );
        update(
            &mut body.border_left,
            lines.next(),
            toml.body.border_left.as_ref(),
            global.border_left.as_ref(),
        );
        update(
            &mut body.border_right,
            lines.next(),
            toml.body.border_right.as_ref(),
            global.border_right.as_ref(),
        );
        update(
            &mut body.min_width,
            lines.next(),
            toml.body.min_width.as_ref(),
            global.min_width.as_ref(),
        );
        update(
            &mut body.align,
            lines.next(),
            toml.body.align.as_ref(),
            global.align.as_ref(),
        );
        update(
            &mut body.name,
            lines.next(),
            toml.body.name.as_ref(),
            global.name.as_ref(),
        );
        update(
            &mut body.instance,
            lines.next(),
            toml.body.instance.as_ref(),
            global.instance.as_ref(),
        );
        update(
            &mut body.urgent,
            lines.next(),
            toml.body.urgent.as_ref(),
            global.urgent.as_ref(),
        );
        update(
            &mut body.separator,
            lines.next(),
            toml.body.separator.as_ref(),
            global.separator.as_ref(),
        );
        update(
            &mut body.separator_block_width,
            lines.next(),
            toml.body.separator_block_width.as_ref(),
            global.separator_block_width.as_ref(),
        );
        update(
            &mut body.markup,
            lines.next(),
            toml.body.markup.as_ref(),
            global.markup.as_ref(),
        );

        /* full text is prefixed by `prefix`, postfixed by `postfix` field in
         * toml */
        if let Some(ref mut full_text) = body.full_text {
            if let Some(ref prefix) = toml.prefix {
                full_text.insert_str(0, prefix);
            }
        }

        if let Some(ref mut full_text) = body.full_text {
            if let Some(ref postfix) = toml.postfix {
                full_text.push_str(postfix);
            }
        };

        /* consider sending a refresh request */
        let new_body_hash = Hash::new(body);
        if old_body_hash == new_body_hash {
            tracing::trace!("body unchanged, suppressing refresh request");
        } else {
            tracing::trace!("requesting bar refresh");
            bar_tx
                .send(BarMsg::RefreshBlocks)
                .await
                .expect("Bar must outlive its Blocks");
        }
    }

    async fn regenerate_body(&self, init: bool) {
        let span = span!(
            Level::INFO,
            "block_regen_body",
            id = self.id,
            init,
            command = self.toml.command,
            exit_status = field::Empty
        );

        let mut immediate = String::new();
        if init && self.toml.command.is_some() {
            /* we check that there's a command because otherwise we're updating
             * the body twice with the same immediate value */
            let _enter = span.enter();
            // initialize with empty immediate
            Self::update_body(
                immediate.lines(),
                &self.global_body,
                &self.toml,
                &mut *self.body.write().await,
                self.bar_tx.clone(),
            )
            .await;
        }

        if let Some(ref program) = self.toml.command {
            let mut command = Command::new(program);
            command.kill_on_drop(true);
            command.current_dir(&*self.command_dir);
            command.stdout(Stdio::piped());
            command.stderr(Stdio::null());
            command.stdin(Stdio::null());
            {
                let _enter = span.enter();
                tracing::trace!("executing command");
            }
            match command.spawn() {
                Ok(child) => {
                    tokio::select!(
                        () = self.cancel.cancelled() => {
                            let _enter = span.enter();
                            tracing::trace!("command cancelled");
                        }

                        try_output = child.wait_with_output() => {
                            let _enter = span.enter();
                            match try_output {
                                Ok(output) => {
                                    span.record("exit_status", output.status.code());
                                    if !output.status.success() {
                                        tracing::warn!("command exited with failure");
                                    }

                                    match String::from_utf8(output.stdout) {
                                        Ok(stdout) => immediate = stdout,

                                        Err(err) => {
                                            tracing::error!(
                                                err = format_args!("{err}"),
                                                "command produced invalid utf8"
                                            );
                                        }
                                    }
                                }

                                Err(err) => {
                                    let _enter = span.enter();
                                    tracing::error!(err = format_args!("{err}"), "failed to wait for child");
                                }
                            }
                        }
                    );
                }

                Err(err) => {
                    let _enter = span.enter();
                    tracing::error!(err = format_args!("{err}"), "failed to execute command");
                }
            }
        }

        let _enter = span.enter();
        Self::update_body(
            immediate.lines(),
            &self.global_body,
            &self.toml,
            &mut *self.body.write().await,
            self.bar_tx.clone(),
        )
        .await;
    }

    fn interval_handle(&mut self) -> Option<JoinHandle<()>> {
        (!self.interval_handle_created).then(|| {
            self.interval_handle_created = true;
            let tx = self.tx.clone();
            let toml_interval = self.toml.interval;
            let id = self.id;
            task::spawn(async move {
                let span = span!(
                    Level::INFO,
                    "block_interval",
                    id,
                    toml_interval,
                    interval = field::Empty,
                );
                if let Some(toml_interval) = toml_interval {
                    match Duration::try_from_secs_f32(toml_interval) {
                        Ok(mut dur) => {
                            if dur.is_zero() {
                                let _enter = span.enter();
                                tracing::error!("can't have timeout of zero");
                            } else {
                                if dur < Duration::from_millis(1) {
                                    let _enter = span.enter();
                                    dur = Duration::from_millis(1);
                                    span.record("interval", format_args!("{dur:?}"));
                                    tracing::warn!(
                                        "timeout was really small and clamped to a millisecond"
                                    );
                                } else {
                                    span.record("interval", format_args!("{dur:?}"));
                                }

                                let mut interval = time::interval(dur);
                                interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

                                loop {
                                    interval.tick().await;
                                    tx.send(RegenBody { init: false })
                                        .await
                                        .expect("Block must outlive interval handle");
                                }
                            }
                        }

                        Err(err) => {
                            let _enter = span.enter();
                            tracing::warn!(error = format_args!("{err}"), "invalid interval");
                        }
                    }
                } else {
                    let _enter = span.enter();
                    tracing::trace!("no interval defined");
                }
            })
        })
    }

    fn signal_handle(&mut self) -> Option<JoinHandle<()>> {
        (!self.signal_handle_created).then(|| {
            self.signal_handle_created = true;
            let tx = self.tx.clone();
            let toml_signal = self.toml.signal;
            let id = self.id;
            task::spawn(async move {
                let span = span!(Level::INFO, "block_signal", id, signal = field::Empty);
                if let Some(signal) = toml_signal {
                    span.record("signal", format_args!("{signal}"));
                }

                if let Some(signum) = toml_signal {
                    let sig_kind = SignalKind::from_raw(signum.as_raw());
                    if let Ok(mut sig) = signal(sig_kind) {
                        while let Some(()) = sig.recv().await {
                            let _enter = span.enter();
                            tracing::trace!("received signal, requesting Block regenerate body");
                            tx.send(RegenBody { init: false })
                                .await
                                .expect("Block must outlive signal handle");
                        }
                    } else {
                        let _enter = span.enter();
                        tracing::error!("invalid signal");
                    }
                } else {
                    let _enter = span.enter();
                    tracing::trace!("no signal defined");
                }
            })
        })
    }
}
