//! Pure calendar viewport and interaction math for the Yew/MVI calendar.
//!
//! Named timezone identity is intentionally deferred. For this functional pass
//! generated drag/resize targets use explicit UTC RFC3339 instants; imported
//! provider instants remain untouched.

use chrono::{DateTime, Datelike, Duration, NaiveDate, Timelike, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridSpec {
    pub start_hour: u32,
    pub end_hour: u32,
    pub slot_minutes: i64,
}

impl GridSpec {
    pub fn new(start_hour: u32, end_hour: u32, slot_minutes: u32) -> Self {
        let (start_hour, end_hour) = if start_hour < end_hour && end_hour <= 24 {
            (start_hour, end_hour)
        } else {
            (8, 20)
        };
        let slot_minutes = match slot_minutes {
            15 | 30 | 60 => i64::from(slot_minutes),
            _ => 30,
        };
        Self {
            start_hour,
            end_hour,
            slot_minutes,
        }
    }

    pub fn slot_count(self) -> usize {
        (((self.end_hour - self.start_hour) as i64 * 60) / self.slot_minutes)
            .try_into()
            .unwrap_or(24)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthCell {
    pub date: String,
    pub day: u32,
    pub in_month: bool,
}

pub fn date_key(value: &str) -> Option<String> {
    let prefix = value.get(0..10)?;
    NaiveDate::parse_from_str(prefix, "%Y-%m-%d")
        .ok()
        .map(|date| date.format("%Y-%m-%d").to_string())
}

fn cursor_date(cursor: &str) -> Option<NaiveDate> {
    date_key(cursor).and_then(|value| NaiveDate::parse_from_str(&value, "%Y-%m-%d").ok())
}

fn midnight(date: NaiveDate) -> Option<String> {
    date.and_hms_opt(0, 0, 0)
        .map(|value| value.and_utc().to_rfc3339())
}

fn month_start(cursor: &str) -> Option<NaiveDate> {
    let date = cursor_date(cursor)?;
    NaiveDate::from_ymd_opt(date.year(), date.month(), 1)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let next = NaiveDate::from_ymd_opt(next_year, next_month, 1).expect("valid next month");
    (next - Duration::days(1)).day()
}

pub fn shift_month(cursor: &str, delta: i32) -> String {
    let Some(date) = cursor_date(cursor) else {
        return cursor.to_owned();
    };
    let month_index = date.year() * 12 + date.month0() as i32 + delta;
    let year = month_index.div_euclid(12);
    let month0 = month_index.rem_euclid(12) as u32;
    let month = month0 + 1;
    let day = date.day().min(days_in_month(year, month));
    NaiveDate::from_ymd_opt(year, month, day)
        .expect("shifted calendar month is valid")
        .format("%Y-%m-%d")
        .to_string()
}

pub fn shift_cursor(cursor: &str, mode: &str, delta: i32) -> String {
    if mode == "month" {
        return shift_month(cursor, delta);
    }
    let Some(date) = cursor_date(cursor) else {
        return cursor.to_owned();
    };
    let days = match mode {
        "week" | "list" => 7,
        _ => 1,
    };
    (date + Duration::days(i64::from(delta) * days))
        .format("%Y-%m-%d")
        .to_string()
}

pub fn viewport_bounds(cursor: &str, mode: &str) -> Option<(String, String)> {
    match mode {
        "month" => {
            let cells = month_cells(cursor);
            let start = cursor_date(&cells.first()?.date)?;
            let end = cursor_date(&cells.last()?.date)? + Duration::days(1);
            Some((midnight(start)?, midnight(end)?))
        }
        "week" | "list" => {
            let dates = week_dates(cursor);
            let start = cursor_date(dates.first()?)?;
            let end = cursor_date(dates.last()?)? + Duration::days(1);
            Some((midnight(start)?, midnight(end)?))
        }
        _ => {
            let start = cursor_date(cursor)?;
            Some((midnight(start)?, midnight(start + Duration::days(1))?))
        }
    }
}

pub fn month_title(cursor: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    month_start(cursor)
        .map(|date| format!("{} {}", MONTHS[date.month0() as usize], date.year()))
        .unwrap_or_else(|| "Calendar".into())
}

pub fn date_title(cursor: &str) -> String {
    cursor_date(cursor)
        .map(|date| date.format("%A, %B %-d, %Y").to_string())
        .unwrap_or_else(|| "Calendar".into())
}

pub fn month_cells(cursor: &str) -> Vec<MonthCell> {
    let Some(first) = month_start(cursor) else {
        return Vec::new();
    };
    let start = first - Duration::days(first.weekday().num_days_from_sunday() as i64);
    (0..42)
        .map(|offset| {
            let date = start + Duration::days(offset);
            MonthCell {
                date: date.format("%Y-%m-%d").to_string(),
                day: date.day(),
                in_month: date.month() == first.month() && date.year() == first.year(),
            }
        })
        .collect()
}

pub fn week_dates(cursor: &str) -> Vec<String> {
    let Some(date) = cursor_date(cursor) else {
        return Vec::new();
    };
    let sunday = date - Duration::days(date.weekday().num_days_from_sunday() as i64);
    (0..7)
        .map(|offset| {
            (sunday + Duration::days(offset))
                .format("%Y-%m-%d")
                .to_string()
        })
        .collect()
}

pub fn week_title(cursor: &str) -> String {
    let dates = week_dates(cursor);
    let Some(first) = dates.first().and_then(|value| cursor_date(value)) else {
        return "Calendar".into();
    };
    let Some(last) = dates.last().and_then(|value| cursor_date(value)) else {
        return "Calendar".into();
    };
    if first.year() == last.year() && first.month() == last.month() {
        format!(
            "{} {}–{}, {}",
            first.format("%B"),
            first.day(),
            last.day(),
            first.year()
        )
    } else if first.year() == last.year() {
        format!(
            "{} {} – {} {}, {}",
            first.format("%b"),
            first.day(),
            last.format("%b"),
            last.day(),
            first.year()
        )
    } else {
        format!(
            "{} {}, {} – {} {}, {}",
            first.format("%b"),
            first.day(),
            first.year(),
            last.format("%b"),
            last.day(),
            last.year()
        )
    }
}

pub fn time_label(value: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date| date.format("%-I:%M %p").to_string())
}

pub fn day_heading(value: &str) -> String {
    cursor_date(value)
        .map(|date| date.format("%a %-m/%-d").to_string())
        .unwrap_or_else(|| value.to_owned())
}

pub fn midnight_timestamp(date: &str) -> Option<String> {
    midnight(cursor_date(date)?)
}

pub fn slot_label(index: usize, grid: GridSpec) -> String {
    let total = i64::from(grid.start_hour) * 60 + index as i64 * grid.slot_minutes;
    let hour = (total / 60) as u32;
    let minute = (total % 60) as u32;
    let suffix = if hour < 12 { "AM" } else { "PM" };
    let display = match hour % 12 {
        0 => 12,
        value => value,
    };
    format!("{display}:{minute:02} {suffix}")
}

pub fn slot_timestamp(date: &str, index: usize, grid: GridSpec) -> Option<String> {
    let date = cursor_date(date)?;
    let total = i64::from(grid.start_hour) * 60 + index as i64 * grid.slot_minutes;
    let hour = u32::try_from(total / 60).ok()?;
    let minute = u32::try_from(total % 60).ok()?;
    date.and_hms_opt(hour, minute, 0)
        .map(|value| value.and_utc().to_rfc3339())
}

pub fn add_minutes(value: &str, minutes: i64) -> Option<String> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date| (date + Duration::minutes(minutes)).to_rfc3339())
}

