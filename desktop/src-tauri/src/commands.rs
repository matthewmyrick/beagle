//! Tauri commands: the IPC surface the frontend calls.
//!
//! Each command opens the store fresh — the store is a thin handle over
//! paths, and per-call opening keeps commands stateless and the root
//! resolution identical to the CLI's (`--root` flag aside): config file
//! `root`, then the current directory.

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
    let nearest = cwd
        .ancestors()
        .find(|dir| dir.join(beagle::store::RCAS_DIR).is_dir())
        .map(std::path::Path::to_path_buf);
    Ok(nearest.unwrap_or(cwd))
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
