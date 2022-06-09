//! `smolbar`'s [log] implementation.

use log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};

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

/// Initialize logging.
///
/// # Errors
///
/// If logging has already been initialized, this returns [`SetLoggerError`].
pub fn init(level: LevelFilter) -> Result<(), SetLoggerError> {
    log::set_boxed_logger(Box::new(Logger {
        epoch: Instant::now(),
    }))
    .map(|()| log::set_max_level(level))
}
