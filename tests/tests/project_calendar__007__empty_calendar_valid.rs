//! PROJECT.calendar — empty calendar valid (TST-PROJECT-CALENDAR-007).
//!
//! Contract: an empty calendar is a valid calendar — no events, no cursor, and no selection
//! still produce well-formed projections: empty grids, empty lanes, fallbacks that name no
//! month, and a grid default that keeps the screen renderable. Nothing panics, nothing emits
//! a placeholder event, and an unknown viewport mode falls back to the single-day behavior
//! the screen already uses.
//!
//! Level: L1 Component — the production calendar math in `ui::calendar`, the same boundary the
//! Yew/MVI calendar screen renders, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_calendar__007__empty_calendar_valid

use ui::calendar::{self, GridSpec};

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROJECT-CALENDAR-007).
fn project_calendar_007__empty_calendar_valid() {
    // No cursor, no events: every projection is empty but well-formed.
    assert!(calendar::month_cells("").is_empty());
    assert!(calendar::week_dates("").is_empty());
    assert!(calendar::overlap_lanes(&[]).is_empty());
    assert!(calendar::viewport_bounds("", "month").is_none());
    assert!(calendar::viewport_bounds("", "week").is_none());
    assert!(calendar::viewport_bounds("", "day").is_none());

    // Fallback titles name no month rather than inventing one.
    assert_eq!(calendar::month_title(""), "Calendar");
    assert_eq!(calendar::week_title(""), "Calendar");
    assert_eq!(calendar::date_title(""), "Calendar");

    // The default grid keeps an empty screen renderable: twelve hours, thirty-minute slots.
    let grid = GridSpec::new(8, 20, 30);
    assert_eq!(grid.slot_count(), 24);
    assert_eq!(calendar::slot_label(0, grid), "8:00 AM");

    // An unknown viewport mode falls back to the single-day behavior the screen uses.
    assert_eq!(
        calendar::viewport_bounds("2026-09-27", "bogus").unwrap(),
        (
            "2026-09-27T00:00:00+00:00".to_string(),
            "2026-09-28T00:00:00+00:00".to_string()
        )
    );
    assert_eq!(
        calendar::shift_cursor("2026-09-27", "bogus", 1),
        "2026-09-28",
        "an unknown mode steps one day, like the day view"
    );

    // Negative: with no event there is no slot, no span target, and no drag source.
    assert_eq!(calendar::event_slot("", grid), None);
    assert_eq!(
        calendar::move_span("", None, "2026-09-28T09:00:00+00:00"),
        None
    );
    assert_eq!(
        calendar::resize_span("", "2026-09-28T09:00:00+00:00", grid),
        None
    );
}
