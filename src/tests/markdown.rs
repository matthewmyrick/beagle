//! Tests for `markdown`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;

fn line_text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

#[test]
fn headings_are_bold_and_stripped_of_hashes() {
    let text = to_text("# Title\n## Sub");
    assert_eq!(line_text(&text.lines[0]), "Title");
    assert!(text.lines[0].style.add_modifier.contains(Modifier::BOLD));
    assert_eq!(line_text(&text.lines[1]), "Sub");
}

#[test]
fn bullets_get_a_dot_and_keep_indent() {
    let text = to_text("- top\n  - nested");
    assert_eq!(line_text(&text.lines[0]), "• top");
    assert_eq!(line_text(&text.lines[1]), "  • nested");
}

#[test]
fn fence_markers_are_hidden_and_code_gets_a_gutter() {
    let text = to_text("```text\nlet x = 1;\n```\nafter\n```\ndangling");
    // Three fence lines vanish; three content lines remain.
    assert_eq!(text.lines.len(), 3);
    assert_eq!(line_text(&text.lines[0]), "▍ let x = 1;");
    assert_eq!(line_text(&text.lines[1]), "after");
    assert_eq!(line_text(&text.lines[2]), "▍ dangling");
}

#[test]
fn inline_bold_and_code_are_split_into_spans() {
    let text = to_text("a **b** and `c`");
    let spans = &text.lines[0].spans;
    let contents: Vec<&str> = spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(contents, ["a ", "b", " and ", "c"]);
    assert!(spans[1].style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn unbalanced_markers_render_literally() {
    let text = to_text("oops ** not bold\nstray ` tick");
    assert_eq!(line_text(&text.lines[0]), "oops ** not bold");
    assert_eq!(line_text(&text.lines[1]), "stray ` tick");
}

#[test]
fn blockquote_and_rule_render() {
    let text = to_text("> hint\n---");
    assert_eq!(line_text(&text.lines[0]), "│ hint");
    assert!(line_text(&text.lines[1]).starts_with('─'));
}

#[test]
fn empty_input_is_empty_output() {
    assert!(to_text("").lines.is_empty());
}
