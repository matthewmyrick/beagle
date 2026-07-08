//! Full-frame rendering tests (`ui::view`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use super::*;
use crate::model::Severity;

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
