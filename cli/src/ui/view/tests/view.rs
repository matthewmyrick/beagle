//! Full-frame rendering tests (`ui::view`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;
use crate::model::Severity;

fn sample_summary(severity: Severity) -> crate::model::RcaSummary {
    crate::model::RcaSummary {
        id: crate::model::RcaId::new("row-check").expect("id"),
        meta: crate::model::RcaMeta {
            title: "Row check".to_owned(),
            severity,
            status: crate::model::Status::Review,
            created: time::OffsetDateTime::from_unix_timestamp(1_780_000_000).expect("ts"),
            updated: None,
            systems: Vec::new(),
            tags: Vec::new(),
            prs: Vec::new(),
        },
        archived: false,
    }
}

/// The regression from #23: the List-wide `highlight_style` used to patch its
/// background over the severity badge, so the selected row lost its
/// HIGH/MED coloring and looked *un*selected.
#[test]
fn selected_row_keeps_the_badge_background_and_tints_the_rest() {
    let rca = sample_summary(Severity::High);

    let selected = rca_list_item(&rca, 0, false, true, None);
    let badge = &selected[0].spans[0];
    assert_eq!(
        badge.style.bg,
        Some(Color::LightRed),
        "badge keeps its own background when selected"
    );
    let title = &selected[0].spans[2];
    assert_eq!(title.style.bg, Some(SELECTED_BG), "title gets the row tint");
    for line in &selected {
        let width: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(
            width,
            usize::from(SIDEBAR_WIDTH) - 2,
            "row padded to full width so the tint has no gaps"
        );
        assert_eq!(
            line.spans.last().expect("filler").style.bg,
            Some(SELECTED_BG),
            "filler carries the tint"
        );
    }

    let unselected = rca_list_item(&rca, 0, false, false, None);
    assert_eq!(
        unselected[0].spans[0].style.bg,
        Some(Color::LightRed),
        "badge identical when unselected"
    );
    assert_eq!(
        unselected[0].spans[2].style.bg, None,
        "no tint off-selection"
    );
}

/// Checklist progress renders in the detail line — green once complete,
/// absent when the workspace has no checkboxes.
#[test]
fn sidebar_detail_line_shows_checklist_progress() {
    let rca = sample_summary(Severity::High);

    let partial = rca_list_item(&rca, 0, false, false, Some((2, 7)));
    let detail: String = partial[1]
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect();
    assert!(
        detail.contains("☑ 2/7"),
        "progress in detail line: {detail}"
    );

    let complete = rca_list_item(&rca, 0, false, false, Some((4, 4)));
    let progress_span = complete[1]
        .spans
        .iter()
        .find(|s| s.content.contains("4/4"))
        .expect("progress span");
    assert_eq!(progress_span.style.fg, Some(Color::LightGreen));

    let none = rca_list_item(&rca, 0, false, false, None);
    let detail: String = none[1].spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(!detail.contains('☑'), "no progress without checkboxes");
}

/// Renders a full frame into a test backend and checks every banner art
/// row occupies the same columns — the regression that motivated
/// `banner_rect`: per-line right-alignment staggered the art.
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
