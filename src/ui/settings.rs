//! The `S` settings overlay: view and edit the config from inside the TUI.
//!
//! Every change is written straight to the config file (via
//! [`crate::config::upsert`], which preserves comments and validates before
//! writing), so the overlay and the file can never disagree. `notify`
//! applies live; `root` needs a restart (hot-swapping the store and
//! watcher is deliberately out of scope).

use crossterm::event::KeyCode;
use std::path::PathBuf;

use crate::config::{self, Config};

use super::App;

/// The editable fields, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Field {
    /// Default workspace root (string; restart to apply).
    Root,
    /// Editor for `beagle config` (string).
    Editor,
    /// Desktop notifications (bool; applies live).
    Notify,
}

impl Field {
    /// Display order.
    pub const ALL: [Self; 3] = [Self::Root, Self::Editor, Self::Notify];

    /// The config key as written to the file.
    pub fn key(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Editor => "editor",
            Self::Notify => "notify",
        }
    }

    /// One-line description for the pane.
    pub fn describe(self) -> &'static str {
        match self {
            Self::Root => "default workspace root (restart to apply)",
            Self::Editor => "editor for `beagle config`",
            Self::Notify => "desktop notifications (applies live)",
        }
    }

    /// Whether space/enter toggles this field rather than editing text.
    pub fn is_bool(self) -> bool {
        matches!(self, Self::Notify)
    }
}

/// State of the settings overlay.
#[derive(Debug)]
pub(crate) struct SettingsOverlay {
    /// The config as currently on disk.
    pub config: Config,
    /// Where it lives (shown in the pane title).
    pub path: PathBuf,
    /// Index into [`Field::ALL`] of the highlighted row.
    pub selected: usize,
    /// Inline input buffer while editing a string field.
    pub editing: Option<String>,
    /// A post-save note (e.g. "restart to apply").
    pub note: Option<String>,
}

impl SettingsOverlay {
    /// The current display value of a field.
    pub fn value_of(&self, field: Field) -> String {
        match field {
            Field::Root => self.config.root.as_ref().map_or_else(
                || "(current directory)".to_owned(),
                |p| p.display().to_string(),
            ),
            Field::Editor => self
                .config
                .editor
                .clone()
                .unwrap_or_else(|| "(auto: $VISUAL/$EDITOR/vim)".to_owned()),
            Field::Notify => {
                if self.config.notify.unwrap_or(false) {
                    "on".to_owned()
                } else {
                    "off".to_owned()
                }
            }
        }
    }

    /// Whether a field currently has an explicit value in the file (unset
    /// fields render dimmed).
    pub fn is_set(&self, field: Field) -> bool {
        match field {
            Field::Root => self.config.root.is_some(),
            Field::Editor => self.config.editor.is_some(),
            Field::Notify => self.config.notify.is_some(),
        }
    }
}

impl App {
    /// Opens the settings overlay with the config as it stands on disk.
    /// A broken config file surfaces as a status message instead of a
    /// half-loaded overlay — fix it with `beagle config` first.
    pub(crate) fn open_settings(&mut self) {
        let path = config::path();
        match config::load(&path) {
            Ok(loaded) => {
                self.settings = Some(SettingsOverlay {
                    config: loaded.unwrap_or_default(),
                    path,
                    selected: 0,
                    editing: None,
                    note: None,
                });
            }
            Err(e) => self.status = Some(format!("config did not load: {e}")),
        }
    }

    /// Keystrokes while the settings overlay is open.
    pub(crate) fn handle_settings_key(&mut self, code: KeyCode) {
        let editing = self
            .settings
            .as_ref()
            .is_some_and(|overlay| overlay.editing.is_some());
        if editing {
            self.handle_settings_edit_key(code);
            return;
        }
        match code {
            KeyCode::Esc | KeyCode::Char('q' | 'S') => self.settings = None,
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(overlay) = self.settings.as_mut() {
                    overlay.selected = (overlay.selected + 1).min(Field::ALL.len() - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(overlay) = self.settings.as_mut() {
                    overlay.selected = overlay.selected.saturating_sub(1);
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => self.activate_selected_setting(),
            _ => {}
        }
    }

    /// Enter/space on a row: toggle a boolean immediately, or open the
    /// inline editor for a string field.
    fn activate_selected_setting(&mut self) {
        let Some(overlay) = self.settings.as_mut() else {
            return;
        };
        let field = Field::ALL[overlay.selected.min(Field::ALL.len() - 1)];
        if field.is_bool() {
            let next = !overlay.config.notify.unwrap_or(false);
            self.save_setting(field, if next { "true" } else { "false" });
        } else {
            let current = match field {
                Field::Root => overlay
                    .config
                    .root
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
                Field::Editor => overlay.config.editor.clone().unwrap_or_default(),
                Field::Notify => String::new(),
            };
            overlay.editing = Some(current);
        }
    }

    /// Keystrokes while inline-editing a string value.
    fn handle_settings_edit_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                if let Some(overlay) = self.settings.as_mut() {
                    overlay.editing = None; // cancel, nothing written
                }
            }
            KeyCode::Enter => {
                let Some(overlay) = self.settings.as_mut() else {
                    return;
                };
                let field = Field::ALL[overlay.selected.min(Field::ALL.len() - 1)];
                let Some(value) = overlay.editing.take() else {
                    return;
                };
                let trimmed = value.trim().to_owned();
                if trimmed.is_empty() {
                    overlay.note = Some("empty value — unchanged (edit the file to unset)".into());
                    return;
                }
                // TOML string: quote and escape.
                let quoted = format!("\"{}\"", trimmed.replace('\\', "\\\\").replace('"', "\\\""));
                self.save_setting(field, &quoted);
            }
            KeyCode::Backspace => {
                if let Some(Some(buffer)) = self.settings.as_mut().map(|o| o.editing.as_mut()) {
                    buffer.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(Some(buffer)) = self.settings.as_mut().map(|o| o.editing.as_mut()) {
                    buffer.push(c);
                }
            }
            _ => {}
        }
    }

    /// Writes one assignment through [`config::upsert`], refreshes the
    /// overlay from the parsed result, and applies live where possible.
    fn save_setting(&mut self, field: Field, raw_value: &str) {
        let Some(overlay) = self.settings.as_mut() else {
            return;
        };
        match config::upsert(&overlay.path, field.key(), raw_value) {
            Ok(updated) => {
                let notify_now = updated.notify.unwrap_or(false);
                overlay.config = updated;
                overlay.note = Some(match field {
                    Field::Notify => {
                        format!("saved — notifications {}", overlay.value_of(Field::Notify))
                    }
                    Field::Root => "saved — restart beagle to apply the new root".to_owned(),
                    Field::Editor => "saved".to_owned(),
                });
                if field == Field::Notify {
                    // Applies live: the running TUI starts/stops notifying
                    // immediately.
                    self.notify_enabled = notify_now;
                }
            }
            Err(e) => overlay.note = Some(format!("not saved: {e}")),
        }
    }

    /// The settings overlay, when open.
    pub(crate) fn settings(&self) -> Option<&SettingsOverlay> {
        self.settings.as_ref()
    }
}

#[cfg(test)]
#[path = "tests/settings.rs"]
mod tests;
