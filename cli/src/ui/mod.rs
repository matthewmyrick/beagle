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
//! popups), `search` (in-content search), `actions` (copy/export), and
//! `view` (drawing).

mod actions;
mod event_loop;
mod finder;
mod keys;
mod mouse;
mod overlays;
mod pane;
mod search;
mod settings;
mod tabs;
mod view;

pub use pane::Pane;
pub use tabs::{Focus, Tab};

use std::collections::{HashMap, HashSet};

use ratatui::text::Text;

use crate::error::Result;
use crate::model::{RcaId, RcaSummary, SectionKind};
use crate::store::{LoadWarning, Store};

use overlays::{LinksPopup, RelatedPopup};
use pane::PaneKey;
use search::ContentSearch;
use settings::SettingsOverlay;

/// The whole TUI state.
#[allow(clippy::struct_excessive_bools)] // independent UI toggles (help, follow, notify, sidebar, archive), not an encoded state machine
pub struct App {
    store: Store,
    rcas: Vec<RcaSummary>,
    warnings: Vec<LoadWarning>,
    /// Workspace directories that exist but could not load — rendered at
    /// the bottom of the sidebar so they never silently disappear.
    broken: Vec<crate::store::BrokenWorkspace>,
    /// Fuzzy filter over the incident list; empty means "show everything".
    filter: String,
    /// Status facets toggled in filter mode (`i`/`r`/`v`/`f`); empty means
    /// every status passes.
    facet_statuses: HashSet<crate::model::Status>,
    /// Severity facets toggled in filter mode (`c`/`h`/`m`/`l`); empty
    /// means every severity passes.
    facet_severities: HashSet<crate::model::Severity>,
    /// Filter-mode input state: which keys the filter is capturing.
    filter_input: FilterInput,
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
    /// Sidebar collapsed (`s`): the content pane takes the full width.
    /// Never true while the list has focus — anything that returns focus
    /// to the list brings the sidebar back.
    sidebar_collapsed: bool,
    /// Archived incidents shown in the list (`a`), rendered dimmed. Off by
    /// default: the sidebar is about *now*.
    show_archived: bool,
    /// Desktop notifications on new incidents and status changes (config
    /// `notify = true`).
    notify_enabled: bool,
    /// Which lifecycle events fire a notification when `notify_enabled`
    /// (config `[notify_events]`; all events by default).
    notify_events: crate::config::NotifyEvents,
    /// Last-seen modification time per section file, for change detection.
    mtimes: HashMap<(RcaId, SectionKind), std::time::SystemTime>,
    /// Checkbox counts per section, `(checked, total)`, re-scanned only
    /// when the mtime snapshot says the file changed. Sections without
    /// checkboxes have no entry.
    checklists: HashMap<(RcaId, SectionKind), (usize, usize)>,
    /// Sections that changed on disk since the user last viewed them.
    unread: HashSet<(RcaId, SectionKind)>,
    /// Live PR states (url → state), fed by the background `gh` poller.
    /// Empty when `gh` is unavailable — PRs then render as plain links.
    pr_states: HashMap<String, crate::prs::PrState>,
    /// The `o` link popup: attached PRs plus URLs found on the current tab.
    links: Option<LinksPopup>,
    /// The `R` popup: past incidents sharing systems/tags with this one.
    related: Option<RelatedPopup>,
    /// In-content search over the pane (`/` with content focus).
    content_search: Option<ContentSearch>,
    /// The `\` global fuzzy finder; `Some` while its popup is open.
    finder: Option<finder::Finder>,
    /// The `S` settings overlay; `Some` while open.
    settings: Option<SettingsOverlay>,
    /// The `D` delete confirmation popup; `Some` while it awaits y/n.
    confirm_delete: Option<overlays::ConfirmDelete>,
    /// The `t` status picker; `Some` while open.
    status_picker: Option<overlays::StatusPicker>,
    /// Rendered toolbox overlay content; `Some` while the overlay is open.
    toolbox: Option<Text<'static>>,
    /// Vertical scroll of the toolbox overlay.
    toolbox_scroll: u16,
    /// Geometry of the toolbox overlay fed back by the draw pass, for
    /// scroll clamping: (total wrapped lines, visible height).
    pub(crate) toolbox_viewport: (u16, u16),
    /// Set by the draw pass; used to clamp scrolling to real content height.
    pub(crate) viewport: ViewportInfo,
    /// Hit regions fed back by the draw pass, mapping mouse positions onto
    /// the pane / row / tab under the cursor.
    pub(crate) mouse: MouseMap,
}

