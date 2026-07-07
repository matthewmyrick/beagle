//! Disk ⇄ model: the only module that touches the filesystem.
//!
//! Layout under the store root:
//!
//! ```text
//! rcas/
//!   <id>/                 one workspace per debugged system
//!     rca.toml            manifest (title, severity, status, ...)
//!     summary.md          ┐
//!     timeline.md         │
//!     root-cause.md       │ markdown sections, one per tab
//!     impact.md           │ (any may be absent; absent renders as a hint)
//!     remediation.md      │
//!     notes.md            ┘
//!     diagrams/           ASCII diagrams, shown unwrapped on the Diagrams tab
//!       01-topology.txt
//! ```
//!
//! Reads are bounded by [`MAX_FILE_BYTES`]; writes are atomic
//! (temp file + rename) so a concurrently running TUI never observes a
//! half-written manifest.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::error::{Error, Result};
use crate::model::{RcaId, RcaMeta, RcaSummary, SectionKind, Severity, Status};

/// Hard cap on any single section or diagram file. A 2 GB log pasted into a
/// section must not OOM the TUI; over-limit files surface as an error line.
pub const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// Name of the directory holding all workspaces, relative to the store root.
pub const RCAS_DIR: &str = "rcas";

/// Name of the manifest file inside each workspace.
pub const MANIFEST_FILE: &str = "rca.toml";

/// Name of the diagrams directory inside each workspace.
pub const DIAGRAMS_DIR: &str = "diagrams";

/// Name of the directory (under the store root, next to `rcas/`) where
/// exported single-file markdown documents are written.
pub const EXPORTS_DIR: &str = "exports";

/// Name of the toolbox file at the store root: what an investigating agent
/// has to work with (dashboards, CLIs, runbooks). See `beagle init`.
pub const TOOLBOX_FILE: &str = "toolbox.md";

/// Name of the per-system context directory at the store root. File names
/// (minus `.md`) line up with `systems` entries in workspace manifests.
pub const SYSTEMS_DIR: &str = "systems";

/// A non-fatal problem found while listing workspaces (corrupt manifest,
/// stray file, ...). Shown in the status bar; never aborts the listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadWarning(pub String);

/// A diagram file inside a workspace's `diagrams/` directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagramEntry {
    /// File name, e.g. `01-topology.txt`.
    pub name: String,
    /// Absolute path to the file.
    pub path: PathBuf,
}

/// A per-system context document inside the root `systems/` directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemDoc {
    /// The system name: the file name minus `.md` (e.g. `payments-api`),
    /// matching `systems` entries in workspace manifests.
    pub name: String,
    /// Absolute path to the file.
    pub path: PathBuf,
}

/// Handle to an on-disk collection of RCA workspaces.
#[derive(Debug)]
pub struct Store {
    root: PathBuf,
    rcas_root: PathBuf,
}

impl Store {
    /// Opens (creating if needed) the `rcas/` directory under `root`.
    ///
    /// # Errors
    /// Returns [`Error::Io`] if the directory cannot be created.
    pub fn open(root: &Path) -> Result<Self> {
        let rcas_root = root.join(RCAS_DIR);
        fs::create_dir_all(&rcas_root).map_err(|e| Error::io(&rcas_root, e))?;
        Ok(Self {
            root: root.to_owned(),
            rcas_root,
        })
    }

    /// The directory the filesystem watcher should observe.
    #[must_use]
    pub fn watch_root(&self) -> &Path {
        &self.rcas_root
    }

    /// The store root (the directory containing `rcas/` and `exports/`).
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Absolute path of a workspace directory.
    #[must_use]
    pub fn workspace_dir(&self, id: &RcaId) -> PathBuf {
        self.rcas_root.join(id.as_str())
    }

