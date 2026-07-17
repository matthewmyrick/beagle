//! Tests for mouse routing (`ui::mouse`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::ui::testutil::app_with;
use crate::ui::{Focus, Tab, ViewportInfo};

fn mouse(kind: MouseEventKind, x: u16, y: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }
}

/// An app with a hand-fed hit map, as the draw pass would leave it.
fn app_with_map(n: usize) -> crate::ui::App {
    let mut app = app_with(n);
    app.mouse.sidebar = Rect::new(0, 0, 28, 20);
    app.mouse.content = Rect::new(28, 4, 60, 15);
    app.mouse.tabs = vec![
        (Tab::Summary, Rect::new(28, 2, 10, 1)),
        (Tab::Timeline, Rect::new(40, 2, 10, 1)),
    ];
    app.viewport = ViewportInfo {
        content_lines: 100,
        height: 15,
        width: 60,
    };
    app
}

#[test]
fn wheel_scrolls_the_pane_under_the_cursor() {
    let mut app = app_with_map(3);
    // Over the content: scrolls the pane.
    app.handle_mouse(mouse(MouseEventKind::ScrollDown, 30, 10));
    assert_eq!(app.scroll_offsets().0, 3);
    app.handle_mouse(mouse(MouseEventKind::ScrollUp, 30, 10));
    assert_eq!(app.scroll_offsets().0, 0);
    // Over the sidebar: moves the selection instead.
    app.handle_mouse(mouse(MouseEventKind::ScrollDown, 5, 10));
    assert_eq!(app.selected_index(), 1);
    app.handle_mouse(mouse(MouseEventKind::ScrollUp, 5, 10));
    assert_eq!(app.selected_index(), 0);
}

#[test]
fn click_selects_sidebar_rows_switches_tabs_and_focuses_content() {
    let mut app = app_with_map(3);
    // Third row: two lines per row, first content line is y = 1.
    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 5, 5));
    assert_eq!(app.selected_index(), 2);
    assert_eq!(app.focus(), Focus::List);

    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 42, 2));
    assert_eq!(app.tab(), Tab::Timeline);
    assert_eq!(app.focus(), Focus::Content, "tab click focuses content");

    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 30, 10));
    assert_eq!(app.focus(), Focus::Content);
}

#[test]
fn clicks_outside_rows_and_on_empty_stores_do_nothing() {
    let mut app = app_with_map(1);
    // Below the single row (row index 1+) — no selection change, no panic.
    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 5, 9));
    assert_eq!(app.selected_index(), 0);

    let mut empty = app_with_map(0);
    empty.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 30, 10));
    assert_eq!(empty.focus(), Focus::List, "empty store keeps list focus");
    empty.handle_mouse(mouse(MouseEventKind::ScrollDown, 5, 5));
}

#[test]
fn click_on_a_sidebar_row_expands_a_collapsed_sidebar_invariantly() {
    let mut app = app_with_map(2);
    crate::ui::testutil::press(&mut app, crossterm::event::KeyCode::Char('s'));
    assert!(app.sidebar_collapsed());
    // Sidebar rect is zero-sized while collapsed, so a click lands on
    // nothing — but any mouse path that focuses the list must expand it.
    app.mouse.sidebar = Rect::default();
    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 0, 1));
    assert!(
        app.sidebar_collapsed(),
        "click on nothing keeps the collapse"
    );

    // Simulate the draw pass after expansion and click a row.
    app.mouse.sidebar = Rect::new(0, 0, 28, 20);
    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 5, 1));
    assert!(!app.sidebar_collapsed());
    assert_eq!(app.focus(), Focus::List);
}
