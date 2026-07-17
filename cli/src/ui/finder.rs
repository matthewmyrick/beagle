//! The `\` global fuzzy finder: telescope-style discovery across **every**
//! workspace, tab, and line at once.
//!
//! Distinct from `/` (issue #15): `/` is a precise substring search within
//! the selected incident; `\` is for when you don't know the exact string
//! or which incident it lives in — type a few fuzzy characters, see ranked
//! candidates, jump. The corpus is every rendered section line of every
//! loaded workspace (archived included — it doubles as a knowledge-base
//! search) plus incident titles, built once when the popup opens and kept
//! for its lifetime. Rendering matches the pane renderer exactly, so the
//! jump's line index lands where the eye expects.

use crossterm::event::KeyCode;

use crate::model::RcaId;
use crate::{fuzzy, markdown};

use super::{App, Focus, Tab};

/// Most results kept after ranking — rendering more is noise, and the list
/// is re-ranked on every keystroke anyway.
const MAX_MATCHES: usize = 200;

/// One searchable line: where it lives and what it says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FinderEntry {
    /// The workspace the line belongs to.
    pub id: RcaId,
    /// The workspace's title, shown as the result's context.
    pub title: String,
    /// The tab the line lives on.
    pub tab: Tab,
    /// Rendered-line index within that tab (0 for title entries).
    pub line: usize,
    /// The line's visible text (the incident title for title entries).
    pub text: String,
}

/// One ranked result: which corpus entry, and which chars of its text
/// matched (for highlighting).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FinderMatch {
    /// Index into the finder's corpus.
    pub entry: usize,
    /// Char offsets into the entry's text that matched the query.
    pub positions: Vec<usize>,
}

/// Live state of the global finder popup.
#[derive(Debug, Clone)]
pub(crate) struct Finder {
    /// The query as typed.
    pub query: String,
    /// Ranked matches, best first, capped at [`MAX_MATCHES`].
    pub matches: Vec<FinderMatch>,
    /// Index into `matches` of the highlighted result.
    pub selected: usize,
    /// Every searchable line, built when the popup opened.
    corpus: Vec<FinderEntry>,
}

impl Finder {
    /// The corpus entry behind match `index`, if both exist.
    pub(crate) fn entry(&self, index: usize) -> Option<&FinderEntry> {
        self.corpus.get(self.matches.get(index)?.entry)
    }
}

impl App {
    /// `\`: opens the finder over a fresh corpus. With nothing to search
    /// (no workspaces) it explains itself instead of opening empty.
    pub(crate) fn open_finder(&mut self) {
        if self.rcas.is_empty() {
            self.status = Some("no incidents yet — nothing to find".to_owned());
            return;
        }
        let corpus = self.build_finder_corpus();
        let mut finder = Finder {
            query: String::new(),
            matches: Vec::new(),
            selected: 0,
            corpus,
        };
        rescore(&mut finder);
        self.finder = Some(finder);
    }

    /// Every rendered section line of every loaded workspace (archived
    /// included), plus one title entry per workspace. Reads go through the
    /// store's bounded reader, so a pathological file cannot balloon this.
    fn build_finder_corpus(&self) -> Vec<FinderEntry> {
        let mut corpus = Vec::new();
        for rca in &self.rcas {
            corpus.push(FinderEntry {
                id: rca.id.clone(),
                title: rca.meta.title.clone(),
                tab: Tab::Summary,
                line: 0,
                text: rca.meta.title.clone(),
            });
            for tab in Tab::ALL {
                let Some(kind) = tab.section() else { continue };
                let Ok(Some(content)) = self.store.read_section(&rca.id, kind) else {
                    continue;
                };
                for (line, rendered) in markdown::to_text(&content).lines.iter().enumerate() {
                    let text: String = rendered.spans.iter().map(|s| s.content.as_ref()).collect();
                    if text.trim().is_empty() {
                        continue;
                    }
                    corpus.push(FinderEntry {
                        id: rca.id.clone(),
                        title: rca.meta.title.clone(),
                        tab,
                        line,
                        text,
                    });
                }
            }
        }
        corpus
    }

