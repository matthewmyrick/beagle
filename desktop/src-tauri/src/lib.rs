//! Beagle desktop: a Tauri shell over the CLI crate's domain layer.
//!
//! Modules: `dto` (the IPC contract, mirrored by `src/types.ts`) and
//! `commands` (the invoke surface). This file only assembles the app.

mod commands;
mod dto;

/// Builds and runs the Tauri application.
///
/// # Panics
/// Panics only if the webview cannot start at all — there is no UI to
/// degrade to at that point.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[allow(clippy::expect_used)] // no UI exists yet to report through
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::list_workspaces,
            commands::read_section
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
