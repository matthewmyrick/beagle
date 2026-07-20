//! The modal overlays: the `o` link popup, the `R` related-incidents popup,
//! and the `T` toolbox. Each owns the keys while open, so `q`/`j`/`k`
//! scroll or close instead of leaking into the app underneath.

use crossterm::event::KeyCode;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Text};

use crate::markdown;
use crate::model::{RcaId, Severity, Status};

use super::App;

/// State of the `o` link popup.
#[derive(Debug)]
pub(crate) struct LinksPopup {
    /// Links to offer: attached PRs first, then URLs from the current tab.
    pub items: Vec<String>,
    /// Index of the highlighted link.
    pub selected: usize,
}

/// One row of the `R` related-incidents popup.
#[derive(Debug, Clone)]
pub(crate) struct RelatedItem {
    /// The related workspace.
    pub id: RcaId,
    /// Its severity (for the badge).
    pub severity: Severity,
    /// Its status (for the glyph).
    pub status: Status,
    /// Its title.
    pub title: String,
    /// Why it ranked: `system: alloy` / `2 systems, tag: ingestion`.
    pub shared: String,
}

/// State of the `D` delete confirmation popup: the workspace it will
/// delete if confirmed. Pinned by id at open time, so a selection change
/// underneath (e.g. a reload) can never redirect the delete.
#[derive(Debug)]
pub(crate) struct ConfirmDelete {
    /// The workspace to delete on `y`.
    pub id: RcaId,
    /// Its title, shown so the user confirms the right incident.
    pub title: String,
}

/// State of the `R` related-incidents popup.
#[derive(Debug)]
pub(crate) struct RelatedPopup {
    /// Related workspaces, best match first.
    pub items: Vec<RelatedItem>,
    /// Index of the highlighted row.
    pub selected: usize,
}

impl App {
    /// Builds and opens the `o` popup: the workspace's attached PRs first,
    /// then every URL found in the current tab's raw content.
    pub(crate) fn open_links(&mut self) {
        let Some(rca) = self.selected_rca() else {
            self.status = Some("no workspace selected".to_owned());
            return;
        };
        let id = rca.id.clone();
        let mut items: Vec<String> = rca.meta.prs.clone();
        let raw = match self.tab.section() {
            Some(kind) => self.store.read_section(&id, kind).ok().flatten(),
            None => self
                .current_diagram_raw(&id)
                .ok()
                .flatten()
                .map(|(_, content)| content),
        };
        if let Some(raw) = raw {
            for url in crate::links::extract_urls(&raw) {
                if !items.contains(&url) {
                    items.push(url);
                }
            }
        }
        if items.is_empty() {
            self.status = Some("no attached PRs or links on this tab".to_owned());
            return;
        }
        self.links = Some(LinksPopup { items, selected: 0 });
    }