    /// Lists every workspace, sorted for the sidebar (open incidents first,
    /// then severity, then newest). Unreadable or corrupt workspaces are
    /// skipped and reported as warnings rather than failing the listing.
    ///
    /// This reads only the small manifests — section content stays on disk
    /// until a tab asks for it.
    ///
    /// # Errors
    /// Returns [`Error::Io`] only if the `rcas/` directory itself cannot be
    /// read.
    pub fn list(&self) -> Result<(Vec<RcaSummary>, Vec<LoadWarning>)> {
        let entries = fs::read_dir(&self.rcas_root).map_err(|e| Error::io(&self.rcas_root, e))?;

        let mut summaries = Vec::new();
        let mut warnings = Vec::new();
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    warnings.push(LoadWarning(format!("unreadable directory entry: {e}")));
                    continue;
                }
            };
            let path = entry.path();
            if !path.is_dir() {
                continue; // stray files next to workspaces are fine to ignore
            }
            let dir_name = entry.file_name().to_string_lossy().into_owned();
            match Self::load_summary(&dir_name, &path) {
                Ok(summary) => summaries.push(summary),
                Err(e) => warnings.push(LoadWarning(format!("skipped `{dir_name}`: {e}"))),
            }
        }
        summaries.sort_by_key(RcaSummary::sort_key);
        Ok((summaries, warnings))
    }

    fn load_summary(dir_name: &str, dir: &Path) -> Result<RcaSummary> {
        let id = RcaId::new(dir_name)?;
        let manifest_path = dir.join(MANIFEST_FILE);
        let raw = read_bounded(&manifest_path)?;
        let meta: RcaMeta = toml::from_str(&raw).map_err(|source| Error::ParseManifest {
            path: manifest_path,
            source: Box::new(source),
        })?;
        Ok(RcaSummary { id, meta })
    }

    /// Reads one markdown section of a workspace.
    ///
    /// Returns `Ok(None)` if the section file does not exist — an absent
    /// section is a normal state (the investigation just hasn't got there
    /// yet), not an error.
    ///
    /// # Errors
    /// Returns [`Error::Io`] on any failure other than the file being absent,
    /// or [`Error::FileTooLarge`] past [`MAX_FILE_BYTES`].
    pub fn read_section(&self, id: &RcaId, kind: SectionKind) -> Result<Option<String>> {
        read_optional(&self.workspace_dir(id).join(kind.file_name()))
    }

    /// Lists a workspace's diagram files, sorted by name (hence the
    /// `01-`, `02-` prefix convention). Missing `diagrams/` yields an empty
    /// list.
    ///
    /// # Errors
    /// Returns [`Error::Io`] if `diagrams/` exists but cannot be read.
    pub fn list_diagrams(&self, id: &RcaId) -> Result<Vec<DiagramEntry>> {
        let dir = self.workspace_dir(id).join(DIAGRAMS_DIR);
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(Error::io(&dir, e)),
        };
        let mut diagrams: Vec<DiagramEntry> = entries
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.path().is_file())
            .map(|entry| DiagramEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: entry.path(),
            })
            .collect();
        diagrams.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(diagrams)
    }

    /// Reads one diagram file as raw text (diagrams are never wrapped or
    /// markdown-rendered, so alignment survives).
    ///
    /// # Errors
    /// Returns `Ok(None)` if the file vanished since listing; otherwise
    /// [`Error::Io`] / [`Error::FileTooLarge`] as for sections.
    pub fn read_diagram(&self, entry: &DiagramEntry) -> Result<Option<String>> {
        read_optional(&entry.path)
    }

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
}

impl Store {
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

    /// Modification times of every section file present on disk, for
    /// change/unread tracking. Sections without a file are simply absent
    /// from the map; metadata errors are skipped (a vanished file is not
    /// worth failing a listing over).
    #[must_use]
    pub fn section_mtimes(&self, id: &RcaId) -> std::collections::HashMap<SectionKind, SystemTime> {
        let dir = self.workspace_dir(id);
        SectionKind::ALL
            .into_iter()
            .filter_map(|kind| {
                let modified = fs::metadata(dir.join(kind.file_name()))
                    .and_then(|m| m.modified())
                    .ok()?;
                Some((kind, modified))
            })
            .collect()
    }

    /// Reads the root `toolbox.md` — the investigation context agents read
    /// before starting. `Ok(None)` when it does not exist.
    ///
    /// # Errors
    /// [`Error::Io`] / [`Error::FileTooLarge`] as for sections.
    pub fn read_toolbox(&self) -> Result<Option<String>> {
        read_optional(&self.root.join(TOOLBOX_FILE))
    }

