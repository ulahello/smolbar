//! `smolbar`'s [log] implementation.

use log::{Level, LevelFilter, Log, Metadata, Record};

use std::time::Instant;

struct Logger {
    epoch: Instant,
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let level = match record.level() {
                Level::Error => "error",
                Level::Warn => "warning",
                Level::Info => "info",
                Level::Debug => "debug",
                Level::Trace => "trace",
            };

            eprintln!(
                "[{:.3}] {}: {}",
                self.epoch.elapsed().as_secs_f32(),
                level,
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

/// Set the level of logging.
///
/// This also initializes logging if it has not been already.
pub fn set_level(level: LevelFilter) {
    let _ = log::set_boxed_logger(Box::new(Logger {
        epoch: Instant::now(),
    }));
    log::set_max_level(level);
}
