// copyright (C) 2022-2023 Ula Shipman <ula.hello@mailbox.org>
// licensed under GPL-3.0-or-later

#![forbid(unsafe_code)]
#![warn(clippy::pedantic, clippy::cargo)]

mod bar;
mod block;
mod config;
mod protocol;

use anyhow::Context;
use clap::Parser;
use dirs::config_dir;
use termcolor::{BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};
use tokio::select;
use tokio::signal::unix::{signal, Signal, SignalKind};
use tokio::sync::{mpsc, Notify};
use tokio::task::{self, JoinHandle};
use tracing::{info, span, trace, warn, Level};

use std::io::{stderr, stdout, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use crate::bar::{ContOrStop, Smolbar};
use crate::config::Config;
use crate::protocol::Header;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
#[clap(help_template(
    "{before-help}{name} {version}
{author-with-newline}{about-with-newline}
{usage-heading} {usage}

{all-args}{after-help}"
))]
struct Args {
    /// Path to the configuration file
    ///
    /// If this isn't specified, it falls back to "smolbar/config.toml" in the
    /// current user's config directory.
    #[clap(short, long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Decrease log verbosity
    #[clap(short, long)]
    terse: bool,

    /// Print license information
    #[clap(short, long)]
    license: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args = Args::parse();
    tracing_subscriber::fmt()
        .with_writer(stderr)
        .with_max_level(if args.terse {
            Level::INFO
        } else {
            Level::TRACE
        })
        .with_timer(tracing_subscriber::fmt::time::time())
        .init();