    /// Lists the root `systems/*.md` context documents, sorted by system
    /// name. A missing `systems/` directory yields an empty list.
    ///
    /// # Errors
    /// [`Error::Io`] if the directory exists but cannot be read.
    pub fn list_system_docs(&self) -> Result<Vec<SystemDoc>> {
        let dir = self.root.join(SYSTEMS_DIR);
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(Error::io(&dir, e)),
        };
        let mut docs: Vec<SystemDoc> = entries
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.path().is_file())
            .filter_map(|entry| {
                let path = entry.path();
                let name = path.file_stem()?.to_string_lossy().into_owned();
                (path.extension()? == "md").then_some(SystemDoc { name, path })
            })
            .collect();
        docs.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(docs)
    }

    /// Reads one system context document. `Ok(None)` if it vanished since
    /// listing.
    ///
    /// # Errors
    /// [`Error::Io`] / [`Error::FileTooLarge`] as for sections.
    pub fn read_system_doc(&self, doc: &SystemDoc) -> Result<Option<String>> {
        read_optional(&doc.path)
    }

    /// Scaffolds the investigation context at the store root: `toolbox.md`
    /// and `systems/` with an example document. Never overwrites — existing
    /// files are skipped. Returns the paths actually created.
    ///
    /// # Errors
    /// [`Error::Io`] on write failures.
    pub fn init_context(&self) -> Result<Vec<PathBuf>> {
        let mut created = Vec::new();

        let toolbox = self.root.join(TOOLBOX_FILE);
        if !toolbox.exists() {
            write_atomic(&toolbox, TOOLBOX_TEMPLATE)?;
            created.push(toolbox);
        }

        let systems = self.root.join(SYSTEMS_DIR);
        fs::create_dir_all(&systems).map_err(|e| Error::io(&systems, e))?;
        let example = systems.join("example-service.md");
        if !example.exists() && self.list_system_docs()?.is_empty() {
            write_atomic(&example, SYSTEM_TEMPLATE)?;
            created.push(example);
        }
        Ok(created)
    }
}

/// Template for `toolbox.md`, written by [`Store::init_context`].
pub const TOOLBOX_TEMPLATE: &str = "\
# Toolbox

> What an investigating agent has to work with here. Keep this current —
> agents read it before touching any telemetry, and `T` shows it in the TUI.

## Observability

- Grafana: <https://grafana.example.com> — start with the `service overview`
  dashboard
- Logs: Loki via Grafana Explore (labels: `service`, `env`, `level`)
- Errors: Sentry — <https://sentry.io/organizations/example>

## CLIs available

- `gh` — GitHub (PRs, issues, API)
- `kubectl` — cluster access (read-only context: `prod-ro`)

## Runbooks & escalation

- Runbooks: <https://wiki.example.com/runbooks>
- Escalation: #incident-response

## Conventions

- Times in UTC everywhere.
- One `systems/<name>.md` per service — read the ones matching the incident
  before investigating, and update them when you learn something durable.
";

/// Template for a `systems/<name>.md` document, written by
/// [`Store::init_context`].
pub const SYSTEM_TEMPLATE: &str = "\
# example-service

> Rename this file to match a real system (the name used in `rca.toml`
> `systems`). One file per service.

## Telemetry

- Dashboard: <https://grafana.example.com/d/example-service>
- Log labels: `service=\"example-service\"`
- Sentry project: `example-service`

## Depends on

- postgres (primary), redis (sessions), sendgrid (email)

## Known failure modes

- Redis pool exhaustion under login storms — p99 spikes, `pool timeout`
  in logs. See rcas/2026-06-30-... for the worked incident.

## Oncall / ownership

- Team: payments · Slack: #team-payments
";

