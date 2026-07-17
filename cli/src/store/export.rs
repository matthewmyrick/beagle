//! Single-file markdown export: YAML frontmatter from the manifest, every
//! present section in tab order, then diagrams with ANSI colors stripped.

use std::fs;
use std::path::{Path, PathBuf};

use time::format_description::well_known::Rfc3339;

use crate::error::{Error, Result};
use crate::model::{RcaId, SectionKind};

use super::fsio::write_atomic;
use super::{Store, EXPORTS_DIR};

impl Store {
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
            None => self.root().join(EXPORTS_DIR).join(format!("{id}.md")),
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

#[cfg(test)]
#[path = "tests/export.rs"]
mod tests;