    #[allow(let_underscore_drop)]
    if let Err(err) = try_main(args).await {
        let bufwtr = BufferWriter::stderr(ColorChoice::Auto);
        let mut buffer = bufwtr.buffer();
        let mut spec = ColorSpec::new();
        _ = buffer.set_color(spec.set_fg(Some(Color::Red)));
        _ = write!(&mut buffer, "error: ");
        spec.clear();
        _ = buffer.set_color(&spec);
        _ = writeln!(&mut buffer, "{err}");

        if err.chain().nth(1).is_some() {
            _ = buffer.set_color(spec.set_fg(Some(Color::Red)));
            _ = writeln!(&mut buffer, "because:");
            spec.clear();
            _ = buffer.set_color(&spec);
        }
        for cause in err.chain().skip(1) {
            _ = writeln!(&mut buffer, "  {cause}");
        }

        _ = bufwtr.print(&buffer);
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

async fn try_main(args: Args) -> anyhow::Result<()> {
    /* print license information */
    if args.license {
        writeln!(stdout(), "{}", env!("CARGO_PKG_LICENSE"))?;
        return Ok(());
    }

    /* get configuration file */
    let path = match args.config {
        Some(path) => path,
        None => {
            if let Some(mut fallback) = config_dir() {
                fallback.push("smolbar");
                fallback.push("config.toml");
                fallback
            } else {
                return Err(anyhow::anyhow!(
                    "no configuration path found (try passing one with `--config`)"
                ));
            }
        }
    };

    info!(path = path.display().to_string(), "set config path");

    /* load configuration */
    let config = Config::read_from_path(&path).context("failed to load config")?;

    /* prepare to send continue and stop msgs to bar */
    let (cont_stop_send, cont_stop_recv) = mpsc::channel(1);
    let cont_halt = Arc::new(Notify::new());
    let cont_halt_ack = Arc::new(Notify::new());
    let mut signal_listeners = Vec::with_capacity(2);

    for (sig, msg, name) in [
        (
            config
                .toml
                .header
                .cont_signal
                .unwrap_or(Header::DEFAULT_CONT_SIG),
            ContOrStop::Cont,
            "cont",
        ),
        (
            config
                .toml
                .header
                .stop_signal
                .unwrap_or(Header::DEFAULT_STOP_SIG),
            ContOrStop::Stop,
            "stop",
        ),
    ] {
        let span = span!(Level::TRACE, "signal_consider", name, sig);
        let _enter = span.enter();

        let sig = SignalKind::from_raw(sig);

        if let Ok(stream) = signal(sig) {
            trace!("signal is valid, listening");

            let send = cont_stop_send.clone();
            let cont_halt = Arc::clone(&cont_halt);
            let cont_halt_ack = Arc::clone(&cont_halt_ack);
            signal_listeners.push(match msg {
                ContOrStop::Cont => cont_listener(stream, sig, send, cont_halt, cont_halt_ack),
                ContOrStop::Stop => stop_listener(stream, sig, send, cont_halt, cont_halt_ack),
            });
        } else {
            warn!("signal is invalid");
            let cont_halt = Arc::clone(&cont_halt);
            let cont_halt_ack = Arc::clone(&cont_halt_ack);
            match msg {
                ContOrStop::Cont => signal_listeners.push(cont_shim(sig, cont_halt, cont_halt_ack)),
                ContOrStop::Stop => (), // no valid stop signal, so graceful shutdown is not an option
            }
        }
    }

    /* instantiate bar from config */
    let bar = Smolbar::new(config, cont_stop_recv).await;

    /* start printing and updating blocks */
    bar.run().await?;

    /* wait for signal listeners to halt */
    trace!("waiting for signal listeners to halt");
    for task in signal_listeners {
        task.await.unwrap();
    }

    Ok(())
}

fn cont_listener(
    mut signal: Signal,
    sig_kind: SignalKind,
    send: mpsc::Sender<ContOrStop>,
    halt: Arc<Notify>,
    halt_ack: Arc<Notify>,
) -> JoinHandle<()> {
    task::spawn(async move {
        let span = span!(Level::TRACE, "cont_listener", sig = sig_kind.as_raw_value());
        loop {
            select!(
                sig = signal.recv() => {
                    if sig.is_some() {
                        let _enter = span.enter();
                        trace!("received signal");

                        // while this task is alive, the bar must be alive.
                        send.send(ContOrStop::Cont).await.unwrap();
                    }
                }

                () = halt.notified() => {
                    let _enter = span.enter();
                    trace!("received halt from stop_listener");

                    // make sure stop_listener is aware of this
                    halt_ack.notify_one();

                    break;
                }
            );
        }
    })
}

fn stop_listener(
    mut signal: Signal,
    sig_kind: SignalKind,
    send: mpsc::Sender<ContOrStop>,
    cont_halt: Arc<Notify>,
    cont_halt_ack: Arc<Notify>,
) -> JoinHandle<()> {
    task::spawn(async move {
        let span = span!(Level::TRACE, "stop_listener", sig = sig_kind.as_raw_value());
        loop {
            if signal.recv().await.is_some() {
                let _enter = span.enter();
                trace!("received signal");

                // we have to stop cont_listener; it cannot be allowed to send
                // messages to bar bc we're about to drop it
                trace!("requesting cont_listener halt");
                cont_halt.notify_one();

                // its crucial to wait for acknowledgement here, so we are
                // guarenteed that the cont_listener won't try to send to the
                // bar after we've dropped it
                trace!("waiting for acknowledgement from cont_listener");
                cont_halt_ack.notified().await;

                // the receiver must not drop until we tell it to
                trace!("sending stop to bar");
                send.send(ContOrStop::Stop).await.unwrap();

                // we're halting now!
                break;
            }
        }
    })
}

fn cont_shim(sig_kind: SignalKind, halt: Arc<Notify>, halt_ack: Arc<Notify>) -> JoinHandle<()> {
    task::spawn(async move {
        let span = span!(Level::TRACE, "cont_shim", sig = sig_kind.as_raw_value());

        // wait for halt msg
        halt.notified().await;

        let _enter = span.enter();
        trace!("received halt");

        // make sure sender is aware of this
        halt_ack.notify_one();
    })
}
