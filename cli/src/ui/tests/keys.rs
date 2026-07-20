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
fn tab_keys_on_an_empty_store_explain_instead_of_silence() {
    // The welcome screen has no tab bar, so a mute Tab press reads as a
    // broken keybinding. Every tab-switching key must say why instead.
    let mut app = app_with(0);
    for code in [
        KeyCode::Tab,
        KeyCode::BackTab,
        KeyCode::Right,
        KeyCode::Char('3'),
    ] {
        press(&mut app, code);
        let status = app.status_line().expect("status explains the no-op");
        assert!(status.contains("no incidents yet"), "got: {status}");
        assert_eq!(app.focus(), Focus::List, "focus stays on the list");
    }
    assert_eq!(app.tab(), Tab::Summary, "tab state unchanged");
}

#[test]
fn tab_keys_with_a_non_matching_filter_point_at_the_filter() {
    let mut app = app_with(2);
    press(&mut app, KeyCode::Char('f'));
    press(&mut app, KeyCode::Char('/')); // facet mode → typing
    press(&mut app, KeyCode::Char('z')); // matches nothing
    press(&mut app, KeyCode::Enter); // keep filter, leave search mode
    assert_eq!(app.visible_len(), 0);

    press(&mut app, KeyCode::Tab);
    let status = app.status_line().expect("status set");
    assert!(status.contains("filter"), "got: {status}");
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
    assert_eq!(app.tab(), Tab::FinalReview);
    press(&mut app, KeyCode::Char('7'));
    assert_eq!(app.tab(), Tab::Diagrams);
}

#[test]
fn key_9_jumps_to_the_log_tab() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Char('9'));
    assert_eq!(app.tab(), Tab::Log);
    assert_eq!(Tab::Log.section(), Some(crate::model::SectionKind::Log));
}

#[test]
fn switching_tab_resets_scroll() {
    let mut app = app_with(1);
    app.viewport = ViewportInfo {
        content_lines: 100,
        height: 10,
        width: 80,
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
        width: 80,
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
        width: 80,
    };
    assert!(!app.follow());
    press(&mut app, KeyCode::Char('F'));
    assert!(app.follow());
    assert_eq!(
        app.scroll_offsets().0,
        u16::MAX,
        "jumps to tail (draw clamps)"
    );
    press(&mut app, KeyCode::Char('F'));
    assert!(!app.follow());
}

#[test]
fn esc_exits_follow_mode_from_either_focus() {
    let mut app = app_with(1);

    // Content focus: esc peels follow before leaving the pane.
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('F'));
    assert!(app.follow());
    press(&mut app, KeyCode::Esc);
    assert!(!app.follow(), "first esc exits follow");
    assert_eq!(app.focus(), Focus::Content, "still on the content");
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.focus(), Focus::List, "second esc leaves the pane");

    // List focus: esc exits follow there too.
    press(&mut app, KeyCode::Char('F'));
    assert!(app.follow());
    press(&mut app, KeyCode::Esc);
    assert!(!app.follow());
}

#[test]
fn f_filter_narrows_the_list_and_esc_peels() {
    let mut app = app_with(3); // titles "RCA 0".."RCA 2"
    press(&mut app, KeyCode::Char('f'));
    press(&mut app, KeyCode::Char('/'));
    press(&mut app, KeyCode::Char('2'));
    assert_eq!(app.visible_len(), 1);
    assert_eq!(app.selected_rca().map(|r| r.id.as_str()), Some("rca-2"));

    press(&mut app, KeyCode::Esc);
    assert!(app.search_active(), "first esc only stops typing");
    assert_eq!(app.filter(), "2", "query survives the peel");

    press(&mut app, KeyCode::Esc);
    assert!(!app.search_active());
    assert!(app.filter().is_empty());
    assert_eq!(app.visible_len(), 3, "second esc restores the full list");
}

