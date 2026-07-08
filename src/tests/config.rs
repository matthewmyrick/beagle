//! Tests for `config`.
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use super::*;

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
    };
    assert_eq!(editor(Some(&config)), "hx");
}
