//! Tests for the tab enum (`ui::tabs`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

#[test]
fn tab_next_prev_are_inverse_and_wrap() {
    for tab in Tab::ALL {
        assert_eq!(tab.next().prev(), tab);
    }
    assert_eq!(Tab::Log.next(), Tab::Summary);
    assert_eq!(Tab::Summary.prev(), Tab::Log);
}
