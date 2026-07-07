//! Drawing: a pure projection of [`App`] onto the frame.
//!
//! The only mutation allowed here is feeding viewport geometry back into the
//! app (for scroll clamping) — no state transitions, no I/O, and no markdown
//! parsing (content is pre-rendered by the app when it changes, not per
//! frame).

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
};
use ratatui::Frame;
use time::macros::format_description;

use crate::model::{RcaSummary, Severity, Status};

use super::{App, Focus, Pane, Tab};

const SIDEBAR_WIDTH: u16 = 34;

/// Spinner frames for [`Status::Investigating`], advanced by the app tick.
/// Braille dots: distinct at 4 fps and exactly one cell wide, so the sidebar
/// and header never shift as it animates.
const INVESTIGATING_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Minimum columns the header (title + meta) keeps for itself before the
/// banner is allowed to claim the top-right corner.
const MIN_HEADER_COLS: u16 = 44;

/// Columns the banner claims when shown: the art plus a one-column gap on
/// each side.
const BANNER_COLS: u16 = crate::banner::WIDTH + 2;

pub(crate) fn draw(frame: &mut Frame, app: &mut App) {
    let [main, status_bar] = vertical(frame.area(), &[Constraint::Min(0), Constraint::Length(1)]);
    let [sidebar, content] = horizontal(
        main,
        &[Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(0)],
    );

    draw_sidebar(frame, app, sidebar);
    draw_workspace(frame, app, content);
    draw_status_bar(frame, app, status_bar);

    if app.toolbox().is_some() {
        draw_toolbox(frame, app, frame.area());
    }
    if app.help_visible() {
        draw_help(frame, frame.area());
    }
}