/// Workspaces spanning severities and statuses, for facet tests.
fn app_with_variety() -> crate::ui::App {
    use crate::model::{RcaId, Severity, Status};
    use crate::store::{new_meta, Store};

    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("store");
    let scaffold = |slug: &str, title: &str, severity: Severity, status: Option<Status>| {
        let id = RcaId::new(slug).expect("valid id");
        store
            .scaffold(&id, &new_meta(title.to_owned(), severity))
            .expect("scaffold");
        if let Some(status) = status {
            store.set_status(&id, status).expect("status");
        }
    };
    scaffold(
        "alloy-inv-high",
        "Alloy pool exhausted",
        Severity::High,
        None,
    );
    scaffold("cron-inv-low", "Cron drift", Severity::Low, None);
    scaffold(
        "ses-rev-high",
        "SES blocklist",
        Severity::High,
        Some(Status::Review),
    );
    scaffold(
        "rds-fin-med",
        "RDS locks",
        Severity::Medium,
        Some(Status::Finished),
    );
    std::mem::forget(tmp); // OS cleans the temp root; see app_with
    crate::ui::App::new(store).expect("app")
}

#[test]
fn facet_keys_stack_toggle_and_never_type() {
    let mut app = app_with_variety();
    press(&mut app, KeyCode::Char('f'));

    press(&mut app, KeyCode::Char('h')); // severity: high
    assert_eq!(app.visible_len(), 2, "two high-severity incidents");
    assert!(app.filter().is_empty(), "facet keys do not type");
    assert_eq!(app.facet_label(), "[high]");

    press(&mut app, KeyCode::Char('i')); // + status: investigating
    assert_eq!(app.visible_len(), 1, "facets AND across dimensions");
    assert_eq!(
        app.selected_rca().expect("match").id.as_str(),
        "alloy-inv-high"
    );
    assert_eq!(app.facet_label(), "[high · investigating]");

    press(&mut app, KeyCode::Char('h')); // toggle high off
    assert_eq!(app.visible_len(), 2, "both investigating incidents return");

    press(&mut app, KeyCode::Char('v')); // + final-review (none exist)
    press(&mut app, KeyCode::Char('i')); // investigating off → only v left
    assert_eq!(app.visible_len(), 0, "no final-review incidents");

    press(&mut app, KeyCode::Esc); // facet mode esc clears everything
    assert!(!app.has_active_filter());
    assert_eq!(app.visible_len(), 4);
}

#[test]
fn facets_combine_with_free_text_and_survive_enter() {
    let mut app = app_with_variety();
    press(&mut app, KeyCode::Char('f'));
    press(&mut app, KeyCode::Char('i')); // investigating: 2 left
    press(&mut app, KeyCode::Char('/')); // switch to typing
    for c in "cron".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    assert_eq!(app.visible_len(), 1, "text ranks within the facet set");
    assert_eq!(
        app.selected_rca().expect("match").id.as_str(),
        "cron-inv-low"
    );

    press(&mut app, KeyCode::Enter); // keep everything, leave filter mode
    assert!(!app.search_active());
    assert!(app.has_active_filter(), "facets + text survive enter");

    press(&mut app, KeyCode::Enter); // open the incident → consumed
    assert!(!app.has_active_filter(), "opening consumes facets too");
    assert_eq!(app.visible_len(), 4);
    assert_eq!(
        app.selected_rca().expect("selected").id.as_str(),
        "cron-inv-low"
    );
}

#[test]
fn typing_q_in_filter_mode_filters_instead_of_quitting() {
    let mut app = app_with(2);
    press(&mut app, KeyCode::Char('f'));
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
    press(&mut app, KeyCode::Char('f'));
    press(&mut app, KeyCode::Char('/'));
    press(&mut app, KeyCode::Char('1'));
    press(&mut app, KeyCode::Enter);
    assert!(!app.search_active());
    assert_eq!(app.visible_len(), 1, "filter survives enter");

    press(&mut app, KeyCode::Esc); // list-mode esc clears a kept filter
    assert_eq!(app.visible_len(), 3);
}

#[test]
fn opening_an_incident_consumes_the_filter_and_restores_the_list() {
    let mut app = app_with(3); // titles "RCA 0".."RCA 2"
    press(&mut app, KeyCode::Char('f'));
    press(&mut app, KeyCode::Char('/'));
    press(&mut app, KeyCode::Char('2'));
    press(&mut app, KeyCode::Enter); // commit the filter
    assert_eq!(app.visible_len(), 1);
    let picked = app.selected_rca().expect("match").id.clone();

    press(&mut app, KeyCode::Enter); // open the incident
    assert_eq!(app.focus(), Focus::Content);
    assert!(app.filter().is_empty(), "the pick consumed the filter");
    assert_eq!(app.visible_len(), 3, "everyone is back");
    assert_eq!(
        app.selected_rca().expect("selected").id,
        picked,
        "selection stays on the incident you opened"
    );
}

