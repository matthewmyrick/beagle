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
    let mut lang = String::new();

    for raw in source.lines() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with("```") {
            if in_code_fence {
                // Closing fence: a dim rule so the block's end is unmistakable.
                lines.push(fence_rule("╰", ""));
                in_code_fence = false;
                lang.clear();
            } else {
                // Opening fence: a dim rule labelled with the language, so
                // where the code starts (and in what language) is obvious.
                trimmed.trim_start_matches('`').trim().clone_into(&mut lang);
                lines.push(fence_rule("╭", &lang));
                in_code_fence = true;
            }
            continue;
        }
        if in_code_fence {
            // A slim gutter groups the block; the code itself is syntax-
            // highlighted for the language on the opening fence.
            let mut spans = vec![Span::styled("│ ", Style::default().fg(Color::DarkGray))];
            spans.extend(highlight::code(&lang, raw));
            lines.push(Line::from(spans));
            continue;
        }
        lines.push(render_line(raw));
    }
    Text::from(lines)
}

/// A dim horizontal rule delimiting a code block: `╭─ python ────` at the
/// top, `╰──────` at the bottom.
fn fence_rule(corner: &str, lang: &str) -> Line<'static> {
    let dim = Style::default().fg(Color::DarkGray);
    let label = if lang.is_empty() {
        format!("{corner}{}", "─".repeat(20))
    } else {
        format!("{corner}─ {lang} {}", "─".repeat(16))
    };
    Line::styled(label, dim)
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
        if let Some((done, text)) = checkbox(rest) {
            // Checked items go green — on a verification checklist a tick
            // means "this held". Unchecked stay normal (not red/yellow):
            // pending is the default state, not a warning.
            let (glyph, glyph_style, text_style) = if done {
                (
                    "☑ ",
                    Style::default().fg(Color::Green),
                    Style::default().fg(Color::Green),
                )
            } else {
                ("☐ ", Style::default().fg(Color::Yellow), Style::default())
            };
            let mut spans = vec![
                Span::raw(" ".repeat(indent)),
                Span::styled(glyph, glyph_style),
            ];
            spans.extend(inline_spans(text, text_style));
            return Line::from(spans);
        }
        let mut spans = vec![
            Span::raw(" ".repeat(indent)),
            Span::styled("• ", Style::default().fg(Color::Yellow)),
        ];
        spans.extend(inline_spans(rest, Style::default()));
        return Line::from(spans);
    }
    Line::from(inline_spans(raw, Style::default()))
}

/// The checkbox at the start of a bullet's content, if any: `[ ]`, `[x]`,
/// or `[X]`, either alone or followed by a space and the item text.
/// Anything else (`[x]done`, `[y] ...`) is an ordinary bullet.
fn checkbox(rest: &str) -> Option<(bool, &str)> {
    let (done, after) = if let Some(after) = rest.strip_prefix("[ ]") {
        (false, after)
    } else {
        let after = rest
            .strip_prefix("[x]")
            .or_else(|| rest.strip_prefix("[X]"))?;
        (true, after)
    };
    match after.strip_prefix(' ') {
        Some(text) => Some((done, text)),
        None if after.is_empty() => Some((done, "")),
        None => None,
    }
}

/// Counts markdown checkboxes in `source`: `(checked, total)`. Follows the
/// renderer's rules — code fences are skipped, `- ` and `* ` bullets at any
/// indent count, `[x]`/`[X]` are checked.
#[must_use]
pub fn checklist_stats(source: &str) -> (usize, usize) {
    let mut checked = 0;
    let mut total = 0;
    let mut in_code_fence = false;
    for raw in source.lines() {
        if raw.trim_start().starts_with("```") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence {
            continue;
        }
        let trimmed = raw.trim_start();
        if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            if let Some((done, _)) = checkbox(rest) {
                total += 1;
                checked += usize::from(done);
            }
        }
    }
    (checked, total)
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

/// Lightweight, dependency-free syntax highlighting for fenced code. A
/// single-pass tokenizer colours comments, strings, numbers, and a
/// language's keywords; everything else is left plain. Deliberately
/// approximate — good enough to read a command or a snippet, never a full
/// language grammar. Unknown languages fall through to the generic rules
/// (strings + numbers + `#` comments), so nothing is ever worse than plain.
mod highlight {
    use ratatui::style::{Color, Style};
    use ratatui::text::Span;

    /// Highlights one line of code in `lang` into styled spans.
    pub(super) fn code(lang: &str, line: &str) -> Vec<Span<'static>> {
        let kws = keywords(lang);
        let comment = Style::default().fg(Color::DarkGray);
        let string = Style::default().fg(Color::Green);
        let number = Style::default().fg(Color::Yellow);
        let keyword = Style::default().fg(Color::Magenta);
        let plain = Style::default().fg(Color::Gray);

        let chars: Vec<char> = line.chars().collect();
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            // `#` (outside a string) runs a comment to end of line — the
            // comment style for python, bash, and the generic fallback.
            if c == '#' {
                spans.push(Span::styled(chars[i..].iter().collect::<String>(), comment));
                break;
            }
            if c == '"' || c == '\'' {
                let start = i;
                i += 1;
                while i < chars.len() && chars[i] != c {
                    i += 1;
                }
                i = (i + 1).min(chars.len()); // include the closing quote if present
                spans.push(Span::styled(
                    chars[start..i].iter().collect::<String>(),
                    string,
                ));
                continue;
            }
            if is_word(c) {
                let start = i;
                while i < chars.len() && is_word(chars[i]) {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let style = if word.chars().all(|c| c.is_ascii_digit() || c == '.') {
                    number
                } else if kws.contains(&word.as_str()) {
                    keyword
                } else {
                    plain
                };
                spans.push(Span::styled(word, style));
                continue;
            }
            // Operators/punctuation/whitespace: coalesce a run as plain.
            let start = i;
            while i < chars.len() && !is_word(chars[i]) && !matches!(chars[i], '#' | '"' | '\'') {
                i += 1;
            }
            spans.push(Span::styled(
                chars[start..i].iter().collect::<String>(),
                plain,
            ));
        }
        if spans.is_empty() {
            spans.push(Span::styled(String::new(), plain));
        }
        spans
    }

    fn is_word(c: char) -> bool {
        c.is_alphanumeric() || c == '_' || c == '.'
    }

    /// Keywords per language. Approximate sets — the common ones that make a
    /// snippet scannable, not the full grammar.
    fn keywords(lang: &str) -> &'static [&'static str] {
        match lang.to_ascii_lowercase().as_str() {
            "python" | "py" => &[
                "def", "class", "import", "from", "as", "return", "if", "elif", "else", "for",
                "while", "in", "is", "not", "and", "or", "try", "except", "finally", "with",
                "lambda", "yield", "raise", "pass", "break", "continue", "global", "nonlocal",
                "assert", "del", "None", "True", "False", "self", "async", "await",
            ],
            "bash" | "sh" | "shell" | "zsh" | "console" => &[
                "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac",
                "function", "in", "return", "exit", "export", "local", "set", "echo", "cd",
                "source",
            ],
            "rust" | "rs" => &[
                "fn", "let", "mut", "pub", "struct", "enum", "impl", "trait", "use", "mod",
                "match", "if", "else", "for", "while", "loop", "return", "self", "Self", "async",
                "await", "move", "ref", "const", "static", "where", "as", "dyn", "true", "false",
            ],
            _ => &[],
        }
    }
}

#[cfg(test)]
#[path = "tests/markdown.rs"]
mod tests;
