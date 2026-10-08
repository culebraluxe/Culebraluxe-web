//! PROJECT.calendar — crossing day boundary (TST-PROJECT-CALENDAR-005).
//!
//! Contract: a drag that crosses midnight preserves the absolute duration — an evening event
//! moved past midnight ends on the next day, and moving between all-day and timed keeps the
//! span explicit: timed-to-all-day becomes midnight-to-midnight, multi-day all-day keeps its
//! day count. An unparseable drop target moves nothing.
//!
//! Level: L1 Component — the production calendar math in `ui::calendar`, the same boundary the
//! Yew/MVI calendar screen renders, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_calendar__005__crossing_day_boundary

use ui::calendar;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROJECT-CALENDAR-005).
fn project_calendar_005__crossing_day_boundary() {
    // A 23:00–01:00 night shift dragged to Tuesday still runs two hours past midnight.
    let (start, end) = calendar::move_span(
        "2026-09-27T23:00:00+00:00",
        Some("2026-09-28T01:00:00+00:00"),
        "2026-09-29T23:00:00+00:00",
    )
    .unwrap();
    assert_eq!(start, "2026-09-29T23:00:00+00:00");
    assert_eq!(
        end, "2026-09-30T01:00:00+00:00",
        "the two-hour duration survives the midnight crossing"
    );

    // A timed event dropped on a day cell becomes that whole day, midnight to midnight.
    let (day_start, day_end) = calendar::move_to_all_day(
        "2026-09-27T09:00:00+00:00",
        Some("2026-09-27T10:00:00+00:00"),
        false,
        "2026-09-29",
    )
    .unwrap();
    assert_eq!(day_start, "2026-09-29T00:00:00+00:00");
    assert_eq!(day_end, "2026-09-30T00:00:00+00:00");

    // A three-day all-day block moved to another date stays three days.
    let (block_start, block_end) = calendar::move_to_all_day(
        "2026-09-27T00:00:00+00:00",
        Some("2026-09-30T00:00:00+00:00"),
        true,
        "2026-10-05",
    )
    .unwrap();
    assert_eq!(block_start, "2026-10-05T00:00:00+00:00");
    assert_eq!(block_end, "2026-10-08T00:00:00+00:00");

    // Negative: an unparseable drop target or date moves nothing instead of guessing a day.
    assert_eq!(
        calendar::move_span(
            "2026-09-27T23:00:00+00:00",
            Some("2026-09-28T01:00:00+00:00"),
            "not-a-time"
        ),
        None
    );
    assert_eq!(
        calendar::move_to_all_day(
            "2026-09-27T09:00:00+00:00",
            Some("2026-09-27T10:00:00+00:00"),
            false,
            "not-a-date"
        ),
        None
    );
}
