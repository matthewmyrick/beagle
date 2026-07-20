//! Tests for `config`.
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use super::*;

#[test]
fn upsert_replaces_uncomments_appends_and_preserves_comments() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("config.toml");

    // Missing file: created from the template, key uncommented in place.
    let config = upsert(&path, "notify", "true").expect("create + set");
    assert_eq!(config.notify, Some(true));
    let content = std::fs::read_to_string(&path).expect("read");
    assert!(content.contains("notify = true"));
    assert!(
        content.contains("# Desktop notifications"),
        "surrounding comments survive: {content}"
    );

    // Active line: replaced, not duplicated.
    let config = upsert(&path, "notify", "false").expect("flip");
    assert_eq!(config.notify, Some(false));
    let content = std::fs::read_to_string(&path).expect("read");
    assert_eq!(content.matches("notify =").count(), 1, "{content}");

    // Missing key with no template line: appended.
    std::fs::write(&path, "# my precious comment\n").expect("write");
    let config = upsert(&path, "editor", "\"hx\"").expect("append");
    assert_eq!(config.editor.as_deref(), Some("hx"));
    let content = std::fs::read_to_string(&path).expect("read");
    assert!(content.starts_with("# my precious comment\n"));
    assert!(content.contains("editor = \"hx\""));

    // Invalid value: rejected, file untouched.
    let before = std::fs::read_to_string(&path).expect("read");
    assert!(upsert(&path, "notify", "\"not-a-bool\"").is_err());
    assert_eq!(std::fs::read_to_string(&path).expect("read"), before);
}

#[test]
fn template_is_a_valid_all_defaults_config() {
    let config = parse(TEMPLATE).expect("template must parse");
    assert_eq!(config, Config::default(), "template is fully commented");
}

#[test]
fn empty_config_is_valid() {
    assert_eq!(parse("").expect("empty parses"), Config::default());
}

#[test]
fn full_config_round_trips() {
    let config = parse("root = \"/oncall\"\neditor = \"code -w\"\n").expect("parses");
    assert_eq!(config.root.as_deref(), Some(Path::new("/oncall")));
    assert_eq!(config.editor.as_deref(), Some("code -w"));
}

#[test]
fn unknown_fields_are_rejected() {
    let err = parse("roots = \"/typo\"").expect_err("typo must be caught");
    assert!(err.to_string().contains("roots"), "error names the field");
}

#[test]
fn invalid_types_are_rejected() {
    assert!(parse("root = 7").is_err());
}

#[test]
fn missing_file_loads_as_none() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let missing = tmp.path().join("nope.toml");
    assert_eq!(load(&missing).expect("absent is fine"), None);
}

#[test]
fn config_editor_wins_over_fallback() {
    let config = Config {
        root: None,
        editor: Some("hx".to_owned()),
        notify: None,
        notify_events: None,
        handoff: None,
    };
    assert_eq!(editor(Some(&config)), "hx");
}

#[test]
fn notify_events_parses_a_partial_table() {
    use crate::config::parse;
    let cfg = parse("notify = true\n[notify_events]\nfinal_review = true\nfinished = true\n")
        .expect("parses");
    let events = cfg.notify_events.expect("table present");
    assert!(events.final_review && events.finished);
    assert!(!events.investigating && !events.review && !events.agent && !events.new_incident);
}

#[test]
fn notify_events_all_enables_every_event() {
    use crate::config::NotifyEvents;
    let all = NotifyEvents::all();
    assert!(
        all.new_incident
            && all.investigating
            && all.review
            && all.agent
            && all.final_review
            && all.finished
    );
    // Absent by default (off).
    assert_eq!(NotifyEvents::default(), NotifyEvents::default());
    assert!(!NotifyEvents::default().finished);
}

