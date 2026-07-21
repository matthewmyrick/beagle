//! One-shot user actions: copy a tab or the whole workspace to the
//! clipboard, export the workspace to a markdown file, and the `V`
//! final-review sign-off.

use crate::model::{RcaId, Status};

use super::App;

impl App {
    /// `V`: signs off a `final-review` workspace as verified → `finished`.
    /// Viewing never mutates state; this explicit keypress is the human
    /// confirming the Final Review checklist held up.
    pub(crate) fn verify_final_review(&mut self) {
        let Some(rca) = self.selected_rca() else {
            self.status = Some("no workspace selected".to_owned());
            return;
        };
        let id = rca.id.clone();
        match rca.meta.status {
            Status::FinalReview => match self.store.set_status(&id, Status::Finished) {
                Ok(_) => {
                    self.status = Some(format!("{id} verified → finished ✔"));
                    let _ = self.reload();
                }
                Err(e) => self.status = Some(format!("sign-off failed: {e}")),
            },
            other => {
                self.status = Some(format!(
                    "V signs off a final-review incident — this one is {other}"
                ));
            }
        }
    }

    /// Copies the current tab's raw content (markdown or diagram source) to
    /// the clipboard. Reads from disk on demand — the pane cache holds
    /// styled text, and the raw bytes are what's useful in a paste.
    pub(crate) fn copy_current_tab(&mut self) {
        let Some(rca) = self.selected_rca() else {
            return;
        };
        let id = rca.id.clone();
        let loaded = match self.tab.section() {
            Some(kind) => self
                .store
                .read_section(&id, kind)
                .map(|c| c.map(|content| (kind.title().to_owned(), content))),
            None => self.current_diagram_raw(&id),
        };
        match loaded {
            Ok(Some((label, content))) => self.finish_copy(&label, &content),
            Ok(None) => self.status = Some("nothing to copy on this tab yet".to_owned()),
            Err(e) => self.status = Some(format!("copy failed: {e}")),
        }
    }

    /// The name and raw content of the currently shown diagram, if any.
    pub(crate) fn current_diagram_raw(
        &self,
        id: &RcaId,
    ) -> crate::error::Result<Option<(String, String)>> {
        let entries = self.store.list_diagrams(id)?;
        let Some(entry) = entries.get(self.diagram_index.min(entries.len().saturating_sub(1)))
        else {
            return Ok(None);
        };
        Ok(self
            .store
            .read_diagram(entry)?
            .map(|content| (entry.name.clone(), content)))
    }

    /// Copies the whole workspace as one markdown document — the exact same
    /// document [`Store::export_markdown`](crate::store::Store::export_markdown)
    /// writes, so clipboard and export never drift apart.
    pub(crate) fn copy_workspace(&mut self) {
        let Some(rca) = self.selected_rca() else {
            return;
        };
        let id = rca.id.clone();
        match self.store.export_markdown(&id) {
            Ok(doc) => self.finish_copy("whole RCA", &doc),
            Err(e) => self.status = Some(format!("copy failed: {e}")),
        }
    }

    /// Exports the selected workspace to `<root>/exports/<id>.md` —
    /// deterministic, ready to sync anywhere (e.g. an Obsidian vault).
    pub(crate) fn export_current(&mut self) {
        let Some(rca) = self.selected_rca() else {
            return;
        };
        let id = rca.id.clone();
        self.status = Some(match self.store.export_to(&id, None) {
            Ok(path) => {
                // Relative to the store root: short enough to survive a
                // one-line status bar; the full path is deducible from it.
                let shown = path.strip_prefix(self.store.root()).unwrap_or(&path);
                format!("exported to {}", shown.display())
            }
            Err(e) => format!("export failed: {e}"),
        });
    }

    pub(super) fn finish_copy(&mut self, label: &str, content: &str) {
        self.status = Some(match crate::clipboard::copy(content) {
            Ok(method) => format!(
                "copied {label} ({}) via {method}",
                human_size(content.len())
            ),
            Err(e) => format!("copy failed: {e}"),
        });
    }
}

/// Formats a byte count for the status bar: `842 B`, `1.3 KB`, `2.0 MB`.
fn human_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        #[allow(clippy::cast_precision_loss)] // < 1 MiB, exactly representable
        let kb = bytes as f64 / 1024.0;
        format!("{kb:.1} KB")
    } else {
        #[allow(clippy::cast_precision_loss)] // display only; precision loss is fine
        let mb = bytes as f64 / (1024.0 * 1024.0);
        format!("{mb:.1} MB")
    }
}

#[cfg(test)]
#[path = "tests/actions.rs"]
mod tests;
