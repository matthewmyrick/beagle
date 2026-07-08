//! A small, panic-free markdown renderer targeting ratatui [`Text`].
//!
//! Deliberately minimal: it supports the subset RCA authors actually use
//! (headings, bullets, code fences, blockquotes, rules, `**bold**` and
//! `` `code` `` inline) and treats everything else as plain text. Parsing is
//! single-pass and line-based; content is re-rendered only when it changes,
//! never per frame.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

/// Renders markdown `source` into styled text.
///
/// Never fails and never panics: unrecognized syntax falls through as plain
/// text, and an unclosed code fence simply styles the rest of the document as
/// code.
#[must_use]
pub fn to_text(source: &str) -> Text<'static> {
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(source.lines().count());
    let mut in_code_fence = false;

    for raw in source.lines() {
        if raw.trim_start().starts_with("```") {
            // Fence markers (and their language tag) are syntax, not content:
            // they are hidden entirely.
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence {
            // Code blocks get a slim gutter instead of a background patch —
            // backgrounds render as ragged boxes on most terminal themes.
            lines.push(Line::from(vec![
                Span::styled("▍ ", Style::default().fg(Color::DarkGray)),
                Span::raw(raw.to_owned()),
            ]));
            continue;
        }
        lines.push(render_line(raw));
    }
    Text::from(lines)
}

fn render_line(raw: &str) -> Line<'static> {
    let trimmed = raw.trim_start();
    let indent = raw.len() - trimmed.len();

    if let Some(rest) = trimmed.strip_prefix("### ") {
        return heading(rest, Color::Blue, false);
    }
    if let Some(rest) = trimmed.strip_prefix("## ") {
        return heading(rest, Color::Cyan, false);
    }
    if let Some(rest) = trimmed.strip_prefix("# ") {
        return heading(rest, Color::Cyan, true);
    }
    if trimmed == "---" || trimmed == "***" {
        return Line::styled("─".repeat(40), Style::default().fg(Color::DarkGray));
    }
    if let Some(rest) = trimmed
        .strip_prefix("> ")
        .or_else(|| (trimmed == ">").then_some(""))
    {
        let mut spans = vec![Span::styled("│ ", Style::default().fg(Color::DarkGray))];
        spans.extend(inline_spans(
            rest,
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        ));
        return Line::from(spans);
    }
    if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
    {
        let mut spans = vec![
            Span::raw(" ".repeat(indent)),
            Span::styled("• ", Style::default().fg(Color::Yellow)),
        ];
        spans.extend(inline_spans(rest, Style::default()));
        return Line::from(spans);
    }
    Line::from(inline_spans(raw, Style::default()))
}

fn heading(text: &str, color: Color, top_level: bool) -> Line<'static> {
    let style = Style::default().fg(color).add_modifier(Modifier::BOLD);
    if top_level {
        Line::styled(text.to_owned(), style.add_modifier(Modifier::UNDERLINED))
    } else {
        Line::styled(text.to_owned(), style)
    }
}

/// Splits a line into spans, honoring `` `code` `` and `**bold**` markers.
/// Unbalanced markers are emitted literally — an author's stray `**` must
/// never eat the rest of the line.
fn inline_spans(text: &str, base: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut plain = String::new();
    let mut rest = text;

    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("**") {
            if let Some(end) = after.find("**") {
                flush(&mut spans, &mut plain, base);
                spans.push(Span::styled(
                    after[..end].to_owned(),
                    base.add_modifier(Modifier::BOLD),
                ));
                rest = &after[end + 2..];
                continue;
            }
        }
        if let Some(after) = rest.strip_prefix('`') {
            if let Some(end) = after.find('`') {
                flush(&mut spans, &mut plain, base);
                spans.push(Span::styled(after[..end].to_owned(), base.fg(Color::Cyan)));
                rest = &after[end + 1..];
                continue;
            }
        }
        // Advance one char; `chars().next()` is Some because rest is non-empty.
        let ch = rest.chars().next().unwrap_or('\u{0}');
        plain.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
    flush(&mut spans, &mut plain, base);
    if spans.is_empty() {
        spans.push(Span::styled(String::new(), base));
    }
    spans
}

fn flush(spans: &mut Vec<Span<'static>>, plain: &mut String, base: Style) {
    if !plain.is_empty() {
        spans.push(Span::styled(std::mem::take(plain), base));
    }
}

#[cfg(test)]
#[path = "tests/markdown.rs"]
mod tests;
