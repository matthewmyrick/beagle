//! Tests for skill installation (`skill`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

#[test]
fn bundled_skill_is_embedded_and_looks_like_the_skill() {
    assert!(
        SKILL_MD.contains("name: beagle"),
        "the embedded content is the beagle skill"
    );
    assert!(
        SKILL_MD.contains("## 1. Scaffold"),
        "the embedded content is the full skill body"
    );
}

#[test]
fn body_without_frontmatter_strips_the_yaml_block() {
    let md = "---\nname: beagle\ndescription: x\n---\n\n# Heading\n\nbody";
    assert_eq!(body_without_frontmatter(md), "# Heading\n\nbody");
    // No frontmatter → unchanged.
    assert_eq!(body_without_frontmatter("# Just a doc\n"), "# Just a doc\n");
    // The real skill has frontmatter, so stripping shortens it.
    assert!(body_without_frontmatter(SKILL_MD).len() < SKILL_MD.len());
    assert!(!body_without_frontmatter(SKILL_MD).starts_with("---"));
}

#[test]
fn agents_write_to_the_expected_paths() {
    let home = Path::new("/home/dev");
    assert!(Agent::Claude
        .skill_path(home)
        .ends_with(".claude/skills/beagle/SKILL.md"));
    assert!(Agent::Codex
        .skill_path(home)
        .ends_with(".codex/prompts/beagle.md"));
    // Claude keeps frontmatter; Codex gets the body only.
    assert!(Agent::Claude.content().starts_with("---"));
    assert!(!Agent::Codex.content().starts_with("---"));
}

#[test]
fn status_is_missing_then_current_then_outdated() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path();

    assert_eq!(status(Agent::Claude, home), SkillStatus::Missing);

    let path = install(Agent::Claude, home).expect("install");
    assert!(path.exists());
    assert_eq!(status(Agent::Claude, home), SkillStatus::Current);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        Agent::Claude.content()
    );

    std::fs::write(&path, "hand-edited\n").expect("edit");
    assert_eq!(status(Agent::Claude, home), SkillStatus::Outdated);
}

#[test]
fn install_creates_missing_directories_for_both_agents() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path();
    for agent in Agent::ALL {
        let path = install(agent, home).expect("install");
        assert!(path.exists(), "{} installed", agent.name());
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            agent.content()
        );
    }
}

#[test]
fn is_present_follows_the_config_directory() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path();
    assert!(!Agent::Claude.is_present(home));
    std::fs::create_dir_all(home.join(".claude")).expect("mkdir");
    assert!(Agent::Claude.is_present(home));
    assert!(!Agent::Codex.is_present(home));
}
