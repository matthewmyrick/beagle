//! The interactive version picker behind `beagle version list`.

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};
use ratatui::DefaultTerminal;

use crate::error::Result;

use super::{Release, Version};

/// Runs the interactive version picker: j/k or arrows move, enter returns
/// the chosen version, q/esc/ctrl-c returns `None`. Sets up and restores the
/// terminal itself.
///
/// # Errors
/// [`Error::Terminal`](crate::Error::Terminal) on draw or input failures.
pub fn pick_version(releases: &[Release], current: Version) -> Result<Option<Version>> {
    if releases.is_empty() {
        return Ok(None);
    }
    let mut terminal = ratatui::init();
    let picked = run_picker(&mut terminal, releases, current);
    ratatui::restore();
    picked
}

fn run_picker(
    terminal: &mut DefaultTerminal,
    releases: &[Release],
    current: Version,
) -> Result<Option<Version>> {
    let mut selected = 0usize;
    loop {
        terminal.draw(|frame| draw_picker(frame, releases, current, selected))?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(None);
        }
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                selected = (selected + 1).min(releases.len() - 1);
            }
            KeyCode::Char('k') | KeyCode::Up => selected = selected.saturating_sub(1),
            KeyCode::Char('g') | KeyCode::Home => selected = 0,
            KeyCode::Char('G') | KeyCode::End => selected = releases.len() - 1,
            KeyCode::Enter => return Ok(releases.get(selected).map(|r| r.version)),
            KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
            _ => {}
        }
    }
}

fn draw_picker(
    frame: &mut ratatui::Frame,
    releases: &[Release],
    current: Version,
    selected: usize,
) {
    let items: Vec<ListItem<'_>> = releases
        .iter()
        .enumerate()
        .map(|(i, release)| ListItem::new(release_line(release.version, current, i == 0)))
        .collect();
    let block = Block::default()
        .title(" beagle versions — enter install · j/k move · q cancel ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));
    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::Rgb(40, 44, 60))
            .add_modifier(Modifier::BOLD),
    );
    let mut state = ListState::default();
    state.select(Some(selected));

    let [area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0)])
        .areas(frame.area());
    frame.render_stateful_widget(list, area, &mut state);
}

/// One picker row: the tag plus `latest` / `current` markers.
fn release_line(version: Version, current: Version, is_latest: bool) -> Line<'static> {
    let mut spans = vec![Span::raw(format!(" {:<12}", version.tag()))];
    if is_latest {
        spans.push(Span::styled(
            " latest",
            Style::default().fg(Color::LightGreen),
        ));
    }
    if version == current {
        spans.push(Span::styled(
            " ← current",
            Style::default().fg(Color::Yellow),
        ));
    }
    Line::from(spans)
}
