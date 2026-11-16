use std::io::IsTerminal;

use unicode_width::UnicodeWidthStr;

/// A small set of monochrome text styles, mapped to ANSI SGR attributes.
/// Only weight, slant, dimness and underline are used — never color.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum St {
    Normal,
    Bold,
    Italic,
    Code,
    Dim,
    Accent,
}

#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub ansi: bool,
}

impl Style {
    pub const PLAIN: Style = Style { ansi: false };

    pub fn paint(self, st: St, text: &str) -> String {
        if !self.ansi || st == St::Normal {
            return text.to_string();
        }
        let code = match st {
            St::Normal => return text.to_string(),
            St::Bold => "1",
            St::Italic => "3",
            St::Code => "4",
            St::Dim => "2",
            St::Accent => "1",
        };
        format!("\x1b[{code}m{text}\x1b[0m")
    }
}

pub fn visible_width(text: &str) -> usize {
    text.width()
}

pub fn terminal_width() -> usize {
    terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(100)
}

pub fn stdout_is_tty() -> bool {
    std::io::stdout().is_terminal()
}

pub fn stderr_is_tty() -> bool {
    std::io::stderr().is_terminal()
}

/// True when ANSI styling should be enabled for the given mode choice.
pub fn ansi_enabled(mode: crate::ColorMode) -> bool {
    use crate::ColorMode;
    match mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => {
            stdout_is_tty()
                && std::env::var_os("NO_COLOR").is_none()
                && std::env::var("TERM").map(|t| t != "dumb").unwrap_or(true)
        }
    }
}
