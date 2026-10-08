//! PROJECT.calendar — timezone behavior (TST-PROJECT-CALENDAR-006).
//!
//! Contract: the grid does absolute-time math on the RFC3339 instants production stores — a
//! drag between instants that carry a non-UTC offset preserves the absolute duration, the
//! instant's own offset is never silently rewritten to UTC, labels render the instant's local
//! wall-clock time, and the date key reads the calendar date prefix. Unparseable instants
//! return `None`, never a fabricated time.
//!
//! Level: L1 Component — the production calendar math in `ui::calendar`, the same boundary the
//! Yew/MVI calendar screen renders, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_calendar__006__timezone_behavior

use ui::calendar;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROJECT-CALENDAR-006).
fn project_calendar_006__timezone_behavior() {
    // A one-hour meeting stored with a +02:00 offset drags to another +02:00 slot intact:
    // same wall clock, same absolute duration.
    let (start, end) = calendar::move_span(
        "2026-09-27T09:00:00+02:00",
        Some("2026-09-27T10:00:00+02:00"),
        "2026-09-28T14:00:00+02:00",
    )
    .unwrap();
    assert_eq!(start, "2026-09-28T14:00:00+02:00");
    assert_eq!(
        end, "2026-09-28T15:00:00+02:00",
        "the hour survives the move without touching the offset"
    );

    // Cross-offset drags keep absolute duration: 09:00+02:00 is 07:00Z, so a drop on
    // 10:00Z preserves the hour in absolute time.
    let (_, cross_end) = calendar::move_span(
        "2026-09-27T09:00:00+02:00",
        Some("2026-09-27T10:00:00+02:00"),
        "2026-09-28T10:00:00+00:00",
    )
    .unwrap();
    assert_eq!(cross_end, "2026-09-28T11:00:00+00:00");

    // Minute arithmetic is absolute too: +90 minutes past a +05:30 instant lands correctly.
    assert_eq!(
        calendar::add_minutes("2026-09-27T09:00:00+05:30", 90).unwrap(),
        "2026-09-27T10:30:00+05:30"
    );

    // Labels render the instant's own wall-clock time, not a converted one.
    assert_eq!(
        calendar::time_label("2026-09-27T14:00:00+02:00").unwrap(),
        "2:00 PM"
    );
    assert_eq!(
        calendar::date_key("2026-09-27T14:00:00+02:00").unwrap(),
        "2026-09-27"
    );

    // Negative: unparseable instants yield nothing instead of a fabricated time.
    assert_eq!(
        calendar::move_span(
            "2026-09-27T09:00:00+02:00",
            Some("2026-09-27T10:00:00+02:00"),
            "tomorrow-ish"
        ),
        None
    );
    assert!(calendar::time_label("tomorrow-ish").is_none());
    assert!(calendar::add_minutes("tomorrow-ish", 30).is_none());
    assert!(calendar::date_key("tomorrow-ish").is_none());
}
