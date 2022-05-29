use libc::{SIGCONT, SIGSTOP};
use serde_derive::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Header {
    pub version: i32,
    pub click_events: bool,
    pub cont_signal: i32,
    pub stop_signal: i32,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            version: 1,
            click_events: false,
            cont_signal: SIGCONT,
            stop_signal: SIGSTOP,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Body {
    pub full_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_top: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_bottom: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_left: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_right: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_width: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<Align>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urgent: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub separator: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub separator_block_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markup: Option<Markup>,
}

impl Body {
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

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Left,
    Right,
    Center,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Markup {
    Pango,
    None,
}

#[derive(Clone)]
pub struct ClickEvent {
    name: String,
    instance: String,
    x: i32,
    y: i32,
    button: i32,
    event: i32,
    relative_x: i32,
    relative_y: i32,
    width: u32,
    height: u32,
}
