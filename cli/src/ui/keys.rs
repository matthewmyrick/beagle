//! Keypress → state transition. Every branch here is unit-testable without
//! a terminal: `handle_key` mutates [`App`] and returns whether to keep
//! running.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, Focus, Pane, Tab};

/// What the key loop should do next. `Edit` carries the file to open —
/// state transitions stay pure; the event loop owns the terminal and does
/// the suspend/spawn/restore dance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Flow {
    Continue,
    Quit,
    /// Suspend the TUI and open this file in the user's editor.
    Edit(std::path::PathBuf),
}

impl App {
    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> Flow {
        self.status = None;
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Flow::Quit;
        }
        if self.route_modal_key(key) {
            return Flow::Continue;
        }
        match key.code {
            // Shift-Q only: a plain `q` next to tab/scroll keys quit the app
            // by accident too easily. Ctrl-C is handled above.
            KeyCode::Char('Q') => return Flow::Quit,
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('T') => self.open_toolbox(),
            KeyCode::Char('o') => self.open_links(),
            KeyCode::Char('R') => self.open_related(),
            KeyCode::Char('V') => self.verify_final_review(),
            KeyCode::Char('S') => self.open_settings(),
            // `/` always searches keywords across the selected incident;
            // `f` filters the incident list; `F` follows (tail -f). Each
            // also moves the focus to where the action is.
            KeyCode::Char('/') => self.start_content_search(),
            KeyCode::Char('\\') => self.open_finder(),
            KeyCode::Char('f') => {
                self.filter_input = super::FilterInput::Facets;
                self.focus = Focus::List;
            }
            // Shift-D, list focus only: deleting is a list-management
            // action, and the guard keeps a stray D while reading content
            // from opening a destructive prompt.
            KeyCode::Char('D') if self.focus == Focus::List => self.open_confirm_delete(),
            KeyCode::Char('b') => self.focus = Focus::List,
            KeyCode::Char('s') => self.toggle_sidebar(),
            KeyCode::Char('a') => self.toggle_archived(),
            KeyCode::Char('E') => {
                if let Some(flow) = self.edit_request() {
                    return flow;
                }
            }
            KeyCode::Char('c') => self.copy_current_tab(),
            KeyCode::Char('C') => self.copy_workspace(),
            KeyCode::Char('e') => self.export_current(),
            KeyCode::Char('r') => {
                let _ = self.reload();
                self.status = Some("reloaded".to_owned());
            }
            KeyCode::Char('F') => {
                self.follow = !self.follow;
                if self.follow {
                    self.scroll = u16::MAX; // jump to the tail immediately
                    self.status =
                        Some("following — reloads stick to the bottom · esc exits".to_owned());
                } else {
                    self.status = Some("follow off".to_owned());
                }
            }
            KeyCode::Tab | KeyCode::Char(']') | KeyCode::Right => {
                self.switch_tab(self.tab.next());
            }
            KeyCode::BackTab | KeyCode::Char('[') | KeyCode::Left => {
                self.switch_tab(self.tab.prev());
            }
            KeyCode::Char(c @ '1'..='9') => {
                // '1'..='9' maps exactly onto Tab::ALL's nine entries.
                let index = (c as usize).saturating_sub('1' as usize);
                if let Some(tab) = Tab::ALL.get(index) {
                    self.switch_tab(*tab);
                }
            }
            // A committed search owns n/N until esc clears it — including on
            // the Diagrams tab, where p still cycles diagrams.
            KeyCode::Char('n') if self.content_search.is_some() => self.search_step(1),
            KeyCode::Char('N') if self.content_search.is_some() => self.search_step(-1),
            KeyCode::Char('n') if self.tab == Tab::Diagrams => self.cycle_diagram(1),
            KeyCode::Char('p') if self.tab == Tab::Diagrams => self.cycle_diagram(-1),
            _ => match self.focus {
                Focus::List => self.handle_list_key(key.code),
                Focus::Content => self.handle_content_key(key.code),
            },
        }
        // Invariant: the sidebar is never collapsed while the list has
        // focus — `b`, esc, `f`, and every other back-to-list path would
        // otherwise leave the selection cursor invisible.
        if self.focus == Focus::List {
            self.sidebar_collapsed = false;
        }
        Flow::Continue
    }

    /// Routes the key to whichever modal owns it, returning whether one
    /// did. The delete confirmation is checked first: while it is open,
    /// nothing else may interpret a key — least of all `y`.
    fn route_modal_key(&mut self, key: KeyEvent) -> bool {
        if self.confirm_delete.is_some() {
            self.handle_confirm_delete_key(key.code);
        } else if self.finder.is_some() {
            self.handle_finder_key(key.code, key.modifiers.contains(KeyModifiers::CONTROL));
        } else if self.filter_input != super::FilterInput::Off {
            self.handle_search_key(key.code);
        } else if self.content_search.as_ref().is_some_and(|s| s.typing) {
            self.handle_content_search_key(key.code);
        } else if self.links.is_some() {
            self.handle_links_key(key.code);
        } else if self.related.is_some() {
            self.handle_related_key(key.code);
        } else if self.settings.is_some() {
            self.handle_settings_key(key.code);
        } else if self.toolbox.is_some() {
            self.handle_toolbox_key(key.code);
        } else if self.show_help {
            self.show_help = false;
        } else {
            return false;
        }
        true
    }

    /// `E`: the file behind the current tab, to open in the user's editor.
    /// Section tabs edit their markdown file (created by the editor if the
    /// section doesn't exist yet); the Diagrams tab edits the current
    /// diagram. `None` (with a status message) when there is nothing to
    /// edit.
    fn edit_request(&mut self) -> Option<Flow> {
        let Some(rca) = self.selected_rca() else {
            self.status = Some("no incident selected — nothing to edit".to_owned());
            return None;
        };
        let id = rca.id.clone();
        if let Some(kind) = self.tab.section() {
            let path = self.store.workspace_dir(&id).join(kind.file_name());
            return Some(Flow::Edit(path));
        }
        // Diagrams tab: edit the diagram currently on screen.
        match self.store.list_diagrams(&id) {
            Ok(diagrams) if !diagrams.is_empty() => {
                let index = self.diagram_index.min(diagrams.len() - 1);
                Some(Flow::Edit(diagrams[index].path.clone()))
            }
            _ => {
                self.status = Some("no diagram to edit — add one under diagrams/ first".to_owned());
                None
            }
        }
    }

    /// `a`: toggle archived incidents into or out of the list, keeping the
    /// current selection when it survives the toggle.
    fn toggle_archived(&mut self) {
        self.show_archived = !self.show_archived;
        let keep = self.selected_rca().map(|rca| rca.id.clone());
        self.recompute_visible(keep);
        let archived = self.archived_count();
        self.status = Some(if self.show_archived {
            format!("showing {archived} archived (dimmed) — a hides them")
        } else {
            "archived hidden — a shows them".to_owned()
        });
    }

    /// `s`: collapse the sidebar for a full-width content pane, or bring
    /// it back. Collapsing moves focus to the content — a hidden list
    /// cannot hold the cursor.
    fn toggle_sidebar(&mut self) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        if self.sidebar_collapsed {
            self.focus = Focus::Content;
            self.status = Some("sidebar hidden — s or b brings it back".to_owned());
        } else {
            self.status = Some("sidebar shown".to_owned());
        }
    }

    /// Keystrokes in filter mode. Facets first: single keys toggle status
    /// (`i`/`r`/`v`/`f`) and severity (`c`/`h`/`m`/`l`) filters instantly,
    /// stacking across dimensions; `/` switches to free-text typing. Arrows
    /// (and `j`/`k` while not typing) still move the selection so filtering
    /// and picking interleave naturally.
    fn handle_search_key(&mut self, code: KeyCode) {
        use crate::model::{Severity, Status};

        if self.filter_input == super::FilterInput::Typing {
            match code {
                // Esc peels: stop typing first (query kept), facets next.
                KeyCode::Esc => self.filter_input = super::FilterInput::Facets,
                KeyCode::Enter => self.filter_input = super::FilterInput::Off,
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
            return;
        }
        match code {
            KeyCode::Esc => {
                self.clear_filter();
                self.filter_input = super::FilterInput::Off;
                self.reset_scroll();
            }
            KeyCode::Enter => self.filter_input = super::FilterInput::Off,
            KeyCode::Char('/') => self.filter_input = super::FilterInput::Typing,
            KeyCode::Char('i') => self.toggle_status_facet(Status::Investigating),
            KeyCode::Char('r') => self.toggle_status_facet(Status::Review),
            KeyCode::Char('a') => self.toggle_status_facet(Status::Agent),
            KeyCode::Char('v') => self.toggle_status_facet(Status::FinalReview),
            KeyCode::Char('f') => self.toggle_status_facet(Status::Finished),
            KeyCode::Char('c') => self.toggle_severity_facet(Severity::Critical),
            KeyCode::Char('h') => self.toggle_severity_facet(Severity::High),
            KeyCode::Char('m') => self.toggle_severity_facet(Severity::Medium),
            KeyCode::Char('l') => self.toggle_severity_facet(Severity::Low),
            KeyCode::Backspace => {
                self.filter.pop();
                self.recompute_visible(None);
            }
            KeyCode::Char('j') | KeyCode::Down => self.select(self.selected.saturating_add(1)),
            KeyCode::Char('k') | KeyCode::Up => self.select(self.selected.saturating_sub(1)),
            _ => {}
        }
    }

    fn toggle_status_facet(&mut self, status: crate::model::Status) {
        if !self.facet_statuses.remove(&status) {
            self.facet_statuses.insert(status);
        }
        self.recompute_visible(None);
    }

    fn toggle_severity_facet(&mut self, severity: crate::model::Severity) {
        if !self.facet_severities.remove(&severity) {
            self.facet_severities.insert(severity);
        }
        self.recompute_visible(None);
    }

    fn handle_list_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('j') | KeyCode::Down => self.select(self.selected.saturating_add(1)),
            KeyCode::Char('k') | KeyCode::Up => self.select(self.selected.saturating_sub(1)),
            KeyCode::Char('g') | KeyCode::Home => self.select(0),
            KeyCode::Char('G') | KeyCode::End => self.select(usize::MAX),
            KeyCode::Esc if self.has_active_filter() => self.clear_filter(),
            KeyCode::Esc if self.follow => {
                self.follow = false;
                self.status = Some("follow off".to_owned());
            }
            KeyCode::Enter | KeyCode::Char('l') if !self.visible.is_empty() => {
                // Opening an incident consumes the filter (text and facets
                // alike): the pick has been made, so bring the full list
                // back with the chosen incident still selected.
                if self.has_active_filter() {
                    self.clear_filter();
                }
                self.focus = Focus::Content;
            }
            _ => {}
        }
    }

    fn handle_content_key(&mut self, code: KeyCode) {
        let page = self.viewport.height.saturating_sub(1).max(1);
        match code {
            // Esc peels one layer at a time: search highlights, then follow
            // mode, then back to the list.
            KeyCode::Esc if self.content_search.is_some() => self.clear_content_search(),
            KeyCode::Esc if self.follow => {
                self.follow = false;
                self.status = Some("follow off".to_owned());
            }
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

    pub(super) fn select(&mut self, index: usize) {
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

    pub(super) fn switch_tab(&mut self, tab: Tab) {
        // Tabs belong to a selected incident. With nothing visible the
        // welcome screen has no tab bar, so a silent state change would
        // read as a broken keybinding — say why instead.
        if self.visible.is_empty() {
            self.status = Some(if self.rcas.is_empty() {
                "no incidents yet — tabs appear once a workspace exists \
                 (beagle new <slug> --title \"...\")"
                    .to_owned()
            } else {
                "no incident matches the filter — esc clears it".to_owned()
            });
            return;
        }
        if tab != self.tab {
            self.tab = tab;
            self.reset_scroll();
        }
        self.focus = Focus::Content;
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
    pub(super) fn scroll_to(&mut self, target: u16) {
        let max = self
            .viewport
            .content_lines
            .saturating_sub(self.viewport.height);
        self.scroll = target.min(max);
    }

    pub(crate) fn reset_scroll(&mut self) {
        self.scroll = 0;
        self.hscroll = 0;
    }
}

#[cfg(test)]
#[path = "tests/keys.rs"]
mod tests;
