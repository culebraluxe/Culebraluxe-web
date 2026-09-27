//! Pure calendar viewport math for the Yew/MVI calendar.
//!
//! No DOM and no clock reads live here. The server supplies "today"; reducer
//! messages move the cursor; views only render the resulting 42-cell month.

use chrono::{DateTime, Datelike, Duration, NaiveDate};

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

pub fn month_title(cursor: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January", "February", "March", "April", "May", "June",
        "July", "August", "September", "October", "November", "December",
    ];
    month_start(cursor)
        .map(|date| format!("{} {}", MONTHS[date.month0() as usize], date.year()))
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

pub fn time_label(value: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date| date.format("%-I:%M %p").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_grid_is_six_weeks_and_sunday_aligned() {
        let cells = month_cells("2026-09-27");
        assert_eq!(cells.len(), 42);
        assert_eq!(cells.first().unwrap().date, "2026-08-30");
        assert_eq!(cells.last().unwrap().date, "2026-10-10");
        assert_eq!(month_title("2026-09-27"), "September 2026");
    }

    #[test]
    fn shifting_month_clamps_the_day_instead_of_overflowing() {
        assert_eq!(shift_month("2026-01-31", 1), "2026-02-28");
        assert_eq!(shift_month("2024-01-31", 1), "2024-02-29");
        assert_eq!(shift_month("2026-12-15", 1), "2027-01-15");
        assert_eq!(shift_month("2026-01-15", -1), "2025-12-15");
    }
}
