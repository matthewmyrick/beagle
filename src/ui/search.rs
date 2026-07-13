//! In-content search: `/` from anywhere, `n`/`N` between matches.
//!
//! One query searches **every section tab of the selected incident** —
//! you're hunting through an investigation, not a file. Matching runs over
//! the rendered lines (what you search is what you see), case-insensitively,
//! as an exact substring. `n`/`N` walk the hits in tab order and switch tabs
//! automatically when the next hit lives on another one; jumps account for
//! line wrapping by measuring the wrapped height above the match.
//!
//! Diagrams are outside the search (they have their own `n`/`p` cycling and
//! per-file panning); the query survives tab and workspace switches, with
//! hits recomputed whenever the pane reloads.

use ratatui::text::{Line, Text};
use ratatui::widgets::Paragraph;

use crate::markdown;

use super::{App, Focus, Tab};

/// One search hit: a rendered line on a section tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SearchHit {
    /// The tab the hit lives on.
    pub tab: Tab,
    /// Rendered-line index within that tab.
    pub line: usize,
}

/// Live state of the in-content search.
#[derive(Debug, Clone)]
pub(crate) struct ContentSearch {
    /// The query as typed.
    pub query: String,
    /// True while keystrokes are still editing the query.
    pub typing: bool,
    /// Every hit across the incident's section tabs, in tab order.
    pub hits: Vec<SearchHit>,
    /// Index into `hits` of the current hit.
    pub current: usize,
    /// Rendered text per section tab, snapshotted so hit line numbers,
    /// highlights, and scroll targets all agree. Rebuilt on pane reloads.
    corpus: Vec<(Tab, Text<'static>)>,
}

/// Rendered lines of `text` whose flattened content contains `query`,
/// case-insensitively. Empty queries match nothing (not everything): an
/// empty search highlighting every line would be noise.
pub(crate) fn find_match_lines(text: &Text<'_>, query: &str) -> Vec<usize> {
    if query.is_empty() {
        return Vec::new();
    }
    let needle = query.to_lowercase();
    text.lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line_content(line).to_lowercase().contains(&needle))
        .map(|(index, _)| index)
        .collect()
}

/// A line's visible text, flattened across spans.
fn line_content(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

/// Rebuilds `line` with `highlight` patched onto every occurrence of
/// `query` (case-insensitive) — and only the occurrence, not the whole
/// line. Spans are split at match boundaries, so a match straddling two
/// differently-styled spans highlights cleanly across both.
pub(crate) fn highlight_occurrences(
    line: &Line<'static>,
    query: &str,
    highlight: ratatui::style::Style,
) -> Line<'static> {
    // Per-char case folding keeps char offsets 1:1 with the original text
    // (full to_lowercase() can change lengths for exotic chars).
    let fold = |c: char| c.to_lowercase().next().unwrap_or(c);
    let flat: Vec<char> = line.spans.iter().flat_map(|s| s.content.chars()).collect();
    let needle: Vec<char> = query.chars().map(fold).collect();
    if needle.is_empty() || flat.len() < needle.len() {
        return line.clone();
    }
    let folded: Vec<char> = flat.iter().map(|&c| fold(c)).collect();

    // Non-overlapping match ranges, in char offsets.
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i + needle.len() <= folded.len() {
        if folded[i..i + needle.len()] == needle[..] {
            ranges.push((i, i + needle.len()));
            i += needle.len();
        } else {
            i += 1;
        }
    }
    if ranges.is_empty() {
        return line.clone();
    }
    let in_match = |index: usize| {
        ranges
            .iter()
            .any(|&(start, end)| index >= start && index < end)
    };

    let mut spans_out: Vec<ratatui::text::Span<'static>> = Vec::new();
    let mut offset = 0usize;
    for span in &line.spans {
        let mut segment = String::new();
        let mut segment_matched = None;
        let mut flush = |segment: &mut String, matched: bool| {
            if !segment.is_empty() {
                let style = if matched {
                    span.style.patch(highlight)
                } else {
                    span.style
                };
                spans_out.push(ratatui::text::Span::styled(std::mem::take(segment), style));
            }
        };
        for c in span.content.chars() {
            let matched = in_match(offset);
            if segment_matched.is_some_and(|m| m != matched) {
                flush(&mut segment, !matched);
            }
            segment_matched = Some(matched);
            segment.push(c);
            offset += 1;
        }
        if let Some(matched) = segment_matched {
            flush(&mut segment, matched);
        }
    }
    let mut out = Line::from(spans_out);
    out.style = line.style;
    out.alignment = line.alignment;
    out
}

