//! A minimal, panic-free ANSI SGR renderer for diagram files.
//!
//! Diagrams are ASCII art whose alignment must survive rendering, so the only
//! sane way to let authors add color and bold is ANSI escape sequences — they
//! are zero-width on screen. This module converts a string containing SGR
//! sequences (`ESC [ … m`) into styled ratatui [`Text`]; every other escape
//! sequence is stripped, and malformed input degrades to plain text rather
//! than erroring.
//!
//! Supported SGR codes: `0` reset, `1` bold, `2` dim, `3` italic,
//! `4` underline, `7` reverse, `9` strikethrough (and `22`/`23`/`24`/`27`/`29`
//! to undo them), `30–37`/`90–97` foreground, `40–47`/`100–107` background,
//! `39`/`49` default fore/background, and `38`/`48` with `5;n` (256-color) or
//! `2;r;g;b` (truecolor) parameters. Unknown codes are ignored.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

const ESC: char = '\u{1b}';

/// Renders `source`, interpreting ANSI SGR sequences as styling.
///
/// Style state carries across newlines (a color set on one line applies to
/// the next until reset), matching how terminals behave.
#[must_use]
pub fn to_text(source: &str) -> Text<'static> {
    let mut style = Style::default();
    let lines: Vec<Line<'static>> = source
        .lines()
        .map(|raw| render_line(raw, &mut style))
        .collect();
    Text::from(lines)
}

fn render_line(raw: &str, style: &mut Style) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut plain = String::new();
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != ESC {
            plain.push(ch);
            continue;
        }
        flush(&mut spans, &mut plain, *style);
        if chars.peek() == Some(&'[') {
            chars.next();
            let mut body = String::new();
            let mut terminator = None;
            for c in chars.by_ref() {
                // CSI sequences end at the first byte in `@`..=`~`.
                if ('\u{40}'..='\u{7e}').contains(&c) {
                    terminator = Some(c);
                    break;
                }
                body.push(c);
            }
            if terminator == Some('m') {
                apply_sgr(&body, style);
            }
            // Any other CSI sequence (cursor moves, clears, …) is stripped;
            // an unterminated one simply consumes the rest of the line.
        }
        // A bare ESC not followed by `[` is dropped.
    }
    flush(&mut spans, &mut plain, *style);
    Line::from(spans)
}

fn flush(spans: &mut Vec<Span<'static>>, plain: &mut String, style: Style) {
    if !plain.is_empty() {
        spans.push(Span::styled(std::mem::take(plain), style));
    }
}

/// Removes every ANSI escape sequence, returning plain text. Used when
/// exporting diagrams to markdown, where raw ESC bytes don't belong.
/// Newlines and alignment are preserved (the codes are zero-width).
#[must_use]
pub fn strip(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != ESC {
            out.push(ch);
            continue;
        }
        if chars.peek() == Some(&'[') {
            chars.next();
            for c in chars.by_ref() {
                if ('\u{40}'..='\u{7e}').contains(&c) {
                    break;
                }
            }
        }
        // A bare ESC is dropped, matching the renderer.
    }
    out
}

fn apply_sgr(body: &str, style: &mut Style) {
    let params: Vec<u16> = if body.is_empty() {
        vec![0] // `ESC[m` is shorthand for reset
    } else {
        body.split(';').map(|p| p.parse().unwrap_or(0)).collect()
    };

    let mut i = 0;
    while i < params.len() {
        match params[i] {
            0 => *style = Style::default(),
            1 => style.add_modifier.insert(Modifier::BOLD),
            2 => style.add_modifier.insert(Modifier::DIM),
            3 => style.add_modifier.insert(Modifier::ITALIC),
            4 => style.add_modifier.insert(Modifier::UNDERLINED),
            7 => style.add_modifier.insert(Modifier::REVERSED),
            9 => style.add_modifier.insert(Modifier::CROSSED_OUT),
            22 => style.add_modifier.remove(Modifier::BOLD | Modifier::DIM),
            23 => style.add_modifier.remove(Modifier::ITALIC),
            24 => style.add_modifier.remove(Modifier::UNDERLINED),
            27 => style.add_modifier.remove(Modifier::REVERSED),
            29 => style.add_modifier.remove(Modifier::CROSSED_OUT),
            p @ 30..=37 => style.fg = Some(basic_color(p - 30)),
            39 => style.fg = None,
            p @ 90..=97 => style.fg = Some(bright_color(p - 90)),
            p @ 40..=47 => style.bg = Some(basic_color(p - 40)),
            49 => style.bg = None,
            p @ 100..=107 => style.bg = Some(bright_color(p - 100)),
            p @ (38 | 48) => {
                let (color, consumed) = extended_color(&params[i + 1..]);
                if let Some(color) = color {
                    if p == 38 {
                        style.fg = Some(color);
                    } else {
                        style.bg = Some(color);
                    }
                }
                i += consumed;
            }
            _ => {} // unknown codes are ignored, never fatal
        }
        i += 1;
    }
}

