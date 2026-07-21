//! Mouse event → state transition, mirroring `keys` — the wheel scrolls
//! whatever is under the cursor, left-click selects/focuses/switches.
//! Keys remain the primary interface; every mouse action is a shortcut for
//! an existing key path, so no state is reachable only by mouse.
//!
//! Hit-testing uses [`super::MouseMap`], the geometry the draw pass fed
//! back last frame. A one-frame stale rect can at worst misroute a single
//! event by a row — acceptable for pointing, never for correctness.

use crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Position;

use super::{App, Focus};

/// How many lines one wheel notch scrolls in the content pane.
const WHEEL_LINES: u16 = 3;

impl App {
    pub(crate) fn handle_mouse(&mut self, event: MouseEvent) {
        let position = Position::new(event.column, event.row);
        match event.kind {
            MouseEventKind::ScrollUp => self.wheel(position, -1),
            MouseEventKind::ScrollDown => self.wheel(position, 1),
            MouseEventKind::Down(MouseButton::Left) => self.click(position),
            _ => {}
        }
        // Same invariant as the key path: a hidden sidebar never holds the
        // cursor, so any mouse action that lands focus on the list also
        // brings the list back.
        if self.focus == Focus::List {
            self.sidebar_collapsed = false;
        }
    }

    /// Wheel: an open overlay owns the wheel wherever the cursor is (they
    /// render centered over everything); otherwise the pane under the
    /// cursor scrolls.
    fn wheel(&mut self, position: Position, direction: i16) {
        let arrow = if direction < 0 {
            KeyCode::Up
        } else {
            KeyCode::Down
        };
        if self.confirm_delete.is_some() {
            return; // y/n only — the wheel must not touch the list beneath
        }
        if self.status_picker.is_some() {
            self.handle_status_picker_key(arrow);
            return;
        }
        if self.tags_editor.is_some() {
            self.handle_tags_editor_key(arrow);
            return;
        }
        if self.show_errors {
            self.handle_errors_key(arrow);
            return;
        }
        if self.finder.is_some() {
            self.handle_finder_key(arrow, false);
            return;
        }
        if self.toolbox.is_some() {
            self.handle_toolbox_key(arrow);
            return;
        }
        if self.links.is_some() {
            self.handle_links_key(arrow);
            return;
        }
        if self.related.is_some() {
            self.handle_related_key(arrow);
            return;
        }
        if self.settings.is_some() {
            self.handle_settings_key(arrow);
            return;
        }
        if self.show_help {
            return; // static sheet; any key or click closes it
        }
        if self.mouse.sidebar.contains(position) {
            let selected = self.selected;
            self.select(if direction < 0 {
                selected.saturating_sub(1)
            } else {
                selected.saturating_add(1)
            });
        } else if self.mouse.content.contains(position) {
            let scroll = self.scroll;
            self.scroll_to(if direction < 0 {
                scroll.saturating_sub(WHEEL_LINES)
            } else {
                scroll.saturating_add(WHEEL_LINES)
            });
        }
    }

    /// Left click: sidebar row selects, tab label switches, content pane
    /// focuses. Clicks while an overlay is open only close the help sheet —
    /// pickers keep their keyboard flow.
    fn click(&mut self, position: Position) {
        if self.show_help {
            self.close_help();
            return;
        }
        if self.finder.is_some()
            || self.toolbox.is_some()
            || self.links.is_some()
            || self.related.is_some()
            || self.settings.is_some()
            || self.confirm_delete.is_some()
            || self.status_picker.is_some()
            || self.tags_editor.is_some()
        {
            return;
        }
        if self.show_errors {
            self.show_errors = false; // any click dismisses the overlay
            return;
        }
        if let Some(row) = self.sidebar_row_at(position) {
            self.select(row);
            self.focus = Focus::List;
            return;
        }
        // A click in the sidebar below the last incident row — the broken
        // region — opens the errors overlay, matching the `!` key and the
        // "! or click to view" hint shown there.
        if !self.broken().is_empty() && self.mouse.sidebar.contains(position) {
            self.open_errors();
            return;
        }
        if let Some((tab, _)) = self
            .mouse
            .tabs
            .iter()
            .find(|(_, rect)| rect.contains(position))
        {
            self.switch_tab(*tab);
            return;
        }
        if self.mouse.content.contains(position) && !self.visible.is_empty() {
            self.focus = Focus::Content;
        }
    }

    /// Maps a click inside the sidebar onto a visible row index. Rows are
    /// two lines tall; `sidebar_offset` accounts for list scrolling.
    fn sidebar_row_at(&self, position: Position) -> Option<usize> {
        let area = self.mouse.sidebar;
        if !area.contains(position) || area.width < 2 || area.height < 2 {
            return None;
        }
        // Inside the block's border: first content line is area.y + 1.
        let inner_y = position.y.checked_sub(area.y.checked_add(1)?)?;
        if inner_y >= area.height.saturating_sub(2) {
            return None; // bottom border
        }
        let row = self
            .mouse
            .sidebar_offset
            .saturating_add(usize::from(inner_y) / 2);
        (row < self.visible.len()).then_some(row)
    }
}

#[cfg(test)]
#[path = "tests/mouse.rs"]
mod tests;
