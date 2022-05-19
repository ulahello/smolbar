use signal_hook::consts;
use signal_hook::iterator::Signals;
use std::io::Error;

use crate::bar::Bar;

pub fn start(mut bar: Box<dyn Bar>) -> Result<(), Error> {
    let header = bar.header();

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
            bar.cont();
        } else if sig == header.stop_signal {
            drop(bar);
            return Ok(());
        } else {
            unreachable!();
        }
    }

    Ok(())
}
