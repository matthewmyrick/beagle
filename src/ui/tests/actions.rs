//! Tests for copy and export actions (`ui::actions`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crossterm::event::KeyCode;

use crate::ui::testutil::{app_with, press};

use super::*;

#[test]
fn e_exports_the_selected_workspace_and_reports_a_short_relative_path() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Char('e'));
    let status = app.status_line().expect("status set").to_owned();
    // Relative path only — a full path was too wide for the status bar.
    assert_eq!(status, "exported to exports/rca-0.md");
    let path = app.store.root().join("exports/rca-0.md");
    let doc = std::fs::read_to_string(&path).expect("export file exists");
    assert!(doc.starts_with("---\n"), "frontmatter present");
    assert!(doc.contains("title: \"RCA 0\""));
}

#[test]
fn copy_on_empty_store_is_a_no_op() {
    let mut app = app_with(0);
    press(&mut app, KeyCode::Char('c'));
    press(&mut app, KeyCode::Char('C'));
    assert!(app.selected_rca().is_none());
}

#[test]
fn human_size_formats_reasonably() {
    assert_eq!(human_size(842), "842 B");
    assert_eq!(human_size(1300), "1.3 KB");
    assert_eq!(human_size(2 * 1024 * 1024), "2.0 MB");
}