/// All hits for `query` across a corpus, in tab order.
fn find_hits(corpus: &[(Tab, Text<'static>)], query: &str) -> Vec<SearchHit> {
    corpus
        .iter()
        .flat_map(|(tab, text)| {
            find_match_lines(text, query)
                .into_iter()
                .map(|line| SearchHit { tab: *tab, line })
        })
        .collect()
}

impl App {
    /// Enters search-typing mode with a fresh query, snapshotting every
    /// section of the selected incident as the search corpus. Works from
    /// either focus — `/` searches the selected incident wherever you are.
    pub(crate) fn start_content_search(&mut self) {
        if self.selected_rca().is_none() {
            self.status = Some("no incident selected — nothing to search".to_owned());
            return;
        }
        // Searching is content work: land on the incident immediately so
        // the live highlights appear where you're looking.
        self.focus = Focus::Content;
        self.content_search = Some(ContentSearch {
            query: String::new(),
            typing: true,
            hits: Vec::new(),
            current: 0,
            corpus: self.build_search_corpus(),
        });
    }

    /// Rendered text of every present section tab, in tab order. Rendering
    /// here matches the pane renderer exactly, so line indices line up.
    fn build_search_corpus(&self) -> Vec<(Tab, Text<'static>)> {
        let Some(rca) = self.selected_rca() else {
            return Vec::new();
        };
        let id = rca.id.clone();
        Tab::ALL
            .iter()
            .filter_map(|tab| {
                let kind = tab.section()?;
                let content = self.store.read_section(&id, kind).ok().flatten()?;
                Some((*tab, markdown::to_text(&content)))
            })
            .collect()
    }

    /// Keystrokes while the search query is being typed.
    pub(crate) fn handle_content_search_key(&mut self, code: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;
        match code {
            KeyCode::Esc => self.clear_content_search(),
            KeyCode::Enter => {
                let empty = self
                    .content_search
                    .as_ref()
                    .map_or(true, |s| s.query.is_empty());
                if empty {
                    self.content_search = None;
                } else {
                    if let Some(search) = self.content_search.as_mut() {
                        search.typing = false;
                    }
                    // Committing may land on a hit on another tab.
                    self.jump_to_current_hit();
                }
            }
            KeyCode::Backspace => {
                if let Some(search) = self.content_search.as_mut() {
                    search.query.pop();
                }
                self.rescan_hits(true);
            }
            KeyCode::Char(c) => {
                if let Some(search) = self.content_search.as_mut() {
                    search.query.push(c);
                }
                self.rescan_hits(true);
            }
            _ => {}
        }
    }

    /// Jumps to the next (`+1`) or previous (`-1`) hit, wrapping — and
    /// switching tabs when the hit lives on another one.
    pub(crate) fn search_step(&mut self, direction: i8) {
        let Some(search) = self.content_search.as_mut() else {
            return;
        };
        let total = search.hits.len();
        if total == 0 {
            return;
        }
        let total_i = isize::try_from(total).unwrap_or(isize::MAX);
        let current = isize::try_from(search.current).unwrap_or(0);
        let next = (current + isize::from(direction)).rem_euclid(total_i);
        search.current = usize::try_from(next).unwrap_or(0);
        self.jump_to_current_hit();
    }

    /// Drops the search and its highlights.
    pub(crate) fn clear_content_search(&mut self) {
        self.content_search = None;
    }

    /// The live search state, when one exists (typing or committed).
    pub(crate) fn content_search(&self) -> Option<&ContentSearch> {
        self.content_search.as_ref()
    }

    /// Match lines on `tab`, each flagged as the current hit or not — what
    /// the draw pass highlights.
    pub(crate) fn search_highlights(&self, tab: Tab) -> Vec<(usize, bool)> {
        let Some(search) = self.content_search.as_ref() else {
            return Vec::new();
        };
        search
            .hits
            .iter()
            .enumerate()
            .filter(|(_, hit)| hit.tab == tab)
            .map(|(index, hit)| (hit.line, index == search.current))
            .collect()
    }

    /// Re-runs the scan against a fresh corpus. Called on query edits
    /// (`jump` = true: scroll to the nearest hit) and on pane reloads
    /// (`jump` = false: switching tabs must not yank the scroll around).
    pub(crate) fn recompute_content_matches(&mut self, jump: bool) {
        if self.content_search.is_some() {
            if let Some(search) = self.content_search.as_mut() {
                search.corpus = Vec::new(); // replaced below; avoids a clone
            }
            let corpus = self.build_search_corpus();
            if let Some(search) = self.content_search.as_mut() {
                search.corpus = corpus;
            }
            self.rescan_hits(jump);
        }
    }

    /// Recomputes `hits` from the current corpus, preferring a hit on the
    /// tab already in view; scrolls to it when `jump` is set (never
    /// switching tabs — only explicit enter/n/N do that).
    fn rescan_hits(&mut self, jump: bool) {
        let tab = self.tab;
        let Some(search) = self.content_search.as_mut() else {
            return;
        };
        search.hits = find_hits(&search.corpus, &search.query);
        search.current = search
            .hits
            .iter()
            .position(|hit| hit.tab == tab)
            .unwrap_or(0);
        let on_screen = search
            .hits
            .get(search.current)
            .is_some_and(|hit| hit.tab == tab);
        if jump && on_screen {
            self.scroll_to_current_hit();
        }
    }

    /// Brings the current hit on screen, switching tabs if it lives on
    /// another one.
    fn jump_to_current_hit(&mut self) {
        let Some(hit) = self
            .content_search
            .as_ref()
            .and_then(|s| s.hits.get(s.current).copied())
        else {
            return;
        };
        if hit.tab != self.tab {
            self.tab = hit.tab;
            self.reset_scroll();
        }
        // Navigating to a hit is content navigation, wherever it started.
        self.focus = Focus::Content;
        self.scroll_to_current_hit();
    }

    /// Scrolls the pane so the current hit is on screen, accounting for
    /// wrapped lines.
    fn scroll_to_current_hit(&mut self) {
        let Some(search) = self.content_search.as_ref() else {
            return;
        };
        let Some(hit) = search.hits.get(search.current) else {
            return;
        };
        let Some((_, text)) = search.corpus.iter().find(|(tab, _)| *tab == hit.tab) else {
            return;
        };
        let target = wrapped_rows_before(text, hit.line, self.viewport.width);
        // A couple of context lines above the hit; the draw pass clamps to
        // the real content height.
        self.scroll = target.saturating_sub(2);
    }
}

/// Rows the first `line_index` lines occupy after wrapping at `width`.
/// Before the first draw the width is unknown (0) — treat lines as
/// unwrapped, which the next draw's clamp corrects.
fn wrapped_rows_before(text: &Text<'_>, line_index: usize, width: u16) -> u16 {
    if width == 0 {
        return u16::try_from(line_index).unwrap_or(u16::MAX);
    }
    let rows: usize = text
        .lines
        .iter()
        .take(line_index)
        .map(|line| {
            Paragraph::new(Text::from(line.clone()))
                .wrap(ratatui::widgets::Wrap { trim: false })
                .line_count(width)
        })
        .sum();
    u16::try_from(rows).unwrap_or(u16::MAX)
}

#[cfg(test)]
#[path = "tests/search.rs"]
mod tests;
