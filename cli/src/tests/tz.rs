//! Tests for display time zones (`tz`).
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use time::macros::{datetime, offset};

use super::*;

fn local(tz: &str, t: OffsetDateTime) -> (OffsetDateTime, String) {
    let zone = TimeZone::parse(tz).expect("parses");
    let (at, abbr) = zone.localize(t);
    (at, abbr.to_owned())
}

#[test]
fn utc_is_the_default_and_a_no_op() {
    let t = datetime!(2026-10-22 13:05 UTC);
    let zone = TimeZone::default();
    let (at, abbr) = zone.localize(t);
    assert_eq!((at, abbr), (t, "UTC"));
}

#[test]
fn eastern_switches_between_est_and_edt() {
    let (summer, abbr) = local("America/New_York", datetime!(2026-10-22 13:05 UTC));
    assert_eq!(abbr, "EDT");
    assert_eq!(summer.offset(), offset!(-4));
    assert_eq!((summer.hour(), summer.minute()), (9, 5));

    let (winter, abbr) = local("EST", datetime!(2026-12-01 13:05 UTC));
    assert_eq!(abbr, "EST");
    assert_eq!(winter.offset(), offset!(-5));
}

#[test]
fn us_transitions_land_at_2am_local() {
    // 2026: DST starts Sun Mar 8, 02:00 EST (07:00 UTC) and ends Sun Nov 1,
    // 02:00 EDT (06:00 UTC).
    assert_eq!(local("EST", datetime!(2026-03-08 06:59 UTC)).1, "EST");
    assert_eq!(local("EST", datetime!(2026-03-08 07:00 UTC)).1, "EDT");
    assert_eq!(local("EST", datetime!(2026-11-01 05:59 UTC)).1, "EDT");
    assert_eq!(local("EST", datetime!(2026-11-01 06:00 UTC)).1, "EST");
}

#[test]
fn last_week_rule_and_custom_transition_time() {
    // London: last Sunday of March at 01:00 GMT; 2026-03-29.
    assert_eq!(
        local("Europe/London", datetime!(2026-03-29 00:59 UTC)).1,
        "GMT"
    );
    assert_eq!(
        local("Europe/London", datetime!(2026-03-29 01:00 UTC)).1,
        "BST"
    );
}

#[test]
fn southern_hemisphere_dst_wraps_the_year() {
    // Sydney: AEDT from first Sunday of October to first Sunday of April.
    let rule = "AEST-10AEDT,M10.1.0,M4.1.0/3";
    let (jan, abbr) = local(rule, datetime!(2026-01-15 00:00 UTC));
    assert_eq!(abbr, "AEDT");
    assert_eq!(jan.offset(), offset!(+11));
    assert_eq!(local(rule, datetime!(2026-07-15 00:00 UTC)).1, "AEST");
}

#[test]
fn fixed_and_quoted_offsets_parse() {
    let (at, abbr) = local("<+0530>-5:30", datetime!(2026-01-01 00:00 UTC));
    assert_eq!(abbr, "+0530");
    assert_eq!(at.offset(), offset!(+5:30));
    assert_eq!(local("utc", datetime!(2026-01-01 00:00 UTC)).1, "UTC");
}

#[test]
fn garbage_is_rejected_with_the_input_named() {
    for bad in [
        "",
        "Mars/Olympus",
        "EST5EDT",
        "EST5EDT,M13.1.0,M11.1.0",
        "E5",
        "EST5EDT,J60,M11.1.0",
    ] {
        let err = TimeZone::parse(bad).expect_err(bad);
        assert!(err.contains("unknown timezone"), "{bad}: {err}");
    }
}