/// Parses the parameters after a `38`/`48`: `5;n` or `2;r;g;b`. Returns the
/// color (if well-formed) and how many parameters were consumed.
fn extended_color(rest: &[u16]) -> (Option<Color>, usize) {
    match rest {
        [5, n, ..] => (u8::try_from(*n).ok().map(Color::Indexed), 2),
        [2, r, g, b, ..] => {
            let rgb = match (u8::try_from(*r), u8::try_from(*g), u8::try_from(*b)) {
                (Ok(r), Ok(g), Ok(b)) => Some(Color::Rgb(r, g, b)),
                _ => None,
            };
            (rgb, 4)
        }
        // Malformed: consume everything so trailing params are not misread.
        _ => (None, rest.len()),
    }
}

fn basic_color(index: u16) -> Color {
    match index {
        0 => Color::Black,
        1 => Color::Red,
        2 => Color::Green,
        3 => Color::Yellow,
        4 => Color::Blue,
        5 => Color::Magenta,
        6 => Color::Cyan,
        _ => Color::Gray,
    }
}

fn bright_color(index: u16) -> Color {
    match index {
        0 => Color::DarkGray,
        1 => Color::LightRed,
        2 => Color::LightGreen,
        3 => Color::LightYellow,
        4 => Color::LightBlue,
        5 => Color::LightMagenta,
        6 => Color::LightCyan,
        _ => Color::White,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)] // panicking is the correct failure mode in tests

    use super::*;

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn plain_text_passes_through_unchanged() {
        let text = to_text("┌──┐\n│ x│\n└──┘");
        let rendered: Vec<String> = text.lines.iter().map(line_text).collect();
        assert_eq!(rendered, ["┌──┐", "│ x│", "└──┘"]);
    }

    #[test]
    fn sgr_styles_apply_and_reset() {
        let text = to_text("a \u{1b}[1;31mBUG\u{1b}[0m b");
        let spans = &text.lines[0].spans;
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].content.as_ref(), "BUG");
        assert_eq!(spans[1].style.fg, Some(Color::Red));
        assert!(spans[1].style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(spans[2].style, Style::default());
    }

    #[test]
    fn escape_codes_are_zero_width() {
        let source = "│ \u{1b}[32mhealthy\u{1b}[0m │";
        assert_eq!(line_text(&to_text(source).lines[0]), "│ healthy │");
    }

    #[test]
    fn style_persists_across_lines_until_reset() {
        let text = to_text("\u{1b}[33mline one\nline two\u{1b}[0m\nline three");
        assert_eq!(text.lines[1].spans[0].style.fg, Some(Color::Yellow));
        assert_eq!(text.lines[2].spans[0].style.fg, None);
    }

    #[test]
    fn indexed_and_truecolor_parse() {
        let text = to_text("\u{1b}[38;5;196mX\u{1b}[0m\u{1b}[38;2;10;20;30mY");
        let spans = &text.lines[0].spans;
        assert_eq!(spans[0].style.fg, Some(Color::Indexed(196)));
        assert_eq!(spans[1].style.fg, Some(Color::Rgb(10, 20, 30)));
    }

    #[test]
    fn non_sgr_sequences_are_stripped() {
        // Cursor-up and erase-line sequences must vanish without styling.
        assert_eq!(line_text(&to_text("\u{1b}[2Ka\u{1b}[1Ab").lines[0]), "ab");
    }

    #[test]
    fn strip_removes_codes_and_preserves_text_and_newlines() {
        let source = "│ \u{1b}[1;31mBUG\u{1b}[0m │\nplain\n\u{1b}[2Kx";
        assert_eq!(strip(source), "│ BUG │\nplain\nx");
        assert_eq!(strip("no codes"), "no codes");
    }

    #[test]
    fn malformed_input_never_panics() {
        for source in [
            "\u{1b}",
            "\u{1b}[",
            "\u{1b}[31",
            "\u{1b}[;;;m x",
            "\u{1b}[38;5m x",
        ] {
            let _ = to_text(source); // must not panic
        }
        // Unterminated CSI swallows the rest of its line only.
        let text = to_text("\u{1b}[31 lost\nkept");
        assert_eq!(line_text(&text.lines[1]), "kept");
    }
}