pub fn move_span(start: &str, end: Option<&str>, target_start: &str) -> Option<(String, String)> {
    let old_start = DateTime::parse_from_rfc3339(start).ok()?;
    let duration = end
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|old_end| old_end - old_start)
        .filter(|duration| *duration > Duration::zero())
        .unwrap_or_else(|| Duration::minutes(60));
    let new_start = DateTime::parse_from_rfc3339(target_start).ok()?;
    Some((new_start.to_rfc3339(), (new_start + duration).to_rfc3339()))
}

pub fn move_to_timed(
    start: &str,
    end: Option<&str>,
    was_all_day: bool,
    target_start: &str,
) -> Option<(String, String)> {
    if was_all_day {
        let target = DateTime::parse_from_rfc3339(target_start).ok()?;
        return Some((
            target.to_rfc3339(),
            (target + Duration::hours(1)).to_rfc3339(),
        ));
    }
    move_span(start, end, target_start)
}

pub fn move_to_all_day(
    start: &str,
    end: Option<&str>,
    was_all_day: bool,
    target_date: &str,
) -> Option<(String, String)> {
    let target = cursor_date(target_date)?;
    let days = if was_all_day {
        let old_start = DateTime::parse_from_rfc3339(start).ok()?;
        end.and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|old_end| (old_end - old_start).num_days())
            .unwrap_or(1)
            .max(1)
    } else {
        1
    };
    Some((
        midnight(target)?,
        midnight(target + Duration::days(days))?,
    ))
}

