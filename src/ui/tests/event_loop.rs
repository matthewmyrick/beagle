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

    press(&mut app, KeyCode::Char('8')); // Notes tab
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
    let delta = app.reload();
    assert_eq!(delta.arrived, ["Fresh incident"]);
    assert!(app.reload().arrived.is_empty(), "only reported once");
}

#[test]
fn reload_reports_status_transitions_observed_on_disk() {
    use crate::model::Status;

    let mut app = app_with(1);
    let id = app.selected_rca().expect("selected").id.clone();

    // A status change made outside this App (agent, CLI, another beagle)
    // shows up in the delta exactly once.
    app.store.set_status(&id, Status::Review).expect("review");
    let delta = app.reload();
    assert_eq!(delta.status_changes.len(), 1);
    let (title, from, to) = &delta.status_changes[0];
    assert_eq!(title, "RCA 0");
    assert_eq!(*from, Status::Investigating);
    assert_eq!(*to, Status::Review);
    assert!(delta.arrived.is_empty());
    assert!(app.reload().status_changes.is_empty(), "only reported once");
}

#[test]
fn last_activity_tracks_the_newest_section_write() {
    let mut app = app_with(1);
    let id = app.selected_rca().expect("selected").id.clone();
    let before = app.last_activity(&id).expect("scaffold has mtimes");

    std::fs::write(
        app.store.workspace_dir(&id).join("log.md"),
        "# Log\n- **12:00 UTC** — checking\n",
    )
    .expect("write");
    app.reload();
    let after = app.last_activity(&id).expect("still tracked");
    assert!(after >= before, "newest write moves the activity forward");
}

#[test]
fn review_advances_to_final_review_only_when_every_pr_is_merged() {
    use crate::model::Status;
    use crate::prs::PrState;

    let mut app = app_with(1);
    let id = app.selected_rca().expect("selected").id.clone();
    app.store.set_status(&id, Status::Review).expect("review");
    app.store
        .add_pr(&id, "https://github.com/o/r/pull/1")
        .expect("pr 1");
    app.store
        .add_pr(&id, "https://github.com/o/r/pull/2")
        .expect("pr 2");
    app.reload();

    // One PR merged, one still open: stays in review.
    app.pr_states
        .insert("https://github.com/o/r/pull/1".to_owned(), PrState::Merged);
    app.pr_states
        .insert("https://github.com/o/r/pull/2".to_owned(), PrState::Open);
    app.advance_merged_reviews();
    assert_eq!(
        app.selected_rca().expect("selected").meta.status,
        Status::Review,
        "a still-open PR blocks the transition"
    );

    // Second PR merges: auto-advance fires, on disk and in memory.
    app.pr_states
        .insert("https://github.com/o/r/pull/2".to_owned(), PrState::Merged);
    app.advance_merged_reviews();
    assert_eq!(
        app.selected_rca().expect("selected").meta.status,
        Status::FinalReview
    );
    assert_eq!(
        app.store.read_meta(&id).expect("read").status,
        Status::FinalReview,
        "written to the manifest"
    );
    assert!(app
        .status_line()
        .expect("announced")
        .contains("final-review"));

    // Idempotent: nothing left in review, nothing changes.
    app.advance_merged_reviews();
    assert_eq!(
        app.selected_rca().expect("selected").meta.status,
        Status::FinalReview
    );
}

#[test]
fn workspaces_without_prs_never_auto_advance() {
    use crate::model::Status;

    let mut app = app_with(1);
    let id = app.selected_rca().expect("selected").id.clone();
    app.store.set_status(&id, Status::Review).expect("review");
    app.reload();

    app.advance_merged_reviews();
    assert_eq!(
        app.selected_rca().expect("selected").meta.status,
        Status::Review,
        "no attached PRs → nothing to observe merge"
    );
}

#[test]
fn v_signs_off_final_review_and_explains_elsewhere() {
    use crate::model::Status;

    let mut app = app_with(1);
    let id = app.selected_rca().expect("selected").id.clone();

    // V on an investigating workspace explains instead of mutating.
    press(&mut app, KeyCode::Char('V'));
    assert_eq!(
        app.selected_rca().expect("selected").meta.status,
        Status::Investigating
    );
    assert!(app
        .status_line()
        .expect("status set")
        .contains("V signs off"));

    // V on final-review finishes it, on disk and in memory.
    app.store
        .set_status(&id, Status::FinalReview)
        .expect("final-review");
    app.reload();
    press(&mut app, KeyCode::Char('V'));
    assert_eq!(
        app.selected_rca().expect("selected").meta.status,
        Status::Finished
    );
    assert_eq!(
        app.store.read_meta(&id).expect("read").status,
        Status::Finished
    );
}

#[test]
fn checklist_progress_aggregates_sections_and_tracks_reloads() {
    use crate::ui::testutil::app_with;

    let mut app = app_with(1);
    let id = crate::model::RcaId::new("rca-0").expect("id");
    assert_eq!(app.checklist_progress(&id), None, "no checkboxes yet");

    let dir = app.store.workspace_dir(&id);
    std::fs::write(dir.join("summary.md"), "- [x] a\n- [ ] b\n").expect("write");
    std::fs::write(dir.join("final-review.md"), "- [ ] c\n").expect("write");
    app.reload();
    assert_eq!(
        app.checklist_progress(&id),
        Some((1, 3)),
        "counts aggregate across sections"
    );

    // Ticking a box updates the cached count on the next reload. The
    // mtime snapshot has second granularity on some filesystems, so
    // backdate the old snapshot instead of sleeping.
    let key = (id.clone(), crate::model::SectionKind::FinalReview);
    let old = *app.mtimes.get(&key).expect("snapshot");
    app.mtimes
        .insert(key, old - std::time::Duration::from_secs(2));
    std::fs::write(dir.join("final-review.md"), "- [x] c\n").expect("write");
    app.reload();
    assert_eq!(app.checklist_progress(&id), Some((2, 3)));
}
