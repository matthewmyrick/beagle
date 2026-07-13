//! Keypress → state transition. Every branch here is unit-testable without
//! a terminal: `handle_key` mutates [`App`] and returns whether to keep
//! running.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, Focus, Pane, Tab};

/// Whether the key loop should keep running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flow {
    Continue,
    Quit,
}

impl App {
    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> Flow {
        self.status = None;
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Flow::Quit;
        }
        if self.search_active {
            self.handle_search_key(key.code);
            return Flow::Continue;
        }
        if self.content_search.as_ref().is_some_and(|s| s.typing) {
            self.handle_content_search_key(key.code);
            return Flow::Continue;
        }
        if self.links.is_some() {
            self.handle_links_key(key.code);
            return Flow::Continue;
        }
        if self.related.is_some() {
            self.handle_related_key(key.code);
            return Flow::Continue;
        }
        if self.toolbox.is_some() {
            self.handle_toolbox_key(key.code);
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
            KeyCode::Char('T') => self.open_toolbox(),
            KeyCode::Char('o') => self.open_links(),
            KeyCode::Char('R') => self.open_related(),
            KeyCode::Char('V') => self.verify_final_review(),
            // `/` always searches keywords across the selected incident;
            // `f` filters the incident list; `F` follows (tail -f). Each
            // also moves the focus to where the action is.
            KeyCode::Char('/') => self.start_content_search(),
            KeyCode::Char('f') => {
                self.search_active = true;
                self.focus = Focus::List;
            }
            KeyCode::Char('b') => self.focus = Focus::List,
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
            KeyCode::Esc if self.follow => {
                self.follow = false;
                self.status = Some("follow off".to_owned());
            }
            KeyCode::Enter | KeyCode::Char('l') if !self.visible.is_empty() => {
                // Opening an incident consumes the filter: the pick has
                // been made, so bring the full list back with the chosen
                // incident still selected.
                if !self.filter.is_empty() {
                    let keep = self.selected_rca().map(|r| r.id.clone());
                    self.filter.clear();
                    self.recompute_visible(keep);
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
    fn scroll_to(&mut self, target: u16) {
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
