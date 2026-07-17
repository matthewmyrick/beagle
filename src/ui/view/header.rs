//! The workspace header: title and meta lines, the flowing tab bar, and the
//! BEAGLE banner art.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;
use time::macros::format_description;

use crate::model::{RcaSummary, Status};
use crate::ui::Tab;

use super::style::{pr_color, severity_badge, status_symbol};

/// Minimum columns the header (title + meta) keeps for itself before the
/// banner is allowed to claim the top-right corner.
const MIN_HEADER_COLS: u16 = 44;

/// Columns the banner claims when shown: the art plus a one-column gap on
/// each side.
pub(super) const BANNER_COLS: u16 = crate::banner::WIDTH + 2;

/// Whether the workspace area has room for the banner beside the header —
/// wide enough that the header column keeps [`MIN_HEADER_COLS`], tall enough
/// that four rows of art don't crowd out the content.
pub(super) fn banner_fits(area: Rect) -> bool {
    area.width >= BANNER_COLS + MIN_HEADER_COLS && area.height >= 16
}

/// The exact-width rect the banner renders into, pinned to the right edge
/// of `area` with a one-column margin.
fn banner_rect(area: Rect) -> Rect {
    let width = crate::banner::WIDTH.min(area.width);
    Rect {
        x: area.x + area.width.saturating_sub(width + 1),
        width,
        ..area
    }
}

