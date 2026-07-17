//! Tests for the `\` global fuzzy finder (`ui::finder`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crossterm::event::KeyCode;

use crate::model::RcaId;
use crate::ui::testutil::{app_with, press};
use crate::ui::{Focus, Tab};

#[test]
fn backslash_opens_quiet_and_fills_as_you_type() {
    let mut app = app_with(2);
    press(&mut app, KeyCode::Char('\\'));
    let finder = app.finder().expect("finder open");
    assert!(finder.query.is_empty());
    assert!(
        finder.matches.is_empty(),
        "an empty query matches nothing — the popup opens quiet"
    );

    for c in "rca".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    let finder = app.finder().expect("still open");
    assert!(!finder.matches.is_empty(), "typing fills the results");
    // Title entries exist in the corpus: a global jump-to. Which workspace
    // ranks first is scoring's business; assert shape on the top hit.
    let first = finder.entry(0).expect("entry");
    assert!(!first.text.is_empty());
    assert!(first.id.as_str().starts_with("rca-"));

    press(&mut app, KeyCode::Esc);
    assert!(app.finder().is_none(), "esc closes");
}

#[test]
fn backslash_on_an_empty_store_explains_itself() {
    let mut app = app_with(0);
    press(&mut app, KeyCode::Char('\\'));
    assert!(app.finder().is_none());
    assert!(app
        .status_line()
        .is_some_and(|s| s.contains("no incidents")));
}

#[test]
fn typing_filters_and_ranks_with_match_positions() {
    let mut app = app_with(2);
    let id = RcaId::new("rca-1").expect("id");
    let dir = app.store.workspace_dir(&id);
    std::fs::write(dir.join("notes.md"), "redis pool exhausted\n").expect("write");
    app.reload();

    press(&mut app, KeyCode::Char('\\'));
    for c in "redispool".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    let finder = app.finder().expect("open");
    let top = finder.entry(0).expect("top match");
    assert_eq!(top.tab, Tab::Notes);
    assert!(top.text.contains("redis pool"));
    assert_eq!(
        finder.matches[0].positions.len(),
        "redispool".chars().count(),
        "one highlight position per typed char"
    );

    // Backspace re-ranks without panicking.
    press(&mut app, KeyCode::Backspace);
    assert!(app.finder().expect("open").query.ends_with("poo"));
}

#[test]
fn enter_jumps_to_the_picked_incident_tab_and_focuses_content() {
    let mut app = app_with(3);
    let id = RcaId::new("rca-2").expect("id");
    let dir = app.store.workspace_dir(&id);
    std::fs::write(dir.join("root-cause.md"), "the cause was zebra striping\n").expect("write");
    app.reload();

    press(&mut app, KeyCode::Char('\\'));
    for c in "zebrastriping".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);

    assert!(app.finder().is_none(), "jump closes the popup");
    assert_eq!(app.selected_rca().expect("selection").id, id);
    assert_eq!(app.tab(), Tab::RootCause);
    assert_eq!(app.focus(), Focus::Content);
}

#[test]
fn enter_reveals_an_archived_incident_before_jumping() {
    let mut app = app_with(2);
    let id = RcaId::new("rca-0").expect("id");
    app.store
        .set_status(&id, crate::model::Status::Finished)
        .expect("finish");
    app.store.archive(&id, false).expect("archive");
    app.reload();
    assert_eq!(app.visible_len(), 1, "archived hidden");

    press(&mut app, KeyCode::Char('\\'));
    // The scaffold summary contains the workspace title "RCA 0".
    for c in "rca0".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);

    assert!(app.show_archived(), "jump revealed the archive");
    assert_eq!(app.selected_rca().expect("selection").id, id);
}

#[test]
fn arrows_move_the_selection_clamped() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Char('\\'));
    for c in "rca".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    assert!(
        app.finder().expect("open").matches.len() > 1,
        "needs at least two hits to walk"
    );
    press(&mut app, KeyCode::Up);
    assert_eq!(app.finder().expect("open").selected, 0, "clamped at top");
    press(&mut app, KeyCode::Down);
    assert_eq!(app.finder().expect("open").selected, 1);
}
