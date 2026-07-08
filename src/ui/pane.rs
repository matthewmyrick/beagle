//! Content loading and caching for the right-hand pane.
//!
//! Section markdown is read when its tab is opened, cached for the current
//! (workspace, tab, diagram) triple, and dropped on switch — memory scales
//! with what's on screen, not with the corpus.

use ratatui::text::Text;

use crate::markdown;
use crate::model::RcaId;
use crate::store::DiagramEntry;

use super::{App, Tab};

/// What the content pane currently shows.
#[derive(Debug)]
pub enum Pane {
    /// Rendered markdown for a section tab.
    Section(Text<'static>),
    /// A diagram (raw, unwrapped text) plus its position among siblings.
    Diagram {
        /// Rendered (raw) diagram text.
        text: Text<'static>,
        /// Name of the current diagram file.
        name: String,
        /// Zero-based index among the workspace's diagrams.
        index: usize,
        /// Total number of diagrams in the workspace.
        total: usize,
    },
    /// Nothing on disk yet for this tab; the string is a hint for the user.
    Empty(String),
    /// The tab's file exists but could not be loaded.
    LoadError(String),
}

/// Identity of the cached pane content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PaneKey {
    pub(crate) rca: RcaId,
    pub(crate) tab: Tab,
    pub(crate) diagram_index: usize,
}

impl App {
    /// Loads (or reuses) the content for the current workspace + tab.
    pub(crate) fn ensure_pane(&mut self) {
        let Some(rca) = self.selected_rca() else {
            self.pane = None;
            return;
        };
        let key = PaneKey {
            rca: rca.id.clone(),
            tab: self.tab,
            diagram_index: self.diagram_index,
        };
        if self.pane.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        let pane = self.load_pane(&key);
        // Looking at a tab reads it.
        if let Some(kind) = key.tab.section() {
            self.unread.remove(&(key.rca.clone(), kind));
        }
        self.pane = Some((key, pane));
    }

    fn load_pane(&mut self, key: &PaneKey) -> Pane {
        match key.tab.section() {
            Some(kind) => match self.store.read_section(&key.rca, kind) {
                Ok(Some(content)) => Pane::Section(markdown::to_text(&content)),
                Ok(None) => Pane::Empty(format!(
                    "No {} yet.\n\nAsk Claude to write `rcas/{}/{}` and it will appear here live.",
                    kind.title().to_lowercase(),
                    key.rca,
                    kind.file_name(),
                )),
                Err(e) => Pane::LoadError(e.to_string()),
            },
            None => self.load_diagram_pane(key),
        }
    }

    fn load_diagram_pane(&mut self, key: &PaneKey) -> Pane {
        let entries: Vec<DiagramEntry> = match self.store.list_diagrams(&key.rca) {
            Ok(entries) => entries,
            Err(e) => return Pane::LoadError(e.to_string()),
        };
        if entries.is_empty() {
            return Pane::Empty(format!(
                "No diagrams yet.\n\nDrop ASCII diagrams into `rcas/{}/diagrams/` \
                 (e.g. `01-topology.txt`) and cycle them here with n / p.",
                key.rca,
            ));
        }
        // Clamp the requested index so deleting files can't strand us.
        self.diagram_index = key.diagram_index.min(entries.len() - 1);
        let entry = &entries[self.diagram_index];
        match self.store.read_diagram(entry) {
            Ok(Some(content)) => Pane::Diagram {
                // Diagrams support ANSI SGR colors; codes are zero-width so
                // alignment is preserved.
                text: crate::ansi::to_text(&content),
                name: entry.name.clone(),
                index: self.diagram_index,
                total: entries.len(),
            },
            Ok(None) => Pane::Empty("diagram vanished; press r to reload".to_owned()),
            Err(e) => Pane::LoadError(e.to_string()),
        }
    }

    /// Read-only view of the current pane for drawing.
    #[must_use]
    pub(crate) fn pane(&self) -> Option<&Pane> {
        self.pane.as_ref().map(|(_, p)| p)
    }
}

#[cfg(test)]
#[path = "tests/pane.rs"]
mod tests;
