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
    let max_height = area.height.saturating_sub(4).clamp(3, 30);
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

/// The `t` status picker: the five lifecycle stages with their glyphs and
/// a one-line hint each, the incident's current stage marked. Enter
/// applies, esc closes.
pub(super) fn draw_status_picker(frame: &mut Frame, app: &App, area: Rect) {
    use crate::model::Status;

    let Some(picker) = app.status_picker() else {
        return;
    };
    let width = area.width.saturating_sub(6).clamp(40, 74);
    let height = u16::try_from(Status::ALL.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .min(area.height.saturating_sub(2));
    let rect = center(area, width, height);

    let items: Vec<ListItem<'_>> = Status::ALL
        .iter()
        .map(|&status| {
            let (symbol, symbol_style) = super::style::status_symbol(status, 0);
            let hint = match status {
                Status::Investigating => "actively debugging",
                Status::Review => "write-up done; fix PR out for review",
                Status::Agent => "handed off to an automated agent",
                Status::FinalReview => "fix merged — work the checklist, V signs off",
                Status::Finished => "verified and closed",
            };
            let marker = if status == picker.current {
                " (current)"
            } else {
                ""
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {symbol} "), symbol_style),
                Span::raw(format!("{:<13}", status.as_str())),
                Span::styled(
                    format!("{hint}{marker}"),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let block = Block::default()
        .title(format!(
            " status — {} ",
            truncate(&picker.title, usize::from(width).saturating_sub(12))
        ))
        .title_alignment(Alignment::Center)
        .title_bottom(Line::from(" enter apply · j/k move · esc cancel ").centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::Rgb(40, 44, 60))
            .add_modifier(Modifier::BOLD),
    );
    let mut state = ListState::default();
    state.select(Some(picker.selected));
    frame.render_widget(Clear, rect);
    frame.render_stateful_widget(list, rect, &mut state);
}

/// The `#` tags editor: current tags as rows, a trailing `+ add tag` row,
/// and an inline input while typing. Adds/deletes write straight to the
/// manifest, so what you see is what's on disk.
pub(super) fn draw_tags_editor(frame: &mut Frame, app: &App, area: Rect) {
    let Some(editor) = app.tags_editor() else {
        return;
    };
    let width = area.width.saturating_sub(6).clamp(36, 64);
    let inner = usize::from(width.saturating_sub(6));

    let mut items: Vec<ListItem<'_>> = editor
        .tags
        .iter()
        .map(|tag| {
            let special = tag == crate::model::SKIP_FINAL_REVIEW_TAG;
            let mut spans = vec![Span::raw(format!(" {}", truncate(tag, inner)))];
            if special {
                spans.push(Span::styled(
                    "  (merged PRs → finished)",
                    Style::default().fg(Color::Yellow),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    items.push(ListItem::new(Line::styled(
        match &editor.typing {
            Some(buffer) => format!(" + {buffer}▌"),
            None => " + add tag".to_owned(),
        },
        Style::default().fg(Color::DarkGray),
    )));

    let height = u16::try_from(items.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .min(area.height.saturating_sub(2));
    let rect = center(area, width, height);

    let block = Block::default()
        .title(format!(
            " tags — {} ",
            truncate(&editor.title, usize::from(width).saturating_sub(10))
        ))
        .title_alignment(Alignment::Center)
        .title_bottom(
            Line::from(if editor.typing.is_some() {
                " type tag · enter add · esc back "
            } else {
                " a add · d delete · j/k move · esc close "
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
    state.select(Some(editor.selected));
    frame.render_widget(Clear, rect);
    frame.render_stateful_widget(list, rect, &mut state);
}

/// The `D` delete confirmation: names the incident (title + slug) and
/// waits for an explicit `y` or `n`. Red border — this one is destructive.
pub(super) fn draw_confirm_delete(frame: &mut Frame, app: &App, area: Rect) {
    let Some(confirm) = app.confirm_delete() else {
        return;
    };
    let width = area.width.saturating_sub(6).clamp(30, 72);
    let inner = usize::from(width.saturating_sub(4));

    let lines = vec![
        Line::from(""),
        Line::styled(
            truncate(&confirm.title, inner),
            Style::default().add_modifier(Modifier::BOLD),
        )
        .centered(),
        Line::styled(
            truncate(&confirm.id.to_string(), inner),
            Style::default().fg(Color::DarkGray),
        )
        .centered(),
        Line::from(""),
        Line::styled(
            "permanently deletes the workspace directory and all its files",
            Style::default().fg(Color::Red),
        )
        .centered(),
    ];
    let height = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .min(area.height.saturating_sub(2));
    let rect = center(area, width, height);

    let block = Block::default()
        .title(" delete this incident? ")
        .title_alignment(Alignment::Center)
        .title_bottom(Line::from(" y delete · n / esc cancel ").centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Red));
    frame.render_widget(Clear, rect);
    frame.render_widget(Paragraph::new(lines).block(block), rect);
}

/// The `!` errors overlay: every broken workspace with its full reason,
/// then any load warnings. Scrollable; geometry is fed back for clamping,
/// like the toolbox and content panes.
pub(super) fn draw_errors(frame: &mut Frame, app: &mut App, area: Rect) {
    if !app.errors_visible() {
        return;
    }
    let width = area.width.saturating_sub(6).clamp(30, 96);
    let height = area.height.saturating_sub(2).max(5);
    let popup = center(area, width, height);

    let mut lines: Vec<Line<'static>> = Vec::new();
    if !app.broken().is_empty() {
        lines.push(Line::styled(
            "Unloadable workspaces",
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        ));
        for broken in app.broken() {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled(
                    " ⚠ ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {}", broken.dir_name),
                    Style::default()
                        .fg(Color::LightRed)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::styled(
                broken.reason.clone(),
                Style::default().fg(Color::Red),
            ));
        }
    }
    if !app.warnings().is_empty() {
        if !app.broken().is_empty() {
            lines.push(Line::from(""));
        }
        lines.push(Line::styled(
            "Warnings",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        for warning in app.warnings() {
            lines.push(Line::styled(
                format!("• {}", warning.0),
                Style::default().fg(Color::Yellow),
            ));
        }
    }

    let block = Block::default()
        .title(" load errors — j/k scroll · c copy · !/esc close ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Red));
    let inner = block.inner(popup);

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    let content_lines = u16::try_from(paragraph.line_count(inner.width)).unwrap_or(u16::MAX);
    app.errors_viewport = (content_lines, inner.height);
    let scroll = app
        .errors_scroll()
        .min(content_lines.saturating_sub(inner.height));

    frame.render_widget(Clear, popup);
    frame.render_widget(paragraph.block(block).scroll((scroll, 0)), popup);
}

/// Every keybinding row: `(keys, action)`. Kept as a const so the pane can
/// size itself to the widest row and filter without re-listing.
const HELP_ROWS: &[(&str, &str)] = &[
    ("j / k, ↓ / ↑", "select incident or scroll content"),
    ("enter / l", "focus the content pane"),
    ("b / esc", "back to the incident list"),
    ("tab / [ / ], ← / →", "cycle tabs"),
    ("1–8", "jump to a tab"),
    ("f", "filter list: i/r/a/v/f status · c/h/m/l sev · / text"),
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
        "y",
        "copy / yank the incident id to clipboard (/beagle-review)",
    ),
    (
        "e",
        "export RCA to exports/<id>.md (frontmatter + all tabs)",
    ),
    ("E", "open this tab's file in your editor"),
    ("T", "toolbox: toolbox.md + systems/ context"),
    ("o", "links: open attached PRs / URLs on this tab"),
    ("R", "related incidents (shared systems/tags); enter jumps"),
    ("V", "sign off final-review as verified \u{2192} finished"),
    ("t", "set status: pick the RCA's lifecycle stage"),
    ("#", "edit tags: add / remove, incl. skip-final-review"),
    ("!", "view load errors / warnings (broken workspaces)"),
    ("D", "delete the selected incident (y/n confirm popup)"),
    ("S", "settings: view + edit the config file"),
    ("n / p", "next / previous diagram"),
    ("h / l, ← / →", "pan diagrams horizontally"),
    ("space / pgdn / pgup", "page through content"),
    ("g / G", "top / bottom"),
    ("r", "reload from disk"),
    ("Q / ctrl-c", "quit"),
];

/// Column width reserved for the keys before the action text.
const HELP_KEY_COL: usize = 20;

/// Renders one help row (`  <keys padded><action>`) as styled spans. The
/// keys are yellow; when `query` is filtering, the characters it fuzzy-
/// matched are highlighted (same treatment as the `\` finder) so it's
/// clear why the row matched. Runs of the same style coalesce into spans.
fn help_row_spans(
    keys: &str,
    action: &str,
    key_col: usize,
    query: Option<&str>,
) -> Vec<Span<'static>> {
    let text = format!("  {keys:<key_col$}{action}");
    let keys_end = 2 + keys.chars().count();
    let matched: Vec<usize> = match query {
        Some(q) if !q.is_empty() => crate::fuzzy::indices(q, &text)
            .map(|(_, positions)| positions)
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    // 0 = matched (highlight), 1 = keys (yellow), 2 = normal.
    let class = |i: usize| -> u8 {
        if matched.contains(&i) {
            0
        } else if (2..keys_end).contains(&i) {
            1
        } else {
            2
        }
    };
    let style_of = |c: u8| match c {
        0 => Style::default()
            .fg(Color::Rgb(214, 160, 30))
            .add_modifier(Modifier::BOLD),
        1 => Style::default().fg(Color::Yellow),
        _ => Style::default(),
    };

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let mut buf_class = 2u8;
    let mut started = false;
    for (i, ch) in text.chars().enumerate() {
        let c = class(i);
        if started && c != buf_class {
            spans.push(Span::styled(std::mem::take(&mut buf), style_of(buf_class)));
        }
        buf_class = c;
        started = true;
        buf.push(ch);
    }
    if started {
        spans.push(Span::styled(buf, style_of(buf_class)));
    }
    spans
}

pub(super) fn draw_help(frame: &mut Frame, app: &App, area: Rect) {
    let query = app.help_filter();

    // Width fits the widest full row, so nothing is truncated on a normal
    // terminal (clamped to what's on screen).
    let needed = HELP_ROWS
        .iter()
        .map(|(k, a)| 4 + HELP_KEY_COL.max(k.chars().count()) + a.chars().count())
        .max()
        .unwrap_or(40);
    let width = u16::try_from(needed)
        .unwrap_or(u16::MAX)
        .clamp(40, area.width.saturating_sub(4).max(40));
    // Height comes from the FULL row count, so filtering narrows the list
    // without ever resizing the box.
    let height = u16::try_from(HELP_ROWS.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2)
        .min(area.height.saturating_sub(2));
    let popup = center(area, width, height);
    let action_room = usize::from(width).saturating_sub(4 + HELP_KEY_COL);
    let key_col = HELP_KEY_COL;

    let mut items: Vec<ListItem<'_>> = HELP_ROWS
        .iter()
        .filter(|(keys, action)| match query {
            Some(q) if !q.is_empty() => {
                crate::fuzzy::score(q, &format!("{keys} {action}")).is_some()
            }
            _ => true,
        })
        .map(|(keys, action)| {
            ListItem::new(Line::from(help_row_spans(
                keys,
                &truncate(action, action_room),
                key_col,
                query,
            )))
        })
        .collect();
    if items.is_empty() {
        items.push(ListItem::new(Line::styled(
            "  (no matching keys)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let bottom = match query {
        Some(q) => format!(" filter: {q}▌ · esc clears "),
        None => " f filter · any key closes ".to_owned(),
    };
    let block = Block::default()
        .title(" keys ")
        .title_alignment(Alignment::Center)
        .title_bottom(Line::from(bottom).centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(Clear, popup);
    frame.render_widget(List::new(items).block(block), popup);
}
