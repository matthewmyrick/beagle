//! Tests for the link popup and toolbox overlay (`ui::overlays`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crossterm::event::KeyCode;
use ratatui::text::Text;

use crate::ui::keys::Flow;
use crate::ui::testutil::{app_with, press};

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
