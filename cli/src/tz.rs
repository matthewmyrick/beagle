//! Display time zones for timestamps shown in the UI.
//!
//! Everything on disk stays UTC; this only converts for display. `time`
//! ships no zone database and reading the OS's local offset is unsound
//! once threads exist, so zones are POSIX TZ rules (`EST5EDT,M3.2.0,M11.1.0`)
//! — the same rule tzdata itself uses for current and future transitions —
//! plus a short table of common IANA names that map onto them.

use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset, Weekday};

/// Common IANA names (and colloquial abbreviations) → POSIX TZ rules.
/// Anything else must be a POSIX rule string itself.
const ALIASES: [(&[&str], &str); 7] = [
    (&["UTC", "GMT", "Etc/UTC", "Z"], "UTC0"),
    (
        &["America/New_York", "US/Eastern", "Eastern", "EST", "EDT"],
        "EST5EDT,M3.2.0,M11.1.0",
    ),
    (
        &["America/Chicago", "US/Central", "Central", "CST", "CDT"],
        "CST6CDT,M3.2.0,M11.1.0",
    ),
    (
        &["America/Denver", "US/Mountain", "Mountain", "MST", "MDT"],
        "MST7MDT,M3.2.0,M11.1.0",
    ),
    (
        &["America/Los_Angeles", "US/Pacific", "Pacific", "PST", "PDT"],
        "PST8PDT,M3.2.0,M11.1.0",
    ),
    (&["Europe/London", "BST"], "GMT0BST,M3.5.0/1,M10.5.0"),
    (
        &[
            "Europe/Paris",
            "Europe/Berlin",
            "Europe/Amsterdam",
            "CET",
            "CEST",
        ],
        "CET-1CEST,M3.5.0,M10.5.0/3",
    ),
];

/// A display time zone: a standard offset, optionally with a DST rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeZone {
    std_abbr: String,
    std_offset: UtcOffset,
    dst: Option<Dst>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Dst {
    abbr: String,
    offset: UtcOffset,
    start: Rule,
    end: Rule,
}

/// `Mm.w.d[/time]`: weekday `d` (0 = Sunday) of week `w` (5 = last) of
/// month `m`, at `time` seconds past local midnight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rule {
    month: Month,
    week: u8,
    weekday: Weekday,
    secs: i64,
}

impl Default for TimeZone {
    fn default() -> Self {
        Self::utc()
    }
}

impl TimeZone {
    /// Plain UTC — the default.
    #[must_use]
    pub fn utc() -> Self {
        Self {
            std_abbr: "UTC".to_owned(),
            std_offset: UtcOffset::UTC,
            dst: None,
        }
    }

    /// Parses an alias (`America/New_York`, `EST`, `UTC`, ...) or a POSIX TZ
    /// rule (`EST5EDT,M3.2.0,M11.1.0`, `<+0530>-5:30`).
    ///
    /// # Errors
    /// A message naming the bad input when it is neither.
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();
        let rule = ALIASES
            .iter()
            .find(|(names, _)| names.iter().any(|n| n.eq_ignore_ascii_case(s)))
            .map_or(s, |(_, rule)| rule);
        parse_posix(rule).ok_or_else(|| {
            format!(
                "unknown timezone `{s}` (use e.g. America/New_York, UTC, or a POSIX \
                 rule like EST5EDT,M3.2.0,M11.1.0)"
            )
        })
    }

    /// `t` converted to this zone, with the abbreviation in effect then.
    #[must_use]
    pub fn localize(&self, t: OffsetDateTime) -> (OffsetDateTime, &str) {
        match &self.dst {
            Some(dst) if dst.contains(t, self.std_offset) => {
                (t.to_offset(dst.offset), dst.abbr.as_str())
            }
            _ => (t.to_offset(self.std_offset), self.std_abbr.as_str()),
        }
    }
}

impl Dst {
    /// Whether DST is in effect at `t`. Transitions are written in the
    /// local time in force just before them: standard time for the start,
    /// DST for the end.
    fn contains(&self, t: OffsetDateTime, std: UtcOffset) -> bool {
        let year = t.to_offset(std).year();
        let (Some(start), Some(end)) = (self.start.at(year, std), self.end.at(year, self.offset))
        else {
            return false;
        };
        if start < end {
            start <= t && t < end
        } else {
            // Southern hemisphere: DST spans the new year.
            t >= start || t < end
        }
    }
}