pub fn resize_span(
    start: &str,
    target_slot: &str,
    grid: GridSpec,
) -> Option<(String, String)> {
    let start = DateTime::parse_from_rfc3339(start).ok()?;
    let slot = DateTime::parse_from_rfc3339(target_slot).ok()?;
    let end = slot + Duration::minutes(grid.slot_minutes);
    if end <= start {
        return None;
    }
    Some((start.to_rfc3339(), end.to_rfc3339()))
}

pub fn event_slot(value: &str, grid: GridSpec) -> Option<usize> {
    let date = DateTime::parse_from_rfc3339(value).ok()?;
    let minutes = i64::from(date.hour()) * 60 + i64::from(date.minute());
    let start = i64::from(grid.start_hour) * 60;
    let end = i64::from(grid.end_hour) * 60;
    if minutes < start || minutes >= end {
        return None;
    }
    usize::try_from((minutes - start) / grid.slot_minutes).ok()
}

pub fn event_span_slots(start: &str, end: Option<&str>, grid: GridSpec) -> usize {
    let Some(start) = DateTime::parse_from_rfc3339(start).ok() else {
        return 2;
    };
    let duration = end
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|end| end - start)
        .filter(|duration| *duration > Duration::zero())
        .unwrap_or_else(|| Duration::minutes(60));
    let minutes = duration.num_minutes().max(grid.slot_minutes);
    usize::try_from((minutes + grid.slot_minutes - 1) / grid.slot_minutes)
        .unwrap_or(1)
        .clamp(1, grid.slot_count())
}

pub fn now_utc_date() -> String {
    Utc::now().date_naive().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> GridSpec {
        GridSpec::new(8, 20, 30)
    }

    #[test]
    fn month_grid_is_six_weeks_and_sunday_aligned() {
        let cells = month_cells("2026-09-27");
        assert_eq!(cells.len(), 42);
        assert_eq!(cells.first().unwrap().date, "2026-08-30");
        assert_eq!(cells.last().unwrap().date, "2026-10-10");
        assert_eq!(month_title("2026-09-27"), "September 2026");
        assert_eq!(
            viewport_bounds("2026-09-27", "month").unwrap(),
            (
                "2026-08-30T00:00:00+00:00".into(),
                "2026-10-11T00:00:00+00:00".into()
            )
        );
    }

    #[test]
    fn shifting_month_clamps_the_day_instead_of_overflowing() {
        assert_eq!(shift_month("2026-01-31", 1), "2026-02-28");
        assert_eq!(shift_month("2024-01-31", 1), "2024-02-29");
        assert_eq!(shift_month("2026-12-15", 1), "2027-01-15");
        assert_eq!(shift_month("2026-01-15", -1), "2025-12-15");
    }

    #[test]
    fn week_and_day_navigation_share_one_cursor() {
        assert_eq!(shift_cursor("2026-09-27", "week", 1), "2026-10-04");
        assert_eq!(shift_cursor("2026-09-27", "day", -1), "2026-09-26");
        assert_eq!(week_dates("2026-09-30").first().unwrap(), "2026-09-27");
    }

    #[test]
    fn grid_preference_and_drag_math_are_deterministic() {
        let grid = grid();
        assert_eq!(grid.slot_count(), 24);
        let target = slot_timestamp("2026-09-28", 2, grid).unwrap();
        let (start, end) = move_span(
            "2026-09-27T09:00:00+00:00",
            Some("2026-09-27T10:30:00+00:00"),
            &target,
        )
        .unwrap();
        assert_eq!(start, "2026-09-28T09:00:00+00:00");
        assert_eq!(end, "2026-09-28T10:30:00+00:00");

        let (_, resized) =
            resize_span(&start, &slot_timestamp("2026-09-28", 5, grid).unwrap(), grid).unwrap();
        assert_eq!(resized, "2026-09-28T11:00:00+00:00");
    }

    #[test]
    fn moving_between_all_day_and_timed_has_explicit_semantics() {
        let target = slot_timestamp("2026-09-28", 4, grid()).unwrap();
        let timed = move_to_timed(
            "2026-09-27T00:00:00+00:00",
            Some("2026-09-28T00:00:00+00:00"),
            true,
            &target,
        )
        .unwrap();
        assert_eq!(timed.1, "2026-09-28T11:00:00+00:00");

        let all_day = move_to_all_day(
            "2026-09-27T09:00:00+00:00",
            Some("2026-09-27T10:00:00+00:00"),
            false,
            "2026-09-29",
        )
        .unwrap();
        assert_eq!(all_day.0, "2026-09-29T00:00:00+00:00");
        assert_eq!(all_day.1, "2026-09-30T00:00:00+00:00");
    }
}
