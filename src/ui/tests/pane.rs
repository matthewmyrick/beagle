//! Tests for pane loading and caching (`ui::pane`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crossterm::event::KeyCode;

use crate::ui::testutil::{app_with, press};

use super::*;

#[test]
fn sections_render_and_missing_tab_content_is_hint_not_error() {
    let mut app = app_with(1);
    app.ensure_pane();
    assert!(matches!(app.pane(), Some(Pane::Section(_))));

    // Diagrams dir is empty after scaffold → Empty hint, not an error.
    press(&mut app, KeyCode::Char('6'));
    app.ensure_pane();
    assert!(matches!(app.pane(), Some(Pane::Empty(_))));
}
