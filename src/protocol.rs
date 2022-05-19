use rgb::RGBA16;
use signal_hook::consts::signal::*;

pub struct Header {
    version: i32,
    click_events: Option<bool>,
    cont_signal: Option<i32>,
    stop_signal: Option<i32>,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            version: 1,
            click_events: Some(false),
            cont_signal: Some(SIGCONT),
            stop_signal: Some(SIGSTOP),
        }
    }
}

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

pub enum Align {
    Left,
    Right,
    Center,
}

pub enum Markup {
    Pango,
    None,
}
