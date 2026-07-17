//! Shared visual vocabulary: severity badges, status glyphs, PR colors,
//! bordered blocks, and small layout helpers.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders};

use crate::model::{Severity, Status};

/// Width of the incident list on the left.
pub(super) const SIDEBAR_WIDTH: u16 = 34;

/// Spinner frames for [`Status::Investigating`], advanced by the app tick.
/// Braille dots: distinct at 4 fps and exactly one cell wide, so the sidebar
/// and header never shift as it animates.
const INVESTIGATING_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub(super) fn severity_badge(severity: Severity) -> (&'static str, Style) {
    let bold = Modifier::BOLD;
    match severity {
        Severity::Critical => (
            "CRIT",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Red)
                .add_modifier(bold),
        ),
        Severity::High => (
            "HIGH",
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightRed)
                .add_modifier(bold),
        ),
        Severity::Medium => (
            "MED ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(bold),
        ),
        Severity::Low => (
            "LOW ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(bold),
        ),
        Severity::Info => (
            "INFO",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Gray)
                .add_modifier(bold),
        ),
    }
}

/// The one-cell status marker. `investigating` animates — a live spinner is
/// the "we're on it" signal — driven by `tick`; every other status is a
/// fixed glyph, so `tick` is ignored for them.
pub(super) fn status_symbol(status: Status, tick: usize) -> (&'static str, Style) {
    match status {
        Status::Investigating => (
            INVESTIGATING_FRAMES[tick % INVESTIGATING_FRAMES.len()],
            Style::default().fg(Color::LightRed),
        ),
        Status::Review => ("◐", Style::default().fg(Color::Yellow)),
        Status::Agent => ("⚙", Style::default().fg(Color::Magenta)),
        Status::FinalReview => ("◒", Style::default().fg(Color::LightBlue)),
        Status::Finished => ("✔", Style::default().fg(Color::Green)),
    }
}

/// Display color for a PR state: merged = done, open = attention,
/// draft = quiet, closed-unmerged = warning.
pub(super) fn pr_color(state: crate::prs::PrState) -> Color {
    use crate::prs::PrState;
    match state {
        PrState::Merged => Color::Green,
        PrState::Open => Color::Yellow,
        PrState::Draft => Color::Gray,
        PrState::Closed => Color::LightRed,
    }
}

pub(super) fn pane_block(title: String, focused: bool) -> Block<'static> {
    let border = if focused {
        Color::Yellow
    } else {
        Color::DarkGray
    };
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border))
}

pub(super) fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_owned()
    } else {
        let cut: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

pub(super) fn vertical<const N: usize>(area: Rect, constraints: &[Constraint; N]) -> [Rect; N] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .areas(area)
}

pub(super) fn horizontal<const N: usize>(area: Rect, constraints: &[Constraint; N]) -> [Rect; N] {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .areas(area)
}

pub(super) fn inset(area: Rect, left: u16) -> Rect {
    Rect {
        x: area.x.saturating_add(left),
        width: area.width.saturating_sub(left),
        ..area
    }
}

pub(super) fn center(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

#[cfg(test)]
#[path = "tests/style.rs"]
mod tests;
