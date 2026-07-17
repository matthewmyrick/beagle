//! Tests for `ansi`.
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
