//! Tests for reload and unread tracking (`ui::event_loop`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crossterm::event::KeyCode;

use crate::model::{RcaId, Severity};
use crate::store::new_meta;
use crate::ui::testutil::{app_with, press};
use crate::ui::Tab;

#[test]
fn changed_sections_are_unread_until_viewed() {
    let mut app = app_with(1);
    let id = app.selected_rca().expect("selected").id.clone();

    // Nothing is unread at startup, and an untouched reload adds nothing.
    assert!(!app.has_unread(&id));
    app.reload();
    assert!(!app.has_unread(&id));

    // The agent writes notes.md → unread until the Notes tab is opened.
    let notes = app.store.workspace_dir(&id).join("notes.md");
    std::fs::write(&notes, "# Notes\n\nfresh evidence\n").expect("write");
    app.reload();
    assert!(app.is_unread(&id, Tab::Notes));
    assert!(!app.is_unread(&id, Tab::Summary), "summary untouched");
    assert!(app.has_unread(&id));

    press(&mut app, KeyCode::Char('7')); // Notes tab
    app.ensure_pane();
    assert!(!app.is_unread(&id, Tab::Notes), "viewing clears the dot");
    assert!(!app.has_unread(&id));
}

#[test]
fn reload_reports_workspaces_that_appeared() {
    let mut app = app_with(1);
    let id = RcaId::new("rca-fresh").expect("valid id");
    app.store
        .scaffold(&id, &new_meta("Fresh incident".to_owned(), Severity::High))
        .expect("scaffold");
    let arrived = app.reload();
    assert_eq!(arrived, ["Fresh incident"]);
    assert!(app.reload().is_empty(), "only reported once");
}
