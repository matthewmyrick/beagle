//! Tests for `config`.
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::*;

/// Writes `contents` to a temp file and loads it through the real loader, so
/// path expansion and validation run exactly as in production.
fn load_str(contents: &str) -> Result<Config, LoadError> {
    let file = tempfile::NamedTempFile::new().expect("temp file");
    std::fs::write(file.path(), contents).expect("write config");
    load(file.path())
}

#[test]
fn missing_file_loads_as_empty() {
    let config = load(Path::new("/no/such/dir/agents.toml")).expect("missing = empty");
    assert!(config.agents.is_empty());
}

#[test]
fn parses_full_config_and_applies_defaults() {
    let config = load_str(
        r#"
[[agent]]
id = "rca-remediation"
enabled = false
trigger = { kind = "rca-status", status = "agent" }
target_repo = "/home/me/beagle"
prompt = "/home/me/prompt.txt"
allowed_tools = ["Read", "Edit", "Bash(git:*)"]
poll_interval = "90s"
max_concurrent = 3

[[agent]]
id = "minimal"
trigger = { kind = "rca-status", status = "agent" }
target_repo = "/repo"
prompt = "/p.txt"
"#,
    )
    .expect("valid config");

    assert_eq!(config.agents.len(), 2);

    let full = &config.agents[0];
    assert_eq!(full.id, "rca-remediation");
    assert!(!full.enabled);
    match &full.trigger {
        Trigger::RcaStatus { status } => assert_eq!(status, "agent"),
    }
    assert_eq!(full.target_repo, PathBuf::from("/home/me/beagle"));
    assert_eq!(full.prompt, PathBuf::from("/home/me/prompt.txt"));
    assert_eq!(full.allowed_tools, ["Read", "Edit", "Bash(git:*)"]);
    assert_eq!(full.poll_interval, Duration::from_secs(90));
    assert_eq!(full.max_concurrent, 3);

    // Omitted fields fall back to their defaults.
    let minimal = &config.agents[1];
    assert!(minimal.enabled);
    assert!(minimal.allowed_tools.is_empty());
    assert_eq!(minimal.poll_interval, Duration::from_secs(60));
    assert_eq!(minimal.max_concurrent, 2);
}

#[test]
fn unknown_top_level_key_is_rejected() {
    let err = load_str("nonsense = true\n").expect_err("unknown key");
    assert!(matches!(err, LoadError::Parse { .. }), "got {err:?}");
}

#[test]
fn unknown_agent_field_is_rejected() {
    let err = load_str(
        r#"
[[agent]]
id = "x"
trigger = { kind = "rca-status", status = "agent" }
target_repo = "/r"
prompt = "/p"
bogus = 1
"#,
    )
    .expect_err("unknown field");
    assert!(matches!(err, LoadError::Parse { .. }), "got {err:?}");
}

#[test]
fn unknown_trigger_kind_is_rejected() {
    let err = load_str(
        r#"
[[agent]]
id = "x"
trigger = { kind = "cron", schedule = "* * * * *" }
target_repo = "/r"
prompt = "/p"
"#,
    )
    .expect_err("unknown trigger kind");
    assert!(matches!(err, LoadError::Parse { .. }), "got {err:?}");
}

#[test]
fn bad_duration_is_rejected() {
    for bad in ["\"5x\"", "\"soon\"", "\"60\""] {
        let toml = format!(
            r#"
[[agent]]
id = "x"
trigger = {{ kind = "rca-status", status = "agent" }}
target_repo = "/r"
prompt = "/p"
poll_interval = {bad}
"#
        );
        let err = load_str(&toml).expect_err("bad duration");
        assert!(matches!(err, LoadError::Parse { .. }), "{bad}: {err:?}");
    }
}

#[test]
fn empty_id_is_invalid() {
    let err = load_str(
        r#"
[[agent]]
id = "   "
trigger = { kind = "rca-status", status = "agent" }
target_repo = "/r"
prompt = "/p"
"#,
    )
    .expect_err("empty id");
    assert!(matches!(err, LoadError::Invalid { .. }), "got {err:?}");
}

#[test]
fn zero_max_concurrent_is_invalid() {
    let err = load_str(
        r#"
[[agent]]
id = "x"
trigger = { kind = "rca-status", status = "agent" }
target_repo = "/r"
prompt = "/p"
max_concurrent = 0
"#,
    )
    .expect_err("zero concurrency");
    assert!(matches!(err, LoadError::Invalid { .. }), "got {err:?}");
}

#[test]
fn zero_poll_interval_is_invalid() {
    let err = load_str(
        r#"
[[agent]]
id = "x"
trigger = { kind = "rca-status", status = "agent" }
target_repo = "/r"
prompt = "/p"
poll_interval = "0s"
"#,
    )
    .expect_err("zero interval");
    assert!(matches!(err, LoadError::Invalid { .. }), "got {err:?}");
}

#[test]
fn empty_trigger_status_is_invalid() {
    let err = load_str(
        r#"
[[agent]]
id = "x"
trigger = { kind = "rca-status", status = "" }
target_repo = "/r"
prompt = "/p"
"#,
    )
    .expect_err("empty status");
    assert!(matches!(err, LoadError::Invalid { .. }), "got {err:?}");
}

#[test]
fn parse_duration_units() {
    assert_eq!(parse_duration("500ms"), Ok(Duration::from_millis(500)));
    assert_eq!(parse_duration("30s"), Ok(Duration::from_secs(30)));
    assert_eq!(parse_duration("5m"), Ok(Duration::from_secs(300)));
    assert_eq!(parse_duration(" 2h "), Ok(Duration::from_secs(7200)));
    assert!(parse_duration("").is_err());
    assert!(parse_duration("10").is_err());
    assert!(parse_duration("10x").is_err());
    assert!(parse_duration("abc").is_err());
}

#[test]
fn expand_tilde_expands_only_leading_tilde() {
    let home = OsStr::new("/home/me");
    assert_eq!(
        expand_tilde(Path::new("~/x/y"), Some(home)),
        PathBuf::from("/home/me/x/y")
    );
    assert_eq!(
        expand_tilde(Path::new("~"), Some(home)),
        PathBuf::from("/home/me")
    );
    // No leading tilde: unchanged.
    assert_eq!(
        expand_tilde(Path::new("/abs/path"), Some(home)),
        PathBuf::from("/abs/path")
    );
    // Tilde but no HOME: unchanged.
    assert_eq!(expand_tilde(Path::new("~/x"), None), PathBuf::from("~/x"));
}

#[test]
fn default_path_prefers_xdg_then_home() {
    let xdg = OsStr::new("/cfg");
    let home = OsStr::new("/home/me");
    assert_eq!(
        default_path_from(Some(xdg), Some(home)),
        Some(PathBuf::from("/cfg/beagle/agents.toml"))
    );
    // Empty XDG falls through to HOME.
    assert_eq!(
        default_path_from(Some(OsStr::new("")), Some(home)),
        Some(PathBuf::from("/home/me/.config/beagle/agents.toml"))
    );
    assert_eq!(
        default_path_from(None, Some(home)),
        Some(PathBuf::from("/home/me/.config/beagle/agents.toml"))
    );
    assert_eq!(default_path_from(None, None), None);
}
