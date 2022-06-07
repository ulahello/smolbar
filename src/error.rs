use std::fmt::{self, Display};
use std::io;

/// Fatal errors that can occur at runtime.
#[derive(Debug)]
pub enum Error {
    /// Wrapper around [`std::io::Error`]
    Io(io::Error),
    /// No configuration path was found
    NoConfig,
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            Self::Io(error) => write!(f, "{}", error)?,
            Self::NoConfig => write!(
                f,
                "no configuration path found (try passing one with `--config`)"
            )?,
        }

        Ok(())
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Io(error.into())
    }
}

impl From<toml::de::Error> for Error {
    fn from(error: toml::de::Error) -> Self {
        Self::Io(error.into())
    }
}