    /// Keystrokes while the finder is open: chars type, arrows (or
    /// ctrl-n/ctrl-p) move, enter jumps, esc closes.
    pub(crate) fn handle_finder_key(&mut self, code: KeyCode, ctrl: bool) {
        let Some(finder) = self.finder.as_mut() else {
            return;
        };
        match code {
            KeyCode::Esc => self.finder = None,
            KeyCode::Enter => self.finder_jump(),
            KeyCode::Down => move_selection(finder, 1),
            KeyCode::Up => move_selection(finder, -1),
            KeyCode::Char('n' | 'j') if ctrl => move_selection(finder, 1),
            KeyCode::Char('p' | 'k') if ctrl => move_selection(finder, -1),
            KeyCode::Backspace => {
                finder.query.pop();
                rescore(finder);
            }
            KeyCode::Char(c) if !ctrl => {
                finder.query.push(c);
                rescore(finder);
            }
            _ => {}
        }
    }

    /// Enter: close the popup and land on the picked line — selecting the
    /// incident (revealing it if a filter or the archive toggle hides it),
    /// switching to its tab, and scrolling the line into view.
    fn finder_jump(&mut self) {
        let Some(entry) = self
            .finder
            .as_ref()
            .and_then(|f| f.entry(f.selected))
            .cloned()
        else {
            self.finder = None;
            return;
        };
        self.finder = None;

        // The target must be selectable: drop any filter hiding it and
        // reveal the archive if that's where it lives.
        let archived = self
            .rcas
            .iter()
            .any(|rca| rca.id == entry.id && rca.archived);
        if archived {
            self.show_archived = true;
        }
        if self.has_active_filter() {
            self.clear_filter();
        }
        self.recompute_visible(Some(entry.id.clone()));

        self.tab = entry.tab;
        self.reset_scroll();
        self.focus = Focus::Content;

        // Scroll the picked line into view, wrap-aware — same math as the
        // in-content search, over the same rendering.
        if let Ok(Some(content)) = self.store.read_section(
            &entry.id,
            entry
                .tab
                .section()
                .unwrap_or(crate::model::SectionKind::Summary),
        ) {
            let text = markdown::to_text(&content);
            let target = super::search::wrapped_rows_before(&text, entry.line, self.viewport.width);
            self.scroll = target.saturating_sub(2);
        }
    }

    /// The live finder state, when open.
    pub(crate) fn finder(&self) -> Option<&Finder> {
        self.finder.as_ref()
    }
}

/// Re-ranks the corpus against the query. An empty query matches nothing —
/// the popup opens quiet and fills as you type.
fn rescore(finder: &mut Finder) {
    if finder.query.is_empty() {
        finder.matches = Vec::new();
    } else {
        let mut scored: Vec<(i32, FinderMatch)> = finder
            .corpus
            .iter()
            .enumerate()
            .filter_map(|(entry, e)| {
                let (score, positions) = fuzzy::indices(&finder.query, &e.text)?;
                Some((score, FinderMatch { entry, positions }))
            })
            .collect();
        scored.sort_by_key(|&(score, ref m)| (-score, m.entry));
        finder.matches = scored
            .into_iter()
            .take(MAX_MATCHES)
            .map(|(_, m)| m)
            .collect();
    }
    finder.selected = 0;
}

/// Moves the highlighted result, clamped at both ends.
fn move_selection(finder: &mut Finder, direction: i8) {
    if finder.matches.is_empty() {
        finder.selected = 0;
        return;
    }
    let last = finder.matches.len() - 1;
    finder.selected = if direction < 0 {
        finder.selected.saturating_sub(1)
    } else {
        finder.selected.saturating_add(1).min(last)
    };
}

#[cfg(test)]
#[path = "tests/finder.rs"]
mod tests;
