//! INT.APPLE — Calls (TST-INT-APPLE-005).
//!
//! CONTRACT.
//!
//! ```text
//! Apple CallHistory rows are normalized into relationship evidence: the replay key is Apple's
//! ZUNIQUE_ID (rowid fallback), FaceTime is a separate source from Phone (never merged), direction
//! is derived from `originated` only, durations are rounded for the CRM but kept precise at
//! landing, and the canonical interaction is the NEWEST call per Person x channel.
//! ```
//!
//! The rules live in `model::apple_calls` (pure, no I/O). The test exercises the same boundary
//! production uses: the classification functions, the evidence builder that aggregates calls per
//! address, and the interaction builder that picks the newest call.
//!
//! The negative case is the payload itself: a call with no unique id and no rowid has no replay
//! key, a negative duration is clamped to zero, a call with no date is not a moment to remember,
//! and an unrecognized `originated` value means "inbound" (never a guess).
//!
//! Level: L1 Component — the model crate's call-classification boundary with deterministic
//! collaborators. No database, no socket, deterministic.

use model::apple_calls::{
    build_call_evidence, call_date_iso, call_duration_raw, call_duration_seconds, call_direction,
    call_latest_interaction, call_landing_direction, call_source, call_unique_id, flag,
    is_facetime_call, AppleCallRecord, APPLE_CALL_HISTORY_ACCOUNT, APPLE_CALLS_SOURCE,
    APPLE_FACETIME_SOURCE,
};
use serde_json::json;

