// copyright (C) 2022  Ula Shipman <ula.hello@mailbox.org>
// licensed under GPL-3.0-or-later

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]
#![feature(duration_checked_float)]

//! `smolbar` is a status command for sway.

pub mod bar;
pub mod block;
pub mod config;
mod error;
pub mod protocol;

pub use error::Error;
