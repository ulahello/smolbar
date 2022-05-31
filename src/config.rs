use log::trace;
use serde_derive::{Deserialize, Serialize};

use std::fs;
use std::path::PathBuf;

use crate::protocol::{Body, Header};
use crate::Error;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlConfig {
    pub command_dir: Option<String>,
    pub header: Header,
    #[serde(flatten)]
    pub body: Body,
    #[serde(rename = "block")]
    pub blocks: Vec<TomlBlock>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlBlock {
    pub command: String,
    pub prefix: Option<String>,
    pub postfix: Option<String>,
    pub interval: Option<u32>,
    pub signal: Option<i32>,

    #[serde(flatten)]
    pub body: Body,
}

pub struct Config {
    pub path: PathBuf,
    pub command_dir: PathBuf,
    pub toml: TomlConfig,
}

impl Config {
    pub fn read_from_path(path: PathBuf) -> Result<Config, Error> {
        let toml: TomlConfig = toml::from_str(&fs::read_to_string(path.clone())?)?;

        // command_dir is either the config's parent path or whatever is
        // specified in toml
        let mut command_dir = path.parent().unwrap_or(&path).to_path_buf();
        if let Some(ref dir) = toml.command_dir {
            // if the toml command_dir is relative, its appended to the config
            // path parent
            command_dir.push(dir);
        }

        trace!(
            "read {} block(s) from {}",
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
