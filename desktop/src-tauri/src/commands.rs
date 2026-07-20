//! Tauri commands: the IPC surface the frontend calls.
//!
//! Each command opens the store fresh — the store is a thin handle over
//! paths, and per-call opening keeps commands stateless and the root
//! resolution identical to the CLI's (`--root` flag aside): config file
//! `root`, then the current directory.

use std::collections::HashMap;
use std::path::PathBuf;

use beagle::model::{RcaId, SectionKind};
use beagle::store::Store;

use crate::dto;

/// Resolves the workspace root: config file `root` first (same as the
/// CLI), then the nearest ancestor of the working directory that already
/// contains an `rcas/` directory, then the working directory itself. The
/// ancestor walk matters in `tauri dev`, whose working directory is
/// `desktop/src-tauri` — without it the app would scaffold a stray
/// `rcas/` there instead of finding the repo's.
fn effective_root() -> Result<PathBuf, String> {
    if let Ok(Some(config)) = beagle::config::load_default() {
        if let Some(root) = config.root {
            return Ok(root);
        }
    }
    let cwd =
        std::env::current_dir().map_err(|e| format!("cannot resolve a workspace root: {e}"))?;
    Ok(beagle::store::discover_root(&cwd))
}

fn open_store() -> Result<Store, String> {
    Store::open(&effective_root()?).map_err(|e| e.to_string())
}

fn parse_id(id: &str) -> Result<RcaId, String> {
    RcaId::new(id).map_err(|e| e.to_string())
}

/// Every section file name the frontend may ask for, mapped to its kind.
fn parse_kind(file: &str) -> Result<SectionKind, String> {
    SectionKind::ALL
        .into_iter()
        .find(|kind| kind.file_name() == file)
        .ok_or_else(|| format!("unknown section `{file}`"))
}

/// Active and archived workspaces plus load problems, sidebar-sorted.
#[tauri::command]
pub fn list_workspaces() -> Result<dto::Listing, String> {
    let store = open_store()?;
    let listing = store.list_all().map_err(|e| e.to_string())?;
    Ok(dto::Listing {
        root: store.root().display().to_string(),
        workspaces: listing.summaries.iter().map(dto::Workspace::from).collect(),
        broken: listing
            .broken
            .into_iter()
            .map(|b| dto::Broken {
                dir_name: b.dir_name,
                reason: b.reason,
            })
            .collect(),
        warnings: listing.warnings.into_iter().map(|w| w.0).collect(),
    })
}

/// One section's raw markdown; `None` when the file doesn't exist yet.
#[tauri::command]
pub fn read_section(id: &str, file: &str) -> Result<Option<String>, String> {
    let store = open_store()?;
    store
        .read_section(&parse_id(id)?, parse_kind(file)?)
        .map_err(|e| e.to_string())
}

/// The workspace's diagram file names, sorted (the `01-` prefix
/// convention). Empty when there is no diagrams directory.
#[tauri::command]
pub fn list_diagrams(id: &str) -> Result<Vec<String>, String> {
    let store = open_store()?;
    Ok(store
        .list_diagrams(&parse_id(id)?)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|entry| entry.name)
        .collect())
}

/// One diagram's raw text, ANSI intact — the frontend converts the SGR
/// color codes to styled spans (see `src/lib/ansi.ts`). `None` when the
/// file vanished since listing.
#[tauri::command]
pub fn read_diagram(id: &str, name: &str) -> Result<Option<String>, String> {
    let store = open_store()?;
    let id = parse_id(id)?;
    let Some(entry) = store
        .list_diagrams(&id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|entry| entry.name == name)
    else {
        return Ok(None);
    };
    store.read_diagram(&entry).map_err(|e| e.to_string())
}

/// Archives a workspace (requires `finished`, like the CLI without
/// `--force` — the error message explains the sign-off path).
#[tauri::command]
pub fn archive_workspace(id: &str) -> Result<String, String> {
    let store = open_store()?;
    store
        .archive(&parse_id(id)?, false)
        .map(|dest| dest.display().to_string())
        .map_err(|e| e.to_string())
}

/// Moves an archived workspace back to the active list.
#[tauri::command]
pub fn unarchive_workspace(id: &str) -> Result<String, String> {
    let store = open_store()?;
    store
        .unarchive(&parse_id(id)?)
        .map(|dest| dest.display().to_string())
        .map_err(|e| e.to_string())
}

/// Every non-blank section line of every workspace (archived included)
/// plus one title entry per workspace — the `\` finder's corpus. Long
/// lines are capped so a pasted log can't bloat the IPC payload.
#[tauri::command]
pub fn search_corpus() -> Result<Vec<dto::CorpusLine>, String> {
    const MAX_LINE_CHARS: usize = 240;
    let store = open_store()?;
    let listing = store.list_all().map_err(|e| e.to_string())?;
    let mut corpus = Vec::new();
    for rca in &listing.summaries {
        corpus.push(dto::CorpusLine {
            id: rca.id.to_string(),
            title: rca.meta.title.clone(),
            file: SectionKind::Summary.file_name().to_owned(),
            line: 0,
            text: rca.meta.title.clone(),
        });
        for kind in SectionKind::ALL {
            let Ok(Some(content)) = store.read_section(&rca.id, kind) else {
                continue;
            };
            for (line, raw) in content.lines().enumerate() {
                let text = raw.trim();
                if text.is_empty() {
                    continue;
                }
                corpus.push(dto::CorpusLine {
                    id: rca.id.to_string(),
                    title: rca.meta.title.clone(),
                    file: kind.file_name().to_owned(),
                    line,
                    text: text.chars().take(MAX_LINE_CHARS).collect(),
                });
            }
        }
    }
    Ok(corpus)
}

/// Attaches a remediation PR URL to the workspace manifest. Returns
/// `false` when the URL was already attached (idempotent, like the CLI).
#[tauri::command]
pub fn add_pr(id: &str, url: &str) -> Result<bool, String> {
    let store = open_store()?;
    store.add_pr(&parse_id(id)?, url).map_err(|e| e.to_string())
}

/// Live PR states via the `gh` CLI, url → lowercase label ("open",
/// "draft", "merged", "closed"). Async so the shell-outs run off the main
/// thread; without `gh` the map is empty and PRs render as plain links —
/// degraded, never broken.
#[tauri::command]
pub async fn pr_states(urls: Vec<String>) -> Result<HashMap<String, String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !beagle::prs::gh_available() {
            return HashMap::new();
        }
        urls.iter()
            .filter_map(|url| {
                beagle::prs::state_of(url).map(|state| (url.clone(), state.label().to_owned()))
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}