/// Renders the banner art left-aligned inside [`banner_rect`]. Left
/// alignment inside a pinned rect is what keeps the art intact: per-line
/// right-alignment would stagger the ragged line ends.
pub(super) fn draw_banner(frame: &mut Frame, area: Rect) {
    let lines: Vec<Line<'static>> = crate::banner::BANNER
        .lines()
        .map(|l| {
            Line::styled(
                l.to_owned(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), banner_rect(area));
}

/// Lays the eight tab labels out left to right, flowing onto additional
/// lines when the width runs out — never truncating a label. `unread[i]`
/// marks tab `i` with a dot: its file changed on disk since last viewed.
pub(super) fn flow_tabs(selected: Tab, width: u16, unread: &[bool]) -> Vec<Line<'static>> {
    let width = usize::from(width.max(1));
    let mut lines = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;

    for (i, tab) in Tab::ALL.iter().enumerate() {
        let is_unread = unread.get(i).copied().unwrap_or(false);
        let dot = if is_unread { "●" } else { "" };
        let label = format!(" {} {}{dot} ", i + 1, tab.title());
        let label_width = label.chars().count();
        let style = if *tab == selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else if is_unread {
            // Changed since last viewed: brighter than the idle gray so the
            // dot has somewhere to point.
            Style::default().fg(Color::LightYellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        // +1 for the "·" divider that precedes every label but the line's first.
        if !spans.is_empty() && used + 1 + label_width > width {
            lines.push(Line::from(std::mem::take(&mut spans)));
            used = 0;
        }
        if !spans.is_empty() {
            spans.push(Span::styled("·", Style::default().fg(Color::DarkGray)));
            used += 1;
        }
        spans.push(Span::styled(label, style));
        used += label_width;
    }
    if !spans.is_empty() {
        lines.push(Line::from(spans));
    }
    lines
}

/// An investigating agent quieter than this is worth a yellow glance —
/// crashed and hard-at-work look identical otherwise.
pub(super) const QUIET_AFTER: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// The liveness fragment for an investigating incident: what to say about
/// the time since the workspace last changed, and whether it deserves the
/// warning color.
pub(super) fn activity_label(age: std::time::Duration) -> (String, bool) {
    let minutes = i64::try_from(age.as_secs() / 60).unwrap_or(i64::MAX);
    let quiet = age >= QUIET_AFTER;
    let label = if quiet {
        format!("quiet {}", minutes_label(minutes))
    } else {
        format!("active {} ago", minutes_label(minutes))
    };
    (label, quiet)
}

pub(super) fn header_paragraph(
    rca: &RcaSummary,
    tick: usize,
    prs: &[(String, Option<crate::prs::PrState>)],
    activity: Option<std::time::Duration>,
    progress: Option<(usize, usize)>,
) -> Paragraph<'static> {
    let (badge, badge_style) = severity_badge(rca.meta.severity);
    let (symbol, symbol_style) = status_symbol(rca.meta.status, tick);

    let opened = rca
        .meta
        .created
        .format(format_description!(
            "[year]-[month]-[day] [hour]:[minute] UTC"
        ))
        .unwrap_or_else(|_| rca.meta.created.to_string());

    let mut meta_spans = vec![Span::styled(
        format!("{symbol} {}", rca.meta.status),
        symbol_style,
    )];
    if rca.meta.status == Status::Investigating {
        // Ticking "how long has this been open" — the redraw cadence that
        // drives the spinner keeps this current too.
        let elapsed = elapsed_label(rca.meta.created, time::OffsetDateTime::now_utc());
        meta_spans.push(Span::styled(
            format!(" · {elapsed}"),
            Style::default().fg(Color::Gray),
        ));
        // Liveness: a spinner says someone is on it; this says whether
        // they still are.
        if let Some(age) = activity {
            let (label, quiet) = activity_label(age);
            let style = if quiet {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Gray)
            };
            meta_spans.push(Span::styled(format!(" · {label}"), style));
        }
    }
    if let Some((checked, total)) = progress {
        // Checklist progress across the workspace's sections, next to the
        // status — the same count the sidebar shows.
        let style = if checked == total {
            Style::default().fg(Color::LightGreen)
        } else {
            Style::default().fg(Color::Gray)
        };
        meta_spans.push(Span::styled(format!(" · ☑ {checked}/{total}"), style));
    }
    meta_spans.extend([
        Span::styled("  ·  ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" {badge} "), badge_style),
        Span::styled("  ·  ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("opened {opened}"), Style::default().fg(Color::Gray)),
    ]);
    if !rca.meta.systems.is_empty() {
        meta_spans.push(Span::styled("  ·  ", Style::default().fg(Color::DarkGray)));
        meta_spans.push(Span::styled(
            rca.meta.systems.join(", "),
            Style::default().fg(Color::LightBlue),
        ));
    }

    let mut lines = vec![
        Line::styled(
            rca.meta.title.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Line::from(meta_spans),
    ];
    if !prs.is_empty() {
        lines.push(pr_line(prs));
    }
    Paragraph::new(lines).wrap(Wrap { trim: false })
}

/// The attached-PRs header line: `fixes: ✓ #12 merged · ○ #13 open`.
/// PRs without a polled state (no `gh`, or not yet fetched) render as their
/// short label only — degraded, never broken.
fn pr_line(prs: &[(String, Option<crate::prs::PrState>)]) -> Line<'static> {
    let dim = Style::default().fg(Color::DarkGray);
    let mut spans = vec![Span::styled("fixes: ", dim)];
    for (i, (url, state)) in prs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", dim));
        }
        let label = crate::prs::short_label(url);
        match state {
            Some(state) => spans.push(Span::styled(
                format!("{} {label} {}", state.glyph(), state.label()),
                Style::default().fg(pr_color(*state)),
            )),
            None => spans.push(Span::styled(label, Style::default().fg(Color::Gray))),
        }
    }
    Line::from(spans)
}

/// Human elapsed time since `from`: `3m`, `1h 05m`, `2d 4h`.
fn elapsed_label(from: time::OffsetDateTime, now: time::OffsetDateTime) -> String {
    minutes_label((now - from).whole_minutes().max(0))
}

/// A minute count as a short label: `3m`, `1h 05m`, `2d 4h`.
fn minutes_label(minutes: i64) -> String {
    if minutes < 60 {
        format!("{minutes}m")
    } else if minutes < 24 * 60 {
        format!("{}h {:02}m", minutes / 60, minutes % 60)
    } else {
        format!("{}d {}h", minutes / (24 * 60), (minutes % (24 * 60)) / 60)
    }
}

#[cfg(test)]
#[path = "tests/header.rs"]
mod tests;
