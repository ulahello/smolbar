// copyright (C) 2022  Ula Shipman <ula.hello@mailbox.org>
// licensed under GPL-3.0-or-later

//! Implementation of `swaybar-protocol(7)`.

use libc::{SIGCONT, SIGSTOP};
use serde_derive::{Deserialize, Serialize};

use core::str::FromStr;

/// Header object as defined in `swaybar-protocol(7)`.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Header {
    /// "The protocol version to use. Currently, this must be 1"
    #[serde(default = "Header::default_version")]
    pub version: i32,
    /// "Whether to receive click event information to standard input"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub click_events: Option<bool>,
    /// "The signal that swaybar should send to continue processing"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cont_signal: Option<i32>,
    /// "The signal that swaybar should send to stop processing"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_signal: Option<i32>,
}

impl Header {
    /// Default value of [`Header::version`].
    pub const DEFAULT_VERSION: i32 = 1;
    /// Default value of [`Header::cont_signal`].
    pub const DEFAULT_CONT_SIG: i32 = SIGCONT;
    /// Default value of [`Header::stop_signal`].
    pub const DEFAULT_STOP_SIG: i32 = SIGSTOP;

    const fn default_version() -> i32 {
        Self::DEFAULT_VERSION
    }
}

impl Default for Header {
    fn default() -> Self {
        Self {
            version: 1,
            click_events: Some(false),
            cont_signal: Some(Self::DEFAULT_CONT_SIG),
            stop_signal: Some(Self::DEFAULT_STOP_SIG),
        }
    }
}

/// Body element as defined in `swaybar-protocol(7)`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Body {
    /// "The text that will be displayed. If missing, the block will be skipped."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_text: Option<String>,
    /// "If given and the text needs to be shortened due to space, this will be
    /// displayed instead of full_text"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_text: Option<String>,
    /// "The text color to use in #RRGGBBAA or #RRGGBB notation"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// "The background color for the block in #RRGGBBAA or #RRGGBB notation"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    /// "The border color for the block in #RRGGBBAA or #RRGGBB notation"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<String>,
    /// "The height in pixels of the top border. The default is 1"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_top: Option<u32>,
    /// "The height in pixels of the bottom border. The default is 1"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_bottom: Option<u32>,
    /// "The width in pixels of the left border. The default is 1"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_left: Option<u32>,
    /// "The width in pixels of the right border. The default is 1"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_right: Option<u32>,
    /// "The minimum width to use for the block. This can either be given in
    /// pixels or a string can be given to allow for it to be calculated based
    /// on the width of the string."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_width: Option<String>,
    /// "If the text does not span the full width of the block, this specifies
    /// how the text should be aligned inside of the block. This can be left
    /// (default), right, or center."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<Align>,
    /// "A name for the block. This is only used to identify the block for click
    /// events. If set, each block should have a unique name and instance pair."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// "The instance of the name for the block. This is only used to identify
    /// the block for click events. If set, each block should have a unique name
    /// and instance pair."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    /// "Whether the block should be displayed as urgent. Currently swaybar
    /// utilizes the colors set in the sway config for urgent workspace buttons.
    /// See sway-bar(5) for more information on bar color configuration."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urgent: Option<bool>,
    /// "Whether the bar separator should be drawn after the block. See
    /// sway-bar(5) for more information on how to set the separator text."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub separator: Option<bool>,
    /// "The amount of pixels to leave blank after the block. The separator text
    /// will be displayed centered in this gap. The default is 9 pixels."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub separator_block_width: Option<u32>,
    /// "The type of markup to use when parsing the text for the block. This can
    /// either be pango or none (default)."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markup: Option<Markup>,
}

impl Body {
    /// Returns a new [`Body`] with all optional fields blank.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            full_text: None,
            short_text: None,
            color: None,
            background: None,
            border: None,
            border_top: None,
            border_bottom: None,
            border_left: None,
            border_right: None,
            min_width: None,
            align: None,
            name: None,
            instance: None,
            urgent: None,
            separator: None,
            separator_block_width: None,
            markup: None,
        }
    }
}

impl Default for Body {
    fn default() -> Self {
        Self {
            full_text: None,
            short_text: None,
            color: None,
            background: None,
            border: None,
            border_top: Some(1),
            border_bottom: Some(1),
            border_left: Some(1),
            border_right: Some(1),
            min_width: None,
            align: Some(Align::Left),
            name: None,
            instance: None,
            urgent: None,
            separator: None,
            separator_block_width: Some(9),
            markup: Some(Markup::None),
        }
    }
}

/// [Body alignment](Body::align), as defined in `swaybar-protocol(7)`.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    /// Left alignment
    Left,
    /// Right alignment
    Right,
    /// Center alignment
    Center,
}

impl FromStr for Align {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err> {
        match s.to_lowercase().as_ref() {
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            "center" => Ok(Self::Center),
            _ => Err(()),
        }
    }
}

/// [Body markup](Body::markup), as defined in `swaybar-protocol(7)`.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Markup {
    /// Use [Pango markup](https://docs.gtk.org/Pango/pango_markup.html)
    Pango,
    /// No markup, plain text
    None,
}

impl FromStr for Markup {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err> {
        match s.to_lowercase().as_ref() {
            "pango" => Ok(Self::Pango),
            "none" => Ok(Self::None),
            _ => Err(()),
        }
    }
}

/// Click event, as defined in `swaybar-protocol(7)`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClickEvent {
    /// "The name of the block, if set"
    pub name: Option<String>,
    /// "The instance of the block, if set"
    pub instance: Option<String>,
    /// "The x location that the click occurred at"
    pub x: i32,
    /// "The y location that the click occurred at"
    pub y: i32,
    /// "The x11 button number for the click. If the button does not have an x11
    /// button mapping, this will be 0."
    pub button: i32,
    /// "The event code that corresponds to the button for the click"
    pub event: i32,
    /// "The x location of the click relative to the top-left of the block"
    pub relative_x: i32,
    /// "The y location of the click relative to the top-left of the block"
    pub relative_y: i32,
    /// "The width of the block in pixels"
    pub width: u32,
    /// "The height of the block in pixels"
    pub height: u32,
}
