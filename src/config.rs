// copyright (C) 2022-2023 Ula Shipman <ula.hello@mailbox.org>
// licensed under GPL-3.0-or-later

use anyhow::{anyhow, Context};
use semver::{Version, VersionReq};
use serde_derive::{Deserialize, Serialize};
use tracing::{span, Level};

use core::str;
use std::fs::OpenOptions;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::protocol::{Body, Header, Signal};

/// Bar configuration, directly deserialized.
#[derive(Clone, Debug, Deserialize, Serialize)]
// TODO: don't deny unknown fields for compatibility, but do warn about them
#[serde(deny_unknown_fields)]
pub struct TomlBar {
    command_dir: Option<String>,
    #[serde(default = "TomlBar::default_smolbar_version_req")]
    smolbar_version: VersionReq,
    /// Configured [`Header`]
    #[serde(default = "Header::default")]
    pub header: Header,
    /// [`Body`] configured at `global` scope
    #[serde(flatten)]
    pub body: Body,
    /// The bar's configured [blocks](TomlBlock)
    #[serde(default = "Vec::new", rename = "block")]
    pub blocks: Vec<TomlBlock>,
}

impl TomlBar {
    pub const fn default_smolbar_version_req() -> VersionReq {
        VersionReq::STAR
    }

    pub fn current_smolbar_version() -> Version {
        env!("CARGO_PKG_VERSION")
            .parse()
            .expect("Cargo correctly sets version information")
    }
}

/// Block configuration, directly deserialized.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
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
    /// [`Duration`](core::time::Duration), or is not finite, it is ignored.
    pub interval: Option<f32>,
    /// Operating system signal to refresh the block when received
    pub signal: Option<Signal>,

    /// Body configured at `local` scope
    #[serde(flatten)]
    pub body: Body,
}

/// Convenience struct for easy access to all configuration options.
#[derive(Debug)]
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
    /// - `path` contents may contain invalid UTF-8
    /// - `path` contents may be invalid TOML
    #[tracing::instrument]
    pub fn read_from_path(path: &Path) -> anyhow::Result<Self> {
        /* canonicalize path before doing anything else. this is important for
         * getting `command_dir` bc its `path`'s parent */
        let path = path
            .canonicalize()
            .context("failed to canonicalize config path")?;

        let mut toml: TomlBar = {
            // TODO: would be nice to parse toml from `impl Read`
            let mut file = OpenOptions::new()
                .read(true)
                .open(&path)
                .context("failed to open config file")?;
            let file_size = file
                .metadata()
                .ok()
                .map(|metadata| metadata.len())
                .and_then(|len| usize::try_from(len).ok())
                .unwrap_or(0);
            let mut bytes = Vec::new();
            bytes
                .try_reserve(file_size)
                .context("failed to allocate memory for config file")?;
            file.read_to_end(&mut bytes)
                .context("failed to read config file")?;
            let utf8 = str::from_utf8(&bytes).context("invalid utf-8")?;
            toml::from_str(utf8)?
        };

        /* check version, just in case */
        if toml.header.version != Header::DEFAULT_VERSION {
            tracing::warn!(
                header.version = toml.header.version,
                "swaybar-protocol(7) requires header.version to be {default}",
                default = Header::DEFAULT_VERSION
            );
        }

        /* HACK: if full_text is not defined, we still want prefix and postfix
         * to apply to it (it being "") */
        if toml.body.full_text.is_none() {
            toml.body.full_text = Some(String::new());
        }

        /* check smolbar version */
        {
            let current = TomlBar::current_smolbar_version();
            let required = &toml.smolbar_version;

            let span = span!(
                Level::INFO,
                "config_check_version",
                current = format_args!(r#""{current}""#),
                required = format_args!(r#""{required}""#)
            );
            let _enter = span.enter();

            if required.matches(&current) {
                tracing::debug!("smolbar_version is satisfied");
            } else {
                tracing::error!("smolbar_version is unsatisfied");
                Err(anyhow!(
                    r#"current version "{current}" does not satisfy requirement "{required}""#
                ))
                .context("this configuration is unsupported by the current version of smolbar")?;
            }
        }

        /* command_dir is either the config's parent path or whatever is
         * specified in toml */
        let mut command_dir = path.parent().unwrap_or(&path).to_path_buf();
        if let Some(ref dir) = toml.command_dir {
            /* if the toml command_dir is relative, its appended to the config
             * path parent. otherwise, it replaces it. */
            command_dir.push(dir);
        }

        /* before pushing toml specified dir, it is canonical. however, since we
         * push an uncanonicalized path, we should canonicalize here. */
        tracing::trace!(
            path = format_args!(r#""{}""#, command_dir.display()),
            "canonicalizing command_dir",
        );
        command_dir = command_dir
            .canonicalize()
            .context("failed to canonicalize command_dir")?;

        tracing::info!(
            path = format_args!(r#""{}""#, command_dir.display()),
            "set command_dir"
        );

        tracing::trace!(
            num = toml.blocks.len(),
            path = format_args!(r#""{}""#, path.display()),
            "read block(s)",
        );

        Ok(Self {
            path,
            command_dir,
            toml,
        })
    }
}
