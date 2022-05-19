use std::io::Error;
use std::process;

use smolbar::runtime;
use smolbar::bar::Bar;

fn main() {
    if let Err(err) = try_main() {
        eprintln!("fatal: {}", err);
        process::exit(1);
    }
}

fn try_main() -> Result<(), Error> {
    /* get bar */
    let bar: Box<dyn Bar> = todo!();

    runtime::start(bar)?;

    Ok(())
}
