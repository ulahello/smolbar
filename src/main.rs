use clap::Parser;
use dirs::config_dir;
use log::{info, trace, LevelFilter};
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::{broadcast, mpsc};
use tokio::task;

use std::path::PathBuf;
use std::process;

use smolbar::bar::Smolbar;
use smolbar::config::Config;
use smolbar::logger;
use smolbar::protocol::Header;
use smolbar::Error;

// TODO: full documentation
// TODO: when reloading config to no blocks, blocks stay visible (must send empty for each block)

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Path to the configuration file
    ///
    /// If this isn't specified, it falls back to "smolbar/config.toml" in your
    /// config directory.
    #[clap(short, long)]
    config: Option<PathBuf>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    logger::init(LevelFilter::Trace).unwrap();

    if let Err(err) = try_main(Args::parse()).await {
        eprintln!("fatal: {}", err);
        process::exit(1);
    }
}

async fn try_main(args: Args) -> Result<(), Error> {
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
    let config = Config::read_from_path(path)?;

    /* prepare to send continue and stop msgs to bar */
    // NOTE: signals may be forbidden, so a channel may not always be possible (hence option)
    let mut cont_recv = None;
    let mut stop_recv = None;

    let (sig_halt_send, _) = broadcast::channel(1);

    for (sig, recv, name) in [
        (
            config
                .toml
                .header
                .cont_signal
                .unwrap_or(Header::DEFAULT_CONT_SIG),
            &mut cont_recv,
            "cont",
        ),
        (
            config
                .toml
                .header
                .stop_signal
                .unwrap_or(Header::DEFAULT_STOP_SIG),
            &mut stop_recv,
            "stop",
        ),
    ] {
        let sig = SignalKind::from_raw(sig);

        if let Ok(mut stream) = signal(sig) {
            // stream is ok, so make channel
            trace!("{} signal {} is valid, listening", name, sig.as_raw_value());

            let mut halt_recv = sig_halt_send.subscribe();

            let channel = mpsc::channel(1);
            let send = channel.0;
            *recv = Some(channel.1);

            task::spawn(async move {
                loop {
                    select!(
                        stream = stream.recv() => {
                            stream.unwrap();
                            send.send(true).await.unwrap();
                        }

                        halt = halt_recv.recv() => {
                            halt.unwrap();
                            // halt the receiving end
                            send.send(false).await.unwrap();

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
    let bar = Smolbar::new(config, cont_recv, stop_recv, sig_halt_send)?;

    /* start printing and updating blocks */
    bar.init()?;
    bar.run().await?;

    info!("shutting down");

    Ok(())
}
