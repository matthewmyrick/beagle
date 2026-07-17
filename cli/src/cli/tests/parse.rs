//! Tests for CLI argument parsing (`cli`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

fn parse(argv: &[&str]) -> Result<Command, String> {
    parse_args(argv.iter().map(ToString::to_string))
}

#[test]
fn bare_invocation_is_tui_with_no_explicit_root() {
    assert!(matches!(parse(&[]), Ok(Command::Tui { root: None })));
    assert!(matches!(
        parse(&["--root", "/x"]),
        Ok(Command::Tui { root: Some(_) })
    ));
}

#[test]
fn new_parses_all_flags() {
    let parsed = parse(&[
        "new",
        "pay-latency",
        "--title",
        "Payments latency",
        "--severity",
        "high",
        "--system",
        "payments-api",
        "--system",
        "redis",
        "--root",
        "/tmp/x",
    ]);
    match parsed {
        Ok(Command::New {
            root,
            id,
            title,
            severity,
            systems,
        }) => {
            assert_eq!(root, Some(PathBuf::from("/tmp/x")));
            assert_eq!(id.as_str(), "pay-latency");
            assert_eq!(title, "Payments latency");
            assert_eq!(severity, Severity::High);
            assert_eq!(systems, ["payments-api", "redis"]);
        }
        other => panic!("unexpected parse: {other:?}"),
    }
}

#[test]
fn new_rejects_bad_input() {
    assert!(parse(&["new"]).is_err(), "missing id");
    assert!(parse(&["new", "ok-id"]).is_err(), "missing --title");
    assert!(
        parse(&["new", "Bad_Id", "--title", "t"]).is_err(),
        "invalid slug"
    );
    assert!(parse(&["new", "ok-id", "--title", "t", "--severity", "huge"]).is_err());
    assert!(
        parse(&["new", "ok-id", "--title", "   "]).is_err(),
        "blank title"
    );
}

#[test]
fn export_parses_id_out_and_root() {
    let parsed = parse(&[
        "export",
        "my-rca",
        "--out",
        "/tmp/vault/note.md",
        "--root",
        "/x",
    ]);
    match parsed {
        Ok(Command::Export { root, id, out }) => {
            assert_eq!(root, Some(PathBuf::from("/x")));
            assert_eq!(id.as_str(), "my-rca");
            assert_eq!(out, Some(PathBuf::from("/tmp/vault/note.md")));
        }
        other => panic!("unexpected parse: {other:?}"),
    }
    assert!(parse(&["export"]).is_err(), "missing id");
    assert!(parse(&["export", "Bad Slug"]).is_err(), "invalid slug");
}

#[test]
fn unknown_flags_and_commands_are_rejected() {
    assert!(parse(&["--frobnicate"]).is_err());
    assert!(parse(&["destroy"]).is_err());
}

#[test]
fn list_parses_filters() {
    match parse(&["list", "--status", "investigating", "--severity", "high"]) {
        Ok(Command::List {
            status, severity, ..
        }) => {
            assert_eq!(status, Some(Status::Investigating));
            assert_eq!(severity, Some(Severity::High));
        }
        other => panic!("unexpected parse: {other:?}"),
    }
    match parse(&["list"]) {
        Ok(Command::List {
            status, severity, ..
        }) => {
            assert_eq!(status, None);
            assert_eq!(severity, None);
        }
        other => panic!("unexpected parse: {other:?}"),
    }
    assert!(parse(&["list", "--status", "closed"]).is_err());
    assert!(parse(&["list", "--out", "x"]).is_err());
}

#[test]
fn status_parses_id_status_and_root() {
    match parse(&["status", "my-rca", "investigating", "--root", "/x"]) {
        Ok(Command::SetStatus { root, id, status }) => {
            assert_eq!(root, Some(PathBuf::from("/x")));
            assert_eq!(id.as_str(), "my-rca");
            assert_eq!(status, Status::Investigating);
        }
        other => panic!("unexpected parse: {other:?}"),
    }
    assert!(parse(&["status"]).is_err(), "missing id");
    assert!(parse(&["status", "my-rca"]).is_err(), "missing status");
    assert!(parse(&["status", "my-rca", "closed"]).is_err());
    assert!(parse(&["status", "Bad Slug", "resolved"]).is_err());
}

#[test]
fn log_parses_multi_word_messages_and_root() {
    match parse(&[
        "log",
        "my-rca",
        "checked",
        "the",
        "dashboard",
        "--root",
        "/x",
    ]) {
        Ok(Command::Log { root, id, message }) => {
            assert_eq!(root, Some(PathBuf::from("/x")));
            assert_eq!(id.as_str(), "my-rca");
            assert_eq!(message, "checked the dashboard");
        }
        other => panic!("unexpected parse: {other:?}"),
    }
    assert!(parse(&["log"]).is_err(), "missing id");
    assert!(parse(&["log", "my-rca"]).is_err(), "missing message");
    assert!(parse(&["log", "my-rca", "msg", "--force"]).is_err());
}

#[test]
fn similar_parses_id_and_root() {
    match parse(&["similar", "my-rca", "--root", "/x"]) {
        Ok(Command::Similar { root, id }) => {
            assert_eq!(root, Some(PathBuf::from("/x")));
            assert_eq!(id.as_str(), "my-rca");
        }
        other => panic!("unexpected parse: {other:?}"),
    }
    assert!(parse(&["similar"]).is_err(), "missing id");
    assert!(parse(&["similar", "Bad Slug"]).is_err(), "invalid slug");
    assert!(parse(&["similar", "my-rca", "--frob"]).is_err());
}

