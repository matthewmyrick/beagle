//! Popup rendering: the `o` link picker, the `R` related-incidents picker,
//! the `T` toolbox overlay, and the `?` help sheet. All render centered
//! over the main layout with a `Clear` underneath.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};
use ratatui::Frame;

use crate::ui::App;

use super::style::{center, pr_color, truncate};

/// The `o` popup: attached PRs (with live state glyphs when known) plus
/// URLs found on the current tab. Enter opens in the browser.
pub(super) fn draw_links(frame: &mut Frame, app: &App, area: Rect) {
    let Some(popup) = app.links() else { return };
    let width = area.width.saturating_sub(6).clamp(20, 100);
    let height = u16::try_from(popup.items.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .min(area.height.saturating_sub(2));
    let rect = center(area, width, height);

    let items: Vec<ListItem<'_>> = popup
        .items
        .iter()
        .map(|url| {
            let mut spans = Vec::new();
            if let Some(state) = app.pr_state(url) {
                spans.push(Span::styled(
                    format!(" {} {} ", state.glyph(), state.label()),
                    Style::default().fg(pr_color(state)),
                ));
            } else {
                spans.push(Span::raw(" "));
            }
            spans.push(Span::raw(truncate(
                url,
                usize::from(width).saturating_sub(14),
            )));
            ListItem::new(Line::from(spans))
        })
        .collect();
    let block = Block::default()
        .title(" links — enter open · j/k move · esc close ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::Rgb(40, 44, 60))
            .add_modifier(Modifier::BOLD),
    );
    let mut state = ListState::default();
    state.select(Some(popup.selected));
    frame.render_widget(Clear, rect);
    frame.render_stateful_widget(list, rect, &mut state);
}

/// The `R` popup: past incidents sharing systems/tags with the selected
/// one, best match first. Enter jumps to the workspace.
pub(super) fn draw_related(frame: &mut Frame, app: &App, area: Rect) {
    let Some(popup) = app.related() else { return };
    let width = area.width.saturating_sub(6).clamp(30, 110);
    let height = u16::try_from(popup.items.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .min(area.height.saturating_sub(2));
    let rect = center(area, width, height);

    let items: Vec<ListItem<'_>> = popup
        .items
        .iter()
        .map(|item| {
            let (badge, badge_style) = super::style::severity_badge(item.severity);
            let (symbol, symbol_style) = super::style::status_symbol(item.status, 0);
            // Leave room for badge/status/shared so the title never pushes
            // the "why it ranked" note off screen.
            let title_room = usize::from(width).saturating_sub(item.shared.len() + 24);
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {badge} "), badge_style),
                Span::styled(format!(" {symbol} "), symbol_style),
                Span::raw(truncate(&item.title, title_room.max(12))),
                Span::styled(
                    format!("  ({})", item.shared),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();
    let block = Block::default()
        .title(" related incidents — enter jump · j/k move · esc close ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::Rgb(40, 44, 60))
            .add_modifier(Modifier::BOLD),
    );
    let mut state = ListState::default();
    state.select(Some(popup.selected));
    frame.render_widget(Clear, rect);
    frame.render_stateful_widget(list, rect, &mut state);
}

/// The toolbox overlay: root `toolbox.md` plus relevant `systems/*.md`,
/// pre-rendered by the app when opened. Scrollable; geometry is fed back
/// for clamping, like the content pane.
pub(super) fn draw_toolbox(frame: &mut Frame, app: &mut App, area: Rect) {
    let Some(text) = app.toolbox() else { return };
    let width = area.width.saturating_sub(6).clamp(20, 96);
    let height = area.height.saturating_sub(2).max(5);
    let popup = center(area, width, height);

    let block = Block::default()
        .title(" Toolbox — j/k scroll · T/esc close ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(popup);

    let paragraph = Paragraph::new(text.clone()).wrap(Wrap { trim: false });
    let content_lines = u16::try_from(paragraph.line_count(inner.width)).unwrap_or(u16::MAX);
    app.toolbox_viewport = (content_lines, inner.height);
    let scroll = app
        .toolbox_scroll()
        .min(content_lines.saturating_sub(inner.height));

    frame.render_widget(Clear, popup);
    frame.render_widget(paragraph.block(block).scroll((scroll, 0)), popup);
}

pub(super) fn draw_help(frame: &mut Frame, area: Rect) {
    let width = 62.min(area.width.saturating_sub(4));
    let height = 24.min(area.height.saturating_sub(2));
    let popup = center(area, width, height);

    let rows = [
        ("j / k, ↓ / ↑", "select incident or scroll content"),
        ("enter / l", "focus the content pane"),
        ("b / esc", "back to the incident list"),
        ("tab / [ / ], ← / →", "cycle tabs"),
        ("1–8", "jump to a tab"),
        ("f", "follow: reloads stick to the bottom (tail -f)"),
        ("/", "filter incidents (list) / search content (pane)"),
        ("n / N", "next / previous search match"),
        ("c / C", "copy this tab / whole RCA to clipboard"),
        (
            "e",
            "export RCA to exports/<id>.md (frontmatter + all tabs)",
        ),
        ("T", "toolbox: toolbox.md + systems/ context"),
        ("o", "links: open attached PRs / URLs on this tab"),
        ("R", "related incidents (shared systems/tags); enter jumps"),
        ("V", "sign off final-review as verified \u{2192} finished"),
        ("n / p", "next / previous diagram"),
        ("h / l, ← / →", "pan diagrams horizontally"),
        ("space / pgdn / pgup", "page through content"),
        ("g / G", "top / bottom"),
        ("r", "reload from disk"),
        ("Q / ctrl-c", "quit"),
    ];
    let mut lines = vec![Line::from("")];
    lines.extend(rows.iter().map(|(keys, action)| {
        Line::from(vec![
            Span::styled(format!("  {keys:<20}"), Style::default().fg(Color::Yellow)),
            Span::raw(*action),
        ])
    }));

    let block = Block::default()
        .title(" keys (any key to close) ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(Clear, popup);
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}
