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

/// The `S` settings overlay: config fields with their current values, a
/// toggle/edit affordance per row, and the file path in the title. Every
/// change is written straight to the config file.
pub(super) fn draw_settings(frame: &mut Frame, app: &App, area: Rect) {
    use crate::ui::settings::Field;

    let Some(overlay) = app.settings() else {
        return;
    };
    let width = area.width.saturating_sub(8).clamp(44, 90);
    let height = u16::try_from(Field::ALL.len())
        .unwrap_or(u16::MAX)
        .saturating_add(4)
        .min(area.height.saturating_sub(2));
    let rect = center(area, width, height);

    let items: Vec<ListItem<'_>> = Field::ALL
        .iter()
        .map(|field| {
            let editing_here =
                overlay.editing.is_some() && Field::ALL.get(overlay.selected) == Some(field);
            // Set values pop green; unset ones keep the normal foreground
            // (dark gray was unreadable) — the parentheses already say
            // "this is a default".
            let value_style = if overlay.is_set(*field) || editing_here {
                Style::default().fg(Color::LightGreen)
            } else {
                Style::default()
            };
            let value = if editing_here {
                format!("{}▌", overlay.editing.clone().unwrap_or_default())
            } else {
                overlay.value_of(*field)
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" {:<8}", field.key()),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(format!("{value:<28} "), value_style),
                Span::styled(
                    field.describe().to_owned(),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let block = Block::default()
        .title(format!(" Settings — {} ", overlay.path.display()))
        .title_alignment(Alignment::Center)
        .title_bottom(
            Line::from(if overlay.editing.is_some() {
                " type value · enter save · esc cancel "
            } else {
                " enter/space edit or toggle · j/k move · esc close "
            })
            .centered(),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::Rgb(40, 44, 60))
            .add_modifier(Modifier::BOLD),
    );
    let mut state = ListState::default();
    state.select(Some(overlay.selected));
    frame.render_widget(Clear, rect);
    frame.render_stateful_widget(list, rect, &mut state);

    if let Some(note) = &overlay.note {
        let note_rect = Rect {
            y: rect.y + rect.height.saturating_sub(2),
            height: 1,
            x: rect.x + 2,
            width: rect.width.saturating_sub(4),
        };
        frame.render_widget(
            Paragraph::new(Span::styled(
                note.clone(),
                Style::default().fg(Color::LightGreen),
            )),
            note_rect,
        );
    }
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

/// The `\` global finder: prompt on top, ranked results under it, matched
/// characters highlighted. Centered, and sized to its content: a slim
/// bar while the query is empty, one row per match as results arrive,
/// capped well short of the full screen — the incident stays visible
/// behind it.
pub(super) fn draw_finder(frame: &mut Frame, app: &App, area: Rect) {
    let Some(finder) = app.finder() else { return };
    let width = area.width.saturating_sub(6).clamp(30, 110);
    let max_height = area.height.saturating_sub(4).min(30).max(3);
    let height = u16::try_from(finder.matches.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2) // the borders carry the title and the prompt
        .clamp(3, max_height);
    let rect = center(area, width, height);
    let inner_width = usize::from(width.saturating_sub(2));

    let highlight = Style::default()
        .fg(Color::Rgb(214, 160, 30))
        .add_modifier(Modifier::BOLD);
    let items: Vec<ListItem<'_>> = finder
        .matches
        .iter()
        .enumerate()
        .filter_map(|(index, m)| {
            let entry = finder.entry(index)?;
            // Context first, dimmed; the line text carries the highlights.
            let context = format!(" {} · {} · ", truncate(&entry.title, 24), entry.tab.title());
            let room = inner_width.saturating_sub(context.chars().count() + 1);
            let mut spans = vec![Span::styled(context, Style::default().fg(Color::DarkGray))];
            let mut segment = String::new();
            let mut segment_matched = None;
            for (offset, c) in entry.text.chars().take(room).enumerate() {
                let matched = m.positions.contains(&offset);
                if segment_matched.is_some_and(|was| was != matched) && !segment.is_empty() {
                    let style = if matched { Style::default() } else { highlight };
                    spans.push(Span::styled(std::mem::take(&mut segment), style));
                }
                segment_matched = Some(matched);
                segment.push(c);
            }
            if !segment.is_empty() {
                let style = if segment_matched == Some(true) {
                    highlight
                } else {
                    Style::default()
                };
                spans.push(Span::styled(segment, style));
            }
            Some(ListItem::new(Line::from(spans)))
        })
        .collect();

    let title = if finder.query.is_empty() {
        " find everywhere — type to search · esc closes ".to_owned()
    } else {
        format!(
            " find everywhere ({} matches) — ↑/↓ move · enter jump · esc ",
            finder.matches.len()
        )
    };
    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .title_bottom(Line::from(format!(" \\ {}▌ ", finder.query)).left_aligned())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::Rgb(40, 44, 60))
            .add_modifier(Modifier::BOLD),
    );
    let mut state = ListState::default();
    if !finder.matches.is_empty() {
        state.select(Some(finder.selected));
    }
    frame.render_widget(Clear, rect);
    frame.render_stateful_widget(list, rect, &mut state);
}

pub(super) fn draw_help(frame: &mut Frame, area: Rect) {
    let rows = [
        ("j / k, ↓ / ↑", "select incident or scroll content"),
        ("enter / l", "focus the content pane"),
        ("b / esc", "back to the incident list"),
        ("tab / [ / ], ← / →", "cycle tabs"),
        ("1–8", "jump to a tab"),
        ("f", "filter list: i/r/v/f status · c/h/m/l sev · / text"),
        ("F", "follow: reloads stick to the bottom (tail -f)"),
        ("s", "hide / show the sidebar (full-width content)"),
        ("a", "show / hide archived incidents (dimmed)"),
        ("/", "search the incident: every tab, live highlight"),
        ("n / N", "next / previous search match"),
        (
            "\\",
            "find everywhere: fuzzy across all incidents; enter jumps",
        ),
        ("c / C", "copy this tab / whole RCA to clipboard"),
        (
            "e",
            "export RCA to exports/<id>.md (frontmatter + all tabs)",
        ),
        ("E", "open this tab's file in your editor"),
        ("T", "toolbox: toolbox.md + systems/ context"),
        ("o", "links: open attached PRs / URLs on this tab"),
        ("R", "related incidents (shared systems/tags); enter jumps"),
        ("V", "sign off final-review as verified \u{2192} finished"),
        ("S", "settings: view + edit the config file"),
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

    // Sized to the actual row count (plus borders) so the last rows are
    // never silently clipped as keys are added.
    let width = 62.min(area.width.saturating_sub(4));
    let height = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .min(area.height.saturating_sub(2));
    let popup = center(area, width, height);

    let block = Block::default()
        .title(" keys (any key to close) ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(Clear, popup);
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}
