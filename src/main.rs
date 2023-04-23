// copyright (C) 2022-2023 Ula Shipman <ula.hello@mailbox.org>
// licensed under GPL-3.0-or-later

#![forbid(unsafe_code)]
#![warn(clippy::pedantic, clippy::cargo)]

mod bar;
mod block;
mod blocks;
mod config;
mod protocol;

extern crate alloc;

use anyhow::Context;
use argh::FromArgs;
use dirs::config_dir;
use termcolor::{BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};
use tokio::task;
use tracing::Level;

use std::io::{self, stderr, stdout, BufWriter, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use crate::bar::Bar;
use crate::config::Config;

/// smol status command for sway
#[derive(FromArgs, Debug)]
struct Args {
    /// path to configuration file [default: config.toml in $XDG_CONFIG_HOME/smolbar or $HOME/.config/smolbar]
    #[argh(option, short = 'c')]
    config: Option<PathBuf>,

    /// decrease log verbosity
    #[argh(switch, short = 't')]
    terse: bool,

    /// print license information
    #[argh(switch, short = 'l')]
    license: bool,

    /// print version
    #[argh(switch, short = 'V')]
    version: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    fn pretty_err(err: anyhow::Error) -> io::Result<()> {
        let bufwtr = BufferWriter::stderr(ColorChoice::Auto);
        let mut buffer = bufwtr.buffer();
        let mut spec = ColorSpec::new();
        buffer.set_color(spec.set_fg(Some(Color::Red)))?;
        write!(&mut buffer, "error: ")?;
        spec.clear();
        buffer.set_color(&spec)?;
        writeln!(&mut buffer, "{err}")?;

        if err.chain().nth(1).is_some() {
            buffer.set_color(spec.set_fg(Some(Color::Red)))?;
            writeln!(&mut buffer, "because:")?;
            spec.clear();
            buffer.set_color(&spec)?;
        }
        for cause in err.chain().skip(1) {
            writeln!(&mut buffer, "  {cause}")?;
        }
        drop(err);

        bufwtr.print(&buffer)
    }

    let args: Args = argh::from_env();
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
        _ = pretty_err(err);
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

async fn try_main(args: Args) -> anyhow::Result<()> {
    /* print version */
    if args.version {
        writeln!(
            stdout(),
            "{} {}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        )?;
        return Ok(());
    }

    /* print license information */
    if args.license {
        let mut stdout = BufWriter::new(stdout());
        // NOTE: volatile, copypasted data
        for (name, license, owners) in [
            (
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_LICENSE"),
                env!("CARGO_PKG_AUTHORS"),
            ),
            (
                "anyhow",
                "MIT OR Apache-2.0",
                "David Tolnay <dtolnay@gmail.com>",
            ),
            (
                "argh",
                "BSD-3-Clause",
                "Taylor Cramer <cramertj@google.com>, Benjamin Brittain <bwb@google.com>, Erick Tryzelaar <etryzelaar@google.com>",
            ),
            (
                "dirs",
                "MIT OR Apache-2.0",
                "Simon Ochsenreither <simon@ochsenreither.de>",
            ),
            (
                "libc",
                "MIT OR Apache-2.0",
                "The Rust Project Developers",
            ),
            (
                "serde",
                "MIT OR Apache-2.0",
                "Erick Tryzelaar <erick.tryzelaar@gmail.com>, David Tolnay <dtolnay@gmail.com>",
            ),
            (
                "serde_derive",
                "MIT OR Apache-2.0",
                "Erick Tryzelaar <erick.tryzelaar@gmail.com>, David Tolnay <dtolnay@gmail.com>",
            ),
            (
                "serde_json",
                "MIT OR Apache-2.0",
                "Erick Tryzelaar <erick.tryzelaar@gmail.com>, David Tolnay <dtolnay@gmail.com>",
            ),
            (
                "signal-hook-registry",
                "Apache-2.0/MIT",
                "Michal 'vorner' Vaner <vorner@vorner.cz>, Masaki Hara <ackie.h.gmai@gmail.com>",
            ),
            (
                "termcolor",
                "Unlicense OR MIT",
                "Andrew Gallant <jamslam@gmail.com>",
            ),
            (
                "tokio",
                "MIT",
                "Tokio Contributors <team@tokio.rs>",
            ),
            (
                "tokio-util",
                "MIT",
                "Tokio Contributors <team@tokio.rs>",
            ),
            (
                "toml",
                "MIT OR Apache-2.0",
                "Alex Crichton <alex@alexcrichton.com>",
            ),
            (
                "tracing",
                "MIT",
                "Eliza Weisman <eliza@buoyant.io>, Tokio Contributors <team@tokio.rs>",
            ),
            (
                "tracing-subscriber",
                "MIT",
                "Eliza Weisman <eliza@buoyant.io>, David Barsky <me@davidbarsky.com>, Tokio Contributors <team@tokio.rs>",
            ),
        ] {
            writeln!(
                stdout,
                "'{name}' by {owners}, licensed under '{license}'",
            )?;
        }
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

    tracing::info!(
        path = format_args!(r#""{}""#, path.display()),
        "set config path"
    );

    /* load configuration */
    let config = Config::read_from_path(&path).context("failed to load config")?;

    /* bar runtime */
    let (mut bar, _tx) = Bar::new(config);
    bar.write_header()?;

    // start main loop
    bar.listen().await?;

    tracing::debug!("goodbye");

    Ok(())
}

async fn await_cancellable<T>(handle: task::JoinHandle<T>) -> Option<T> {
    match handle.await {
        Ok(t) => Some(t),
        Err(err) => {
            if err.is_panic() {
                Err::<T, _>(err).unwrap();
            } else if err.is_cancelled() {
            } else {
                unreachable!("JoinError must be either panicked or cancelled");
            }
            None
        }
    }
}
