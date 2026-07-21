//! The bundled beagle skills and installing them for coding agents.
//!
//! Each skill's markdown is embedded into the binary at compile time from
//! the repo's canonical `.claude/skills/<slug>/SKILL.md`, so a release
//! carries a snapshot and `beagle update` can offer to install or refresh
//! them wherever an agent looks — Claude Code
//! (`~/.claude/skills/<slug>/SKILL.md`) and Codex
//! (`~/.codex/prompts/<slug>.md`). Everything here is pure given a home
//! directory, so it tests against a temp dir without touching the real one.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// A bundled beagle skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skill {
    /// `/beagle` — author and maintain RCA workspaces.
    Beagle,
    /// `/beagle-review` — load an RCA by id and answer questions about it.
    Review,
}

impl Skill {
    /// Every bundled skill, in display order.
    pub const ALL: [Self; 2] = [Self::Beagle, Self::Review];

    /// The skill slug — its directory name and the `/name` agents invoke.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Beagle => "beagle",
            Self::Review => "beagle-review",
        }
    }

    /// The skill markdown, embedded from the repo's single source of truth.
    /// Rebuilding the binary re-snapshots it; that is how a newer release
    /// ships newer skills.
    #[must_use]
    pub fn markdown(self) -> &'static str {
        match self {
            Self::Beagle => include_str!("../../.claude/skills/beagle/SKILL.md"),
            Self::Review => include_str!("../../.claude/skills/beagle-review/SKILL.md"),
        }
    }
}

/// The `/beagle` skill markdown. Retained for callers that want the primary
/// skill directly; equivalent to `Skill::Beagle.markdown()`.
pub const SKILL_MD: &str = include_str!("../../.claude/skills/beagle/SKILL.md");

/// A coding agent beagle can install its skills for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    /// Anthropic's Claude Code — skills live under `~/.claude/skills/`.
    Claude,
    /// The Codex coding agent — custom prompts live under
    /// `~/.codex/prompts/`.
    Codex,
}

impl Agent {
    /// Every agent, in display order.
    pub const ALL: [Self; 2] = [Self::Claude, Self::Codex];

    /// Human-readable name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
        }
    }

    /// The agent's config directory under `home`; its existence is how we
    /// detect that the agent is set up on this machine.
    fn config_dir(self, home: &Path) -> PathBuf {
        match self {
            Self::Claude => home.join(".claude"),
            Self::Codex => home.join(".codex"),
        }
    }

    /// Where this agent reads `skill` from.
    #[must_use]
    pub fn skill_path(self, skill: Skill, home: &Path) -> PathBuf {
        match self {
            Self::Claude => home
                .join(".claude")
                .join("skills")
                .join(skill.slug())
                .join("SKILL.md"),
            Self::Codex => home
                .join(".codex")
                .join("prompts")
                .join(format!("{}.md", skill.slug())),
        }
    }

    /// The content to write for this agent. Claude keeps the YAML
    /// frontmatter (it drives skill discovery); a Codex prompt is injected
    /// verbatim, so it gets the body only.
    #[must_use]
    pub fn content(self, skill: Skill) -> &'static str {
        match self {
            Self::Claude => skill.markdown(),
            Self::Codex => body_without_frontmatter(skill.markdown()),
        }
    }

    /// Whether this agent appears set up (its config directory exists).
    #[must_use]
    pub fn is_present(self, home: &Path) -> bool {
        self.config_dir(home).is_dir()
    }
}

/// How the installed skill compares to the bundled one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillStatus {
    /// No skill file for this agent yet.
    Missing,
    /// A skill file exists but differs from the bundled version.
    Outdated,
    /// The installed skill matches the bundled version.
    Current,
}

/// Compares the on-disk copy of `skill` for `agent` against the bundled
/// content.
#[must_use]
pub fn status(skill: Skill, agent: Agent, home: &Path) -> SkillStatus {
    match std::fs::read_to_string(agent.skill_path(skill, home)) {
        Ok(existing) if existing == agent.content(skill) => SkillStatus::Current,
        Ok(_) => SkillStatus::Outdated,
        Err(_) => SkillStatus::Missing,
    }
}

/// Writes the bundled `skill` for `agent`, creating parent directories.
///
/// # Errors
/// [`Error::Io`] if the directories or file cannot be written.
pub fn install(skill: Skill, agent: Agent, home: &Path) -> Result<PathBuf> {
    let path = agent.skill_path(skill, home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    std::fs::write(&path, agent.content(skill)).map_err(|e| Error::io(&path, e))?;
    Ok(path)
}

/// The user's home directory from `$HOME`.
///
/// # Errors
/// [`Error::Tool`] when `$HOME` is unset or empty.
pub fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| Error::Tool {
            tool: "skill",
            message: "cannot locate your home directory ($HOME is unset)".to_owned(),
        })
}

/// The markdown with a leading `---\n … \n---` YAML frontmatter block
/// removed (and the blank lines after it). Returns the input unchanged when
/// there is no frontmatter.
#[must_use]
pub fn body_without_frontmatter(md: &str) -> &str {
    if let Some(rest) = md.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            return rest[end + "\n---\n".len()..].trim_start_matches('\n');
        }
    }
    md
}

#[cfg(test)]
#[path = "tests/skill.rs"]
mod tests;
