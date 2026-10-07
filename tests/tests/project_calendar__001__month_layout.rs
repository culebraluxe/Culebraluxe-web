//! PROJECT.calendar — month layout (TST-PROJECT-CALENDAR-001).
//!
//! Contract: the month viewport is a six-week, Sunday-aligned grid of 42 cells covering the
//! cursor's month — cells inside the month are flagged, the title names the month and year, and
//! the viewport bounds run from the first cell's midnight to the day after the last cell. An
//! unparseable cursor degrades to an empty grid and a fallback title, never a panic.
//!
//! Level: L1 Component — the production calendar math in `ui::calendar`, the same boundary the
//! Yew/MVI calendar screen renders, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_calendar__001__month_layout

use ui::calendar::{self, MonthCell};

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROJECT-CALENDAR-001).
fn project_calendar_001__month_layout() {
    // October 2026 opens on a Thursday, so the grid starts Sunday September 27th.
    let cells: Vec<MonthCell> = calendar::month_cells("2026-10-15");
    assert_eq!(cells.len(), 42, "a month is always six weeks");
    assert_eq!(cells.first().unwrap().date, "2026-09-27");
    assert_eq!(cells.last().unwrap().date, "2026-11-07");

    // The in-month flag separates October days from the leading/trailing padding.
    assert!(!cells.first().unwrap().in_month);
    let oct_first = cells.iter().find(|cell| cell.date == "2026-10-01").unwrap();
    assert!(oct_first.in_month);
    assert_eq!(oct_first.day, 1);
    let in_month = cells.iter().filter(|cell| cell.in_month).count();
    assert_eq!(in_month, 31, "October contributes exactly its 31 days");

    // Every cell carries its calendar day number and the dates run consecutively.
    for pair in cells.windows(2) {
        assert_eq!(
            pair[1].date,
            calendar::shift_cursor(&pair[0].date, "day", 1),
            "the grid is one unbroken run of days"
        );
    }

    assert_eq!(calendar::month_title("2026-10-15"), "October 2026");

    // Viewport bounds cover the rendered grid exactly: first midnight to day-after-last.
    assert_eq!(
        calendar::viewport_bounds("2026-10-15", "month").unwrap(),
        (
            "2026-09-27T00:00:00+00:00".to_string(),
            "2026-11-08T00:00:00+00:00".to_string()
        )
    );

    // Negative: an unparseable cursor degrades honestly instead of panicking.
    assert!(calendar::month_cells("not-a-date").is_empty());
    assert_eq!(calendar::month_title("not-a-date"), "Calendar");
    assert!(calendar::viewport_bounds("not-a-date", "month").is_none());
}
