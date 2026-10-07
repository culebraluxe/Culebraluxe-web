//! PROJECT.calendar — time-grid positioning (TST-PROJECT-CALENDAR-003).
//!
//! Contract: the time grid maps instants to slot indices and spans to slot counts on the same
//! grid the screen renders — slot labels name the wall-clock time, an event outside grid hours
//! has no slot, and a zero-length event still occupies one slot. Overlapping events are dealt
//! into stable lanes per collision cluster; disjoint events share lane zero.
//!
//! Level: L1 Component — the production calendar math in `ui::calendar`, the same boundary the
//! Yew/MVI calendar screen renders, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_calendar__003__time_grid_positioning

use ui::calendar::{self, GridSpec};

fn grid() -> GridSpec {
    GridSpec::new(8, 20, 30)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROJECT-CALENDAR-003).
fn project_calendar_003__time_grid_positioning() {
    let grid = grid();
    assert_eq!(grid.slot_count(), 24, "8:00–20:00 in thirty-minute slots");

    // Slot labels name the wall-clock time the screen prints.
    assert_eq!(calendar::slot_label(0, grid), "8:00 AM");
    assert_eq!(calendar::slot_label(2, grid), "9:00 AM");
    assert_eq!(calendar::slot_label(23, grid), "7:30 PM");

    // Slot timestamps pin an event to the grid: 9:00 lands on slot 2.
    let nine = calendar::slot_timestamp("2026-09-28", 2, grid).unwrap();
    assert_eq!(nine, "2026-09-28T09:00:00+00:00");
    assert_eq!(calendar::event_slot(&nine, grid), Some(2));

    // A ninety-minute event spans three slots; a zero-length event still takes one.
    assert_eq!(
        calendar::event_span_slots(
            "2026-09-28T09:00:00+00:00",
            Some("2026-09-28T10:30:00+00:00"),
            grid
        ),
        3
    );
    assert_eq!(
        calendar::event_span_slots("2026-09-28T09:00:00+00:00", None, grid),
        2,
        "an open-ended event defaults to one hour"
    );

    // Overlapping events are dealt into lanes; the disjoint pair shares lane zero.
    assert_eq!(
        calendar::overlap_lanes(&[(2, 4), (3, 2), (7, 2), (8, 1)]),
        vec![(0, 2), (1, 2), (0, 2), (1, 2)]
    );
    assert_eq!(
        calendar::overlap_lanes(&[(1, 1), (2, 1)]),
        vec![(0, 1), (0, 1)]
    );
    assert!(calendar::overlap_lanes(&[]).is_empty());

    // Negative: outside grid hours there is no slot — the screen must not place the event.
    assert_eq!(
        calendar::event_slot("2026-09-28T07:30:00+00:00", grid),
        None
    );
    assert_eq!(
        calendar::event_slot("2026-09-28T20:00:00+00:00", grid),
        None
    );
    assert_eq!(calendar::event_slot("not-a-time", grid), None);

    // Negative: a backwards grid preference falls back to the honest default.
    assert_eq!(GridSpec::new(20, 8, 7), GridSpec::new(8, 20, 30));
}
