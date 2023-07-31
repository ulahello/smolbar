// copyright (C) 2022-2023 Ula Shipman <ula.hello@mailbox.org>
// licensed under GPL-3.0-or-later

// TODO: testing?

#![forbid(unsafe_code)]
#![warn(clippy::pedantic, clippy::cargo)]

#[cfg(not(unix))]
compile_error!("smolbar only supports Unix platforms");

mod bar;
mod block;
mod blocks;
mod config;
mod protocol;

extern crate alloc;

use anyhow::Context;
use argh::FromArgs;
use nu_ansi_term::Color;
use tokio::task;
use tracing::{span, Level};

use core::hash::{Hash as HashTrait, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::env;
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
    #[allow(clippy::needless_pass_by_value)]
    fn pretty_err<W: Write>(mut out: W, err: anyhow::Error) -> io::Result<()> {
        writeln!(out, "{} {err}", Color::Red.paint("error:"))?;
        if err.chain().nth(1).is_some() {
            writeln!(out, "{}", Color::Red.paint("because:"))?;
        }
        for cause in err.chain().skip(1) {
            writeln!(out, "  {cause}")?;
        }
        out.flush()
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
        _ = pretty_err(stderr(), err);
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn print_version<W: Write>(mut out: W) -> io::Result<()> {
    writeln!(
        out,
        "{} {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    )
}

fn print_license<W: Write>(mut out: W) -> io::Result<()> {
    write!(out, "{}", include_str!("../docs/COPYRIGHT"))
}

async fn try_main(args: Args) -> anyhow::Result<()> {
    /* print version */
    if args.version {
        print_version(stdout())?;
        return Ok(());
    }

    /* print license information */
    if args.license {
        let stdout = BufWriter::new(stdout());
        print_license(stdout)?;
        return Ok(());
    }

    /* get configuration file */
    let path = {
        let span = span!(Level::TRACE, "get_config_path");
        let _enter = span.enter();
        if let Some(path) = args.config {
            tracing::trace!("using value of `--config`");
            path
        } else {
            let config_dir = if let Some(xdg_config_home) = env::var_os("XDG_CONFIG_HOME") {
                tracing::trace!("using $XDG_CONFIG_HOME");
                Some(PathBuf::from(xdg_config_home))
            } else if let Some(home) = env::var_os("HOME") {
                tracing::trace!("using $HOME");
                let mut dir = PathBuf::from(home);
                dir.push(".config");
                Some(dir)
            } else {
                None
            };
            if let Some(mut dir) = config_dir {
                dir.push("smolbar");
                dir.push("config.toml");
                dir
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Hash(u64);

impl Hash {
    pub fn new<T: HashTrait>(item: &T) -> Self {
        let mut hasher = DefaultHasher::new();
        item.hash(&mut hasher);
        Self(hasher.finish())
    }
}
