//! Tests for `banner`.
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
