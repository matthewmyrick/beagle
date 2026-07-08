//! Investigation context at the store root: `toolbox.md` (what an agent has
//! to work with) and `systems/*.md` (per-system knowledge). Scaffolded by
//! `beagle init` and shown in the TUI via `T`.

use std::fs;
use std::path::PathBuf;

use crate::error::{Error, Result};

use super::fsio::{read_optional, write_atomic};
use super::{Store, SYSTEMS_DIR, TOOLBOX_FILE};

/// A per-system context document inside the root `systems/` directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemDoc {
    /// The system name: the file name minus `.md` (e.g. `payments-api`),
    /// matching `systems` entries in workspace manifests.
    pub name: String,
    /// Absolute path to the file.
    pub path: PathBuf,
}

impl Store {
    /// Reads the root `toolbox.md` — the investigation context agents read
    /// before starting. `Ok(None)` when it does not exist.
    ///
    /// # Errors
    /// [`Error::Io`] / [`Error::FileTooLarge`] as for sections.
    pub fn read_toolbox(&self) -> Result<Option<String>> {
        read_optional(&self.root().join(TOOLBOX_FILE))
    }

    /// Lists the root `systems/*.md` context documents, sorted by system
    /// name. A missing `systems/` directory yields an empty list.
    ///
    /// # Errors
    /// [`Error::Io`] if the directory exists but cannot be read.
    pub fn list_system_docs(&self) -> Result<Vec<SystemDoc>> {
        let dir = self.root().join(SYSTEMS_DIR);
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

        let toolbox = self.root().join(TOOLBOX_FILE);
        if !toolbox.exists() {
            write_atomic(&toolbox, TOOLBOX_TEMPLATE)?;
            created.push(toolbox);
        }

        let systems = self.root().join(SYSTEMS_DIR);
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

#[cfg(test)]
#[path = "tests/context.rs"]
mod tests;
