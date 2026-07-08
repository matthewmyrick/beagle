//! The TUI: application state machine, event loop, and key handling.
//!
//! All state lives in [`App`] and mutates only in response to key presses or
//! filesystem events — drawing (the private `view` module) is a pure function
//! of `App` except for scroll clamping. That split keeps every state
//! transition unit-testable without a terminal.
//!
//! Submodules by responsibility: `tabs` (the tab/focus enums), `event_loop`
//! (the blocking input/watcher loop), `keys` (keypress → state transition),
//! `pane` (content loading and caching), `overlays` (the toolbox and link
//! popups), `actions` (copy/export), and `view` (drawing).

mod actions;
mod event_loop;
mod keys;
mod overlays;
mod pane;
mod tabs;
mod view;

pub use pane::Pane;
pub use tabs::{Focus, Tab};

use std::collections::{HashMap, HashSet};

use ratatui::text::Text;

use crate::error::Result;
use crate::model::{RcaId, RcaSummary, SectionKind};
use crate::store::{LoadWarning, Store};

use overlays::LinksPopup;
use pane::PaneKey;

/// The whole TUI state.
pub struct App {
    store: Store,
    rcas: Vec<RcaSummary>,
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
    /// Follow mode: filesystem reloads keep the current tab scrolled to the
    /// bottom, tail-f style.
    follow: bool,
    /// Last-seen modification time per section file, for change detection.
    mtimes: HashMap<(RcaId, SectionKind), std::time::SystemTime>,
    /// Sections that changed on disk since the user last viewed them.
    unread: HashSet<(RcaId, SectionKind)>,
    /// Live PR states (url → state), fed by the background `gh` poller.
    /// Empty when `gh` is unavailable — PRs then render as plain links.
    pr_states: HashMap<String, crate::prs::PrState>,
    /// The `o` link popup: attached PRs plus URLs found on the current tab.
    links: Option<LinksPopup>,
    /// Rendered toolbox overlay content; `Some` while the overlay is open.
    toolbox: Option<Text<'static>>,
    /// Vertical scroll of the toolbox overlay.
    toolbox_scroll: u16,
    /// Geometry of the toolbox overlay fed back by the draw pass, for
    /// scroll clamping: (total wrapped lines, visible height).
    pub(crate) toolbox_viewport: (u16, u16),
    /// Set by the draw pass; used to clamp scrolling to real content height.
    pub(crate) viewport: ViewportInfo,
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
        let mut app = Self {
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
            follow: false,
            mtimes: HashMap::new(),
            unread: HashSet::new(),
            pr_states: HashMap::new(),
            links: None,
            toolbox: None,
            toolbox_scroll: 0,
            toolbox_viewport: (0, 0),
            viewport: ViewportInfo::default(),
        };
        // Baseline snapshot: nothing is "unread" at startup.
        app.refresh_mtimes(false);
        Ok(app)
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
    pub fn selected_rca(&self) -> Option<&RcaSummary> {
        self.visible
            .get(self.selected)
            .and_then(|&index| self.rcas.get(index))
    }

    pub(crate) fn rcas(&self) -> &[RcaSummary] {
        &self.rcas
    }

    /// Workspaces matching the current filter, best match first.
    pub(crate) fn visible_rcas(&self) -> impl Iterator<Item = &RcaSummary> {
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

    pub(crate) fn follow(&self) -> bool {
        self.follow
    }

    /// Whether `tab` of workspace `id` changed on disk since last viewed.
    pub(crate) fn is_unread(&self, id: &RcaId, tab: Tab) -> bool {
        tab.section()
            .is_some_and(|kind| self.unread.contains(&(id.clone(), kind)))
    }

    /// Whether any section of workspace `id` is unread.
    pub(crate) fn has_unread(&self, id: &RcaId) -> bool {
        self.unread.iter().any(|(unread_id, _)| unread_id == id)
    }

    /// Every unique attached-PR URL across all workspaces, sorted, for the
    /// status poller.
    fn pr_urls(&self) -> Vec<String> {
        let mut urls: Vec<String> = Vec::new();
        for rca in &self.rcas {
            for url in &rca.meta.prs {
                if !urls.contains(url) {
                    urls.push(url.clone());
                }
            }
        }
        urls.sort();
        urls
    }

    /// The polled state of one attached PR, if known.
    pub(crate) fn pr_state(&self, url: &str) -> Option<crate::prs::PrState> {
        self.pr_states.get(url).copied()
    }
}

/// Sets up the terminal, runs the app, and unconditionally restores the
/// terminal — including on error paths.
///
/// # Errors
/// Propagates any [`Error`](crate::Error) from app construction or the
/// event loop.
pub fn run(store: Store) -> Result<()> {
    let mut terminal = ratatui::init();
    let result = App::new(store).and_then(|app| app.run(&mut terminal));
    ratatui::restore();
    result
}

#[cfg(test)]
#[path = "tests/util.rs"]
pub(crate) mod testutil;
