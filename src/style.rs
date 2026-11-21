use std::env;
use std::io::IsTerminal;

use clap::ValueEnum;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

/// A small set of monochrome text styles, mapped to ANSI SGR attributes.
/// Only weight, slant, dimness and underline are used, plus a palette accent.
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

    pub fn new(mode: ColorMode) -> Self {
        let clicolor = env::var("CLICOLOR").ok();
        let force = env::var("CLICOLOR_FORCE").ok();
        let term = env::var("TERM").ok();
        Self {
            ansi: color_enabled(
                mode,
                env::var_os("NO_COLOR").is_some(),
                clicolor.as_deref(),
                force.as_deref(),
                term.as_deref(),
                std::io::stdout().is_terminal(),
            ),
        }
    }

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
            St::Accent => "1;36",
        };
        format!("\x1b[{code}m{text}\x1b[0m")
    }
}

fn color_enabled(
    mode: ColorMode,
    no_color: bool,
    clicolor: Option<&str>,
    force: Option<&str>,
    term: Option<&str>,
    tty: bool,
) -> bool {
    match mode {
        ColorMode::Never => false,
        ColorMode::Always => true,
        ColorMode::Auto => {
            if no_color {
                return false;
            }
            if force.is_some_and(|value| !value.is_empty() && value != "0") {
                return true;
            }
            if clicolor == Some("0") || term == Some("dumb") {
                return false;
            }
            tty
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_modes_win() {
        assert!(!color_enabled(
            ColorMode::Never,
            false,
            None,
            None,
            None,
            true
        ));
        assert!(color_enabled(
            ColorMode::Always,
            true,
            Some("0"),
            None,
            Some("dumb"),
            false
        ));
    }

    #[test]
    fn auto_respects_the_environment() {
        assert!(color_enabled(
            ColorMode::Auto,
            false,
            None,
            None,
            Some("xterm"),
            true
        ));
        assert!(!color_enabled(
            ColorMode::Auto,
            false,
            None,
            None,
            Some("xterm"),
            false
        ));
        assert!(!color_enabled(
            ColorMode::Auto,
            true,
            None,
            None,
            Some("xterm"),
            true
        ));
        assert!(!color_enabled(
            ColorMode::Auto,
            false,
            Some("0"),
            None,
            Some("xterm"),
            true
        ));
        assert!(!color_enabled(
            ColorMode::Auto,
            false,
            None,
            None,
            Some("dumb"),
            true
        ));
        assert!(color_enabled(
            ColorMode::Auto,
            false,
            Some("0"),
            Some("1"),
            Some("dumb"),
            false
        ));
    }

    #[test]
    fn accent_uses_the_palette() {
        let style = Style { ansi: true };
        assert_eq!(style.paint(St::Accent, "bandit"), "\x1b[1;36mbandit\x1b[0m");
        assert_eq!(style.paint(St::Normal, "body"), "body");
    }
}