impl Store {
    /// Reads and parses one workspace's manifest.
    ///
    /// # Errors
    /// [`Error::Io`] if the manifest is unreadable, [`Error::ParseManifest`]
    /// if it is invalid.
    pub fn read_meta(&self, id: &RcaId) -> Result<RcaMeta> {
        Ok(Self::load_summary(id.as_str(), &self.workspace_dir(id))?.meta)
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

    /// Builds the canonical single-file markdown export of a workspace:
    /// YAML frontmatter from the manifest (Obsidian-compatible — `tags`
    /// become vault tags), every present section in tab order, then diagrams
    /// in code fences with ANSI colors stripped.
    ///
    /// Deterministic by design: the same files on disk always produce the
    /// same document, so exports are safe to diff and sync without review.
    ///
    /// # Errors
    /// Fails only if the manifest cannot be read; missing sections and
    /// unreadable diagrams are simply omitted.
    pub fn export_markdown(&self, id: &RcaId) -> Result<String> {
        use std::fmt::Write as _;
        let meta = self.read_meta(id)?;

        let mut doc = String::new();
        doc.push_str("---\n");
        // Writing into a String cannot fail; the `let _ =` are for the trait.
        let _ = writeln!(doc, "title: {}", yaml_string(&meta.title));
        let _ = writeln!(doc, "severity: {}", meta.severity);
        let _ = writeln!(doc, "status: {}", meta.status);
        if let Ok(created) = meta.created.format(&Rfc3339) {
            let _ = writeln!(doc, "created: {created}");
        }
        if let Some(updated) = meta.updated {
            if let Ok(updated) = updated.format(&Rfc3339) {
                let _ = writeln!(doc, "updated: {updated}");
            }
        }
        let _ = writeln!(doc, "systems: {}", yaml_list(&meta.systems));
        let _ = writeln!(doc, "tags: {}", yaml_list(&meta.tags));
        doc.push_str("---\n");

        for kind in SectionKind::ALL {
            if let Some(content) = self.read_section(id, kind)? {
                doc.push('\n');
                doc.push_str(content.trim_end());
                doc.push('\n');
            }
        }
        for entry in self.list_diagrams(id)? {
            if let Ok(Some(content)) = self.read_diagram(&entry) {
                let plain = crate::ansi::strip(&content);
                let _ = write!(
                    doc,
                    "\n## Diagram: {}\n\n```\n{}\n```\n",
                    entry.name,
                    plain.trim_end()
                );
            }
        }
        Ok(doc)
    }

    /// Exports a workspace to a markdown file and returns the path written.
    /// With `out = None` the file goes to `<root>/exports/<id>.md`. The
    /// write is atomic, like every other write in this module.
    ///
    /// # Errors
    /// Propagates manifest/IO failures from [`Self::export_markdown`] and
    /// the file write.
    pub fn export_to(&self, id: &RcaId, out: Option<&Path>) -> Result<PathBuf> {
        let doc = self.export_markdown(id)?;
        let path = match out {
            Some(path) => path.to_owned(),
            None => self.root.join(EXPORTS_DIR).join(format!("{id}.md")),
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }
        write_atomic(&path, &doc)?;
        Ok(path)
    }
}

/// Quotes a string for YAML frontmatter (double-quoted style).
fn yaml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Formats a YAML inline list with every element quoted.
fn yaml_list(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|s| yaml_string(s)).collect();
    format!("[{}]", quoted.join(", "))
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
        SectionKind::Notes => "Raw evidence: queries, log excerpts, links, and open questions.",
        SectionKind::Log => {
            "What the investigation did, when — appended live, one \
             `- **HH:MM UTC** — step` bullet at a time (`beagle log <slug> \"...\"`)."
        }
    };
    format!("# {heading} — {title}\n\n> _{hint}_\n")
}

/// Reads a file, returning `Ok(None)` if it does not exist and enforcing the
/// size cap before reading a byte of content.
fn read_optional(path: &Path) -> Result<Option<String>> {
    match fs::metadata(path) {
        Ok(md) if md.len() > MAX_FILE_BYTES => {
            return Err(Error::FileTooLarge {
                path: path.to_owned(),
                size: md.len(),
                limit: MAX_FILE_BYTES,
            });
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::io(path, e)),
    }
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::io(path, e)),
    }
}

fn read_bounded(path: &Path) -> Result<String> {
    read_optional(path)?.ok_or_else(|| {
        Error::io(
            path,
            std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"),
        )
    })
}

/// Writes `content` to a sibling temp file, then renames it into place, so
/// readers never observe a partial write.
fn write_atomic(path: &Path, content: &str) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile_in(parent, path)?;
    tmp.0
        .write_all(content.as_bytes())
        .map_err(|e| Error::io(&tmp.1, e))?;
    tmp.0.sync_all().map_err(|e| Error::io(&tmp.1, e))?;
    drop(tmp.0);
    fs::rename(&tmp.1, path).map_err(|e| Error::io(path, e))?;
    Ok(())
}

