//! Pure timeline/Gantt projection math.
//!
//! This module deliberately renders only facts the current WBS owns. A WBS
//! `due_at` is a deadline, not a task start, so leaves are milestones and parent
//! rows summarize descendant deadline spans. Planned durations are a separate
//! canonical WBS capability and must not be invented by the view.

use chrono::{Duration, NaiveDate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineSpec {
    pub key: &'static str,
    pub pixels_per_day: i64,
    pub margin_before: i64,
    pub margin_after: i64,
}

pub fn spec(mode: &str) -> TimelineSpec {
    match mode {
        "day" => TimelineSpec {
            key: "day",
            pixels_per_day: 34,
            margin_before: 7,
            margin_after: 14,
        },
        "month" => TimelineSpec {
            key: "month",
            pixels_per_day: 8,
            margin_before: 31,
            margin_after: 62,
        },
        _ => TimelineSpec {
            key: "week",
            pixels_per_day: 16,
            margin_before: 14,
            margin_after: 28,
        },
    }
}

pub fn date(value: &str) -> Option<NaiveDate> {
    let prefix = value.get(0..10)?;
    NaiveDate::parse_from_str(prefix, "%Y-%m-%d").ok()
}

pub fn midnight_utc(value: &str) -> Option<String> {
    let date = date(value)?;
    date.and_hms_opt(0, 0, 0)
        .map(|value| value.and_utc().to_rfc3339())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineRange {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

impl TimelineRange {
    pub fn days(self) -> i64 {
        (self.end - self.start).num_days().max(1)
    }

    pub fn width(self, spec: TimelineSpec) -> i64 {
        self.days() * spec.pixels_per_day
    }
}

pub fn range<'a>(
    values: impl IntoIterator<Item = &'a str>,
    fallback: &str,
    mode: &str,
) -> TimelineRange {
    let spec = spec(mode);
    let mut dates = values.into_iter().filter_map(date);
    let fallback = date(fallback)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).expect("the epoch is a valid date"));
    let Some(first) = dates.next() else {
        return TimelineRange {
            start: fallback - Duration::days(spec.margin_before),
            end: fallback + Duration::days(spec.margin_after),
        };
    };

    let mut min = first;
    let mut max = first;
    for value in dates {
        min = min.min(value);
        max = max.max(value);
    }

    TimelineRange {
        start: min - Duration::days(spec.margin_before),
        end: max + Duration::days(spec.margin_after + 1),
    }
}

pub fn x(value: NaiveDate, range: TimelineRange, spec: TimelineSpec) -> i64 {
    (value - range.start).num_days() * spec.pixels_per_day
}

pub fn width_between(
    start: NaiveDate,
    end: NaiveDate,
    range: TimelineRange,
    spec: TimelineSpec,
) -> i64 {
    let start_x = x(start, range, spec);
    let end_x = x(end, range, spec);
    (end_x - start_x).abs().max(spec.pixels_per_day)
}

pub fn days(range: TimelineRange) -> Vec<NaiveDate> {
    (0..range.days())
        .map(|offset| range.start + Duration::days(offset))
        .collect()
}

pub fn progress(status: &str) -> i64 {
    match status {
        "done" | "dismissed" => 100,
        "doing" => 60,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_timeline_is_centered_on_the_server_supplied_day() {
        let range = range(std::iter::empty(), "2026-09-27", "week");
        assert_eq!(range.start.to_string(), "2026-09-13");
        assert_eq!(range.end.to_string(), "2026-10-25");
        assert_eq!(range.width(spec("week")), 42 * 16);
    }

    #[test]
    fn timeline_range_contains_real_dates_without_inventing_duration() {
        let range = range(
            ["2026-09-10T00:00:00Z", "2026-10-02T00:00:00Z"],
            "2026-09-27",
            "day",
        );
        assert_eq!(range.start.to_string(), "2026-09-03");
        assert_eq!(range.end.to_string(), "2026-10-17");
        assert_eq!(x(date("2026-09-10").unwrap(), range, spec("day")), 7 * 34);
    }

    #[test]
    fn due_dates_round_trip_as_date_semantics() {
        assert_eq!(
            midnight_utc("2026-11-03").as_deref(),
            Some("2026-11-03T00:00:00+00:00")
        );
    }
}