/// What filter mode is doing with keystrokes: nothing (`Off`), toggling
/// facets (`Facets`, entered with `f`), or typing free text (`Typing`,
/// entered with `/` inside filter mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FilterInput {
    /// Filter mode closed; keys behave normally.
    Off,
    /// Single keys toggle status/severity facets.
    Facets,
    /// Keys type into the fuzzy query.
    Typing,
}

/// Where things were drawn last frame, for mouse hit-testing. Like
/// [`ViewportInfo`], this is geometry feedback from the (otherwise pure)
/// draw pass — mouse events arrive in screen coordinates and have to be
/// mapped back onto panes, sidebar rows, and tab labels.
#[derive(Debug, Clone, Default)]
pub(crate) struct MouseMap {
    /// The sidebar block, borders included. Zero-sized while collapsed.
    pub sidebar: ratatui::layout::Rect,
    /// The content body (below the header and tab bar).
    pub content: ratatui::layout::Rect,
    /// One clickable rect per tab label, in screen coordinates.
    pub tabs: Vec<(Tab, ratatui::layout::Rect)>,
    /// First visible sidebar item (the list scrolls), for row mapping.
    pub sidebar_offset: usize,
}

/// Geometry facts the draw pass feeds back for scroll clamping.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ViewportInfo {
    /// Total wrapped lines of the current content at the current width.
    pub content_lines: u16,
    /// Visible height of the content area.
    pub height: u16,
    /// Visible width of the content area (for wrap-aware search jumps).
    pub width: u16,
}

impl App {
    /// Builds the app and performs the initial workspace listing.
    ///
    /// # Errors
    /// Fails only if the `rcas/` directory itself is unreadable.
    pub fn new(store: Store) -> Result<Self> {
        let listing = store.list_all()?;
        let visible = (0..listing.summaries.len()).collect();
        let mut app = Self {
            store,
            rcas: listing.summaries,
            warnings: listing.warnings,
            broken: listing.broken,
            filter: String::new(),
            facet_statuses: HashSet::new(),
            facet_severities: HashSet::new(),
            filter_input: FilterInput::Off,
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
            sidebar_collapsed: false,
            show_archived: false,
            notify_enabled: false,
            notify_events: crate::config::NotifyEvents::all(),
            mtimes: HashMap::new(),
            checklists: HashMap::new(),
            unread: HashSet::new(),
            pr_states: HashMap::new(),
            links: None,
            related: None,
            content_search: None,
            finder: None,
            settings: None,
            confirm_delete: None,
            status_picker: None,
            toolbox: None,
            toolbox_scroll: 0,
            toolbox_viewport: (0, 0),
            viewport: ViewportInfo::default(),
            mouse: MouseMap::default(),
        };
        // Baseline snapshot: nothing is "unread" at startup.
        app.refresh_mtimes(false);
        Ok(app)
    }

