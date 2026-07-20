//! Drawing: a pure projection of [`App`] onto the frame.
//!
//! The only mutation allowed here is feeding viewport geometry back into the
//! app (for scroll clamping) — no state transitions, no I/O, and no markdown
//! parsing (content is pre-rendered by the app when it changes, not per
//! frame).
//!
//! Submodules: `header` (title, meta, tab bar, banner), `popups` (links,
//! toolbox, help), and `style` (badges, glyphs, layout helpers).

mod header;
mod popups;
mod style;

use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::model::RcaSummary;

use super::{App, Focus, Pane, Tab};

use header::{banner_fits, draw_banner, flow_tabs, header_paragraph, BANNER_COLS};
use popups::{
    draw_confirm_delete, draw_finder, draw_help, draw_links, draw_related, draw_settings,
    draw_status_picker, draw_tags_editor, draw_toolbox,
};
use style::{
    horizontal, inset, pane_block, severity_badge, status_symbol, truncate, vertical, SIDEBAR_WIDTH,
};

pub(crate) fn draw(frame: &mut Frame, app: &mut App) {
    let [main, status_bar] = vertical(frame.area(), &[Constraint::Min(0), Constraint::Length(1)]);
    let sidebar_width = if app.sidebar_collapsed() {
        0
    } else {
        SIDEBAR_WIDTH
    };
    let [sidebar, content] = horizontal(
        main,
        &[Constraint::Length(sidebar_width), Constraint::Min(0)],
    );

    app.mouse.sidebar = if app.sidebar_collapsed() {
        Rect::default()
    } else {
        sidebar
    };
    if !app.sidebar_collapsed() {
        draw_sidebar(frame, app, sidebar);
    }
    draw_workspace(frame, app, content);
    draw_status_bar(frame, app, status_bar);

    if app.toolbox().is_some() {
        draw_toolbox(frame, app, frame.area());
    }
    draw_links(frame, app, frame.area());
    draw_related(frame, app, frame.area());
    draw_settings(frame, app, frame.area());
    draw_finder(frame, app, frame.area());
    draw_status_picker(frame, app, frame.area());
    draw_tags_editor(frame, app, frame.area());
    draw_confirm_delete(frame, app, frame.area());
    if app.help_visible() {
        draw_help(frame, frame.area());
    }
}

fn draw_sidebar(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus() == Focus::List;
    let title = if app.has_active_filter() {
        use std::fmt::Write as _;
        let mut title = format!(" Incidents ({}/{})", app.visible_len(), app.rcas().len());
        let facets = app.facet_label();
        if !facets.is_empty() {
            let _ = write!(title, " {facets}");
        }
        if !app.filter().is_empty() {
            let _ = write!(title, " /{}", app.filter());
        }
        title.push(' ');
        title
    } else {
        let archived = app.archived_count();
        let active = app.rcas().len() - archived;
        if app.show_archived() && archived > 0 {
            format!(" Incidents ({active} · {archived} archived) ")
        } else {
            format!(" Incidents ({active}) ")
        }
    };
    let block = pane_block(title, focused);

    let tick = app.tick();
    let mut items: Vec<ListItem<'_>> = app
        .visible_rcas()
        .enumerate()
        .map(|(row, rca)| {
            ListItem::new(rca_list_item(
                rca,
                tick,
                app.has_unread(&rca.id),
                row == app.selected_index(),
                app.checklist_progress(&rca.id),
            ))
        })
        .collect();
    // Broken workspaces render at the bottom, unselectable but never
    // hidden: a directory that exists on disk must not silently vanish
    // from the incident list.
    for broken in app.broken() {
        items.push(ListItem::new(broken_list_item(broken)));
    }
    // Selection styling is baked into the items (rca_list_item): a
    // highlight_style here would be patched over every span in the row and
    // wipe out the severity badge's own background — the selected row then
    // looked *less* highlighted than its neighbors.
    let list = List::new(items).block(block);

    let mut state = ListState::default();
    if app.visible_len() > 0 {
        state.select(Some(app.selected_index()));
    }
    frame.render_stateful_widget(list, area, &mut state);
    // Rendering scrolled the list to keep the selection visible; the mouse
    // handler needs the resulting first-visible row to map clicks.
    app.mouse.sidebar_offset = state.offset();
}

/// Background of the selected sidebar row.
const SELECTED_BG: Color = Color::Rgb(40, 44, 60);

