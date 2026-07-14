//! Tests for the header, tab bar, and banner geometry (`ui::view::header`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use ratatui::layout::Rect;
use ratatui::text::Line;

use super::*;

fn rendered(lines: &[Line<'_>]) -> Vec<String> {
    lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
        .collect()
}

#[test]
fn tabs_fit_on_one_line_when_wide() {
    let lines = flow_tabs(Tab::Summary, 200, &[false; 9]);
    assert_eq!(lines.len(), 1);
}

#[test]
fn tabs_flow_to_more_lines_when_narrow_and_keep_every_label() {
    let lines = flow_tabs(Tab::Notes, 40, &[false; 9]);
    assert!(lines.len() > 1, "40 cols cannot fit all seven tabs");
    let all: String = rendered(&lines).join("");
    for (i, tab) in Tab::ALL.iter().enumerate() {
        let label = format!(" {} {} ", i + 1, tab.title());
        assert!(all.contains(&label), "label `{label}` missing");
    }
}

#[test]
fn tabs_survive_absurdly_narrow_width() {
    let lines = flow_tabs(Tab::Summary, 1, &[false; 9]);
    assert_eq!(lines.len(), Tab::ALL.len(), "one label per line");
}

#[test]
fn activity_labels_flip_to_quiet_at_the_threshold() {
    use std::time::Duration;

    let (label, quiet) = activity_label(Duration::from_secs(4 * 60));
    assert_eq!(label, "active 4m ago");
    assert!(!quiet, "under the threshold stays calm");

    let (label, quiet) = activity_label(QUIET_AFTER);
    assert_eq!(label, "quiet 10m");
    assert!(quiet, "at the threshold turns yellow");

    let (label, quiet) = activity_label(Duration::from_secs(95 * 60));
    assert_eq!(label, "quiet 1h 35m");
    assert!(quiet);
}

#[test]
fn unread_tabs_get_a_dot() {
    let mut unread = [false; 9];
    unread[8] = true; // Log
    let lines = flow_tabs(Tab::Summary, 200, &unread);
    let all: String = rendered(&lines).join("");
    assert!(all.contains(" 9 Log● "), "dot on the unread tab: {all}");
    assert!(!all.contains("Summary●"), "read tabs stay plain");
}

#[test]
fn no_line_exceeds_the_given_width_when_labels_fit() {
    let width: usize = 30;
    let lines = flow_tabs(
        Tab::Impact,
        u16::try_from(width).expect("fits"),
        &[false; 9],
    );
    for line in rendered(&lines) {
        assert!(
            line.chars().count() <= width,
            "line `{line}` is wider than {width}"
        );
    }
}

#[test]
fn banner_shows_only_when_the_pane_is_big_enough() {
    let big = Rect::new(0, 0, BANNER_COLS + MIN_HEADER_COLS, 40);
    assert!(banner_fits(big));
    let narrow = Rect::new(0, 0, BANNER_COLS + MIN_HEADER_COLS - 1, 40);
    assert!(
        !banner_fits(narrow),
        "too narrow: header column comes first"
    );
    let short = Rect::new(0, 0, 120, 15);
    assert!(!banner_fits(short), "too short: content comes first");
}

#[test]
fn banner_rect_is_pinned_to_the_right_edge_at_full_art_width() {
    // Alignment correctness: the art must render left-aligned in a rect
    // exactly as wide as its widest line, or ragged line ends stagger.
    let area = Rect::new(10, 2, 60, 6);
    let rect = banner_rect(area);
    assert_eq!(rect.width, crate::banner::WIDTH, "never wider than the art");
    assert_eq!(
        rect.x + rect.width + 1,
        area.x + area.width,
        "one-column right margin"
    );
    assert_eq!(rect.y, area.y);

    // Degenerate area: must not underflow or exceed the area.
    let tiny = Rect::new(0, 0, 10, 6);
    let clamped = banner_rect(tiny);
    assert!(clamped.width <= tiny.width);
    assert!(clamped.x + clamped.width <= tiny.width);
}

#[test]
fn elapsed_label_formats_minutes_hours_days() {
    use time::OffsetDateTime;
    let base = OffsetDateTime::from_unix_timestamp(1_780_000_000).expect("ts");
    let mins = |m: i64| base + time::Duration::minutes(m);
    assert_eq!(elapsed_label(base, mins(0)), "0m");
    assert_eq!(elapsed_label(base, mins(23)), "23m");
    assert_eq!(elapsed_label(base, mins(65)), "1h 05m");
    assert_eq!(elapsed_label(base, mins(26 * 60 + 30)), "1d 2h");
    assert_eq!(
        elapsed_label(mins(10), base),
        "0m",
        "clock skew clamps to 0"
    );
}
