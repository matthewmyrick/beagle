//! Tests for agent-skill installation (`skills`).
#![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

use std::ffi::OsString;
use std::fs;

use super::{agent_targets, binary_on_path, detected, install, Outcome, SKILL_MD};

#[test]
fn the_embedded_skill_is_the_real_one() {
    assert!(
        SKILL_MD.starts_with("---\nname: beagle\n"),
        "frontmatter intact"
    );
    assert!(SKILL_MD.contains("beagle new"), "authoring flow present");
}

#[test]
fn targets_cover_all_three_agents_with_their_conventions() {
    let home = std::path::Path::new("/home/u");
    let targets = agent_targets(home, None);
    let files: Vec<String> = targets
        .iter()
        .map(|t| t.skill_file.display().to_string())
        .collect();
    assert_eq!(
        files,
        [
            "/home/u/.claude/skills/beagle/SKILL.md",
            "/home/u/.codex/skills/beagle/SKILL.md",
            "/home/u/.config/opencode/skill/beagle/SKILL.md",
        ]
    );

    // XDG_CONFIG_HOME moves opencode (and only opencode).
    let xdg = std::path::Path::new("/xdg");
    let targets = agent_targets(home, Some(xdg));
    assert!(targets[2]
        .skill_file
        .starts_with("/xdg/opencode/skill/beagle"));
    assert!(targets[0].skill_file.starts_with("/home/u/.claude"));
}

#[test]
fn detection_finds_binaries_on_path_or_config_dirs() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let bin = tmp.path().join("bin");
    fs::create_dir_all(&bin).expect("bin dir");
    fs::write(bin.join("codex"), "#!/bin/sh\n").expect("fake binary");
    let path_var = OsString::from(bin.display().to_string());

    let targets = agent_targets(tmp.path(), None);
    let claude = &targets[0];
    let codex = &targets[1];
    let opencode = &targets[2];

    assert!(binary_on_path("codex", Some(path_var.as_os_str())));
    assert!(detected(codex, Some(path_var.as_os_str())), "via PATH");
    assert!(
        !detected(claude, Some(path_var.as_os_str())),
        "no binary, no config dir"
    );

    // A config dir counts even without a PATH hit (wrapper functions).
    fs::create_dir_all(&claude.config_dir).expect("claude dir");
    assert!(
        detected(claude, Some(path_var.as_os_str())),
        "via config dir"
    );
    assert!(!detected(opencode, None), "absent everywhere");
}

#[test]
fn install_creates_then_updates_and_skips_missing_agents() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let targets = agent_targets(tmp.path(), None);
    let claude = &targets[0];
    fs::create_dir_all(&claude.config_dir).expect("claude dir");

    assert_eq!(install(claude, None).expect("install"), Outcome::Created);
    let written = fs::read_to_string(&claude.skill_file).expect("skill written");
    assert_eq!(written, SKILL_MD);

    // Overwrite keeps the skill in sync with the binary.
    fs::write(&claude.skill_file, "customized").expect("scribble");
    assert_eq!(install(claude, None).expect("re-install"), Outcome::Updated);
    assert_eq!(
        fs::read_to_string(&claude.skill_file).expect("re-read"),
        SKILL_MD
    );

    // Codex has no binary and no config dir here: skipped, nothing written.
    let codex = &targets[1];
    assert_eq!(install(codex, None).expect("skip"), Outcome::NotFound);
    assert!(!codex.skill_file.exists());
}
