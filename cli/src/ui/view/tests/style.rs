//! Tests for badges and glyphs (`ui::view::style`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

#[test]
fn investigating_symbol_animates_and_others_stay_fixed() {
    let (frame0, _) = status_symbol(Status::Investigating, 0);
    let (frame1, _) = status_symbol(Status::Investigating, 1);
    assert_ne!(frame0, frame1, "investigating must animate");
    let (wrapped, _) = status_symbol(Status::Investigating, INVESTIGATING_FRAMES.len());
    assert_eq!(wrapped, frame0, "frames cycle");

    let (fix0, fix_style) = status_symbol(Status::Fixing, 0);
    let (fix1, _) = status_symbol(Status::Fixing, 1);
    assert_ne!(fix0, fix1, "fixing must animate");
    assert_eq!(fix0, frame0, "fixing reuses the investigating spinner");
    let (_, inv_style) = status_symbol(Status::Investigating, 0);
    assert_eq!(fix_style.fg, Some(FIXING_ORANGE), "fixing is orange");
    assert_ne!(fix_style.fg, inv_style.fg, "fixing is told apart by color");

    for status in [
        Status::Review,
        Status::Agent,
        Status::FinalReview,
        Status::Finished,
    ] {
        let (a, _) = status_symbol(status, 0);
        let (b, _) = status_symbol(status, 7);
        assert_eq!(a, b, "{status} must not animate");
    }
}

#[test]
fn every_spinner_frame_is_one_cell_wide() {
    for frame in INVESTIGATING_FRAMES {
        assert_eq!(frame.chars().count(), 1, "frame `{frame}` shifts layout");
    }
}