/// The two sidebar lines for one workspace. Returned as lines (not a
/// [`ListItem`]) so tests can inspect the span styles.
fn rca_list_item(
    rca: &RcaSummary,
    tick: usize,
    has_unread: bool,
    selected: bool,
    progress: Option<(usize, usize)>,
) -> Vec<Line<'static>> {
    let (badge, mut badge_style) = severity_badge(rca.meta.severity);
    let (symbol, mut symbol_style) = status_symbol(rca.meta.status, tick);

    // The selection background is applied per-span so the badge keeps its
    // own colors; `base` styles everything that has no color identity.
    let mut base = if selected {
        Style::default()
            .bg(SELECTED_BG)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    if rca.archived {
        // Archived rows are uniformly dim — history, not a live incident.
        // The badge drops its background so nothing on the row pops.
        base = base.fg(Color::DarkGray);
        badge_style = base;
        symbol_style = base;
    }
    let tinted = |style: Style| {
        if selected {
            style.bg(SELECTED_BG)
        } else {
            style
        }
    };

    let title_text = truncate(&rca.meta.title, SIDEBAR_WIDTH as usize - 10);
    let title = pad_line(
        vec![
            Span::styled(format!(" {badge} "), badge_style),
            Span::styled(" ", base),
            Span::styled(title_text, base),
        ],
        base,
    );
    let mut detail_spans = vec![
        Span::styled(format!("  {symbol} "), tinted(symbol_style)),
        Span::styled(rca.meta.status.to_string(), tinted(symbol_style)),
    ];
    if rca.archived {
        detail_spans.push(Span::styled(" · archived", tinted(base)));
    }
    if let Some((checked, total)) = progress {
        // Checklist progress across all sections: green once complete.
        let style = if checked == total {
            Style::default().fg(Color::LightGreen)
        } else {
            Style::default().fg(Color::Gray)
        };
        detail_spans.push(Span::styled(
            format!("  ☑ {checked}/{total}"),
            tinted(style),
        ));
    }
    detail_spans.push(Span::styled(
        format!("  {}", truncate(rca.id.as_str(), 20)),
        tinted(Style::default().fg(Color::DarkGray)),
    ));
    if has_unread {
        detail_spans.push(Span::styled(
            " ●",
            tinted(Style::default().fg(Color::LightYellow)),
        ));
    }
    vec![title, pad_line(detail_spans, base)]
}

/// The two sidebar lines for a workspace that failed to load: an
/// unmissable marker plus the reason, so "why did my incident disappear"
/// answers itself in-app.
fn broken_list_item(broken: &crate::store::BrokenWorkspace) -> Vec<Line<'static>> {
    let width = usize::from(SIDEBAR_WIDTH).saturating_sub(2);
    vec![
        Line::from(vec![
            Span::styled(
                " ⚠ ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {}", truncate(&broken.dir_name, width.saturating_sub(5))),
                Style::default().fg(Color::LightRed),
            ),
        ]),
        Line::from(Span::styled(
            format!("   {}", truncate(&broken.reason, width.saturating_sub(3))),
            Style::default().fg(Color::Red),
        )),
    ]
}

