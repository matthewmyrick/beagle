//! Tests for the in-content search (`ui::search`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use crossterm::event::KeyCode;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};

use crate::ui::testutil::{app_with, press};
use crate::ui::{App, Focus, Tab};

use super::{find_match_lines, highlight_occurrences};

#[test]
fn match_lines_are_case_insensitive_and_empty_query_matches_nothing() {
    let text = Text::from("Redis pool exhausted\nall fine\nREDIS timeouts\n");
    assert_eq!(find_match_lines(&text, "redis"), [0, 2]);
    assert_eq!(find_match_lines(&text, "Fine"), [1]);
    assert!(find_match_lines(&text, "grafana").is_empty());
    assert!(
        find_match_lines(&text, "").is_empty(),
        "empty query highlights nothing, not everything"
    );
}

#[test]
fn occurrences_highlight_the_word_not_the_line_even_across_spans() {
    let hl = Style::default().bg(Color::Yellow);
    // The second occurrence straddles two differently-styled spans.
    let line = Line::from(vec![
        Span::raw("Redis pool; re"),
        Span::styled("dis again", Style::default().fg(Color::Green)),
    ]);
    let out = highlight_occurrences(&line, "redis", hl);

    let full: String = out.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(full, "Redis pool; redis again", "text unchanged");

    let highlighted: String = out
        .spans
        .iter()
        .filter(|s| s.style.bg == Some(Color::Yellow))
        .map(|s| s.content.as_ref())
        .collect();
    assert_eq!(highlighted, "Redisredis", "exactly the occurrences");

    let tail = out.spans.last().expect("tail span");
    assert_eq!(tail.style.fg, Some(Color::Green), "span styles preserved");
    assert_ne!(tail.style.bg, Some(Color::Yellow), "non-match not tinted");

    // Cross-span match keeps the original fg under the highlight.
    let dis = out
        .spans
        .iter()
        .find(|s| s.content.as_ref() == "dis")
        .expect("split segment");
    assert_eq!(dis.style.bg, Some(Color::Yellow));
    assert_eq!(
        dis.style.fg,
        Some(Color::Green),
        "highlight patches, not replaces"
    );

    // A line without the query comes back untouched.
    let miss = Line::from("nothing here");
    assert_eq!(highlight_occurrences(&miss, "redis", hl), miss);
}

/// An app whose selected workspace has a summary with two "redis" lines.
fn app_with_redis_summary() -> App {
    let mut app = app_with(1);
    let id = app.selected_rca().expect("selected").id.clone();
    std::fs::write(
        app.store.workspace_dir(&id).join("summary.md"),
        "# Summary\n\nRedis pool exhausted\nall good here\nredis timeouts again\n",
    )
    .expect("write summary");
    app.reload();
    press(&mut app, KeyCode::Enter); // focus content
    app.ensure_pane();
    app
}

#[test]
fn slash_in_content_focus_searches_the_pane_not_the_list() {
    let mut app = app_with_redis_summary();
    assert_eq!(app.focus(), Focus::Content);
    press(&mut app, KeyCode::Char('/'));
    assert!(
        app.content_search().is_some_and(|s| s.typing),
        "content focus: / starts the in-pane search"
    );
    assert!(!app.search_active(), "list filter untouched");
}

#[test]
fn slash_searches_from_list_focus_and_lands_on_the_content_immediately() {
    let mut app = app_with_redis_summary();
    press(&mut app, KeyCode::Char('b')); // back to the list
    assert_eq!(app.focus(), Focus::List);

    press(&mut app, KeyCode::Char('/'));
    assert!(
        app.content_search().is_some_and(|s| s.typing),
        "/ searches regardless of focus"
    );
    assert!(!app.search_active(), "the list filter is untouched");
    assert_eq!(
        app.focus(),
        Focus::Content,
        "/ puts you on the incident you're on, right away"
    );

    for c in "redis".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        app.focus(),
        Focus::Content,
        "still on the content on commit"
    );
}

#[test]
fn f_filters_the_list_and_moves_focus_to_the_incidents_pane() {
    let mut app = app_with(1);
    press(&mut app, KeyCode::Enter); // start from content focus
    assert_eq!(app.focus(), Focus::Content);

    press(&mut app, KeyCode::Char('f'));
    assert!(app.search_active(), "f owns the list filter now");
    assert!(app.content_search().is_none());
    assert_eq!(
        app.focus(),
        Focus::List,
        "filtering is list work — focus follows"
    );
}

