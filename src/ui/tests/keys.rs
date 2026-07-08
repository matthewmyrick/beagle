//! Tests for key handling and navigation (`ui::keys`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crossterm::event::{KeyEvent, KeyModifiers};

use crate::ui::testutil::{app_with, press};
use crate::ui::ViewportInfo;

use super::*;

#[test]
fn only_shift_q_and_ctrl_c_quit() {
    let mut app = app_with(1);
    assert_eq!(
        press(&mut app, KeyCode::Char('q')),
        Flow::Continue,
        "plain q must not quit"
    );
    assert_eq!(press(&mut app, KeyCode::Char('Q')), Flow::Quit);
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(app.handle_key(ctrl_c), Flow::Quit);
}

#[test]
fn arrow_keys_switch_tabs_from_either_pane() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Right); // from list focus
    assert_eq!(app.tab(), Tab::Timeline);
    press(&mut app, KeyCode::Right);
    assert_eq!(app.tab(), Tab::RootCause);
    press(&mut app, KeyCode::Left); // from content focus (Right focused it)
    assert_eq!(app.tab(), Tab::Timeline);
}

#[test]
fn b_returns_focus_to_the_list() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.focus(), Focus::Content);
    press(&mut app, KeyCode::Char('b'));
    assert_eq!(app.focus(), Focus::List);
}

#[test]
fn list_navigation_clamps_at_both_ends() {
    let mut app = app_with(3);
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(app.selected_index(), 0, "cannot go above the first item");
    for _ in 0..10 {
        press(&mut app, KeyCode::Char('j'));
    }
    assert_eq!(app.selected_index(), 2, "cannot go past the last item");
}

#[test]
fn navigation_on_empty_store_does_not_panic() {
    let mut app = app_with(0);
    for code in [
        KeyCode::Char('j'),
        KeyCode::Enter,
        KeyCode::Tab,
        KeyCode::Char('G'),
    ] {
        press(&mut app, code);
    }
    assert!(app.selected_rca().is_none());
}

#[test]
fn enter_focuses_content_and_esc_returns_to_list() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.focus(), Focus::Content);
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.focus(), Focus::List);
}

#[test]
fn number_keys_jump_to_tabs() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Char('3'));
    assert_eq!(app.tab(), Tab::RootCause);
    press(&mut app, KeyCode::Char('6'));
    assert_eq!(app.tab(), Tab::Diagrams);
}

#[test]
fn key_8_jumps_to_the_log_tab() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Char('8'));
    assert_eq!(app.tab(), Tab::Log);
    assert_eq!(Tab::Log.section(), Some(crate::model::SectionKind::Log));
}

#[test]
fn switching_tab_resets_scroll() {
    let mut app = app_with(1);
    app.viewport = ViewportInfo {
        content_lines: 100,
        height: 10,
    };
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char(' '));
    assert!(app.scroll_offsets().0 > 0);
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.scroll_offsets().0, 0);
}

#[test]
fn scroll_clamps_to_content_height() {
    let mut app = app_with(1);
    app.viewport = ViewportInfo {
        content_lines: 30,
        height: 10,
    };
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('G'));
    assert_eq!(
        app.scroll_offsets().0,
        20,
        "bottom = content minus viewport"
    );
}

#[test]
fn follow_mode_pins_scroll_to_the_bottom() {
    let mut app = app_with(1);
    app.viewport = ViewportInfo {
        content_lines: 100,
        height: 10,
    };
    assert!(!app.follow());
    press(&mut app, KeyCode::Char('f'));
    assert!(app.follow());
    assert_eq!(
        app.scroll_offsets().0,
        u16::MAX,
        "jumps to tail (draw clamps)"
    );
    press(&mut app, KeyCode::Char('f'));
    assert!(!app.follow());
}

#[test]
fn slash_filter_narrows_the_list_and_esc_clears() {
    let mut app = app_with(3); // titles "RCA 0".."RCA 2"
    press(&mut app, KeyCode::Char('/'));
    press(&mut app, KeyCode::Char('2'));
    assert_eq!(app.visible_len(), 1);
    assert_eq!(app.selected_rca().map(|r| r.id.as_str()), Some("rca-2"));

    press(&mut app, KeyCode::Esc);
    assert!(!app.search_active());
    assert!(app.filter().is_empty());
    assert_eq!(app.visible_len(), 3, "esc restores the full list");
}

#[test]
fn typing_q_in_search_mode_filters_instead_of_quitting() {
    let mut app = app_with(2);
    press(&mut app, KeyCode::Char('/'));
    assert_eq!(press(&mut app, KeyCode::Char('q')), Flow::Continue);
    assert_eq!(app.filter(), "q");
    assert_eq!(app.visible_len(), 0, "no workspace matches `q`");
    // Backspace repairs the query.
    press(&mut app, KeyCode::Backspace);
    assert_eq!(app.visible_len(), 2);
}

#[test]
fn enter_keeps_filter_and_esc_in_list_mode_clears_it() {
    let mut app = app_with(3);
    press(&mut app, KeyCode::Char('/'));
    press(&mut app, KeyCode::Char('1'));
    press(&mut app, KeyCode::Enter);
    assert!(!app.search_active());
    assert_eq!(app.visible_len(), 1, "filter survives enter");

    press(&mut app, KeyCode::Esc); // list-mode esc clears a kept filter
    assert_eq!(app.visible_len(), 3);
}
