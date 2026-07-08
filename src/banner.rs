//! The BEAGLE ASCII-art banner, shared by the TUI (top-right strip) and the
//! `beagle banner` CLI command.
//!
//! Kept as a plain string constant so both call sites render the exact same
//! art and a test can assert its dimensions stay terminal-friendly.

/// The banner art. Four lines, no trailing newline, longest line
/// [`WIDTH`] columns.
pub const BANNER: &str = r" ___  ___    _     ___  _     ___
| _ )| __|  /_\   / __|| |   | __|
| _ \| _|  / _ \ | (_ || |__ | _|
|___/|___|/_/ \_\ \___||____||___|";

/// Columns of the widest banner line. The TUI hides the banner when the
/// content pane is narrower than this (plus padding) so it never wraps.
pub const WIDTH: u16 = 34;

/// Number of lines in [`BANNER`].
pub const HEIGHT: u16 = 4;

#[cfg(test)]
#[path = "tests/banner.rs"]
mod tests;
