use std::env;
use std::fmt::Display;
use std::io::IsTerminal;
use std::sync::atomic::{AtomicU8, Ordering};

use anyhow::Result;
use serde::Serialize;

use crate::cli::ColorMode;

const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const RESET: &str = "\x1b[0m";

const COLOR_MODE_AUTO: u8 = 0;
const COLOR_MODE_ALWAYS: u8 = 1;
const COLOR_MODE_NEVER: u8 = 2;

static COLOR_MODE_OVERRIDE: AtomicU8 = AtomicU8::new(COLOR_MODE_AUTO);

#[derive(Copy, Clone, Debug, Default)]
pub struct UiOptions {
    pub color_mode: ColorMode,
}

pub fn configure(options: UiOptions) {
    COLOR_MODE_OVERRIDE.store(color_mode_value(options.color_mode), Ordering::Relaxed);
}

pub fn section(title: &str) {
    if use_tty_style() {
        let divider = "-".repeat(title.len());
        println!();
        println!("{BOLD}{CYAN}{title}{RESET} {DIM}{divider}{RESET}",);
    } else {
        println!("\n{title}");
    }
}

pub fn success(message: impl Display) {
    if use_tty_style() {
        println!("{GREEN}[ok]{RESET} {message}");
    } else {
        println!("[ok] {message}");
    }
}

pub fn info(label: &str, value: impl Display) {
    if use_tty_style() {
        println!("  {CYAN}{label}{RESET}: {value}");
    } else {
        println!("  {label}: {value}");
    }
}

pub fn next_step(command: &str) {
    if use_tty_style() {
        println!("  {YELLOW}>{RESET} {command}");
    } else {
        println!("  $ {command}");
    }
}

pub fn action(label: &str, value: impl Display) {
    println!("  {label:<7} {value}");
}

pub fn json(value: impl Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

pub fn shell_arg(value: impl AsRef<str>) -> String {
    let value = value.as_ref();
    if !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | '=' | ':'))
    {
        return value.to_string();
    }

    format!("'{}'", value.replace('\'', "'\\''"))
}

fn use_tty_style() -> bool {
    let color_mode = COLOR_MODE_OVERRIDE.load(Ordering::Relaxed);
    should_use_styling(
        color_mode,
        std::io::stdout().is_terminal(),
        env::var_os("NO_COLOR"),
        env::var_os("TERM"),
        env::var_os("CI"),
    )
}

const fn color_mode_value(color_mode: ColorMode) -> u8 {
    match color_mode {
        ColorMode::Auto => COLOR_MODE_AUTO,
        ColorMode::Always => COLOR_MODE_ALWAYS,
        ColorMode::Never => COLOR_MODE_NEVER,
    }
}

fn should_use_styling(
    color_mode: u8,
    has_tty: bool,
    no_color: Option<std::ffi::OsString>,
    term: Option<std::ffi::OsString>,
    ci: Option<std::ffi::OsString>,
) -> bool {
    match color_mode {
        COLOR_MODE_ALWAYS => true,
        COLOR_MODE_NEVER => false,
        _ => {
            has_tty && no_color.is_none() && ci.is_none() && term.is_none_or(|term| term != "dumb")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn use_tty_style_respects_no_color() {
        assert!(should_use_styling(
            COLOR_MODE_AUTO,
            true,
            None,
            Some("xterm-256color".into()),
            None,
        ));
        assert!(!should_use_styling(
            COLOR_MODE_NEVER,
            true,
            None,
            Some("xterm-256color".into()),
            None,
        ));
        assert!(!should_use_styling(
            COLOR_MODE_AUTO,
            true,
            Some("1".into()),
            Some("xterm-256color".into()),
            None,
        ));
        assert!(!should_use_styling(
            COLOR_MODE_AUTO,
            true,
            None,
            Some("xterm-256color".into()),
            Some("true".into()),
        ));
        assert!(!should_use_styling(
            COLOR_MODE_AUTO,
            true,
            None,
            Some("dumb".into()),
            None
        ));
        assert!(!should_use_styling(
            COLOR_MODE_AUTO,
            false,
            None,
            Some("xterm-256color".into()),
            None,
        ));
        assert!(should_use_styling(
            COLOR_MODE_ALWAYS,
            false,
            Some("1".into()),
            Some("dumb".into()),
            Some("true".into()),
        ));
    }

    #[test]
    fn shell_arg_quotes_only_when_needed() {
        assert_eq!(shell_arg("/tmp/project"), "/tmp/project");
        assert_eq!(shell_arg("/tmp/my project"), "'/tmp/my project'");
        assert_eq!(shell_arg("owner's project"), "'owner'\\''s project'");
        assert_eq!(shell_arg(""), "''");
    }
}