#[test]
fn pr_parses_add_and_list() {
    match parse(&[
        "pr",
        "add",
        "my-rca",
        "https://github.com/o/r/pull/1",
        "--root",
        "/x",
    ]) {
        Ok(Command::PrAdd { root, id, url }) => {
            assert_eq!(root, Some(PathBuf::from("/x")));
            assert_eq!(id.as_str(), "my-rca");
            assert_eq!(url, "https://github.com/o/r/pull/1");
        }
        other => panic!("unexpected parse: {other:?}"),
    }
    assert!(matches!(
        parse(&["pr", "list", "my-rca"]),
        Ok(Command::PrList { .. })
    ));
    assert!(parse(&["pr"]).is_err(), "missing subcommand");
    assert!(parse(&["pr", "close", "my-rca"]).is_err(), "bad subcommand");
    assert!(parse(&["pr", "add", "my-rca"]).is_err(), "missing url");
    assert!(parse(&["pr", "list", "my-rca", "--frob"]).is_err());
}

#[test]
fn banner_parses_and_rejects_arguments() {
    assert!(matches!(parse(&["banner"]), Ok(Command::Banner)));
    assert!(parse(&["banner", "--loud"]).is_err());
}

#[test]
fn config_parses_and_rejects_arguments() {
    assert!(matches!(parse(&["config"]), Ok(Command::Config)));
    assert!(parse(&["config", "extra"]).is_err());
}

#[test]
fn init_parses_with_optional_root() {
    assert!(matches!(parse(&["init"]), Ok(Command::Init { root: None })));
    assert!(matches!(
        parse(&["init", "--root", "/x"]),
        Ok(Command::Init { root: Some(_) })
    ));
    assert!(parse(&["init", "--frob"]).is_err());
}

#[test]
fn version_and_version_list_parse() {
    assert!(matches!(parse(&["version"]), Ok(Command::Version)));
    assert!(matches!(
        parse(&["version", "list"]),
        Ok(Command::VersionList)
    ));
    assert!(parse(&["version", "bump"]).is_err());
    assert!(parse(&["version", "list", "extra"]).is_err());
}

#[test]
fn update_parses_an_optional_target_version() {
    assert!(matches!(
        parse(&["update"]),
        Ok(Command::Update { version: None })
    ));
    match parse(&["update", "--version", "v0.1.0"]) {
        Ok(Command::Update {
            version: Some(version),
        }) => assert_eq!(version.tag(), "v0.1.0"),
        other => panic!("unexpected parse: {other:?}"),
    }
    assert!(parse(&["update", "--version", "latest"]).is_err());
    assert!(parse(&["update", "--force"]).is_err());
}

#[test]
fn archive_parses_id_force_and_root() {
    let cmd = parse(&["archive", "old-rca", "--force", "--root", "/r"]).expect("parse");
    match cmd {
        Command::Archive { root, id, force } => {
            assert_eq!(id.as_str(), "old-rca");
            assert!(force);
            assert_eq!(root, Some(std::path::PathBuf::from("/r")));
        }
        other => panic!("wrong command: {other:?}"),
    }
    assert!(parse(&["archive"]).is_err(), "id is required");
}

#[test]
fn list_parses_the_archived_flag() {
    match parse(&["list", "--archived"]).expect("parse") {
        Command::List { archived, .. } => assert!(archived),
        other => panic!("wrong command: {other:?}"),
    }
    match parse(&["list"]).expect("parse") {
        Command::List { archived, .. } => assert!(!archived),
        other => panic!("wrong command: {other:?}"),
    }
}

#[test]
fn unarchive_parses_id_and_root() {
    match parse(&["unarchive", "old-rca", "--root", "/r"]).expect("parse") {
        Command::Unarchive { root, id } => {
            assert_eq!(id.as_str(), "old-rca");
            assert_eq!(root, Some(std::path::PathBuf::from("/r")));
        }
        other => panic!("wrong command: {other:?}"),
    }
    assert!(parse(&["unarchive"]).is_err(), "id is required");
}

#[test]
fn skill_parses_status_install_and_default() {
    assert!(matches!(
        parse(&["skill"]),
        Ok(Command::Skill {
            action: SkillAction::Status
        })
    ));
    assert!(matches!(
        parse(&["skill", "status"]),
        Ok(Command::Skill {
            action: SkillAction::Status
        })
    ));
    assert!(matches!(
        parse(&["skill", "install"]),
        Ok(Command::Skill {
            action: SkillAction::Install
        })
    ));
    assert!(parse(&["skill", "frobnicate"]).is_err());
    assert!(parse(&["skill", "install", "extra"]).is_err());
}

#[test]
fn publish_and_unpublish_parse_id_and_root() {
    match parse(&["publish", "my-rca", "--root", "/r"]).expect("parse") {
        Command::SetPublished {
            root,
            id,
            published,
        } => {
            assert_eq!(id.as_str(), "my-rca");
            assert!(published);
            assert_eq!(root, Some(std::path::PathBuf::from("/r")));
        }
        other => panic!("wrong command: {other:?}"),
    }
    match parse(&["unpublish", "my-rca"]).expect("parse") {
        Command::SetPublished { published, .. } => assert!(!published),
        other => panic!("wrong command: {other:?}"),
    }
    assert!(parse(&["publish"]).is_err(), "id required");
}

#[test]
fn handoff_parses_id_and_root() {
    match parse(&["handoff", "my-rca", "--root", "/r"]).expect("parse") {
        Command::Handoff { root, id } => {
            assert_eq!(id.as_str(), "my-rca");
            assert_eq!(root, Some(std::path::PathBuf::from("/r")));
        }
        other => panic!("wrong command: {other:?}"),
    }
    assert!(parse(&["handoff"]).is_err(), "id is required");
}