impl Rule {
    /// The UTC instant this rule fires in `year`, for a local clock at
    /// `offset`.
    fn at(self, year: i32, offset: UtcOffset) -> Option<OffsetDateTime> {
        let first = Date::from_calendar_date(year, self.month, 1).ok()?;
        let lead = (7 + self.weekday.number_days_from_sunday()
            - first.weekday().number_days_from_sunday())
            % 7;
        let mut day = 1 + lead + 7 * (self.week - 1);
        let len = self.month.length(year);
        while day > len {
            day -= 7; // week 5 = "last", which may be the 4th
        }
        let date = Date::from_calendar_date(year, self.month, day).ok()?;
        let local =
            PrimitiveDateTime::new(date, Time::MIDNIGHT) + time::Duration::seconds(self.secs);
        Some(local.assume_offset(offset))
    }
}

/// Parses a POSIX TZ string: `std offset [dst [offset] ,start[/time],end[/time]]`.
fn parse_posix(s: &str) -> Option<TimeZone> {
    let mut rest = s;
    let std_abbr = take_abbr(&mut rest)?;
    let std_offset = take_offset(&mut rest)?;
    if rest.is_empty() {
        return Some(TimeZone {
            std_abbr,
            std_offset,
            dst: None,
        });
    }
    let dst_abbr = take_abbr(&mut rest)?;
    let dst_offset = if rest.starts_with(',') {
        UtcOffset::from_whole_seconds(std_offset.whole_seconds() + 3600).ok()?
    } else {
        take_offset(&mut rest)?
    };
    let mut rules = rest.strip_prefix(',')?.split(',');
    let start = parse_rule(rules.next()?)?;
    let end = parse_rule(rules.next()?)?;
    if rules.next().is_some() {
        return None;
    }
    Some(TimeZone {
        std_abbr,
        std_offset,
        dst: Some(Dst {
            abbr: dst_abbr,
            offset: dst_offset,
            start,
            end,
        }),
    })
}

/// `EST` (3+ letters) or `<+0530>` (quoted, may hold digits and signs).
fn take_abbr(rest: &mut &str) -> Option<String> {
    let (abbr, tail) = if let Some(quoted) = rest.strip_prefix('<') {
        let close = quoted.find('>')?;
        (&quoted[..close], &quoted[close + 1..])
    } else {
        let len = rest
            .find(|c: char| !c.is_ascii_alphabetic())
            .unwrap_or(rest.len());
        rest.split_at(len)
    };
    *rest = tail;
    (abbr.len() >= 3).then(|| abbr.to_owned())
}

/// A POSIX offset (`5`, `-5:30`, `+0`), which counts hours *west* of UTC —
/// the opposite sign of a [`UtcOffset`].
fn take_offset(rest: &mut &str) -> Option<UtcOffset> {
    let len = rest
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, ':' | '+' | '-')))
        .unwrap_or(rest.len());
    let (text, tail) = rest.split_at(len);
    *rest = tail;
    let west = parse_clock(text)?;
    UtcOffset::from_whole_seconds(i32::try_from(-west).ok()?).ok()
}

/// `[+-]hh[:mm[:ss]]` as signed seconds.
fn parse_clock(text: &str) -> Option<i64> {
    let (sign, digits) = match text.as_bytes().first()? {
        b'-' => (-1, &text[1..]),
        b'+' => (1, &text[1..]),
        _ => (1, text),
    };
    let mut parts = digits.split(':');
    let mut secs = 0;
    for unit in [3600, 60, 1] {
        match parts.next() {
            Some(p) if !p.is_empty() && p.len() <= 3 => secs += p.parse::<i64>().ok()? * unit,
            Some(_) => return None,
            None => break,
        }
    }
    if parts.next().is_some() || secs > 167 * 3600 {
        return None;
    }
    Some(sign * secs)
}

/// `Mm.w.d[/time]`. The Julian-day forms (`Jn`, `n`) aren't supported —
/// no zone in use today needs them.
fn parse_rule(s: &str) -> Option<Rule> {
    let (date, time) = s.split_once('/').unwrap_or((s, "2"));
    let mut fields = date.strip_prefix('M')?.split('.');
    let month: u8 = fields.next()?.parse().ok()?;
    let week: u8 = fields.next()?.parse().ok()?;
    let weekday: u8 = fields.next()?.parse().ok()?;
    if fields.next().is_some() || !(1..=5).contains(&week) || weekday > 6 {
        return None;
    }
    Some(Rule {
        month: Month::try_from(month).ok()?,
        week,
        weekday: Weekday::Sunday.nth_next(weekday),
        secs: parse_clock(time)?,
    })
}

#[cfg(test)]
#[path = "tests/tz.rs"]
mod tests;