#[test]
fn s_collapses_the_sidebar_and_focuses_content() {
    let mut app = app_with(1);
    assert!(!app.sidebar_collapsed());
    press(&mut app, KeyCode::Char('s'));
    assert!(app.sidebar_collapsed());
    assert_eq!(
        app.focus(),
        Focus::Content,
        "a hidden list cannot hold the cursor"
    );
    press(&mut app, KeyCode::Char('s'));
    assert!(!app.sidebar_collapsed(), "s toggles back");
    assert_eq!(
        app.focus(),
        Focus::Content,
        "expanding does not steal focus"
    );
}

#[test]
fn returning_focus_to_the_list_expands_the_sidebar() {
    // Every back-to-list path must bring the sidebar back: b, esc, f.
    for code in [KeyCode::Char('b'), KeyCode::Esc, KeyCode::Char('f')] {
        let mut app = app_with(1);
        press(&mut app, KeyCode::Char('s'));
        assert!(app.sidebar_collapsed());
        press(&mut app, code);
        assert_eq!(app.focus(), Focus::List, "{code:?} should focus the list");
        assert!(
            !app.sidebar_collapsed(),
            "{code:?} should expand the sidebar"
        );
    }
}

#[test]
fn collapsing_on_an_empty_store_does_not_panic() {
    let mut app = app_with(0);
    press(&mut app, KeyCode::Char('s'));
    assert!(app.sidebar_collapsed());
    press(&mut app, KeyCode::Esc);
    assert!(!app.sidebar_collapsed());
}

#[test]
fn a_toggles_archived_incidents_into_the_list() {
    let mut app = app_with(2);
    let id = crate::model::RcaId::new("rca-1").expect("id");
    app.store
        .set_status(&id, crate::model::Status::Finished)
        .expect("finish");
    app.store.archive(&id, false).expect("archive");
    app.reload();

    assert_eq!(app.visible_len(), 1, "archived hidden by default");
    assert_eq!(app.archived_count(), 1);

    press(&mut app, KeyCode::Char('a'));
    assert_eq!(app.visible_len(), 2, "a shows archived");
    let last = app.visible_rcas().last().expect("row");
    assert!(last.archived, "archived sorts below active");

    press(&mut app, KeyCode::Char('a'));
    assert_eq!(app.visible_len(), 1, "a hides them again");
}

#[test]
fn shift_e_requests_an_edit_of_the_current_tabs_file() {
    let mut app = app_with(1);
    match press(&mut app, KeyCode::Char('E')) {
        Flow::Edit(path) => assert!(
            path.ends_with("rca-0/summary.md"),
            "summary tab edits summary.md: {path:?}"
        ),
        other => panic!("expected Flow::Edit, got {other:?}"),
    }

    press(&mut app, KeyCode::Char('9')); // Log tab
    match press(&mut app, KeyCode::Char('E')) {
        Flow::Edit(path) => assert!(path.ends_with("rca-0/log.md")),
        other => panic!("expected Flow::Edit, got {other:?}"),
    }
}

#[test]
fn shift_e_on_an_empty_diagram_tab_or_empty_store_stays_put() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Char('7')); // Diagrams tab, no diagram files
    assert_eq!(
        press(&mut app, KeyCode::Char('E')),
        Flow::Continue,
        "nothing to edit without diagrams"
    );
    assert!(
        app.status_line().is_some_and(|s| s.contains("diagram")),
        "status explains why"
    );

    let mut empty = app_with(0);
    assert_eq!(press(&mut empty, KeyCode::Char('E')), Flow::Continue);
}

#[test]
fn shift_e_edits_the_current_diagram_when_one_exists() {
    let mut app = app_with(1);
    let id = crate::model::RcaId::new("rca-0").expect("id");
    let dir = app.store.workspace_dir(&id).join("diagrams");
    std::fs::write(dir.join("01-topology.txt"), "box").expect("write");
    std::fs::write(dir.join("02-fix.txt"), "box2").expect("write");
    press(&mut app, KeyCode::Char('7'));
    match press(&mut app, KeyCode::Char('E')) {
        Flow::Edit(path) => assert!(path.ends_with("diagrams/01-topology.txt")),
        other => panic!("expected Flow::Edit, got {other:?}"),
    }
}

