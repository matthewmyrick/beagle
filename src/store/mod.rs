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
//!
//! Submodules by responsibility: `fsio` (bounded reads, atomic writes),
//! `mutate` (scaffold and manifest edits), `context` (toolbox + systems
//! docs), and `export` (single-file markdown export).

mod context;
mod export;
mod fsio;
mod mutate;

pub use context::{SystemDoc, SYSTEM_TEMPLATE, TOOLBOX_TEMPLATE};
pub use mutate::new_meta;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::error::{Error, Result};
use crate::model::{RcaId, RcaMeta, RcaSummary, SectionKind};

use fsio::{read_bounded, read_optional};

/// Hard cap on any single section or diagram file. A 2 GB log pasted into a
/// section must not OOM the TUI; over-limit files surface as an error line.
pub const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// Name of the directory holding all workspaces, relative to the store root.
pub const RCAS_DIR: &str = "rcas";

/// Name of the manifest file inside each workspace.
pub const MANIFEST_FILE: &str = "rca.toml";

/// Name of the diagrams directory inside each workspace.
pub const DIAGRAMS_DIR: &str = "diagrams";

/// Name of the directory under `rcas/` holding archived workspaces —
/// finished incidents moved out of the sidebar without deleting the
/// knowledge base.
pub const ARCHIVE_DIR: &str = "archive";

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

/// A workspace directory that exists on disk but could not be loaded
/// (missing or invalid manifest). Listed so it can be *shown* broken
/// instead of silently disappearing from the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokenWorkspace {
    /// The directory name under `rcas/` (may not even be a valid slug).
    pub dir_name: String,
    /// Why it failed to load, human-readable.
    pub reason: String,
}

/// Everything a workspace listing produced: loadable incidents, broken
/// directories, and non-workspace warnings.
#[derive(Debug, Default)]
pub struct Listing {
    /// Loadable workspaces, sorted for the sidebar.
    pub summaries: Vec<RcaSummary>,
    /// Directories that exist but could not load, sorted by name.
    pub broken: Vec<BrokenWorkspace>,
    /// Problems that are not tied to a specific workspace directory.
    pub warnings: Vec<LoadWarning>,
}

/// A diagram file inside a workspace's `diagrams/` directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagramEntry {
    /// File name, e.g. `01-topology.txt`.
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

    /// Where workspace `id` lives while active.
    pub(crate) fn active_dir(&self, id: &RcaId) -> PathBuf {
        self.rcas_root.join(id.as_str())
    }

    /// Where workspace `id` lives once archived.
    pub(crate) fn archived_dir(&self, id: &RcaId) -> PathBuf {
        self.rcas_root.join(ARCHIVE_DIR).join(id.as_str())
    }

    /// Absolute path of a workspace directory. Prefers the active location;
    /// falls back to `rcas/archive/` when only the archived copy has a
    /// manifest — so reads, exports, log appends, and similar-ranking work
    /// on archived workspaces without callers knowing where they live.
    #[must_use]
    pub fn workspace_dir(&self, id: &RcaId) -> PathBuf {
        let active = self.active_dir(id);
        if active.join(MANIFEST_FILE).exists() {
            return active;
        }
        let archived = self.archived_dir(id);
        if archived.join(MANIFEST_FILE).exists() {
            archived
        } else {
            active
        }
    }

    /// Lists every workspace, sorted for the sidebar (open incidents first,
    /// then severity, then newest). Unreadable or corrupt workspaces are
    /// returned as [`Listing::broken`] — never silently dropped: a
    /// workspace that exists on disk must stay visible, with the reason it
    /// could not load.
    ///
    /// This reads only the small manifests — section content stays on disk
    /// until a tab asks for it.
    ///
    /// # Errors
    /// Returns [`Error::Io`] only if the `rcas/` directory itself cannot be
    /// read.
    pub fn list(&self) -> Result<Listing> {
        Self::list_dir(&self.rcas_root, false)
    }

    /// Lists the workspaces under `rcas/archive/`, sorted like
    /// [`Self::list`]. A missing archive directory is an empty listing, not
    /// an error.
    ///
    /// # Errors
    /// Returns [`Error::Io`] only if the archive directory exists but
    /// cannot be read.
    pub fn list_archived(&self) -> Result<Listing> {
        let archive_root = self.rcas_root.join(ARCHIVE_DIR);
        if !archive_root.is_dir() {
            return Ok(Listing::default());
        }
        Self::list_dir(&archive_root, true)
    }

    /// Active and archived workspaces in one listing, re-sorted so archived
    /// incidents sink below everything active. This is what the TUI loads;
    /// it hides the archived entries until asked.
    ///
    /// # Errors
    /// As [`Self::list`]; an unreadable archive directory degrades to a
    /// warning rather than failing the whole listing.
    pub fn list_all(&self) -> Result<Listing> {
        let mut listing = self.list()?;
        match self.list_archived() {
            Ok(mut archived) => {
                listing.summaries.append(&mut archived.summaries);
                listing.broken.append(&mut archived.broken);
                listing.warnings.append(&mut archived.warnings);
            }
            Err(e) => listing
                .warnings
                .push(LoadWarning(format!("archive unreadable: {e}"))),
        }
        listing.summaries.sort_by_key(RcaSummary::sort_key);
        listing.broken.sort_by(|a, b| a.dir_name.cmp(&b.dir_name));
        Ok(listing)
    }

    fn list_dir(root: &Path, archived: bool) -> Result<Listing> {
        let entries = fs::read_dir(root).map_err(|e| Error::io(root, e))?;

        let mut listing = Listing::default();
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    listing
                        .warnings
                        .push(LoadWarning(format!("unreadable directory entry: {e}")));
                    continue;
                }
            };
            let path = entry.path();
            if !path.is_dir() {
                continue; // stray files next to workspaces are fine to ignore
            }
            let dir_name = entry.file_name().to_string_lossy().into_owned();
            if !archived && dir_name == ARCHIVE_DIR {
                continue; // archived workspaces list via `list_archived`
            }
            match Self::load_summary(&dir_name, &path) {
                Ok(mut summary) => {
                    summary.archived = archived;
                    listing.summaries.push(summary);
                }
                Err(e) => listing.broken.push(BrokenWorkspace {
                    dir_name,
                    reason: e.to_string(),
                }),
            }
        }
        listing.summaries.sort_by_key(RcaSummary::sort_key);
        listing.broken.sort_by(|a, b| a.dir_name.cmp(&b.dir_name));
        Ok(listing)
    }

    fn load_summary(dir_name: &str, dir: &Path) -> Result<RcaSummary> {
        let id = RcaId::new(dir_name)?;
        let manifest_path = dir.join(MANIFEST_FILE);
        let raw = read_bounded(&manifest_path)?;
        let meta: RcaMeta = toml::from_str(&raw).map_err(|source| Error::ParseManifest {
            path: manifest_path,
            source: Box::new(source),
        })?;
        Ok(RcaSummary {
            id,
            meta,
            archived: false,
        })
    }

    /// Reads and parses one workspace's manifest.
    ///
    /// # Errors
    /// [`Error::Io`] if the manifest is unreadable, [`Error::ParseManifest`]
    /// if it is invalid.
    pub fn read_meta(&self, id: &RcaId) -> Result<RcaMeta> {
        Ok(Self::load_summary(id.as_str(), &self.workspace_dir(id))?.meta)
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
}

#[cfg(test)]
#[path = "tests/util.rs"]
pub(crate) mod testutil;

#[cfg(test)]
#[path = "tests/store.rs"]
mod tests;
