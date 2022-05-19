use signal_hook::consts;
use signal_hook::iterator::Signals;
use std::io::Error;
use std::process;

use smolbar::protocol::Header;

fn main() {
    if let Err(err) = try_main() {
        eprintln!("fatal: {}", err);
        process::exit(1);
    }
}

fn try_main() -> Result<(), Error> {
    /* get header */
    let header = Header::default();

    /* handle signals */
    let mut signals = Signals::new(
        // dont handle any forbidden signals (signal_hook will panic)
        [header.cont_signal, header.stop_signal]
            .iter()
            .filter(|signal| !consts::FORBIDDEN.contains(signal)),
    )?;

    for signal in signals.forever() {
        let sig = signal as i32;

        if sig == header.cont_signal {
            todo!("continue");
        } else if sig == header.stop_signal {
            return Ok(());
        } else {
            unreachable!();
        }
    }

    Ok(())
}