#[test]
fn shift_d_confirms_before_deleting_and_only_y_deletes() {
    let mut app = app_with(2);
    let doomed = app.selected_rca().expect("selected").id.clone();
    let dir = app.store.workspace_dir(&doomed);

    // D opens the confirmation; the list is untouched until an answer.
    press(&mut app, KeyCode::Char('D'));
    assert!(app.confirm_delete.is_some(), "popup open");
    assert_eq!(app.rcas.len(), 2);

    // Enter must NOT confirm — a queued enter from navigation would
    // otherwise delete without the user ever seeing the popup.
    press(&mut app, KeyCode::Enter);
    assert!(app.confirm_delete.is_some(), "enter is ignored");
    assert!(dir.exists());

    // n cancels, everything intact.
    press(&mut app, KeyCode::Char('n'));
    assert!(app.confirm_delete.is_none());
    assert!(dir.exists());
    assert_eq!(app.rcas.len(), 2);

    // D then y deletes and reloads.
    press(&mut app, KeyCode::Char('D'));
    press(&mut app, KeyCode::Char('y'));
    assert!(app.confirm_delete.is_none());
    assert!(!dir.exists(), "workspace directory removed");
    assert_eq!(app.rcas.len(), 1);
    assert!(
        app.status_line().is_some_and(|s| s.contains("deleted")),
        "status announces the delete"
    );
}

#[test]
fn shift_d_is_list_only_and_esc_cancels() {
    let mut app = app_with(1);

    // Content focus: D is not a delete trigger while reading.
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('D'));
    assert!(app.confirm_delete.is_none(), "no popup from content focus");

    // Back on the list, esc closes the popup without deleting.
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('D'));
    assert!(app.confirm_delete.is_some());
    press(&mut app, KeyCode::Esc);
    assert!(app.confirm_delete.is_none());
    assert_eq!(app.rcas.len(), 1);

    // Nothing selected: D says so instead of opening an empty popup.
    let mut empty = app_with(0);
    press(&mut empty, KeyCode::Char('D'));
    assert!(empty.confirm_delete.is_none());
    assert!(empty
        .status_line()
        .is_some_and(|s| s.contains("nothing to delete")));
}

#[test]
fn t_opens_the_status_picker_and_enter_applies_the_stage() {
    use crate::model::Status;

    let mut app = app_with(1);
    assert_eq!(
        app.selected_rca().expect("selected").meta.status,
        Status::Investigating
    );

    press(&mut app, KeyCode::Char('t'));
    let picker = app.status_picker.as_ref().expect("picker open");
    assert_eq!(picker.current, Status::Investigating);
    assert_eq!(picker.selected, 0, "current stage highlighted");

    // Move to `review` and apply; the manifest write reloads the list.
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    assert!(app.status_picker.is_none());
    assert_eq!(
        app.selected_rca().expect("selected").meta.status,
        Status::Review
    );
    assert!(
        app.status_line().is_some_and(|s| s.contains("review")),
        "status announces the change"
    );
}

#[test]
fn status_picker_esc_cancels_and_repicking_current_writes_nothing() {
    use crate::model::Status;

    let mut app = app_with(1);
    let before = app
        .store
        .read_meta(&app.selected_rca().expect("selected").id.clone())
        .expect("meta");

    // Esc: no change.
    press(&mut app, KeyCode::Char('t'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Esc);
    assert!(app.status_picker.is_none());
    assert_eq!(
        app.selected_rca().expect("selected").meta.status,
        Status::Investigating
    );

    // Enter on the current stage: closes without stamping `updated`.
    press(&mut app, KeyCode::Char('t'));
    press(&mut app, KeyCode::Enter);
    assert!(app.status_picker.is_none());
    let after = app
        .store
        .read_meta(&app.selected_rca().expect("selected").id.clone())
        .expect("meta");
    assert_eq!(before.updated, after.updated, "no spurious write");

    // Nothing selected: t says so.
    let mut empty = app_with(0);
    press(&mut empty, KeyCode::Char('t'));
    assert!(empty.status_picker.is_none());
    assert!(empty
        .status_line()
        .is_some_and(|s| s.contains("nothing to set")));
}
