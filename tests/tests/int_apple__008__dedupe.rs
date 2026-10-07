//! INT.APPLE — dedupe (TST-INT-APPLE-008).
//!
//! Contract: Apple intake is replay-safe. Every landing write keys on
//! `(coalesce(source_account, ''), source_message_id)` — the calendar upsert
//! refreshes the one row for that key, the message/call/reminder landings
//! `do nothing` on conflict — so re-delivering the same export never appends
//! a second row. The batch landings report how many rows were genuinely new.
//!
//! Level: L1 Component — the executable half pins the replay-key material on
//! the production `CalendarLandingEvent` contract; the structural half pins
//! the conflict rule on the production SQL that owns it. No database, no
//! network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test int_apple__008__dedupe

use model::CalendarLandingEvent;
use serde_json::json;
use test_harness::source;

/// The replay key, exactly as the landing SQL builds it.
fn replay_key(event: &CalendarLandingEvent) -> (String, String) {
    (
        event.source_account.clone(),
        event.source_message_id.clone(),
    )
}

fn landing_event(account: &str, message_id: &str, title: &str) -> CalendarLandingEvent {
    CalendarLandingEvent {
        source_account: account.into(),
        source_message_id: message_id.into(),
        title: title.into(),
        start_at: "2026-10-10T10:00:00Z".into(),
        end_at: "2026-10-10T11:00:00Z".into(),
        all_day: false,
        location: None,
        raw: json!({
            "eventIdentifier": "event-occurrence-1",
            "calendarItemIdentifier": "series-9",
            "occurrenceDate": "2026-10-10T10:00:00Z",
            "recurring": true,
            "detached": false,
        }),
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name; the file and the assay use it.
fn int_apple_008__dedupe() {
    // Positive: a replay carries the same key as the first delivery, even when
    // the content moved on — that shared key is what the conflict rule holds.
    let first = landing_event("mac-edge-1", "series-9|2026-10-10T10:00:00Z", "Showing");
    let replay = landing_event(
        "mac-edge-1",
        "series-9|2026-10-10T10:00:00Z",
        "Showing (moved to 10:30)",
    );
    assert_eq!(
        replay_key(&first),
        replay_key(&replay),
        "a replay shares the landing key even when its content changed"
    );

    // Positive: the key material survives the production serde contract, so a
    // row that crossed the outbox still dedupes against the row that landed.
    let wire = serde_json::to_value(&first).expect("serializes");
    let back: CalendarLandingEvent = serde_json::from_value(wire).expect("deserializes");
    assert_eq!(replay_key(&first), replay_key(&back));

    // Positive: a genuinely new occurrence keys differently — dedupe drops
    // replays, never distinct events.
    let other = landing_event("mac-edge-1", "series-9|2026-10-17T10:00:00Z", "Showing");
    assert_ne!(replay_key(&first), replay_key(&other));

    // The calendar upsert owns the replay rule: one row per key, refreshed.
    let calendar = source::read(&source::workspace_root().join("db/src/calendar.rs"));
    assert!(
        calendar.contains("on conflict ((coalesce(source_account, '')), source_message_id)"),
        "the calendar landing upserts on the replay key"
    );
    assert!(
        calendar.contains("do update set"),
        "a calendar replay refreshes the one row instead of appending"
    );

    // The message, call, mail and WhatsApp landings share the same key and
    // drop the replay instead of refreshing; the reminder landing keys on the
    // same pair and refreshes the one row, like the calendar upsert.
    for (path, needle) in [
        (
            "db/src/landing.rs",
            "on conflict (coalesce(source_account, ''), source_message_id) do nothing",
        ),
        (
            "db/src/apple_ods.rs",
            "on conflict (coalesce(source_account, ''), source_message_id) do nothing",
        ),
        (
            "db/src/whatsapp.rs",
            "on conflict (coalesce(source_account, ''), source_message_id) do nothing",
        ),
        (
            "db/src/wbs.rs",
            "on conflict ((coalesce(source_account, '')), source_message_id)",
        ),
    ] {
        let text = source::read(&source::workspace_root().join(path));
        assert!(
            text.contains(needle),
            "{path} keys its landing on the shared replay key"
        );
    }

    // The batch landings report genuinely-new rows, so a replay is visible as
    // zero rather than as a second row.
    assert!(
        calendar.contains("returning id"),
        "the upsert returns what it inserted so newness is counted"
    );
    let landing = source::read(&source::workspace_root().join("db/src/landing.rs"));
    assert!(
        landing.contains("Returns how many rows were genuinely new"),
        "batch landings count new rows, not attempted rows"
    );

    // Negative: a landing insert without the conflict rule would duplicate on
    // replay. No Apple landing statement may be a bare insert.
    for path in [
        "db/src/calendar.rs",
        "db/src/landing.rs",
        "db/src/apple_ods.rs",
    ] {
        let text = source::read(&source::workspace_root().join(path));
        let bare = text
            .match_indices("insert into l_")
            .filter(|(index, _)| {
                let rest = &text[*index..];
                let end = rest.find(';').map(|i| i + *index).unwrap_or(text.len());
                !text[*index..end].contains("on conflict")
            })
            .count();
        assert_eq!(
            bare, 0,
            "{path} has a landing insert with no conflict rule — a replay would duplicate"
        );
    }
}
