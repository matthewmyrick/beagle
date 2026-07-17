//! Tests for the settings overlay (`ui::settings`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crossterm::event::KeyCode;

use crate::ui::testutil::{app_with, press};

/// One combined test: `BEAGLE_CONFIG` is process-global, so exercising the
/// whole overlay in a single function avoids env races between parallel
/// tests.
#[test]
fn settings_overlay_edits_write_the_config_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("config.toml");
    std::env::set_var(crate::config::CONFIG_ENV, &path);

    let mut app = app_with(1);
    press(&mut app, KeyCode::Char('S'));
    let overlay = app.settings().expect("S opens the overlay");
    assert_eq!(overlay.config, crate::config::Config::default());

    // Toggle notify (third row): the file is created from the template
    // (comments and all) with the value set, and it applies live.
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char(' '));
    let content = std::fs::read_to_string(&path).expect("config written");
    assert!(content.contains("notify = true"), "{content}");
    assert!(
        content.contains("# beagle configuration."),
        "template comments preserved: {content}"
    );
    assert_eq!(
        app.settings().expect("open").config.notify,
        Some(true),
        "overlay refreshed from the parsed file"
    );

    // Toggle again: same line flips, no duplicates.
    press(&mut app, KeyCode::Char(' '));
    let content = std::fs::read_to_string(&path).expect("config re-read");
    assert!(content.contains("notify = false"));
    assert_eq!(
        content
            .lines()
            .filter(|l| l.trim_start().starts_with("notify ="))
            .count(),
        1,
        "exactly one active notify line — the toggle never duplicates it"
    );

    // Inline-edit the editor field; q types instead of closing.
    press(&mut app, KeyCode::Char('k'));
    press(&mut app, KeyCode::Enter);
    for c in "hxq".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Backspace); // drop the q again
    press(&mut app, KeyCode::Enter);
    let content = std::fs::read_to_string(&path).expect("config re-read");
    assert!(content.contains("editor = \"hx\""), "{content}");
    assert_eq!(
        app.settings().expect("open").config.editor.as_deref(),
        Some("hx")
    );

    // Esc cancels an in-flight edit without writing...
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('z'));
    press(&mut app, KeyCode::Esc);
    assert!(
        app.settings().expect("open").editing.is_none(),
        "first esc cancels the edit"
    );
    assert_eq!(
        app.settings().expect("open").config.editor.as_deref(),
        Some("hx"),
        "cancelled edit never touched the file"
    );

    // ...and the next esc closes the overlay.
    press(&mut app, KeyCode::Esc);
    assert!(app.settings().is_none());

    std::env::remove_var(crate::config::CONFIG_ENV);
}