/// The toolbox overlay: root `toolbox.md` plus relevant `systems/*.md`,
/// pre-rendered by the app when opened. Scrollable; geometry is fed back
/// for clamping, like the content pane.
fn draw_toolbox(frame: &mut Frame, app: &mut App, area: Rect) {
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

/// Whether the workspace area has room for the banner beside the header —
/// wide enough that the header column keeps [`MIN_HEADER_COLS`], tall enough
/// that four rows of art don't crowd out the content.
fn banner_fits(area: Rect) -> bool {
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
fn draw_banner(frame: &mut Frame, area: Rect) {
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

fn draw_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus() == Focus::List;
    let title = if app.filter().is_empty() {
        format!(" Incidents ({}) ", app.rcas().len())
    } else {
        format!(
            " Incidents ({}/{})  /{} ",
            app.visible_len(),
            app.rcas().len(),
            app.filter(),
        )
    };
    let block = pane_block(title, focused);

    let tick = app.tick();
    let items: Vec<ListItem<'_>> = app
        .visible_rcas()
        .map(|rca| rca_list_item(rca, tick))
        .collect();
    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::Rgb(40, 44, 60))
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    if app.visible_len() > 0 {
        state.select(Some(app.selected_index()));
    }
    frame.render_stateful_widget(list, area, &mut state);
}

fn rca_list_item(rca: &RcaSummary, tick: usize) -> ListItem<'static> {
    let (badge, badge_style) = severity_badge(rca.meta.severity);
    let (symbol, symbol_style) = status_symbol(rca.meta.status, tick);
    let title = Line::from(vec![
        Span::styled(format!(" {badge} "), badge_style),
        Span::raw(" "),
        Span::raw(truncate(&rca.meta.title, SIDEBAR_WIDTH as usize - 10)),
    ]);
    let detail = Line::from(vec![
        Span::styled(format!("  {symbol} "), symbol_style),
        Span::styled(rca.meta.status.to_string(), symbol_style),
        Span::styled(
            format!("  {}", truncate(rca.id.as_str(), 20)),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    ListItem::new(vec![title, detail])
}

fn draw_workspace(frame: &mut Frame, app: &mut App, area: Rect) {
    let Some(rca) = app.selected_rca().cloned() else {
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
    let header = header_paragraph(&rca, app.tick());
    let header_width = head_width.saturating_sub(1).max(1); // inset by 1 below
    let header_height = u16::try_from(header.line_count(header_width))
        .unwrap_or(2)
        .min(6);

    let tab_lines = flow_tabs(app.tab(), head_width);
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

    draw_content(frame, app, body);
}

/// Lays the seven tab labels out left to right, flowing onto additional
/// lines when the width runs out — never truncating a label.
fn flow_tabs(selected: Tab, width: u16) -> Vec<Line<'static>> {
    let width = usize::from(width.max(1));
    let mut lines = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;

    for (i, tab) in Tab::ALL.iter().enumerate() {
        let label = format!(" {} {} ", i + 1, tab.title());
        let label_width = label.chars().count();
        let style = if *tab == selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
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

fn header_paragraph(rca: &RcaSummary, tick: usize) -> Paragraph<'static> {
    let (badge, badge_style) = severity_badge(rca.meta.severity);
    let (symbol, symbol_style) = status_symbol(rca.meta.status, tick);

    let opened = rca
        .meta
        .created
        .format(format_description!(
            "[year]-[month]-[day] [hour]:[minute] UTC"
        ))
        .unwrap_or_else(|_| rca.meta.created.to_string());

    let mut meta_spans = vec![
        Span::styled(format!("{symbol} {}", rca.meta.status), symbol_style),
        Span::styled("  ·  ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" {badge} "), badge_style),
        Span::styled("  ·  ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("opened {opened}"), Style::default().fg(Color::Gray)),
    ];
    if !rca.meta.systems.is_empty() {
        meta_spans.push(Span::styled("  ·  ", Style::default().fg(Color::DarkGray)));
        meta_spans.push(Span::styled(
            rca.meta.systems.join(", "),
            Style::default().fg(Color::LightBlue),
        ));
    }

    Paragraph::new(vec![
        Line::styled(
            rca.meta.title.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Line::from(meta_spans),
    ])
    .wrap(Wrap { trim: false })
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

    // Feed real geometry back so scrolling clamps to actual wrapped height.
    let mut paragraph = Paragraph::new(text.clone());
    if wrapped {
        paragraph = paragraph.wrap(Wrap { trim: false });
    }
    let content_lines = u16::try_from(paragraph.line_count(inner.width)).unwrap_or(u16::MAX);
    app.viewport = super::ViewportInfo {
        content_lines,
        height: inner.height,
    };
    let max_scroll = content_lines.saturating_sub(inner.height);

    let paragraph = paragraph
        .block(block)
        .scroll((scroll.min(max_scroll), if wrapped { 0 } else { hscroll }));
    frame.render_widget(paragraph, area);
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

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    if app.search_active() {
        let line = Line::from(vec![
            Span::styled("  filter: ", Style::default().fg(Color::Yellow)),
            Span::raw(app.filter().to_owned()),
            Span::styled("▌", Style::default().fg(Color::Yellow)),
            Span::styled(
                "   enter keep · esc clear · ↑/↓ select",
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        frame.render_widget(Paragraph::new(line), area);
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
    let mut spans = vec![Span::styled(
        match app.focus() {
            Focus::List => {
                "  j/k select · enter open · ←/→ tabs · / filter · T toolbox · c copy · r reload · ? help · Q quit"
            }
            Focus::Content => {
                "  j/k scroll · ←/→ tabs · h/l pan · c copy · b back · ? help · Q quit"
            }
        },
        Style::default().fg(Color::DarkGray),
    )];
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

fn draw_help(frame: &mut Frame, area: Rect) {
    let width = 62.min(area.width.saturating_sub(4));
    let height = 19.min(area.height.saturating_sub(2));
    let popup = center(area, width, height);

    let rows = [
        ("j / k, ↓ / ↑", "select incident or scroll content"),
        ("enter / l", "focus the content pane"),
        ("b / esc", "back to the incident list"),
        ("tab / [ / ], ← / →", "cycle tabs"),
        ("1–7", "jump to a tab"),
        ("/", "fuzzy-filter incidents (esc clears)"),
        ("c / C", "copy this tab / whole RCA to clipboard"),
        (
            "e",
            "export RCA to exports/<id>.md (frontmatter + all tabs)",
        ),
        ("T", "toolbox: toolbox.md + systems/ context"),
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

// ---------- small helpers ----------

fn severity_badge(severity: Severity) -> (&'static str, Style) {
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
fn status_symbol(status: Status, tick: usize) -> (&'static str, Style) {
    match status {
        Status::Investigating => (
            INVESTIGATING_FRAMES[tick % INVESTIGATING_FRAMES.len()],
            Style::default().fg(Color::LightRed),
        ),
        Status::Identified => ("◐", Style::default().fg(Color::Yellow)),
        Status::Monitoring => ("◒", Style::default().fg(Color::LightBlue)),
        Status::Resolved => ("✔", Style::default().fg(Color::Green)),
    }
}

fn pane_block(title: String, focused: bool) -> Block<'static> {
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

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_owned()
    } else {
        let cut: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

fn vertical<const N: usize>(area: Rect, constraints: &[Constraint; N]) -> [Rect; N] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .areas(area)
}

fn horizontal<const N: usize>(area: Rect, constraints: &[Constraint; N]) -> [Rect; N] {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .areas(area)
}

fn inset(area: Rect, left: u16) -> Rect {
    Rect {
        x: area.x.saturating_add(left),
        width: area.width.saturating_sub(left),
        ..area
    }
}

fn center(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)] // panicking is the correct failure mode in tests

    use super::*;

    fn rendered(lines: &[Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn tabs_fit_on_one_line_when_wide() {
        let lines = flow_tabs(Tab::Summary, 200);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn tabs_flow_to_more_lines_when_narrow_and_keep_every_label() {
        let lines = flow_tabs(Tab::Notes, 40);
        assert!(lines.len() > 1, "40 cols cannot fit all seven tabs");
        let all: String = rendered(&lines).join("");
        for (i, tab) in Tab::ALL.iter().enumerate() {
            let label = format!(" {} {} ", i + 1, tab.title());
            assert!(all.contains(&label), "label `{label}` missing");
        }
    }

    #[test]
    fn tabs_survive_absurdly_narrow_width() {
        let lines = flow_tabs(Tab::Summary, 1);
        assert_eq!(lines.len(), Tab::ALL.len(), "one label per line");
    }

    #[test]
    fn investigating_symbol_animates_and_others_stay_fixed() {
        let (frame0, _) = status_symbol(Status::Investigating, 0);
        let (frame1, _) = status_symbol(Status::Investigating, 1);
        assert_ne!(frame0, frame1, "investigating must animate");
        let (wrapped, _) = status_symbol(Status::Investigating, INVESTIGATING_FRAMES.len());
        assert_eq!(wrapped, frame0, "frames cycle");

        for status in [Status::Identified, Status::Monitoring, Status::Resolved] {
            let (a, _) = status_symbol(status, 0);
            let (b, _) = status_symbol(status, 7);
            assert_eq!(a, b, "{status} must not animate");
        }
    }

    #[test]
    fn every_spinner_frame_is_one_cell_wide() {
        for frame in INVESTIGATING_FRAMES {
            assert_eq!(frame.chars().count(), 1, "frame `{frame}` shifts layout");
        }
    }

    #[test]
    fn banner_shows_only_when_the_pane_is_big_enough() {
        let big = Rect::new(0, 0, BANNER_COLS + MIN_HEADER_COLS, 40);
        assert!(banner_fits(big));
        let narrow = Rect::new(0, 0, BANNER_COLS + MIN_HEADER_COLS - 1, 40);
        assert!(
            !banner_fits(narrow),
            "too narrow: header column comes first"
        );
        let short = Rect::new(0, 0, 120, 15);
        assert!(!banner_fits(short), "too short: content comes first");
    }

    #[test]
    fn banner_rect_is_pinned_to_the_right_edge_at_full_art_width() {
        // Alignment correctness: the art must render left-aligned in a rect
        // exactly as wide as its widest line, or ragged line ends stagger.
        let area = Rect::new(10, 2, 60, 6);
        let rect = banner_rect(area);
        assert_eq!(rect.width, crate::banner::WIDTH, "never wider than the art");
        assert_eq!(
            rect.x + rect.width + 1,
            area.x + area.width,
            "one-column right margin"
        );
        assert_eq!(rect.y, area.y);

        // Degenerate area: must not underflow or exceed the area.
        let tiny = Rect::new(0, 0, 10, 6);
        let clamped = banner_rect(tiny);
        assert!(clamped.width <= tiny.width);
        assert!(clamped.x + clamped.width <= tiny.width);
    }

    /// Renders a full frame into a test backend and checks every banner art
    /// row occupies the same columns — the regression that motivated
    /// [`banner_rect`]: per-line right-alignment staggered the art.
    #[test]
    fn rendered_banner_rows_are_column_aligned() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = crate::store::Store::open(tmp.path()).expect("store");
        let id = crate::model::RcaId::new("banner-check").expect("id");
        store
            .scaffold(&id, &crate::store::new_meta("T".to_owned(), Severity::High))
            .expect("scaffold");
        let mut app = App::new(store).expect("app");

        let (width, height) = (120u16, 30u16);
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
        terminal.draw(|frame| draw(frame, &mut app)).expect("draw");
        let buffer = terminal.backend().buffer();

        // Where the layout should have pinned the art: right edge of the
        // content column, one-column margin.
        let art_x = usize::from(width - 1 - crate::banner::WIDTH);
        for (y, expected) in crate::banner::BANNER.lines().enumerate() {
            let row: String = (0..width)
                .map(|x| {
                    buffer[(x, u16::try_from(y).expect("small"))]
                        .symbol()
                        .chars()
                        .next()
                        .unwrap_or(' ')
                })
                .collect();
            let segment: String = row
                .chars()
                .skip(art_x)
                .take(expected.chars().count())
                .collect();
            assert_eq!(segment, expected, "banner row {y} misaligned:\n{row}");
        }
    }

    #[test]
    fn no_line_exceeds_the_given_width_when_labels_fit() {
        let width: usize = 30;
        let lines = flow_tabs(Tab::Impact, u16::try_from(width).expect("fits"));
        for line in rendered(&lines) {
            assert!(
                line.chars().count() <= width,
                "line `{line}` is wider than {width}"
            );
        }
    }
}