/// Creates an exclusive temp file next to `target` (same filesystem, so the
/// final rename is atomic). Uses the target's file name to stay debuggable if
/// a crash ever leaves one behind.
fn tempfile_in(parent: &Path, target: &Path) -> Result<(fs::File, PathBuf)> {
    let base = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    for attempt in 0u32..1024 {
        let candidate = parent.join(format!(".{base}.tmp{attempt}"));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((file, candidate)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(Error::io(&candidate, e)),
        }
    }
    Err(Error::io(
        parent,
        std::io::Error::new(std::io::ErrorKind::AlreadyExists, "no free temp file name"),
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

    use super::*;

    fn test_meta(title: &str, severity: Severity) -> RcaMeta {
        RcaMeta {
            title: title.to_owned(),
            severity,
            status: Status::Investigating,
            created: OffsetDateTime::from_unix_timestamp(1_780_000_000)
                .expect("valid test timestamp"),
            updated: None,
            systems: vec!["payments-api".to_owned()],
            tags: vec!["latency".to_owned()],
            prs: Vec::new(),
        }
    }

    fn test_id(slug: &str) -> RcaId {
        RcaId::new(slug).expect("valid test id")
    }

    #[test]
    fn scaffold_then_list_round_trips() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = Store::open(tmp.path()).expect("open store");
        let id = test_id("payments-latency");
        let meta = test_meta("Payments p99 latency", Severity::High);

        store.scaffold(&id, &meta).expect("scaffold");
        let (summaries, warnings) = store.list().expect("list");

        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, id);
        assert_eq!(summaries[0].meta, meta);
    }

    #[test]
    fn scaffold_refuses_to_overwrite() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = Store::open(tmp.path()).expect("open store");
        let id = test_id("dup");
        let meta = test_meta("dup", Severity::Low);

        store.scaffold(&id, &meta).expect("first scaffold");
        assert!(matches!(
            store.scaffold(&id, &meta),
            Err(Error::AlreadyExists(_))
        ));
    }

    #[test]
    fn scaffold_creates_all_sections_and_they_read_back() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = Store::open(tmp.path()).expect("open store");
        let id = test_id("sections");
        store
            .scaffold(&id, &test_meta("Sections", Severity::Medium))
            .expect("scaffold");

        for kind in SectionKind::ALL {
            let content = store.read_section(&id, kind).expect("read section");
            let content = content.unwrap_or_else(|| panic!("section {kind:?} missing"));
            assert!(content.starts_with(&format!("# {}", kind.title())));
        }
    }

    #[test]
    fn missing_section_is_none_not_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = Store::open(tmp.path()).expect("open store");
        let id = test_id("bare");
        // Hand-build a workspace with a manifest but no section files.
        let dir = store.workspace_dir(&id);
        fs::create_dir_all(&dir).expect("mkdir");
        let manifest = toml::to_string_pretty(&test_meta("Bare", Severity::Info)).expect("toml");
        fs::write(dir.join(MANIFEST_FILE), manifest).expect("write manifest");

        assert_eq!(
            store.read_section(&id, SectionKind::Summary).expect("read"),
            None
        );
    }

    #[test]
    fn corrupt_manifest_becomes_warning_not_failure() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = Store::open(tmp.path()).expect("open store");
        store
            .scaffold(&test_id("good"), &test_meta("Good", Severity::Low))
            .expect("scaffold");

        let bad_dir = store.workspace_dir(&test_id("bad"));
        fs::create_dir_all(&bad_dir).expect("mkdir");
        fs::write(bad_dir.join(MANIFEST_FILE), "title = unclosed").expect("write corrupt");

        let (summaries, warnings) = store.list().expect("list");
        assert_eq!(summaries.len(), 1, "the good workspace still lists");
        assert_eq!(warnings.len(), 1, "the bad one is reported");
        assert!(warnings[0].0.contains("bad"));
    }

    #[test]
    fn oversized_file_is_rejected_before_reading() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = Store::open(tmp.path()).expect("open store");
        let id = test_id("big");
        store
            .scaffold(&id, &test_meta("Big", Severity::Low))
            .expect("scaffold");

        let path = store
            .workspace_dir(&id)
            .join(SectionKind::Notes.file_name());
        let file = fs::File::create(&path).expect("create");
        file.set_len(MAX_FILE_BYTES + 1).expect("grow file"); // sparse: no real 4 MB write
        drop(file);

        assert!(matches!(
            store.read_section(&id, SectionKind::Notes),
            Err(Error::FileTooLarge { .. })
        ));
    }

    #[test]
    fn set_status_rewrites_only_status_and_stamps_updated() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = Store::open(tmp.path()).expect("open store");
        let id = test_id("flip");
        let meta = test_meta("Flip", Severity::High);
        store.scaffold(&id, &meta).expect("scaffold");

        let written = store
            .set_status(&id, Status::Identified)
            .expect("set status");
        assert_eq!(written.status, Status::Identified);

        let back = store.read_meta(&id).expect("re-read");
        assert_eq!(back.status, Status::Identified);
        assert!(back.updated.is_some(), "updated stamped");
        assert_eq!(back.title, meta.title, "other fields preserved");
        assert_eq!(back.created, meta.created);
        assert_eq!(back.systems, meta.systems);
        assert_eq!(back.tags, meta.tags);
    }

    #[test]
    fn set_status_on_missing_workspace_is_an_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = Store::open(tmp.path()).expect("open store");
        assert!(store
            .set_status(&test_id("ghost"), Status::Resolved)
            .is_err());
    }

    #[test]
    fn export_is_deterministic_with_frontmatter_sections_and_clean_diagrams() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = Store::open(tmp.path()).expect("open store");
        let id = test_id("export-me");
        let mut meta = test_meta("Export \"quoted\" title", Severity::High);
        meta.tags = vec!["webhooks".to_owned(), "data-loss".to_owned()];
        store.scaffold(&id, &meta).expect("scaffold");
        fs::write(
            store.workspace_dir(&id).join(DIAGRAMS_DIR).join("01-x.txt"),
            "a \u{1b}[1;31mBUG\u{1b}[0m b",
        )
        .expect("write diagram");

        let doc = store.export_markdown(&id).expect("export");
        assert!(doc.starts_with("---\n"), "frontmatter first");
        assert!(doc.contains("title: \"Export \\\"quoted\\\" title\""));
        assert!(doc.contains("severity: high"));
        assert!(doc.contains("tags: [\"webhooks\", \"data-loss\"]"));
        assert!(doc.contains("# Summary"), "sections included");
        assert!(doc.contains("## Diagram: 01-x.txt"));
        assert!(doc.contains("a BUG b"), "ANSI stripped from diagrams");
        assert!(!doc.contains('\u{1b}'), "no raw escape bytes in export");

        let again = store.export_markdown(&id).expect("export twice");
        assert_eq!(doc, again, "deterministic");
    }

    #[test]
    fn export_to_writes_default_path_and_honors_out() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = Store::open(tmp.path()).expect("open store");
        let id = test_id("to-file");
        store
            .scaffold(&id, &test_meta("To file", Severity::Low))
            .expect("scaffold");

        let default_path = store.export_to(&id, None).expect("default export");
        assert_eq!(
            default_path,
            tmp.path().join(EXPORTS_DIR).join("to-file.md")
        );
        assert!(default_path.is_file());

        let custom = tmp.path().join("vault").join("note.md");
        let custom_path = store.export_to(&id, Some(&custom)).expect("custom export");
        assert_eq!(custom_path, custom);
        assert!(custom.is_file(), "parent dirs created");
    }

    #[test]
    fn add_pr_appends_once_stamps_updated_and_rejects_non_urls() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = Store::open(tmp.path()).expect("open store");
        let id = test_id("fixed");
        store
            .scaffold(&id, &test_meta("Fixed", Severity::High))
            .expect("scaffold");

        let url = "https://github.com/org/repo/pull/12";
        assert!(store.add_pr(&id, url).expect("attach"), "first add");
        assert!(!store.add_pr(&id, url).expect("re-attach"), "idempotent");

        let meta = store.read_meta(&id).expect("read");
        assert_eq!(meta.prs, [url]);
        assert!(meta.updated.is_some(), "updated stamped");
        assert_eq!(meta.title, "Fixed", "other fields preserved");

        assert!(store.add_pr(&id, "not-a-url").is_err());
        assert!(store.add_pr(&test_id("ghost"), url).is_err());
    }

    #[test]
    fn append_log_creates_then_appends_timestamped_bullets() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = Store::open(tmp.path()).expect("open store");
        let id = test_id("logged");
        store
            .scaffold(&id, &test_meta("Logged", Severity::Low))
            .expect("scaffold");

        store
            .append_log(&id, "checked p99 dashboard")
            .expect("append");
        store
            .append_log(&id, "  querying loki  ")
            .expect("append again");
        let content = store
            .read_section(&id, SectionKind::Log)
            .expect("read")
            .expect("present");
        let bullets: Vec<&str> = content.lines().filter(|l| l.starts_with("- **")).collect();
        assert_eq!(bullets.len(), 2);
        assert!(bullets[0].contains("UTC** — checked p99 dashboard"));
        assert!(
            bullets[1].ends_with("— querying loki"),
            "message trimmed: {}",
            bullets[1]
        );
    }

    #[test]
    fn append_log_requires_an_existing_workspace() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = Store::open(tmp.path()).expect("open store");
        assert!(store.append_log(&test_id("ghost"), "hello").is_err());
    }

    #[test]
    fn section_mtimes_cover_scaffolded_sections_and_skip_absent_ones() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = Store::open(tmp.path()).expect("open store");
        let id = test_id("mtimes");
        store
            .scaffold(&id, &test_meta("Mtimes", Severity::Low))
            .expect("scaffold");

        let mtimes = store.section_mtimes(&id);
        assert_eq!(mtimes.len(), SectionKind::ALL.len());

        fs::remove_file(
            store
                .workspace_dir(&id)
                .join(SectionKind::Notes.file_name()),
        )
        .expect("remove");
        let mtimes = store.section_mtimes(&id);
        assert!(!mtimes.contains_key(&SectionKind::Notes));
    }

    #[test]
    fn init_context_scaffolds_once_and_never_overwrites() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = Store::open(tmp.path()).expect("open store");

        let created = store.init_context().expect("init");
        assert_eq!(created.len(), 2, "toolbox + example system: {created:?}");
        assert!(tmp.path().join(TOOLBOX_FILE).is_file());
        assert!(tmp
            .path()
            .join(SYSTEMS_DIR)
            .join("example-service.md")
            .is_file());

        // Second run creates nothing and touches nothing.
        fs::write(tmp.path().join(TOOLBOX_FILE), "customized").expect("customize");
        let again = store.init_context().expect("re-init");
        assert!(again.is_empty(), "no overwrites: {again:?}");
        assert_eq!(
            fs::read_to_string(tmp.path().join(TOOLBOX_FILE)).expect("read"),
            "customized"
        );
    }

    #[test]
    fn init_context_skips_the_example_when_real_system_docs_exist() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = Store::open(tmp.path()).expect("open store");
        let systems = tmp.path().join(SYSTEMS_DIR);
        fs::create_dir_all(&systems).expect("mkdir");
        fs::write(systems.join("payments-api.md"), "# payments-api").expect("write");

        let created = store.init_context().expect("init");
        assert_eq!(created.len(), 1, "only the toolbox is missing");
        assert!(
            !systems.join("example-service.md").exists(),
            "no example next to real docs"
        );
    }

    #[test]
    fn toolbox_and_system_docs_read_back() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = Store::open(tmp.path()).expect("open store");

        assert_eq!(store.read_toolbox().expect("absent ok"), None);
        assert!(store.list_system_docs().expect("missing dir ok").is_empty());

        store.init_context().expect("init");
        let toolbox = store.read_toolbox().expect("read").expect("present");
        assert!(toolbox.starts_with("# Toolbox"));

        let systems = tmp.path().join(SYSTEMS_DIR);
        fs::write(systems.join("api.md"), "# api").expect("write");
        fs::write(systems.join("notes.txt"), "not markdown").expect("write");
        let docs = store.list_system_docs().expect("list");
        let names: Vec<&str> = docs.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["api", "example-service"], "sorted, .md only");
        assert_eq!(
            store.read_system_doc(&docs[0]).expect("read").as_deref(),
            Some("# api")
        );
    }

    #[test]
    fn context_templates_are_valid_markdown_seeds() {
        assert!(TOOLBOX_TEMPLATE.starts_with("# Toolbox"));
        assert!(SYSTEM_TEMPLATE.starts_with("# example-service"));
    }

    #[test]
    fn diagrams_list_sorted_and_missing_dir_is_empty() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = Store::open(tmp.path()).expect("open store");
        let id = test_id("diag");
        store
            .scaffold(&id, &test_meta("Diag", Severity::Low))
            .expect("scaffold");

        let ddir = store.workspace_dir(&id).join(DIAGRAMS_DIR);
        fs::write(ddir.join("02-flow.txt"), "b").expect("write");
        fs::write(ddir.join("01-topology.txt"), "a").expect("write");

        let diagrams = store.list_diagrams(&id).expect("list diagrams");
        let names: Vec<&str> = diagrams.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["01-topology.txt", "02-flow.txt"]);

        let no_dir = test_id("nodir");
        fs::create_dir_all(store.workspace_dir(&no_dir)).expect("mkdir");
        assert!(store.list_diagrams(&no_dir).expect("empty").is_empty());
    }
}
