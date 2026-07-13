//! Tests for the link popup, related popup, and toolbox overlay
//! (`ui::overlays`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crossterm::event::KeyCode;
use ratatui::text::Text;

use crate::model::{RcaId, Severity};
use crate::store::{new_meta, Store};
use crate::ui::keys::Flow;
use crate::ui::testutil::{app_with, press};
use crate::ui::App;

/// Flattens rendered text to a plain string for containment asserts.
fn flat(text: &Text<'_>) -> String {
    text.lines
        .iter()
        .flat_map(|l| l.spans.iter())
        .map(|s| s.content.as_ref())
        .collect()
}

#[test]
fn o_opens_the_link_popup_with_attached_prs_and_tab_urls() {
    let mut app = app_with(1);
    let id = app.selected_rca().expect("selected").id.clone();
    app.store
        .add_pr(&id, "https://github.com/o/r/pull/12")
        .expect("attach");
    std::fs::write(
        app.store.workspace_dir(&id).join("summary.md"),
        "# Summary\n\nSee https://grafana.example.com/d/abc for the graph.\n",
    )
    .expect("write");
    app.reload();

    press(&mut app, KeyCode::Char('o'));
    let popup = app.links().expect("popup open");
    assert_eq!(
        popup.items,
        [
            "https://github.com/o/r/pull/12",
            "https://grafana.example.com/d/abc",
        ],
        "PRs first, then tab URLs"
    );

    // j moves and clamps; esc closes without opening anything.
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.links().expect("open").selected, 1, "clamped to last");
    press(&mut app, KeyCode::Esc);
    assert!(app.links().is_none());
}

#[test]
fn o_with_no_links_reports_instead_of_opening_an_empty_popup() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Char('o'));
    assert!(app.links().is_none());
    assert!(app
        .status_line()
        .expect("status set")
        .contains("no attached PRs"));
}

/// An app over three workspaces: `target` and `alloy-again` share the
/// `alloy` system; `unrelated` shares nothing.
fn app_with_related() -> App {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = Store::open(tmp.path()).expect("store");
    let scaffold = |slug: &str, title: &str, systems: &[&str]| {
        let mut meta = new_meta(title.to_owned(), Severity::Medium);
        meta.systems = systems.iter().map(ToString::to_string).collect();
        store
            .scaffold(&RcaId::new(slug).expect("valid id"), &meta)
            .expect("scaffold");
    };
    scaffold("target", "Alloy breaks again", &["alloy"]);
    scaffold("alloy-again", "Alloy OOM last month", &["alloy", "mimir"]);
    scaffold("unrelated", "SendGrid webhooks", &["sendgrid"]);
    std::mem::forget(tmp); // OS cleans the temp root; see app_with
    let mut app = App::new(store).expect("app");
    while app.selected_rca().expect("non-empty").id.as_str() != "target" {
        press(&mut app, KeyCode::Char('j'));
    }
    app
}

#[test]
fn r_opens_related_and_enter_jumps_to_the_workspace() {
    let mut app = app_with_related();
    press(&mut app, KeyCode::Char('R'));
    let popup = app.related().expect("popup open");
    assert_eq!(popup.items.len(), 1, "only the alloy-sharing workspace");
    assert_eq!(popup.items[0].id.as_str(), "alloy-again");
    assert_eq!(popup.items[0].shared, "system: alloy");

    press(&mut app, KeyCode::Enter);
    assert!(app.related().is_none(), "enter closes the popup");
    assert_eq!(
        app.selected_rca().expect("selected").id.as_str(),
        "alloy-again",
        "enter jumps the sidebar selection"
    );
    assert!(app
        .status_line()
        .expect("status set")
        .contains("jumped to alloy-again"));
}

#[test]
fn r_with_nothing_shared_reports_instead_of_an_empty_popup() {
    let mut app = app_with(2); // scaffolds carry no systems or tags
    press(&mut app, KeyCode::Char('R'));
    assert!(app.related().is_none());
    assert!(app
        .status_line()
        .expect("status set")
        .contains("no related incidents"));
}

#[test]
fn related_popup_owns_its_keys_until_closed() {
    let mut app = app_with_related();
    press(&mut app, KeyCode::Char('R'));
    assert_eq!(
        press(&mut app, KeyCode::Char('q')),
        Flow::Continue,
        "q closes the popup instead of quitting"
    );
    assert!(app.related().is_none());
    assert_eq!(
        app.selected_rca().expect("selected").id.as_str(),
        "target",
        "closing without enter leaves the selection alone"
    );
}

#[test]
fn toolbox_opens_scrolls_and_closes_without_leaking_keys() {
    let mut app = app_with(1);
    assert!(app.toolbox().is_none());
    press(&mut app, KeyCode::Char('T'));
    assert!(app.toolbox().is_some(), "T opens the overlay");

    // With no toolbox.md on disk, T scaffolds it on the spot and shows
    // the freshly written template.
    assert!(
        app.store.root().join("toolbox.md").is_file(),
        "T runs the init when the toolbox is missing"
    );
    let rendered = flat(&app.toolbox().expect("open").clone());
    assert!(
        rendered.contains("Scaffolded toolbox.md"),
        "note shown: {rendered}"
    );
    assert!(
        rendered.contains("Toolbox"),
        "template rendered: {rendered}"
    );

    // Keys scroll the overlay instead of the app; q closes instead of
    // typing/quitting.
    app.toolbox_viewport = (50, 10);
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.toolbox_scroll(), 1);
    press(&mut app, KeyCode::Char('G'));
    assert_eq!(app.toolbox_scroll(), 40, "clamped to content bottom");
    assert_eq!(press(&mut app, KeyCode::Char('q')), Flow::Continue);
    assert!(app.toolbox().is_none(), "q closes the overlay");
}

#[test]
fn toolbox_auto_init_never_overwrites_an_existing_toolbox() {
    let mut app = app_with(1);
    std::fs::write(
        app.store.root().join("toolbox.md"),
        "# Toolbox\n\ncustomized\n",
    )
    .expect("write");
    press(&mut app, KeyCode::Char('T'));
    let rendered = flat(&app.toolbox().expect("open").clone());
    assert!(
        rendered.contains("customized"),
        "existing content shown: {rendered}"
    );
    assert!(
        !rendered.contains("Scaffolded"),
        "no scaffold note when the toolbox already exists"
    );
}

#[test]
fn toolbox_renders_toolbox_md_and_matching_system_docs() {
    let mut app = app_with(1); // workspace has no systems → all docs shown
    let root = app.store.root().to_owned();
    std::fs::write(root.join("toolbox.md"), "# Toolbox\n\n- grafana\n").expect("write");
    std::fs::create_dir_all(root.join("systems")).expect("mkdir");
    std::fs::write(root.join("systems/api.md"), "# api\n").expect("write");

    press(&mut app, KeyCode::Char('T'));
    let rendered = flat(&app.toolbox().expect("open").clone());
    assert!(rendered.contains("Toolbox"));
    assert!(rendered.contains("grafana"));
    assert!(rendered.contains("systems/api.md"), "doc separator shown");
}