    /// Keystrokes while the link popup is open: pick, open, or close.
    pub(crate) fn handle_links_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::Char('q' | 'o') => self.links = None,
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(popup) = self.links.as_mut() {
                    popup.selected = (popup.selected + 1).min(popup.items.len().saturating_sub(1));
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(popup) = self.links.as_mut() {
                    popup.selected = popup.selected.saturating_sub(1);
                }
            }
            KeyCode::Enter => {
                let url = self
                    .links
                    .take()
                    .and_then(|popup| popup.items.get(popup.selected).cloned());
                if let Some(url) = url {
                    self.status = Some(match crate::links::open_url(&url) {
                        Ok(()) => format!("opened {url}"),
                        Err(e) => format!("open failed: {e}"),
                    });
                }
            }
            _ => {}
        }
    }

    /// The link popup, when open.
    pub(crate) fn links(&self) -> Option<&LinksPopup> {
        self.links.as_ref()
    }

    /// Opens the `D` delete confirmation for the selected incident. The
    /// delete itself only happens on an explicit `y` — see
    /// [`Self::handle_confirm_delete_key`].
    pub(crate) fn open_confirm_delete(&mut self) {
        let Some(rca) = self.selected_rca() else {
            self.status = Some("no incident selected — nothing to delete".to_owned());
            return;
        };
        self.confirm_delete = Some(ConfirmDelete {
            id: rca.id.clone(),
            title: rca.meta.title.clone(),
        });
    }

    /// Keystrokes while the delete confirmation is open. Only an explicit
    /// `y` deletes; `n`, esc, or `q` cancel. Enter deliberately does
    /// nothing — a queued enter from normal navigation must never confirm
    /// a destructive action.
    pub(crate) fn handle_confirm_delete_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('y' | 'Y') => {
                let Some(confirm) = self.confirm_delete.take() else {
                    return;
                };
                match self.store.delete(&confirm.id) {
                    Ok(_) => {
                        let _ = self.reload();
                        self.status = Some(format!("deleted {}", confirm.id));
                    }
                    Err(e) => self.status = Some(format!("delete failed: {e}")),
                }
            }
            KeyCode::Esc | KeyCode::Char('n' | 'N' | 'q') => {
                self.confirm_delete = None;
                self.status = Some("delete cancelled".to_owned());
            }
            _ => {}
        }
    }

    /// The delete confirmation, when open.
    pub(crate) fn confirm_delete(&self) -> Option<&ConfirmDelete> {
        self.confirm_delete.as_ref()
    }

    /// Builds and opens the `R` popup: past workspaces sharing systems or
    /// tags with the selected one, best match first.
    pub(crate) fn open_related(&mut self) {
        let Some(rca) = self.selected_rca() else {
            self.status = Some("no workspace selected".to_owned());
            return;
        };
        let items: Vec<RelatedItem> = crate::similar::rank(rca, &self.rcas)
            .iter()
            .map(|entry| RelatedItem {
                id: entry.rca.id.clone(),
                severity: entry.rca.meta.severity,
                status: entry.rca.meta.status,
                title: entry.rca.meta.title.clone(),
                shared: crate::similar::shared_label(entry),
            })
            .collect();
        if items.is_empty() {
            self.status = Some(
                "no related incidents — nothing shares systems or tags with this one".to_owned(),
            );
            return;
        }
        self.related = Some(RelatedPopup { items, selected: 0 });
    }

    /// Keystrokes while the related popup is open: pick, jump, or close.
    pub(crate) fn handle_related_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::Char('q' | 'R') => self.related = None,
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(popup) = self.related.as_mut() {
                    popup.selected = (popup.selected + 1).min(popup.items.len().saturating_sub(1));
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(popup) = self.related.as_mut() {
                    popup.selected = popup.selected.saturating_sub(1);
                }
            }
            KeyCode::Enter => {
                let id = self
                    .related
                    .take()
                    .and_then(|popup| popup.items.get(popup.selected).map(|item| item.id.clone()));
                if let Some(id) = id {
                    self.jump_to_workspace(&id);
                }
            }
            _ => {}
        }
    }

    /// Selects workspace `id` in the sidebar, clearing any fuzzy filter
    /// that would hide it.
    fn jump_to_workspace(&mut self, id: &RcaId) {
        self.filter.clear();
        self.filter_input = crate::ui::FilterInput::Off;
        self.recompute_visible(Some(id.clone()));
        self.diagram_index = 0;
        self.reset_scroll();
        self.status = Some(format!("jumped to {id}"));
    }

    /// The related popup, when open.
    pub(crate) fn related(&self) -> Option<&RelatedPopup> {
        self.related.as_ref()
    }

    /// Builds and opens the toolbox overlay: the root `toolbox.md` followed
    /// by the `systems/*.md` docs matching the selected workspace's systems
    /// (all of them when nothing is selected or the workspace lists none).
    ///
    /// A missing toolbox is scaffolded on the spot (`beagle init`) —
    /// pressing `T` means the user wants one, and `init_context` never
    /// overwrites existing files, so this is always safe.
    pub(crate) fn open_toolbox(&mut self) {
        let dim = Style::default().fg(Color::DarkGray);
        let mut lines: Vec<Line<'static>> = Vec::new();

        if matches!(self.store.read_toolbox(), Ok(None)) {
            match self.store.init_context() {
                Ok(created) if !created.is_empty() => {
                    lines.push(Line::styled(
                        "Scaffolded toolbox.md + systems/ (what `beagle init` does) — \
                         fill them in for your stack; agents read them before every \
                         investigation.",
                        Style::default().fg(Color::LightGreen),
                    ));
                    lines.push(Line::from(""));
                }
                Ok(_) => {}
                Err(e) => lines.push(Line::styled(
                    format!("could not scaffold the toolbox: {e}"),
                    Style::default().fg(Color::Red),
                )),
            }
        }

        match self.store.read_toolbox() {
            Ok(Some(content)) => lines.extend(markdown::to_text(&content).lines),
            Ok(None) => lines.push(Line::styled(
                "No toolbox.md yet — run `beagle init` to scaffold the \
                 investigation context (toolbox.md + systems/).",
                dim,
            )),
            Err(e) => lines.push(Line::styled(
                format!("toolbox load error: {e}"),
                Style::default().fg(Color::Red),
            )),
        }

        let docs = match self.store.list_system_docs() {
            Ok(docs) => docs,
            Err(e) => {
                lines.push(Line::styled(
                    format!("systems/ load error: {e}"),
                    Style::default().fg(Color::Red),
                ));
                Vec::new()
            }
        };
        let wanted: Vec<String> = self
            .selected_rca()
            .map(|rca| rca.meta.systems.clone())
            .unwrap_or_default();
        let shown = docs
            .iter()
            .filter(|doc| wanted.is_empty() || wanted.contains(&doc.name));
        for doc in shown {
            lines.push(Line::from(""));
            lines.push(Line::styled(
                format!("─── systems/{}.md ───", doc.name),
                dim,
            ));
            lines.push(Line::from(""));
            match self.store.read_system_doc(doc) {
                Ok(Some(content)) => lines.extend(markdown::to_text(&content).lines),
                Ok(None) => {}
                Err(e) => lines.push(Line::styled(
                    format!("load error: {e}"),
                    Style::default().fg(Color::Red),
                )),
            }
        }
        if !wanted.is_empty() && !docs.iter().any(|d| wanted.contains(&d.name)) {
            lines.push(Line::from(""));
            lines.push(Line::styled(
                format!(
                    "(no systems/ docs for: {} — add systems/<name>.md to give \
                     agents per-system context)",
                    wanted.join(", ")
                ),
                dim,
            ));
        }

        self.toolbox = Some(Text::from(lines));
        self.toolbox_scroll = 0;
    }

    /// Keystrokes while the toolbox overlay is open: scroll or close.
    pub(crate) fn handle_toolbox_key(&mut self, code: KeyCode) {
        let (content_lines, height) = self.toolbox_viewport;
        let max = content_lines.saturating_sub(height);
        let page = height.saturating_sub(1).max(1);
        match code {
            KeyCode::Esc | KeyCode::Char('q' | 'T') => self.toolbox = None,
            KeyCode::Char('j') | KeyCode::Down => {
                self.toolbox_scroll = self.toolbox_scroll.saturating_add(1).min(max);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.toolbox_scroll = self.toolbox_scroll.saturating_sub(1);
            }
            KeyCode::Char(' ') | KeyCode::PageDown => {
                self.toolbox_scroll = self.toolbox_scroll.saturating_add(page).min(max);
            }
            KeyCode::PageUp => self.toolbox_scroll = self.toolbox_scroll.saturating_sub(page),
            KeyCode::Char('g') | KeyCode::Home => self.toolbox_scroll = 0,
            KeyCode::Char('G') | KeyCode::End => self.toolbox_scroll = max,
            _ => {}
        }
    }

    /// The toolbox overlay content, when open.
    pub(crate) fn toolbox(&self) -> Option<&Text<'static>> {
        self.toolbox.as_ref()
    }

    pub(crate) fn toolbox_scroll(&self) -> u16 {
        self.toolbox_scroll
    }
}

#[cfg(test)]
#[path = "tests/overlays.rs"]
mod tests;
