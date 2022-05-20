use rgb::RGBA16;
use signal_hook::consts::signal::*;

#[derive(Clone, Copy)]
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

#[derive(Clone)]
pub struct Body {
    full_text: Option<String>,
    short_text: Option<String>,
    color: Option<RGBA16>,
    background: Option<RGBA16>,
    border: Option<RGBA16>,
    border_top: Option<u32>,
    border_bottom: Option<u32>,
    border_left: Option<u32>,
    border_right: Option<u32>,
    min_width: Option<String>,
    align: Option<Align>,
    name: Option<String>,
    instance: Option<String>,
    urgent: Option<bool>,
    separator: Option<bool>,
    separator_block_width: Option<u32>,
    markup: Option<Markup>,
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

#[derive(Clone, Copy)]
pub enum Align {
    Left,
    Right,
    Center,
}

#[derive(Clone, Copy)]
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