fn call(
    unique_id: &str,
    address: &str,
    date: &str,
    originated: i64,
    provider: Option<&str>,
) -> AppleCallRecord {
    AppleCallRecord {
        rowid: Some(1),
        unique_id: Some(unique_id.to_owned()),
        address: Some(address.to_owned()),
        date_iso: Some(date.to_owned()),
        originated: Some(json!(originated)),
        service_provider: provider.map(str::to_owned),
        call_type: Some(json!(1)),
        duration: Some(json!(42.4)),
        ..Default::default()
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-INT-APPLE-005); the file and the assay use it.
fn int_apple_005__calls() {
    // 1. Apple's booleans arrive as a number, a real boolean, or the text of either. An
    //    unrecognised value means "unknown" (`None`), never `false`.
    assert_eq!(flag(Some(&json!(1))), Some(true));
    assert_eq!(flag(Some(&json!(0))), Some(false));
    assert_eq!(flag(Some(&json!(true))), Some(true));
    assert_eq!(flag(Some(&json!("yes"))), Some(true));
    assert_eq!(flag(Some(&json!("no"))), Some(false));
    assert_eq!(flag(Some(&json!("maybe"))), None, "an unrecognised value is unknown");
    assert_eq!(flag(None), None);

    // 2. The replay key is Apple's ZUNIQUE_ID; the rowid is the fallback the exporter already
    //    substitutes. An absent unique id with no rowid is not replay-safe.
    let with_unique = call("ABC-1", "7875551234", "2026-01-01T10:00:00.000Z", 1, None);
    assert_eq!(call_unique_id(&with_unique).as_deref(), Some("ABC-1"));

    let rowid_only = AppleCallRecord {
        unique_id: None,
        rowid: Some(42),
        ..with_unique.clone()
    };
    assert_eq!(call_unique_id(&rowid_only).as_deref(), Some("42"));

    let no_key = AppleCallRecord {
        unique_id: None,
        rowid: None,
        ..with_unique.clone()
    };
    assert_eq!(call_unique_id(&no_key), None, "no unique id and no rowid is not replay-safe");

    // 3. FaceTime is decided by provider or call type, case-insensitively, never guessed.
    let plain = call("c1", "7875551234", "2026-01-01T10:00:00.000Z", 1, None);
    assert!(!is_facetime_call(&plain));
    assert_eq!(call_source(&plain), APPLE_CALLS_SOURCE);

    let by_provider = call("c1", "7875551234", "2026-01-01T10:00:00.000Z", 1, Some("com.apple.FaceTime"));
    assert!(is_facetime_call(&by_provider));
    assert_eq!(call_source(&by_provider), APPLE_FACETIME_SOURCE);

    let mut by_type = call("c1", "7875551234", "2026-01-01T10:00:00.000Z", 1, None);
    by_type.call_type = Some(json!("FaceTime"));
    assert!(is_facetime_call(&by_type), "the call type is the second signal");

    by_type.call_type = Some(json!("7"));
    assert!(!is_facetime_call(&by_type), "an unrecognized call type is not FaceTime");

    // 4. Direction is derived from `originated` only. The canonical vocabulary is outbound/inbound;
    //    the landing vocabulary is outgoing/incoming.
    let placed = call("c1", "7875551234", "2026-01-01T10:00:00.000Z", 1, None);
    assert_eq!(call_direction(&placed), "outbound");
    assert_eq!(call_landing_direction(&placed), "outgoing");

    let received = call("c1", "7875551234", "2026-01-01T10:00:00.000Z", 0, None);
    assert_eq!(call_direction(&received), "inbound");
    assert_eq!(call_landing_direction(&received), "incoming");

    let unknown = call("c1", "7875551234", "2026-01-01T10:00:00.000Z", 0, None);
    let mut unknown_originated = unknown.clone();
    unknown_originated.originated = Some(json!("maybe"));
    assert_eq!(call_direction(&unknown_originated), "inbound", "an unrecognized originated is inbound");

    // 5. Durations: the landing table keeps Apple's precision; the CRM rounds to whole seconds,
    //    never negative.
    assert_eq!(call_duration_raw(&placed), Some(42.4));
    assert_eq!(call_duration_seconds(&placed), Some(42));

    let negative = AppleCallRecord {
        duration: Some(json!(-3.2)),
        ..placed.clone()
    };
    assert_eq!(call_duration_seconds(&negative), Some(0), "a negative duration is clamped to zero");

    let text_duration = AppleCallRecord {
        duration: Some(json!("12.6")),
        ..placed.clone()
    };
    assert_eq!(call_duration_seconds(&text_duration), Some(13), "a text duration is parsed");

    let null_duration = AppleCallRecord {
        duration: Some(json!(null)),
        ..placed.clone()
    };
    assert_eq!(call_duration_seconds(&null_duration), None);

    // 6. The exported timestamp is trimmed. A call with no date is not a moment to remember.
    assert_eq!(call_date_iso(&placed).as_deref(), Some("2026-01-01T10:00:00.000Z"));
    let undated = AppleCallRecord {
        date_iso: None,
        ..placed.clone()
    };
    assert_eq!(call_date_iso(&undated), None);

    // 7. Evidence aggregates by address and keeps Phone and FaceTime apart. The same number
    //    reached by phone and by FaceTime is two channels, never merged.
    let calls = vec![
        call("c1", "7875551234", "2026-01-01T10:00:00.000Z", 1, None),
        call("c2", "7875551234", "2026-01-02T10:00:00.000Z", 0, None),
        call("c3", "dana@example.com", "2026-01-03T10:00:00.000Z", 0, Some("com.apple.FaceTime")),
    ];
    let evidence = build_call_evidence(&calls, APPLE_CALL_HISTORY_ACCOUNT);
    assert_eq!(evidence.len(), 2, "one phone identity and one FaceTime identity");

    let phone = evidence
        .iter()
        .find(|row| row.source_identity_key == "7875551234")
        .expect("phone evidence");
    assert_eq!(phone.source, APPLE_CALLS_SOURCE);
    assert_eq!(phone.inbound_count, 1);
    assert_eq!(phone.outbound_count, 1);
    assert_eq!(phone.is_two_way, Some(true));

    let facetime = evidence
        .iter()
        .find(|row| row.source == APPLE_FACETIME_SOURCE)
        .expect("facetime evidence");
    assert_eq!(facetime.source_identity_key, "dana@example.com");
    assert_eq!(facetime.inbound_count, 1);
    assert_eq!(facetime.outbound_count, 0);

    // 8. The canonical interaction is the newest call per Person x channel, keyed on the source.
    let interaction = call_latest_interaction(&calls[0], "person-1").expect("a placed call becomes an interaction");
    assert_eq!(interaction.channel, "call");
    assert_eq!(interaction.event_type, "phone_call");
    assert_eq!(interaction.direction, "outbound");
    assert_eq!(interaction.source_system, APPLE_CALLS_SOURCE);
    assert_eq!(interaction.source_external_id, "latest:person-1:call");
    assert_eq!(interaction.duration_seconds, Some(42));

    let facetime_interaction = call_latest_interaction(&calls[2], "person-2").expect("a facetime call becomes an interaction");
    assert_eq!(facetime_interaction.event_type, "facetime_call");
    assert_eq!(facetime_interaction.source_system, APPLE_FACETIME_SOURCE);

    let undated_interaction = call_latest_interaction(&undated, "person-1");
    assert!(undated_interaction.is_none(), "a call with no date is not a moment to remember");
}
