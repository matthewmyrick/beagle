//! The TUI: application state machine, event loop, and key handling.
//!
//! All state lives in [`App`] and mutates only in response to key presses or
//! filesystem events — drawing (the private `view` module) is a pure function
//! of `App` except for scroll clamping. That split keeps every state
//! transition unit-testable without a terminal.

mod view;

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use notify::{RecursiveMode, Watcher as _};
use ratatui::text::Text;
use ratatui::DefaultTerminal;

use crate::error::Result;
use crate::markdown;
use crate::model::{RcaId, SectionKind};
use crate::store::{DiagramEntry, LoadWarning, Store};
#[cfg(doc)]
use crate::Error; // referenced by rustdoc links in this module

/// The tabs of an RCA workspace, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// What broke ([`SectionKind::Summary`]).
    Summary,
    /// What happened when ([`SectionKind::Timeline`]).
    Timeline,
    /// Why it broke ([`SectionKind::RootCause`]).
    RootCause,
    /// Who/what was affected ([`SectionKind::Impact`]).
    Impact,
    /// How to fix it ([`SectionKind::Remediation`]).
    Remediation,
    /// ASCII diagrams from the workspace's `diagrams/` directory.
    Diagrams,
    /// Raw evidence and loose ends ([`SectionKind::Notes`]).
    Notes,
}

impl Tab {
    /// Every tab, in display order.
    pub const ALL: [Self; 7] = [
        Self::Summary,
        Self::Timeline,
        Self::RootCause,
        Self::Impact,
        Self::Remediation,
        Self::Diagrams,
        Self::Notes,
    ];

    /// The tab title shown in the tab bar.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Diagrams => "Diagrams",
            Self::Summary => SectionKind::Summary.title(),
            Self::Timeline => SectionKind::Timeline.title(),
            Self::RootCause => SectionKind::RootCause.title(),
            Self::Impact => SectionKind::Impact.title(),
            Self::Remediation => SectionKind::Remediation.title(),
            Self::Notes => SectionKind::Notes.title(),
        }
    }

    /// The markdown section backing this tab; `None` for [`Tab::Diagrams`].
    #[must_use]
    pub fn section(self) -> Option<SectionKind> {
        match self {
            Self::Summary => Some(SectionKind::Summary),
            Self::Timeline => Some(SectionKind::Timeline),
            Self::RootCause => Some(SectionKind::RootCause),
            Self::Impact => Some(SectionKind::Impact),
            Self::Remediation => Some(SectionKind::Remediation),
            Self::Notes => Some(SectionKind::Notes),
            Self::Diagrams => None,
        }
    }

    /// Position of this tab within [`Tab::ALL`].
    #[must_use]
    pub fn index(self) -> usize {
        // `position` is infallible: every variant is in ALL by construction.
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    /// The next tab, wrapping.
    #[must_use]
    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    /// The previous tab, wrapping.
    #[must_use]
    pub fn prev(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// Which pane owns navigation keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// The workspace list on the left.
    List,
    /// The tabbed content pane on the right.
    Content,
}

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

/// Whether the key loop should keep running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    Continue,
    Quit,
}

/// The whole TUI state.
pub struct App {
    store: Store,
    rcas: Vec<crate::model::RcaSummary>,
    warnings: Vec<LoadWarning>,
    /// Fuzzy filter over the incident list; empty means "show everything".
    filter: String,
    /// True while `/` search input is capturing keystrokes.
    search_active: bool,
    /// Indices into `rcas` that match `filter`, best match first.
    visible: Vec<usize>,
    /// Index into `visible` of the selected workspace, if any match.
    selected: usize,
    focus: Focus,
    tab: Tab,
    diagram_index: usize,
    scroll: u16,
    hscroll: u16,
    show_help: bool,
    status: Option<String>,
    /// Content cache for the current (workspace, tab, diagram) triple.
    pane: Option<(PaneKey, Pane)>,
    /// Animation counter, advanced once per event-loop turn (the loop wakes
    /// at least every 250 ms). Drives the `investigating` spinner.
    tick: usize,
    /// Set by the draw pass; used to clamp scrolling to real content height.
    pub(crate) viewport: ViewportInfo,
}

