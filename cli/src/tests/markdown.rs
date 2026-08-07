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
fn fences_become_delimiters_and_code_gets_a_gutter() {
    let text = to_text("```text\nlet x = 1;\n```\nafter\n```\ndangling");
    // Open rule (labelled), code line, close rule, plain, open rule, code.
    assert_eq!(text.lines.len(), 6);
    assert!(
        line_text(&text.lines[0]).starts_with("╭─ text "),
        "open fence shows the language: {}",
        line_text(&text.lines[0])
    );
    assert_eq!(line_text(&text.lines[1]), "│ let x = 1;");
    assert!(line_text(&text.lines[2]).starts_with('╰'), "close rule");
    assert_eq!(line_text(&text.lines[3]), "after");
    assert!(line_text(&text.lines[4]).starts_with('╭'), "second open");
    assert_eq!(line_text(&text.lines[5]), "│ dangling");
}

#[test]
fn python_code_is_syntax_highlighted() {
    let text = to_text("```python\ndef f():  # note\n    return \"hi\"\n```");
    // Line 1 is the `def f():  # note` code line (after the open rule).
    let code = &text.lines[1];
    let find = |needle: &str| {
        code.spans
            .iter()
            .find(|s| s.content.contains(needle))
            .unwrap_or_else(|| panic!("span {needle:?} not found"))
    };
    assert_eq!(
        find("def").style.fg,
        Some(Color::Magenta),
        "keyword coloured"
    );
    assert_eq!(
        find("# note").style.fg,
        Some(Color::DarkGray),
        "comment dim"
    );
    let string_line = &text.lines[2];
    assert!(
        string_line
            .spans
            .iter()
            .any(|s| s.content.contains("\"hi\"") && s.style.fg == Some(Color::Green)),
        "string coloured green"
    );
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

#[test]
fn checkboxes_render_as_glyphs_with_checked_green() {
    let text = to_text("- [ ] open item\n- [x] done item\n- [X] also done");
    let unchecked = &text.lines[0];
    assert_eq!(unchecked.spans[1].content, "☐ ");
    assert_eq!(unchecked.spans[2].content, "open item");
    assert_eq!(
        unchecked.spans[2].style.fg, None,
        "open items keep normal fg"
    );

    let checked = &text.lines[1];
    assert_eq!(checked.spans[1].content, "☑ ");
    assert_eq!(
        checked.spans[2].style.fg,
        Some(Color::Green),
        "done items go green"
    );
    assert_eq!(
        text.lines[2].spans[1].content, "☑ ",
        "[X] counts as checked"
    );
}

#[test]
fn checkbox_requires_a_space_or_end_of_line() {
    let text = to_text("- [x]glued\n- [y] not a box\n- [ ]");
    assert_eq!(
        text.lines[0].spans[1].content, "• ",
        "no space → plain bullet"
    );
    assert_eq!(
        text.lines[1].spans[1].content, "• ",
        "unknown mark → bullet"
    );
    assert_eq!(text.lines[2].spans[1].content, "☐ ", "bare box is a box");
}

#[test]
fn nested_checkboxes_keep_their_indent() {
    let text = to_text("- [ ] parent\n  - [x] child");
    assert_eq!(text.lines[1].spans[0].content, "  ");
    assert_eq!(text.lines[1].spans[1].content, "☑ ");
}

#[test]
fn checklist_stats_counts_boxes_and_skips_code_fences() {
    let source = "\
- [x] done
- [ ] open
  * [X] nested done
- plain bullet
```
- [ ] inside a fence, not a task
```
text";
    assert_eq!(checklist_stats(source), (2, 3));
    assert_eq!(checklist_stats("no boxes here"), (0, 0));
}