/// Pads a sidebar line with a filler span so a selection background covers
/// the full row width, not just the text.
fn pad_line(spans: Vec<Span<'static>>, base: Style) -> Line<'static> {
    let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let mut spans = spans;
    let width = usize::from(SIDEBAR_WIDTH).saturating_sub(2); // inside the borders
    if used < width {
        spans.push(Span::styled(" ".repeat(width - used), base));
    }
    Line::from(spans)
}

fn draw_workspace(frame: &mut Frame, app: &mut App, area: Rect) {
    let Some(rca) = app.selected_rca().cloned() else {
        app.mouse.tabs.clear();
        app.mouse.content = area;
        draw_welcome(frame, area);
        return;
    };

    // The banner sits beside the header/tab rows at the top right, so it
    // never pushes content down by more than the difference between its
    // height and the rows the header already uses. Hidden when it would
    // squeeze the header below MIN_HEADER_COLS.
    let banner_cols = if banner_fits(area) { BANNER_COLS } else { 0 };
    let head_width = area.width.saturating_sub(banner_cols);

    // Header and tab bar heights are computed from the actual width so
    // nothing is ever cut off on narrow terminals — both flow onto extra
    // lines instead of truncating.
    let prs: Vec<(String, Option<crate::prs::PrState>)> = rca
        .meta
        .prs
        .iter()
        .map(|url| (url.clone(), app.pr_state(url)))
        .collect();
    let activity = app
        .last_activity(&rca.id)
        .and_then(|mtime| mtime.elapsed().ok());
    let header = header_paragraph(
        &rca,
        app.tick(),
        &prs,
        activity,
        app.checklist_progress(&rca.id),
    );
    let header_width = head_width.saturating_sub(1).max(1); // inset by 1 below
    let header_height = u16::try_from(header.line_count(header_width))
        .unwrap_or(2)
        .min(6);

    let unread: Vec<bool> = Tab::ALL
        .iter()
        .map(|tab| app.is_unread(&rca.id, *tab))
        .collect();
    let (tab_lines, tab_hits) = flow_tabs(app.tab(), head_width, &unread);
    let tab_height = u16::try_from(tab_lines.len()).unwrap_or(1);

    let banner_height = if banner_cols > 0 {
        crate::banner::HEIGHT
    } else {
        0
    };
    let top_height = (header_height + tab_height).max(banner_height);
    let [top, body] = vertical(area, &[Constraint::Length(top_height), Constraint::Min(0)]);
    let [head_col, banner_col] =
        horizontal(top, &[Constraint::Min(0), Constraint::Length(banner_cols)]);
    let [header_area, tab_bar] = vertical(
        head_col,
        &[Constraint::Length(header_height), Constraint::Min(0)],
    );

    frame.render_widget(header, inset(header_area, 1));
    frame.render_widget(Paragraph::new(tab_lines), tab_bar);
    if banner_cols > 0 {
        draw_banner(frame, banner_col);
    }

    // Label positions → clickable screen rects for the mouse handler.
    app.mouse.tabs = tab_hits
        .iter()
        .filter(|hit| hit.line < tab_bar.height)
        .map(|hit| {
            (
                hit.tab,
                Rect {
                    x: tab_bar.x + hit.x,
                    y: tab_bar.y + hit.line,
                    width: hit.width.min(tab_bar.width.saturating_sub(hit.x)),
                    height: 1,
                },
            )
        })
        .collect();
    app.mouse.content = body;

    draw_content(frame, app, body);
}

fn draw_content(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus() == Focus::Content;
    let (scroll, hscroll) = app.scroll_offsets();

    let (text, wrapped, title): (&Text<'static>, bool, String) = match app.pane() {
        Some(Pane::Section(text)) => (text, true, format!(" {} ", app.tab().title())),
        Some(Pane::Diagram {
            text,
            name,
            index,
            total,
        }) => (
            text,
            false,
            format!(" {name}  [{}/{total}]  n/p to cycle ", index + 1),
        ),
        Some(Pane::Empty(hint)) => {
            let paragraph = Paragraph::new(hint.clone())
                .style(Style::default().fg(Color::DarkGray))
                .wrap(Wrap { trim: false })
                .block(pane_block(format!(" {} ", app.tab().title()), focused));
            frame.render_widget(paragraph, area);
            return;
        }
        Some(Pane::LoadError(message)) => {
            let paragraph = Paragraph::new(message.clone())
                .style(Style::default().fg(Color::Red))
                .wrap(Wrap { trim: false })
                .block(pane_block(" load error ".to_owned(), focused));
            frame.render_widget(paragraph, area);
            return;
        }
        None => return,
    };

    let block = pane_block(title, focused);
    let inner = block.inner(area);

    let mut text = text.clone();
    highlight_search_matches(&mut text, app);

    // Feed real geometry back so scrolling clamps to actual wrapped height.
    let mut paragraph = Paragraph::new(text);
    if wrapped {
        paragraph = paragraph.wrap(Wrap { trim: false });
    }
    let content_lines = u16::try_from(paragraph.line_count(inner.width)).unwrap_or(u16::MAX);
    app.viewport = super::ViewportInfo {
        content_lines,
        height: inner.height,
        width: inner.width,
    };
    let max_scroll = content_lines.saturating_sub(inner.height);

    let paragraph = paragraph
        .block(block)
        .scroll((scroll.min(max_scroll), if wrapped { 0 } else { hscroll }));
    frame.render_widget(paragraph, area);
}

/// Highlights the matched text itself on the visible tab — the occurrence,
/// not the whole line, so the eye lands exactly on it. The current hit's
/// occurrences pop in amber-on-black; other hits get a quieter steel tint.
fn highlight_search_matches(text: &mut Text<'static>, app: &App) {
    let Some(query) = app.content_search().map(|s| s.query.clone()) else {
        return;
    };
    for (line_index, is_current) in app.search_highlights(app.tab()) {
        if let Some(line) = text.lines.get_mut(line_index) {
            let style = if is_current {
                Style::default()
                    .bg(Color::Rgb(214, 160, 30))
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().bg(Color::Rgb(60, 68, 96))
            };
            *line = super::search::highlight_occurrences(line, &query, style);
        }
    }
}

fn draw_welcome(frame: &mut Frame, area: Rect) {
    let text = Text::from(vec![
        Line::from(""),
        Line::styled(
            "  no RCA workspaces yet",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Line::from(""),
        Line::from("  Create one:"),
        Line::styled(
            "    beagle new my-incident --title \"What broke\" --severity high",
            Style::default().fg(Color::LightGreen),
        ),
        Line::from(""),
        Line::from("  …or ask Claude to debug a system; it will scaffold a workspace"),
        Line::from("  under rcas/ and this view will pick it up live."),
    ]);
    let block = pane_block(" beagle ".to_owned(), false);
    frame.render_widget(Paragraph::new(text).block(block), area);
}

/// The filter-mode prompt: active facet chips, the free-text query (with a
/// cursor while typing), and the keys available in the current sub-mode.
fn filter_prompt_line(app: &App) -> Line<'static> {
    let mut spans = vec![Span::styled(
        "  filter: ",
        Style::default().fg(Color::Yellow),
    )];
    let facets = app.facet_label();
    if !facets.is_empty() {
        spans.push(Span::styled(
            format!("{facets} "),
            Style::default().fg(Color::LightBlue),
        ));
    }
    spans.push(Span::raw(app.filter().to_owned()));
    if app.filter_typing() {
        spans.push(Span::styled("▌", Style::default().fg(Color::Yellow)));
        spans.push(Span::styled(
            "   type keywords · esc back to facets",
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        spans.push(Span::styled(
            "   i/r/a/v/f status · c/h/m/l severity · / type · enter keep · esc clear",
            Style::default().fg(Color::DarkGray),
        ));
    }
    Line::from(spans)
}

/// The in-content search's hint line: live query, match position, or the
/// no-matches report.
fn search_status_line(search: &super::search::ContentSearch) -> Line<'static> {
    if search.typing {
        Line::from(vec![
            Span::styled("  search: ", Style::default().fg(Color::Yellow)),
            Span::raw(search.query.clone()),
            Span::styled("▌", Style::default().fg(Color::Yellow)),
            Span::styled(
                "   enter commit · esc cancel",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else if search.hits.is_empty() {
        Line::from(Span::styled(
            format!(
                "  no matches for \"{}\" anywhere in this incident — esc clears",
                search.query
            ),
            Style::default().fg(Color::Yellow),
        ))
    } else {
        let place = search
            .hits
            .get(search.current)
            .map_or("", |hit| hit.tab.title());
        Line::from(vec![
            Span::styled(
                format!(
                    "  match {}/{} for \"{}\" · {place}",
                    search.current + 1,
                    search.hits.len(),
                    search.query,
                ),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                "   n/N next/prev (across tabs) · esc clear",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    }
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    if app.search_active() {
        frame.render_widget(Paragraph::new(filter_prompt_line(app)), area);
        return;
    }
    // A status message gets the whole line — sharing it with the key hints
    // truncated messages into uselessness. It disappears on the next
    // keypress, so the hints are only ever hidden for one beat.
    if let Some(message) = app.status_line() {
        let (symbol, color) = if message.contains("failed") {
            ("  ✗ ", Color::LightRed)
        } else {
            ("  ✓ ", Color::LightGreen)
        };
        let line = Line::from(vec![
            Span::styled(symbol, Style::default().fg(color)),
            Span::styled(message.to_owned(), Style::default().fg(color)),
        ]);
        frame.render_widget(Paragraph::new(line), area);
        return;
    }
    // In-content search owns the hint line while it exists; a transient
    // status message (handled above) still gets its one beat.
    if let Some(search) = app.content_search() {
        frame.render_widget(Paragraph::new(search_status_line(search)), area);
        return;
    }
    let mut spans = vec![Span::styled(
        match app.focus() {
            Focus::List => {
                "  j/k select · enter open · ←/→ tabs · f filter · / search · T toolbox · R related · c copy · r reload · ? help · Q quit"
            }
            Focus::Content => {
                "  j/k scroll · ←/→ tabs · / search · h/l pan · F follow · s sidebar · o links · c copy · b back · ? help · Q quit"
            }
        },
        Style::default().fg(Color::DarkGray),
    )];
    if app.follow() {
        spans.push(Span::styled(
            "  ·  following (esc exits)",
            Style::default().fg(Color::LightYellow),
        ));
    }
    if !app.warnings().is_empty() {
        spans.push(Span::styled(
            format!(
                "  ·  {} warning(s), first: {}",
                app.warnings().len(),
                app.warnings()[0].0
            ),
            Style::default().fg(Color::Yellow),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

#[cfg(test)]
#[path = "tests/view.rs"]
mod tests;
