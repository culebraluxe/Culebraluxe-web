//! PROJECT.calendar — week layout (TST-PROJECT-CALENDAR-002).
//!
//! Contract: the week viewport is the seven Sunday-to-Saturday dates containing the cursor —
//! the title names the spanned range, week navigation steps a full seven days on the same
//! cursor the other modes use, and the viewport bounds run from the Sunday midnight to the day
//! after Saturday. An unparseable cursor degrades to an empty week, never a panic.
//!
//! Level: L1 Component — the production calendar math in `ui::calendar`, the same boundary the
//! Yew/MVI calendar screen renders, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test project_calendar__002__week_layout

use ui::calendar;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROJECT-CALENDAR-002).
fn project_calendar_002__week_layout() {
    // Wednesday 2026-09-30 sits in the Sunday 27th week.
    let dates = calendar::week_dates("2026-09-30");
    assert_eq!(
        dates,
        vec![
            "2026-09-27",
            "2026-09-28",
            "2026-09-29",
            "2026-09-30",
            "2026-10-01",
            "2026-10-02",
            "2026-10-03",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>(),
        "the week runs Sunday to Saturday and may straddle months"
    );

    // A Sunday cursor opens its own week; a Saturday cursor closes the same one.
    assert_eq!(
        calendar::week_dates("2026-09-27").last().unwrap(),
        "2026-10-03"
    );
    assert_eq!(
        calendar::week_dates("2026-10-03").first().unwrap(),
        "2026-09-27"
    );

    // The title names the spanned range.
    assert_eq!(calendar::week_title("2026-09-30"), "Sep 27 – Oct 3, 2026");
    // A week inside one month uses the compact form.
    assert_eq!(calendar::week_title("2026-10-07"), "October 4–10, 2026");

    // Week navigation shares the cursor: a step is exactly seven days.
    assert_eq!(
        calendar::shift_cursor("2026-09-30", "week", 1),
        "2026-10-07"
    );
    assert_eq!(
        calendar::shift_cursor("2026-09-30", "week", -1),
        "2026-09-23"
    );

    // Viewport bounds cover Sunday midnight to the day after Saturday.
    assert_eq!(
        calendar::viewport_bounds("2026-09-30", "week").unwrap(),
        (
            "2026-09-27T00:00:00+00:00".to_string(),
            "2026-10-04T00:00:00+00:00".to_string()
        )
    );

    // Negative: an unparseable cursor degrades honestly instead of panicking.
    assert!(calendar::week_dates("not-a-date").is_empty());
    assert_eq!(calendar::week_title("not-a-date"), "Calendar");
    assert!(calendar::viewport_bounds("not-a-date", "week").is_none());
}