#[test]
fn slash_on_an_empty_store_explains() {
    let mut empty = app_with(0);
    press(&mut empty, KeyCode::Char('/'));
    assert!(empty.content_search().is_none());
    assert!(empty
        .status_line()
        .expect("status set")
        .contains("no incident selected"));
}

#[test]
fn typing_matches_live_enter_commits_and_n_cycles_with_wrap() {
    let mut app = app_with_redis_summary();
    press(&mut app, KeyCode::Char('/'));
    for c in "redis".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    let search = app.content_search().expect("search open");
    assert_eq!(search.query, "redis");
    assert_eq!(search.hits.len(), 2, "case-insensitive, both lines");
    assert_eq!(search.current, 0);

    press(&mut app, KeyCode::Enter);
    assert!(
        app.content_search().is_some_and(|s| !s.typing),
        "enter commits the query"
    );

    press(&mut app, KeyCode::Char('n'));
    assert_eq!(app.content_search().expect("open").current, 1);
    press(&mut app, KeyCode::Char('n'));
    assert_eq!(app.content_search().expect("open").current, 0, "n wraps");
    press(&mut app, KeyCode::Char('N'));
    assert_eq!(
        app.content_search().expect("open").current,
        1,
        "N wraps back"
    );
}

#[test]
fn esc_peels_search_first_then_returns_to_the_list() {
    let mut app = app_with_redis_summary();
    press(&mut app, KeyCode::Char('/'));
    press(&mut app, KeyCode::Char('r'));
    press(&mut app, KeyCode::Enter);
    assert!(app.content_search().is_some());

    press(&mut app, KeyCode::Esc);
    assert!(
        app.content_search().is_none(),
        "first esc clears the search"
    );
    assert_eq!(app.focus(), Focus::Content, "still on the content");

    press(&mut app, KeyCode::Esc);
    assert_eq!(app.focus(), Focus::List, "second esc leaves the pane");
}

#[test]
fn one_query_finds_hits_across_every_tab_and_n_hops_tabs() {
    let mut app = app_with_redis_summary();
    let id = app.selected_rca().expect("selected").id.clone();
    std::fs::write(
        app.store.workspace_dir(&id).join("notes.md"),
        "# Notes\n\nredis one\nredis two\nredis three\n",
    )
    .expect("write notes");
    app.reload();
    app.ensure_pane();
    assert_eq!(app.tab(), Tab::Summary);

    press(&mut app, KeyCode::Char('/'));
    for c in "redis".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    let search = app.content_search().expect("open");
    assert_eq!(search.hits.len(), 5, "2 in summary + 3 in notes");
    assert_eq!(search.current, 0, "starts on the tab in view");
    assert_eq!(app.tab(), Tab::Summary, "typing never yanks the tab");

    // n walks summary's two hits, then hops to Notes on its own.
    press(&mut app, KeyCode::Char('n'));
    assert_eq!(app.tab(), Tab::Summary);
    press(&mut app, KeyCode::Char('n'));
    assert_eq!(app.tab(), Tab::Notes, "n crosses to the next tab's hit");
    assert_eq!(app.content_search().expect("open").current, 2);

    // Wrapping goes all the way around, back to Summary.
    press(&mut app, KeyCode::Char('n'));
    press(&mut app, KeyCode::Char('n'));
    press(&mut app, KeyCode::Char('n'));
    assert_eq!(app.tab(), Tab::Summary, "wraps back to the first hit's tab");
    assert_eq!(app.content_search().expect("open").current, 0);

    // N walks backwards across the tab boundary too.
    press(&mut app, KeyCode::Char('N'));
    assert_eq!(app.tab(), Tab::Notes);

    // The query (and highlights) survive a manual tab switch.
    press(&mut app, KeyCode::Char('1'));
    app.ensure_pane();
    let search = app.content_search().expect("query survives the switch");
    assert_eq!(search.query, "redis");
    assert_eq!(search.hits.len(), 5);
    assert_eq!(
        app.search_highlights(Tab::Summary).len(),
        2,
        "summary highlights only summary's hits"
    );
}

#[test]
fn typing_q_or_numbers_edits_the_query_instead_of_acting() {
    let mut app = app_with_redis_summary();
    press(&mut app, KeyCode::Char('/'));
    press(&mut app, KeyCode::Char('q'));
    press(&mut app, KeyCode::Char('3'));
    let search = app.content_search().expect("open");
    assert_eq!(search.query, "q3", "keys type instead of quitting/tabbing");
    assert_eq!(app.tab(), crate::ui::Tab::Summary);

    // Backspace repairs; committing an empty query closes the search.
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Backspace);
    press(&mut app, KeyCode::Enter);
    assert!(app.content_search().is_none(), "empty commit = no search");
}