    /// Re-runs the filter over the workspace list, keeping the selection
    /// pinned to `keep` when that workspace still matches. Facets narrow
    /// the candidate set (a workspace must match every non-empty facet
    /// dimension), then the fuzzy query ranks within it.
    fn recompute_visible(&mut self, keep: Option<RcaId>) {
        let mut scored: Vec<(i32, usize)> = self
            .rcas
            .iter()
            .enumerate()
            .filter(|(_, rca)| self.show_archived || !rca.archived)
            .filter(|(_, rca)| {
                self.facet_statuses.is_empty() || self.facet_statuses.contains(&rca.meta.status)
            })
            .filter(|(_, rca)| {
                self.facet_severities.is_empty()
                    || self.facet_severities.contains(&rca.meta.severity)
            })
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

    /// Workspace directories that exist on disk but could not load.
    pub(crate) fn broken(&self) -> &[crate::store::BrokenWorkspace] {
        &self.broken
    }

    /// The newest section-file modification across workspace `id`, straight
    /// from the mtime snapshot kept for unread tracking — zero extra I/O.
    pub(crate) fn last_activity(&self, id: &RcaId) -> Option<std::time::SystemTime> {
        self.mtimes
            .iter()
            .filter(|((mtime_id, _), _)| mtime_id == id)
            .map(|(_, mtime)| *mtime)
            .max()
    }

    /// Whether anything (facets or free text) is narrowing the list.
    pub(crate) fn has_active_filter(&self) -> bool {
        !self.filter.is_empty()
            || !self.facet_statuses.is_empty()
            || !self.facet_severities.is_empty()
    }

    /// Clears every filter dimension and restores the full list, keeping
    /// the current selection when it survives.
    pub(crate) fn clear_filter(&mut self) {
        let keep = self.selected_rca().map(|r| r.id.clone());
        self.filter.clear();
        self.facet_statuses.clear();
        self.facet_severities.clear();
        self.recompute_visible(keep);
    }

    /// The active facets as a compact label for the sidebar title and the
    /// filter prompt: `[high · investigating]`. Empty when no facets are on.
    pub(crate) fn facet_label(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        for severity in crate::model::Severity::ALL {
            if self.facet_severities.contains(&severity) {
                parts.push(severity.as_str());
            }
        }
        for status in crate::model::Status::ALL {
            if self.facet_statuses.contains(&status) {
                parts.push(status.as_str());
            }
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("[{}]", parts.join(" · "))
        }
    }

    pub(crate) fn filter_typing(&self) -> bool {
        self.filter_input == FilterInput::Typing
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
        self.filter_input != FilterInput::Off
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

    pub(crate) fn sidebar_collapsed(&self) -> bool {
        self.sidebar_collapsed
    }

    pub(crate) fn show_archived(&self) -> bool {
        self.show_archived
    }

    /// How many loaded workspaces are archived.
    pub(crate) fn archived_count(&self) -> usize {
        self.rcas.iter().filter(|rca| rca.archived).count()
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

    /// Aggregate checkbox progress across every section of workspace `id`:
    /// `(checked, total)`, or `None` when no section has a checklist.
    pub(crate) fn checklist_progress(&self, id: &RcaId) -> Option<(usize, usize)> {
        let (checked, total) = self
            .checklists
            .iter()
            .filter(|((list_id, _), _)| list_id == id)
            .fold((0, 0), |(c, t), (_, &(checked, total))| {
                (c + checked, t + total)
            });
        (total > 0).then_some((checked, total))
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
pub fn run(store: Store, notify: bool, notify_events: crate::config::NotifyEvents) -> Result<()> {
    let mut terminal = ratatui::init();
    // Mouse capture must be torn down on every exit path — left on, it
    // garbles the user's shell (scrolling emits escape codes). The panic
    // hook wraps the one `ratatui::init` installed, so capture is released
    // before the terminal is restored even when we panic.
    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture);
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
        previous_hook(info);
    }));
    let result = App::new(store).and_then(|mut app| {
        app.notify_enabled = notify;
        app.notify_events = notify_events;
        app.run(&mut terminal)
    });
    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
    ratatui::restore();
    result
}

#[cfg(test)]
#[path = "tests/util.rs"]
pub(crate) mod testutil;
