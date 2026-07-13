//! Writes that create or edit workspaces: scaffolding, status flips, PR
//! attachment, and the append-only investigation log. Every write goes
//! through [`super::fsio::write_atomic`].

use std::fs;
use std::path::PathBuf;

use time::OffsetDateTime;

use crate::error::{Error, Result};
use crate::model::{RcaId, RcaMeta, SectionKind, Severity, Status};

use super::fsio::{read_optional, write_atomic};
use super::{Store, DIAGRAMS_DIR, MANIFEST_FILE};

impl Store {
    /// Creates a new workspace: directory, manifest, section skeletons, and
    /// an empty `diagrams/` directory. Refuses to touch an existing one.
    ///
    /// # Errors
    /// [`Error::AlreadyExists`] if the directory is already present;
    /// [`Error::Io`] / [`Error::SerializeManifest`] on write failures.
    pub fn scaffold(&self, id: &RcaId, meta: &RcaMeta) -> Result<PathBuf> {
        let dir = self.workspace_dir(id);
        if dir.exists() {
            return Err(Error::AlreadyExists(id.to_string()));
        }
        fs::create_dir_all(dir.join(DIAGRAMS_DIR)).map_err(|e| Error::io(&dir, e))?;

        let manifest = toml::to_string_pretty(meta)?;
        write_atomic(&dir.join(MANIFEST_FILE), &manifest)?;

        for kind in SectionKind::ALL {
            let body = section_template(kind, &meta.title);
            write_atomic(&dir.join(kind.file_name()), &body)?;
        }
        Ok(dir)
    }

    /// Appends a timestamped `- **HH:MM UTC** — message` bullet to the
    /// workspace's `log.md`, creating the file (with a heading) if absent.
    /// The write is atomic, so a watching TUI never sees a torn line.
    ///
    /// # Errors
    /// Fails if the workspace does not exist (manifest unreadable) or on
    /// write failure.
    pub fn append_log(&self, id: &RcaId, message: &str) -> Result<PathBuf> {
        let meta = self.read_meta(id)?; // proves the workspace exists
        let path = self.workspace_dir(id).join(SectionKind::Log.file_name());
        let mut content = read_optional(&path)?
            .unwrap_or_else(|| format!("# {} — {}\n", SectionKind::Log.title(), meta.title));
        if !content.ends_with('\n') {
            content.push('\n');
        }
        let stamp = OffsetDateTime::now_utc()
            .format(time::macros::format_description!("[hour]:[minute] UTC"))
            .unwrap_or_else(|_| "??:?? UTC".to_owned());
        let _ = std::fmt::Write::write_fmt(
            &mut content,
            format_args!("- **{stamp}** — {}\n", message.trim()),
        );
        write_atomic(&path, &content)?;
        Ok(path)
    }

    /// Rewrites one workspace's status, stamping `updated` with the current
    /// time. Every other manifest field is preserved. The write is atomic,
    /// so a running TUI (which watches the manifest) picks the change up
    /// live without ever seeing a half-written file.
    ///
    /// # Errors
    /// [`Error::Io`] / [`Error::ParseManifest`] if the manifest cannot be
    /// read, [`Error::SerializeManifest`] / [`Error::Io`] on write failures.
    pub fn set_status(&self, id: &RcaId, status: Status) -> Result<RcaMeta> {
        let mut meta = self.read_meta(id)?;
        meta.status = status;
        meta.updated = Some(OffsetDateTime::now_utc());
        let manifest = toml::to_string_pretty(&meta)?;
        write_atomic(&self.workspace_dir(id).join(MANIFEST_FILE), &manifest)?;
        Ok(meta)
    }

    /// Attaches a remediation PR URL to the workspace manifest, stamping
    /// `updated`. Idempotent: re-attaching an existing URL is a no-op and
    /// returns `false`.
    ///
    /// # Errors
    /// Rejects non-http(s) URLs; otherwise fails as manifest read/write
    /// does.
    pub fn add_pr(&self, id: &RcaId, url: &str) -> Result<bool> {
        let url = url.trim();
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(Error::Tool {
                tool: "pr",
                message: format!("`{url}` is not an http(s) URL"),
            });
        }
        let mut meta = self.read_meta(id)?;
        if meta.prs.iter().any(|existing| existing == url) {
            return Ok(false);
        }
        meta.prs.push(url.to_owned());
        meta.updated = Some(OffsetDateTime::now_utc());
        let manifest = toml::to_string_pretty(&meta)?;
        write_atomic(&self.workspace_dir(id).join(MANIFEST_FILE), &manifest)?;
        Ok(true)
    }
}

/// Builds a fresh manifest for `scaffold`, stamping `created` with the
/// current time.
#[must_use]
pub fn new_meta(title: String, severity: Severity) -> RcaMeta {
    RcaMeta {
        title,
        severity,
        status: Status::Investigating,
        created: OffsetDateTime::now_utc(),
        updated: None,
        systems: Vec::new(),
        tags: Vec::new(),
        prs: Vec::new(),
    }
}

fn section_template(kind: SectionKind, title: &str) -> String {
    let heading = kind.title();
    let hint = match kind {
        SectionKind::Summary => "What broke, in three sentences. Write this tab first.",
        SectionKind::Timeline => {
            "What happened when. One `- **HH:MM UTC** — event` bullet per observation."
        }
        SectionKind::RootCause => "Why it broke. Walk the causal chain from symptom down to root.",
        SectionKind::Impact => {
            "Who and what was affected. Quantify: requests, users, duration, money."
        }
        SectionKind::Remediation => {
            "How to fix it. Immediate mitigation first, then the durable fix."
        }
        SectionKind::FinalReview => {
            "How we'll know the fix worked — write this DURING the \
             investigation. One `- [ ]` checkbox per concrete, checkable \
             prediction (metrics back to normal, errors gone for 24h, alert \
             quiet). Worked through after the fix PR merges; `V` signs off."
        }
        SectionKind::Notes => "Raw evidence: queries, log excerpts, links, and open questions.",
        SectionKind::Log => {
            "What the investigation did, when — appended live, one \
             `- **HH:MM UTC** — step` bullet at a time (`beagle log <slug> \"...\"`)."
        }
    };
    format!("# {heading} — {title}\n\n> _{hint}_\n")
}

#[cfg(test)]
#[path = "tests/mutate.rs"]
mod tests;
