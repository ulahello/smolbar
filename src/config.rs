// copyright (C) 2022  Ula Shipman <ula.hello@mailbox.org>
// licensed under GPL-3.0-or-later

//! Configuration structures for the bar and its blocks.

use log::{info, trace};
use serde_derive::{Deserialize, Serialize};

use std::fs;
use std::path::{Path, PathBuf};

use crate::protocol::{Body, Header};
use crate::Error;

/// Bar configuration, directly deserialized.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlBar {
    command_dir: Option<String>,
    /// Configured [`Header`]
    pub header: Header,
    /// [`Body`] configured at `global` scope
    #[serde(flatten)]
    pub body: Body,
    /// The bar's configured [blocks](TomlBlock)
    #[serde(rename = "block")]
    pub blocks: Vec<TomlBlock>,
}

/// Block configuration, directly deserialized.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlBlock {
    /// Command to execute to configure body at `immediate` scope
    pub command: Option<String>,
    /// String prefixing `full_text`
    pub prefix: Option<String>,
    /// String appended to `full_text`
    pub postfix: Option<String>,
    /// Interval, in seconds, at which to refresh the block
    ///
    /// If the interval is negative, overflows
    /// [`Duration`](std::time::Duration), or is not finite, it is ignored.
    pub interval: Option<f32>,
    /// Operating system signal to refresh the block when received
    pub signal: Option<i32>,

    /// Body configured at `local` scope
    #[serde(flatten)]
    pub body: Body,
}

/// Convenience struct for easy access to all configuration options.
pub struct Config {
    /// Path of the TOML configuration file
    pub path: PathBuf,
    /// Path to execute block commands in
    pub command_dir: PathBuf,
    /// Bar's direct TOML configuration
    pub toml: TomlBar,
}

impl Config {
    /// Read a TOML configuration from the given `path`, and return it
    /// as a [`Config`].
    ///
    /// # Errors
    ///
    /// - Canonicalizing `path` may fail
    /// - Reading from `path` may fail
    /// - `path` contents may be invalid TOML
    pub fn read_from_path(path: &Path) -> Result<Config, Error> {
        // canonicalize path before doing anything else. this is important for
        // getting `command_dir`, which is `path`'s parent
        let path = path.canonicalize()?;

        let toml: TomlBar = toml::from_str(&fs::read_to_string(&path)?)?;

        // command_dir is either the config's parent path or whatever is
        // specified in toml
        let mut command_dir = path.parent().unwrap_or(&path).to_path_buf();
        if let Some(ref dir) = toml.command_dir {
            // if the toml command_dir is relative, its appended to the config
            // path parent
            command_dir.push(dir);
        }
        command_dir = command_dir.canonicalize()?;

        info!("set command_dir to '{}'", command_dir.display());

        trace!(
            "read {} block(s) from '{}'",
            toml.blocks.len(),
            path.display()
        );

        Ok(Self {
            path,
            command_dir,
            toml,
        })
    }
}
