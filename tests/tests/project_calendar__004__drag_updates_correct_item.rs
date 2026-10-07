//! PROJECT.calendar — drag updates correct item (TST-PROJECT-CALENDAR-004).
//!
//! Contract: dragging a timed event moves its start to the drop target and preserves its
//! duration — the item the user grabbed is the item whose span changes. Dropping an all-day
//! event onto the grid gives it one timed hour; resizing extends the end to the target slot
//! and refuses a target at or before the start, so a drag can never invert or erase a span.
//!
//! Level: L1 Component — the production calendar math in `ui::calendar`, the same boundary the
//! Yew/MVI calendar drag interaction commits through, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_calendar__004__drag_updates_correct_item

use ui::calendar::{self, GridSpec};

fn grid() -> GridSpec {
    GridSpec::new(8, 20, 30)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROJECT-CALENDAR-004).
fn project_calendar_004__drag_updates_correct_item() {
    // A ninety-minute standup dragged to Monday keeps its ninety minutes.
    let target = calendar::slot_timestamp("2026-09-28", 2, grid()).unwrap();
    let (start, end) = calendar::move_span(
        "2026-09-27T09:00:00+00:00",
        Some("2026-09-27T10:30:00+00:00"),
        &target,
    )
    .unwrap();
    assert_eq!(start, "2026-09-28T09:00:00+00:00");
    assert_eq!(end, "2026-09-28T10:30:00+00:00");

    // Only the dragged item's span changes: a second item re-dragged from its own start
    // lands on its own target with its own duration intact.
    let other = calendar::move_span(
        "2026-09-27T14:00:00+00:00",
        Some("2026-09-27T14:30:00+00:00"),
        &calendar::slot_timestamp("2026-09-29", 4, grid()).unwrap(),
    )
    .unwrap();
    assert_eq!(other.0, "2026-09-29T10:00:00+00:00");
    assert_eq!(other.1, "2026-09-29T10:30:00+00:00");

    // An all-day event dropped on the grid becomes one timed hour at the drop slot.
    let timed = calendar::move_to_timed(
        "2026-09-27T00:00:00+00:00",
        Some("2026-09-28T00:00:00+00:00"),
        true,
        &target,
    )
    .unwrap();
    assert_eq!(timed.0, "2026-09-28T09:00:00+00:00");
    assert_eq!(timed.1, "2026-09-28T10:00:00+00:00");

    // Resizing extends the end to the end of the target slot.
    let (_, resized) = calendar::resize_span(
        "2026-09-28T09:00:00+00:00",
        &calendar::slot_timestamp("2026-09-28", 5, grid()).unwrap(),
        grid(),
    )
    .unwrap();
    assert_eq!(resized, "2026-09-28T11:00:00+00:00");

    // Negative: a resize target at or before the start is refused — the span is never inverted.
    assert_eq!(
        calendar::resize_span(
            "2026-09-28T09:00:00+00:00",
            "2026-09-28T08:00:00+00:00",
            grid()
        ),
        None
    );
    // Negative: an undroppable target moves nothing instead of fabricating a span.
    assert_eq!(
        calendar::move_span(
            "2026-09-27T09:00:00+00:00",
            Some("2026-09-27T10:00:00+00:00"),
            "not-a-time"
        ),
        None
    );
}
