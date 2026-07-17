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

    for status in [Status::Review, Status::FinalReview, Status::Finished] {
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
