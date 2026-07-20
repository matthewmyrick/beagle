//! Writes that create or edit workspaces: scaffolding, status flips, PR
//! attachment, and the append-only investigation log. Every write goes
//! through [`super::fsio::write_atomic`].

use std::fs;
use std::path::PathBuf;

use time::OffsetDateTime;

use crate::error::{Error, Result};
use crate::model::{RcaId, RcaMeta, SectionKind, Severity, Status};

use super::fsio::{read_optional, write_atomic};
use super::{Store, ARCHIVE_DIR, DIAGRAMS_DIR, MANIFEST_FILE};

impl Store {
    /// Creates a new workspace: directory, manifest, section skeletons, and
    /// an empty `diagrams/` directory. Refuses to touch an existing one —
    /// including an archived one — and refuses the reserved `archive` slug.
    ///
    /// # Errors
    /// [`Error::AlreadyExists`] if the directory is already present;
    /// [`Error::InvalidId`] for the reserved slug; [`Error::Io`] /
    /// [`Error::SerializeManifest`] on write failures.
    pub fn scaffold(&self, id: &RcaId, meta: &RcaMeta) -> Result<PathBuf> {
        if id.as_str() == ARCHIVE_DIR {
            // `rcas/archive/` is where archived workspaces live; a
            // workspace by that name would shadow the whole archive.
            return Err(Error::InvalidId(format!(
                "{ARCHIVE_DIR} (reserved for archived workspaces)"
            )));
        }
        let dir = self.active_dir(id);
        if dir.exists() || self.archived_dir(id).exists() {
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
        let mut meta = match self.read_meta(id) {
            Ok(meta) => meta,
            // The manifest may be unreadable *because* the status is
            // invalid — the exact thing this command should be able to
            // repair. Retry with the bad status overwritten before giving
            // up (otherwise `beagle status` can never fix what broke it).
            Err(original) => match self.repair_status(id, status) {
                Some(meta) => meta,
                None => return Err(original),
            },
        };
        meta.status = status;
        meta.updated = Some(OffsetDateTime::now_utc());
        let manifest = toml::to_string_pretty(&meta)?;
        write_atomic(&self.workspace_dir(id).join(MANIFEST_FILE), &manifest)?;
        Ok(meta)
    }

    /// Attempts to parse the manifest with `status` substituted for
    /// whatever (possibly invalid) value is on disk. `Some` only when the
    /// rest of the manifest is valid — a bad status is repairable, other
    /// corruption is not.
    fn repair_status(&self, id: &RcaId, status: Status) -> Option<RcaMeta> {
        let path = self.workspace_dir(id).join(MANIFEST_FILE);
        let raw = read_optional(&path).ok()??;
        let mut value: toml::Value = toml::from_str(&raw).ok()?;
        value.as_table_mut()?.insert(
            "status".to_owned(),
            toml::Value::String(status.as_str().to_owned()),
        );
        value.try_into().ok()
    }

    /// Moves a workspace to `rcas/archive/<id>` — out of the sidebar, never
    /// out of the knowledge base. Refuses unless the status is `finished`
    /// (`force` overrides): archiving an unverified incident hides live
    /// work. The move is a same-filesystem rename, so a watching TUI sees
    /// one atomic transition.
    ///
    /// # Errors
    /// [`Error::Tool`] when the workspace is missing, already archived, or
    /// not finished (without `force`); [`Error::AlreadyExists`] when the
    /// archive already holds this slug; [`Error::Io`] on the move itself.
    pub fn archive(&self, id: &RcaId, force: bool) -> Result<PathBuf> {
        let source = self.active_dir(id);
        if !source.join(MANIFEST_FILE).exists() {
            let message = if self.archived_dir(id).join(MANIFEST_FILE).exists() {
                format!("{id} is already archived")
            } else {
                format!("no workspace `{id}` under this root")
            };
            return Err(Error::Tool {
                tool: "archive",
                message,
            });
        }
        if !force {
            let status = self.read_meta(id)?.status;
            if status != Status::Finished {
                return Err(Error::Tool {
                    tool: "archive",
                    message: format!(
                        "{id} is `{status}`, not `finished` — verify and sign off \
                         first, or pass --force"
                    ),
                });
            }
        }
        let dest = self.archived_dir(id);
        if dest.exists() {
            return Err(Error::AlreadyExists(format!("{ARCHIVE_DIR}/{id}")));
        }
        // `dest.parent()` is always `rcas/archive/`; create_dir_all is a
        // no-op when it already exists.
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }
        fs::rename(&source, &dest).map_err(|e| Error::io(&source, e))?;
        Ok(dest)
    }

    /// Moves an archived workspace back to the active list — the inverse
    /// of [`Self::archive`]. No status requirement: un-archiving is always
    /// safe, it only makes the workspace visible again.
    ///
    /// # Errors
    /// [`Error::Tool`] when the workspace is missing or not archived;
    /// [`Error::AlreadyExists`] when an active workspace already holds
    /// this slug; [`Error::Io`] on the move itself.
    pub fn unarchive(&self, id: &RcaId) -> Result<PathBuf> {
        let source = self.archived_dir(id);
        if !source.join(MANIFEST_FILE).exists() {
            let message = if self.active_dir(id).join(MANIFEST_FILE).exists() {
                format!("{id} is not archived")
            } else {
                format!("no workspace `{id}` under this root")
            };
            return Err(Error::Tool {
                tool: "unarchive",
                message,
            });
        }
        let dest = self.active_dir(id);
        if dest.exists() {
            return Err(Error::AlreadyExists(id.to_string()));
        }
        fs::rename(&source, &dest).map_err(|e| Error::io(&source, e))?;
        Ok(dest)
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

    /// Deletes a workspace directory — active or archived — permanently,
    /// with everything in it. The manifest must parse first: delete
    /// refuses to guess about a directory it cannot identify as a
    /// workspace (remove broken ones by hand).
    ///
    /// # Errors
    /// [`Error::Tool`] for the reserved `archive` id; otherwise fails as
    /// manifest read does, or [`Error::Io`] if the removal itself fails.
    pub fn delete(&self, id: &RcaId) -> Result<PathBuf> {
        if id.as_str() == ARCHIVE_DIR {
            return Err(Error::Tool {
                tool: "delete",
                message: format!("`{ARCHIVE_DIR}` is the archive directory, not a workspace"),
            });
        }
        let _ = self.read_meta(id)?; // proves this is a loadable workspace
        let dir = self.workspace_dir(id);
        fs::remove_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        Ok(dir)
    }

    /// Publishes or unpublishes an incident: sets the `published` flag and,
    /// when publishing, stamps `published_at` with the current time (clears
    /// it when unpublishing). Stamps `updated` too. Returns the resulting
    /// `published` state. Idempotent — re-publishing keeps the original
    /// `published_at`.
    ///
    /// # Errors
    /// Fails as manifest read/write does.
    pub fn set_published(&self, id: &RcaId, published: bool) -> Result<bool> {
        let mut meta = self.read_meta(id)?;
        if meta.published == published {
            return Ok(published); // no-op; keep the original published_at
        }
        meta.published = published;
        meta.published_at = published.then(OffsetDateTime::now_utc);
        meta.updated = Some(OffsetDateTime::now_utc());
        let manifest = toml::to_string_pretty(&meta)?;
        write_atomic(&self.workspace_dir(id).join(MANIFEST_FILE), &manifest)?;
        Ok(published)
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
        published: false,
        published_at: None,
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
