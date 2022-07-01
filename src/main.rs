#![forbid(unsafe_code)]

use clap::Parser;
use dirs::config_dir;
use log::{error, info, trace, LevelFilter};
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::{broadcast, mpsc};
use tokio::{select, task};

use std::io::{stdout, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use smolbar::bar::{ContOrStop, Smolbar};
use smolbar::config::Config;
use smolbar::logger;
use smolbar::protocol::Header;
use smolbar::Error;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Path to the configuration file
    ///
    /// If this isn't specified, it falls back to "smolbar/config.toml" in the
    /// current user's config directory.
    #[clap(short, long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Print license information
    #[clap(short, long)]
    license: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    // start smolbar with logging enabled
    logger::set_level(LevelFilter::Trace);
    let exit_code = if let Err(err) = try_main(Args::parse()).await {
        error!("{}", err);
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    };

    // disable logging before main returns. mio makes a trace log (from signal
    // handling) when the program ends, so this hides it
    logger::set_level(LevelFilter::Off);

    // return with the appropriate exit code
    exit_code
}

async fn try_main(args: Args) -> Result<(), Error> {
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
                return Err(Error::NoConfig);
            }
        }
    };

    info!("set config path to '{}'", path.display());

    /* load configuration */
    let config = Config::read_from_path(&path)?;

    /* prepare to send continue and stop msgs to bar */
    // NOTE: signals may be forbidden, so a channel may not always be possible (hence option)
    let (cont_stop_send, cont_stop_recv) = mpsc::channel(1);

    let (sig_halt_send, _) = broadcast::channel(1);

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
        let sig = SignalKind::from_raw(sig);

        if let Ok(mut stream) = signal(sig) {
            // stream is ok, so make channel
            trace!("{} signal {} is valid, listening", name, sig.as_raw_value());

            let mut halt_recv = sig_halt_send.subscribe();
            let send = cont_stop_send.clone();

            task::spawn(async move {
                loop {
                    select!(
                        stream = stream.recv() => {
                            stream.unwrap();
                            send.send(msg).await.unwrap();
                        }

                        halt = halt_recv.recv() => {
                            halt.unwrap();

                            // halt
                            trace!("{} signal listener shutting down", name);
                            break;
                        }
                    );
                }
            });
        } else {
            trace!("{} signal {} is invalid", name, sig.as_raw_value());
        }
    }

    /* read bar from config */
    let bar = Smolbar::new(config, cont_stop_recv, sig_halt_send).await;

    /* start printing and updating blocks */
    bar.init()?;
    bar.run().await;

    info!("shutting down");

    Ok(())
}
