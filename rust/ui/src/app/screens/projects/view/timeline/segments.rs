//! The timeline header's bands: one segment per day, week or month, and the `HeaderSegment` shape the
//! parent's header row draws them from.
//!
//! Split out of `timeline.rs` on 2026-09-28 (the file was 811 lines, nine over the 800-line rule). The
//! seam is the header: this is the only code that turns a range into the little labelled boxes above the
//! grid, and the parent only ever asks it for a `Vec<HeaderSegment>` and prints them.

use chrono::{Datelike, Duration, NaiveDate, Weekday};

use crate::timeline::{self, TimelineRange, TimelineSpec};

/// One band in the header: where it starts, how wide it is, and what it says.
pub(super) struct HeaderSegment {
    pub(super) left: i64,
    pub(super) width: i64,
    pub(super) label: String,
}

pub(super) fn header_segments(
    range: TimelineRange,
    spec: TimelineSpec,
    mode: &str,
) -> Vec<HeaderSegment> {
    match mode {
        "month" => month_segments(range, spec),
        "day" => timeline::days(range)
            .into_iter()
            .map(|date| HeaderSegment {
                left: timeline::x(date, range, spec),
                width: spec.pixels_per_day,
                label: date.format("%a %-d").to_string(),
            })
            .collect(),
        _ => week_segments(range, spec),
    }
}

fn week_segments(range: TimelineRange, spec: TimelineSpec) -> Vec<HeaderSegment> {
    let mut out = Vec::new();
    let mut start = range.start;
    while start < range.end {
        let end = (start + Duration::days(7)).min(range.end);
        out.push(HeaderSegment {
            left: timeline::x(start, range, spec),
            width: (end - start).num_days() * spec.pixels_per_day,
            label: format!("{} {}", start.format("%b"), start.day()),
        });
        start = end;
    }
    out
}

fn month_segments(range: TimelineRange, spec: TimelineSpec) -> Vec<HeaderSegment> {
    let mut out = Vec::new();
    let mut start = range.start;
    while start < range.end {
        let next_month = if start.month() == 12 {
            NaiveDate::from_ymd_opt(start.year() + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(start.year(), start.month() + 1, 1)
        }
        .expect("next month is valid");
        let end = next_month.min(range.end);
        out.push(HeaderSegment {
            left: timeline::x(start, range, spec),
            width: (end - start).num_days() * spec.pixels_per_day,
            label: start.format("%b %Y").to_string(),
        });
        start = end;
    }
    out
}

#[allow(dead_code)]
fn _is_weekend(date: NaiveDate) -> bool {
    matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
}
