//! Tests for `prs`.
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use super::*;

#[test]
fn gh_state_json_maps_to_pr_states() {
    assert_eq!(
        parse_state_json(r#"{"state":"OPEN","isDraft":false}"#),
        Some(PrState::Open)
    );
    assert_eq!(
        parse_state_json(r#"{"state":"OPEN","isDraft":true}"#),
        Some(PrState::Draft)
    );
    assert_eq!(
        parse_state_json(r#"{"state":"MERGED","isDraft":false}"#),
        Some(PrState::Merged)
    );
    assert_eq!(
        parse_state_json(r#"{"state":"CLOSED"}"#),
        Some(PrState::Closed)
    );
    assert_eq!(parse_state_json(r#"{"state":"WEIRD"}"#), None);
    assert_eq!(parse_state_json("not json"), None);
}

#[test]
fn short_labels_extract_pr_numbers() {
    assert_eq!(short_label("https://github.com/org/repo/pull/123"), "#123");
    assert_eq!(
        short_label("https://github.com/org/repo/pull/9/files"),
        "#9"
    );
    let plain = short_label("https://gitlab.example.com/mrs/42-long-url-goes-here");
    assert!(plain.starts_with("gitlab.example.com"), "{plain}");
}

#[test]
fn glyphs_are_one_cell_wide() {
    for state in [
        PrState::Open,
        PrState::Draft,
        PrState::Merged,
        PrState::Closed,
    ] {
        assert_eq!(state.glyph().chars().count(), 1);
    }
}