#[test]
fn find_project_file_walks_up_and_prefers_the_nearest() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let outer = tmp.path();
    let inner = outer.join("services").join("payments");
    std::fs::create_dir_all(&inner).expect("mkdir");

    assert_eq!(find_project_file(&inner), None, "no .beagle anywhere yet");

    std::fs::write(outer.join(PROJECT_FILE), "").expect("write outer");
    assert_eq!(
        find_project_file(&inner),
        Some(outer.join(PROJECT_FILE)),
        "found walking up"
    );

    // A nearer one shadows the ancestor, like nested git repos.
    std::fs::write(inner.join(PROJECT_FILE), "").expect("write inner");
    assert_eq!(find_project_file(&inner), Some(inner.join(PROJECT_FILE)));

    // A .beagle *directory* is not a project file.
    let elsewhere = outer.join("elsewhere");
    std::fs::create_dir_all(elsewhere.join(PROJECT_FILE)).expect("mkdir");
    assert_eq!(
        find_project_file(&elsewhere),
        Some(outer.join(PROJECT_FILE))
    );
}

#[test]
fn project_file_pins_the_root_like_git() {
    let dir = Path::new("/work/oncall");
    let file = dir.join(PROJECT_FILE);

    // Empty .beagle: its own directory is the root.
    let config = resolve_project(&file, Config::default(), None);
    assert_eq!(config.root.as_deref(), Some(dir));

    // Relative root resolves against the .beagle's directory.
    let project = parse("root = \"stores/prod\"").expect("parse");
    let config = resolve_project(&file, project, None);
    assert_eq!(
        config.root.as_deref(),
        Some(dir.join("stores/prod").as_path())
    );

    // Absolute root is taken as-is.
    let project = parse("root = \"/srv/rcas\"").expect("parse");
    let config = resolve_project(&file, project, None);
    assert_eq!(config.root.as_deref(), Some(Path::new("/srv/rcas")));
}

#[test]
fn project_fields_win_and_unset_fields_fall_back_to_global() {
    let file = Path::new("/work/oncall").join(PROJECT_FILE);
    let project = parse("editor = \"hx\"").expect("parse");
    let global = parse("root = \"/ignored\"\neditor = \"vim\"\nnotify = true").expect("parse");

    let config = resolve_project(&file, project, Some(global));
    assert_eq!(config.editor.as_deref(), Some("hx"), "project wins");
    assert_eq!(config.notify, Some(true), "unset falls back to global");
    assert_eq!(
        config.root.as_deref(),
        Some(Path::new("/work/oncall")),
        "the .beagle pins the root — the global root never leaks through"
    );
}

#[test]
fn load_effective_reads_a_real_dot_beagle() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let nested = tmp.path().join("deep").join("er");
    std::fs::create_dir_all(&nested).expect("mkdir");
    std::fs::write(tmp.path().join(PROJECT_FILE), "").expect("write");

    let config = load_effective(&nested)
        .expect("load")
        .expect("project config found");
    // Only the root is asserted — other fields may merge from whatever
    // global config exists on the machine running the tests.
    assert_eq!(config.root.as_deref(), Some(tmp.path()));

    // An invalid .beagle is an error, not silently the default config.
    std::fs::write(tmp.path().join(PROJECT_FILE), "root = 42").expect("write");
    assert!(load_effective(&nested).is_err());
}

#[test]
fn write_project_file_pins_roots_and_never_overwrites() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // No root: an empty file — the directory itself is the root.
    let path = write_project_file(tmp.path(), None).expect("write empty");
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "");
    let config = load_effective(tmp.path())
        .expect("load")
        .expect("project config");
    assert_eq!(config.root.as_deref(), Some(tmp.path()));

    // Existing file: refused, contents untouched.
    assert!(matches!(
        write_project_file(tmp.path(), Some(Path::new("/elsewhere"))),
        Err(Error::AlreadyExists(_))
    ));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "");

    // Explicit root: a single valid assignment, relative kept verbatim.
    let other = tmp.path().join("project");
    std::fs::create_dir_all(&other).expect("mkdir");
    let path = write_project_file(&other, Some(Path::new("ops/rcas"))).expect("write");
    let written = std::fs::read_to_string(&path).expect("read");
    assert_eq!(written, "root = \"ops/rcas\"\n");
    let config = load_effective(&other)
        .expect("load")
        .expect("project config");
    assert_eq!(
        config.root.as_deref(),
        Some(other.join("ops/rcas").as_path()),
        "relative root resolves against the .beagle's directory"
    );
}