/// Identity of the cached pane content.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PaneKey {
    rca: RcaId,
    tab: Tab,
    diagram_index: usize,
}

/// Geometry facts the draw pass feeds back for scroll clamping.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ViewportInfo {
    /// Total wrapped lines of the current content at the current width.
    pub content_lines: u16,
    /// Visible height of the content area.
    pub height: u16,
}

impl App {
    /// Builds the app and performs the initial workspace listing.
    ///
    /// # Errors
    /// Fails only if the `rcas/` directory itself is unreadable.
    pub fn new(store: Store) -> Result<Self> {
        let (rcas, warnings) = store.list()?;
        let visible = (0..rcas.len()).collect();
        Ok(Self {
            store,
            rcas,
            warnings,
            filter: String::new(),
            search_active: false,
            visible,
            selected: 0,
            focus: Focus::List,
            tab: Tab::Summary,
            diagram_index: 0,
            scroll: 0,
            hscroll: 0,
            show_help: false,
            status: None,
            pane: None,
            tick: 0,
            viewport: ViewportInfo::default(),
        })
    }

    /// Runs the event loop until the user quits.
    ///
    /// Blocks on input with a timeout; filesystem changes arrive over a
    /// channel from the `notify` watcher, so idle CPU is ~0%.
    ///
    /// # Errors
    /// Returns [`Error::Terminal`] on draw/poll failures or [`Error::Watch`]
    /// if the watcher cannot be attached.
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        let (tx, rx) = mpsc::channel::<()>();
        // The callback does the minimum: signal "something changed".
        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if res.is_ok() {
                    let _ = tx.send(()); // receiver gone means we are shutting down
                }
            })?;
        watcher.watch(self.store.watch_root(), RecursiveMode::Recursive)?;

        loop {
            if drain(&rx) {
                self.reload();
                self.status = Some("reloaded (files changed on disk)".to_owned());
            }
            self.ensure_pane();
            // The loop turns at least every 250 ms (the poll timeout below),
            // so bumping the counter here animates the investigating spinner
            // at ~4 fps without any extra wakeups.
            self.tick = self.tick.wrapping_add(1);
            terminal.draw(|frame| view::draw(frame, &mut self))?;

            if event::poll(Duration::from_millis(250))? {
                match event::read()? {
                    Event::Key(key)
                        if key.kind == KeyEventKind::Press
                            && self.handle_key(key) == Flow::Quit =>
                    {
                        return Ok(());
                    }
                    // Resize triggers a redraw on the next loop turn anyway.
                    _ => {}
                }
            }
        }
    }

    /// Re-lists workspaces, keeping the selection pinned to the same id when
    /// it survives the reload, and drops the content cache.
    fn reload(&mut self) {
        let previous = self.selected_rca().map(|r| r.id.clone());
        match self.store.list() {
            Ok((rcas, warnings)) => {
                self.rcas = rcas;
                self.warnings = warnings;
            }
            Err(e) => {
                self.warnings = vec![LoadWarning(format!("reload failed: {e}"))];
            }
        }
        self.recompute_visible(previous);
        self.pane = None;
    }

    /// Re-runs the fuzzy filter over the workspace list, keeping the
    /// selection pinned to `keep` when that workspace still matches.
    fn recompute_visible(&mut self, keep: Option<RcaId>) {
        let mut scored: Vec<(i32, usize)> = self
            .rcas
            .iter()
            .enumerate()
            .filter_map(|(index, rca)| {
                let haystack = format!(
                    "{} {} {} {}",
                    rca.meta.title,
                    rca.id,
                    rca.meta.systems.join(" "),
                    rca.meta.tags.join(" "),
                );
                crate::fuzzy::score(&self.filter, &haystack).map(|s| (s, index))
            })
            .collect();
        // Best match first; ties keep the sidebar's severity/recency order.
        scored.sort_by_key(|&(score, index)| (-score, index));
        self.visible = scored.into_iter().map(|(_, index)| index).collect();
        self.selected = keep
            .and_then(|id| self.visible.iter().position(|&i| self.rcas[i].id == id))
            .unwrap_or(0);
    }

    /// The currently selected workspace, if any match the filter.
    #[must_use]
    pub fn selected_rca(&self) -> Option<&crate::model::RcaSummary> {
        self.visible
            .get(self.selected)
            .and_then(|&index| self.rcas.get(index))
    }

    /// Loads (or reuses) the content for the current workspace + tab.
    fn ensure_pane(&mut self) {
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

    pub(crate) fn rcas(&self) -> &[crate::model::RcaSummary] {
        &self.rcas
    }

    /// Workspaces matching the current filter, best match first.
    pub(crate) fn visible_rcas(&self) -> impl Iterator<Item = &crate::model::RcaSummary> {
        self.visible
            .iter()
            .filter_map(|&index| self.rcas.get(index))
    }

    pub(crate) fn visible_len(&self) -> usize {
        self.visible.len()
    }

    pub(crate) fn filter(&self) -> &str {
        &self.filter
    }

    pub(crate) fn search_active(&self) -> bool {
        self.search_active
    }

    pub(crate) fn warnings(&self) -> &[LoadWarning] {
        &self.warnings
    }

    pub(crate) fn selected_index(&self) -> usize {
        self.selected
    }

    pub(crate) fn focus(&self) -> Focus {
        self.focus
    }

    pub(crate) fn tab(&self) -> Tab {
        self.tab
    }

    pub(crate) fn scroll_offsets(&self) -> (u16, u16) {
        (self.scroll, self.hscroll)
    }

    pub(crate) fn help_visible(&self) -> bool {
        self.show_help
    }

    pub(crate) fn status_line(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub(crate) fn tick(&self) -> usize {
        self.tick
    }

    fn handle_key(&mut self, key: KeyEvent) -> Flow {
        self.status = None;
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Flow::Quit;
        }
        if self.search_active {
            self.handle_search_key(key.code);
            return Flow::Continue;
        }
        if self.show_help {
            self.show_help = false;
            return Flow::Continue;
        }
        match key.code {
            // Shift-Q only: a plain `q` next to tab/scroll keys quit the app
            // by accident too easily. Ctrl-C is handled above.
            KeyCode::Char('Q') => return Flow::Quit,
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('/') => self.search_active = true,
            KeyCode::Char('b') => self.focus = Focus::List,
            KeyCode::Char('c') => self.copy_current_tab(),
            KeyCode::Char('C') => self.copy_workspace(),
            KeyCode::Char('e') => self.export_current(),
            KeyCode::Char('r') => {
                self.reload();
                self.status = Some("reloaded".to_owned());
            }
            KeyCode::Tab | KeyCode::Char(']') | KeyCode::Right => {
                self.switch_tab(self.tab.next());
            }
            KeyCode::BackTab | KeyCode::Char('[') | KeyCode::Left => {
                self.switch_tab(self.tab.prev());
            }
            KeyCode::Char(c @ '1'..='7') => {
                // '1'..='7' maps exactly onto Tab::ALL's seven entries.
                let index = (c as usize).saturating_sub('1' as usize);
                if let Some(tab) = Tab::ALL.get(index) {
                    self.switch_tab(*tab);
                }
            }
            KeyCode::Char('n') if self.tab == Tab::Diagrams => self.cycle_diagram(1),
            KeyCode::Char('p') if self.tab == Tab::Diagrams => self.cycle_diagram(-1),
            _ => match self.focus {
                Focus::List => self.handle_list_key(key.code),
                Focus::Content => self.handle_content_key(key.code),
            },
        }
        Flow::Continue
    }

    /// Keystrokes while the `/` filter is capturing input. Plain characters
    /// edit the query (so `q` types, it does not quit); arrows still move
    /// the selection so filtering and picking interleave naturally.
    fn handle_search_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                self.filter.clear();
                self.search_active = false;
                self.recompute_visible(None);
                self.reset_scroll();
            }
            KeyCode::Enter => self.search_active = false,
            KeyCode::Backspace => {
                self.filter.pop();
                self.recompute_visible(None);
            }
            KeyCode::Char(c) => {
                self.filter.push(c);
                self.recompute_visible(None);
            }
            KeyCode::Down => self.select(self.selected.saturating_add(1)),
            KeyCode::Up => self.select(self.selected.saturating_sub(1)),
            _ => {}
        }
    }

    fn handle_list_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.select(self.selected.saturating_add(1)),
            KeyCode::Char('k') | KeyCode::Up => self.select(self.selected.saturating_sub(1)),
            KeyCode::Char('g') | KeyCode::Home => self.select(0),
            KeyCode::Char('G') | KeyCode::End => self.select(usize::MAX),
            KeyCode::Esc if !self.filter.is_empty() => {
                self.filter.clear();
                self.recompute_visible(None);
            }
            KeyCode::Enter | KeyCode::Char('l') if !self.visible.is_empty() => {
                self.focus = Focus::Content;
            }
            _ => {}
        }
    }

    fn handle_content_key(&mut self, code: KeyCode) {
        let page = self.viewport.height.saturating_sub(1).max(1);
        match code {
            KeyCode::Esc => {
                self.focus = Focus::List;
            }
            KeyCode::Char('j') | KeyCode::Down => self.scroll_to(self.scroll.saturating_add(1)),
            KeyCode::Char('k') | KeyCode::Up => self.scroll_to(self.scroll.saturating_sub(1)),
            KeyCode::Char(' ') | KeyCode::PageDown => {
                self.scroll_to(self.scroll.saturating_add(page));
            }
            KeyCode::PageUp => self.scroll_to(self.scroll.saturating_sub(page)),
            KeyCode::Char('g') | KeyCode::Home => self.scroll_to(0),
            KeyCode::Char('G') | KeyCode::End => self.scroll_to(u16::MAX),
            // Arrows switch tabs (handled globally); h/l pan diagrams.
            KeyCode::Char('h') => self.hscroll = self.hscroll.saturating_sub(4),
            KeyCode::Char('l') => self.hscroll = self.hscroll.saturating_add(4),
            _ => {}
        }
    }

    fn select(&mut self, index: usize) {
        if self.visible.is_empty() {
            self.selected = 0;
            return;
        }
        let clamped = index.min(self.visible.len() - 1);
        if clamped != self.selected {
            self.selected = clamped;
            self.diagram_index = 0;
            self.reset_scroll();
        }
    }

    fn switch_tab(&mut self, tab: Tab) {
        if tab != self.tab {
            self.tab = tab;
            self.reset_scroll();
        }
        if !self.visible.is_empty() {
            self.focus = Focus::Content;
        }
    }

    /// Copies the current tab's raw content (markdown or diagram source) to
    /// the clipboard. Reads from disk on demand — the pane cache holds
    /// styled text, and the raw bytes are what's useful in a paste.
    fn copy_current_tab(&mut self) {
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

    fn current_diagram_raw(&self, id: &RcaId) -> crate::error::Result<Option<(String, String)>> {
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
    /// document [`Store::export_markdown`] writes, so clipboard and export
    /// never drift apart.
    fn copy_workspace(&mut self) {
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
    fn export_current(&mut self) {
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

    fn finish_copy(&mut self, label: &str, content: &str) {
        self.status = Some(match crate::clipboard::copy(content) {
            Ok(method) => format!(
                "copied {label} ({}) via {method}",
                human_size(content.len())
            ),
            Err(e) => format!("copy failed: {e}"),
        });
    }

    fn cycle_diagram(&mut self, direction: i8) {
        let total = match self.pane() {
            Some(Pane::Diagram { total, .. }) => *total,
            _ => return,
        };
        if total == 0 {
            return;
        }
        let total_i = isize::try_from(total).unwrap_or(isize::MAX);
        let current = isize::try_from(self.diagram_index).unwrap_or(0);
        let next = (current + isize::from(direction)).rem_euclid(total_i);
        self.diagram_index = usize::try_from(next).unwrap_or(0);
        self.reset_scroll();
    }

    /// Clamps vertical scroll to the real content height so the view can
    /// never scroll into the void.
    fn scroll_to(&mut self, target: u16) {
        let max = self
            .viewport
            .content_lines
            .saturating_sub(self.viewport.height);
        self.scroll = target.min(max);
    }

    fn reset_scroll(&mut self) {
        self.scroll = 0;
        self.hscroll = 0;
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

/// Drains every pending watcher event, returning whether any arrived.
/// Editors and LLMs write in bursts; we reload once per burst, not per event.
fn drain(rx: &Receiver<()>) -> bool {
    let mut any = false;
    loop {
        match rx.try_recv() {
            Ok(()) => any = true,
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => return any,
        }
    }
}

/// Sets up the terminal, runs the app, and unconditionally restores the
/// terminal — including on error paths.
///
/// # Errors
/// Propagates any [`Error`] from app construction or the event loop.
pub fn run(store: Store) -> Result<()> {
    let mut terminal = ratatui::init();
    let result = App::new(store).and_then(|app| app.run(&mut terminal));
    ratatui::restore();
    result
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

    use super::*;
    use crate::model::Severity;
    use crate::store::new_meta;

    fn app_with(n: usize) -> App {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = Store::open(tmp.path()).expect("store");
        for i in 0..n {
            let id = RcaId::new(format!("rca-{i}")).expect("valid id");
            store
                .scaffold(&id, &new_meta(format!("RCA {i}"), Severity::Medium))
                .expect("scaffold");
        }
        // Leak the tempdir handle so the files outlive the test setup; the OS
        // cleans the temp root. Simpler than threading the guard through App.
        std::mem::forget(tmp);
        App::new(store).expect("app")
    }

    fn press(app: &mut App, code: KeyCode) -> Flow {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn tab_next_prev_are_inverse_and_wrap() {
        for tab in Tab::ALL {
            assert_eq!(tab.next().prev(), tab);
        }
        assert_eq!(Tab::Notes.next(), Tab::Summary);
        assert_eq!(Tab::Summary.prev(), Tab::Notes);
    }

    #[test]
    fn only_shift_q_and_ctrl_c_quit() {
        let mut app = app_with(1);
        assert_eq!(
            press(&mut app, KeyCode::Char('q')),
            Flow::Continue,
            "plain q must not quit"
        );
        assert_eq!(press(&mut app, KeyCode::Char('Q')), Flow::Quit);
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(app.handle_key(ctrl_c), Flow::Quit);
    }

    #[test]
    fn arrow_keys_switch_tabs_from_either_pane() {
        let mut app = app_with(1);
        press(&mut app, KeyCode::Right); // from list focus
        assert_eq!(app.tab(), Tab::Timeline);
        press(&mut app, KeyCode::Right);
        assert_eq!(app.tab(), Tab::RootCause);
        press(&mut app, KeyCode::Left); // from content focus (Right focused it)
        assert_eq!(app.tab(), Tab::Timeline);
    }

    #[test]
    fn b_returns_focus_to_the_list() {
        let mut app = app_with(1);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.focus(), Focus::Content);
        press(&mut app, KeyCode::Char('b'));
        assert_eq!(app.focus(), Focus::List);
    }

    #[test]
    fn list_navigation_clamps_at_both_ends() {
        let mut app = app_with(3);
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.selected_index(), 0, "cannot go above the first item");
        for _ in 0..10 {
            press(&mut app, KeyCode::Char('j'));
        }
        assert_eq!(app.selected_index(), 2, "cannot go past the last item");
    }

    #[test]
    fn navigation_on_empty_store_does_not_panic() {
        let mut app = app_with(0);
        for code in [
            KeyCode::Char('j'),
            KeyCode::Enter,
            KeyCode::Tab,
            KeyCode::Char('G'),
        ] {
            press(&mut app, code);
        }
        assert!(app.selected_rca().is_none());
    }

    #[test]
    fn enter_focuses_content_and_esc_returns_to_list() {
        let mut app = app_with(1);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.focus(), Focus::Content);
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.focus(), Focus::List);
    }

    #[test]
    fn number_keys_jump_to_tabs() {
        let mut app = app_with(1);
        press(&mut app, KeyCode::Char('3'));
        assert_eq!(app.tab(), Tab::RootCause);
        press(&mut app, KeyCode::Char('6'));
        assert_eq!(app.tab(), Tab::Diagrams);
    }

    #[test]
    fn switching_tab_resets_scroll() {
        let mut app = app_with(1);
        app.viewport = ViewportInfo {
            content_lines: 100,
            height: 10,
        };
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char(' '));
        assert!(app.scroll_offsets().0 > 0);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.scroll_offsets().0, 0);
    }

    #[test]
    fn scroll_clamps_to_content_height() {
        let mut app = app_with(1);
        app.viewport = ViewportInfo {
            content_lines: 30,
            height: 10,
        };
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('G'));
        assert_eq!(
            app.scroll_offsets().0,
            20,
            "bottom = content minus viewport"
        );
    }

    #[test]
    fn sections_render_and_missing_tab_content_is_hint_not_error() {
        let mut app = app_with(1);
        app.ensure_pane();
        assert!(matches!(app.pane(), Some(Pane::Section(_))));

        // Diagrams dir is empty after scaffold → Empty hint, not an error.
        press(&mut app, KeyCode::Char('6'));
        app.ensure_pane();
        assert!(matches!(app.pane(), Some(Pane::Empty(_))));
    }

    #[test]
    fn slash_filter_narrows_the_list_and_esc_clears() {
        let mut app = app_with(3); // titles "RCA 0".."RCA 2"
        press(&mut app, KeyCode::Char('/'));
        press(&mut app, KeyCode::Char('2'));
        assert_eq!(app.visible_len(), 1);
        assert_eq!(app.selected_rca().map(|r| r.id.as_str()), Some("rca-2"));

        press(&mut app, KeyCode::Esc);
        assert!(!app.search_active());
        assert!(app.filter().is_empty());
        assert_eq!(app.visible_len(), 3, "esc restores the full list");
    }

    #[test]
    fn typing_q_in_search_mode_filters_instead_of_quitting() {
        let mut app = app_with(2);
        press(&mut app, KeyCode::Char('/'));
        assert_eq!(press(&mut app, KeyCode::Char('q')), Flow::Continue);
        assert_eq!(app.filter(), "q");
        assert_eq!(app.visible_len(), 0, "no workspace matches `q`");
        // Backspace repairs the query.
        press(&mut app, KeyCode::Backspace);
        assert_eq!(app.visible_len(), 2);
    }

    #[test]
    fn enter_keeps_filter_and_esc_in_list_mode_clears_it() {
        let mut app = app_with(3);
        press(&mut app, KeyCode::Char('/'));
        press(&mut app, KeyCode::Char('1'));
        press(&mut app, KeyCode::Enter);
        assert!(!app.search_active());
        assert_eq!(app.visible_len(), 1, "filter survives enter");

        press(&mut app, KeyCode::Esc); // list-mode esc clears a kept filter
        assert_eq!(app.visible_len(), 3);
    }

    #[test]
    fn e_exports_the_selected_workspace_and_reports_a_short_relative_path() {
        let mut app = app_with(1);
        press(&mut app, KeyCode::Char('e'));
        let status = app.status_line().expect("status set").to_owned();
        // Relative path only — a full path was too wide for the status bar.
        assert_eq!(status, "exported to exports/rca-0.md");
        let path = app.store.root().join("exports/rca-0.md");
        let doc = std::fs::read_to_string(&path).expect("export file exists");
        assert!(doc.starts_with("---\n"), "frontmatter present");
        assert!(doc.contains("title: \"RCA 0\""));
    }

    #[test]
    fn copy_on_empty_store_is_a_no_op() {
        let mut app = app_with(0);
        press(&mut app, KeyCode::Char('c'));
        press(&mut app, KeyCode::Char('C'));
        assert!(app.selected_rca().is_none());
    }

    #[test]
    fn human_size_formats_reasonably() {
        assert_eq!(human_size(842), "842 B");
        assert_eq!(human_size(1300), "1.3 KB");
        assert_eq!(human_size(2 * 1024 * 1024), "2.0 MB");
    }
}
