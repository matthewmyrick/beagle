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
mod tests {
    use super::*;

    #[test]
    fn banner_dimensions_match_the_constants() {
        let lines: Vec<&str> = BANNER.lines().collect();
        assert_eq!(lines.len(), usize::from(HEIGHT));
        let widest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        assert_eq!(widest, usize::from(WIDTH));
    }

    #[test]
    fn banner_is_plain_ascii() {
        // The CLI prints it to arbitrary terminals; keep it escape-free.
        assert!(BANNER.chars().all(|c| c.is_ascii() && c != '\u{1b}'));
    }
}
