//! INT.APPLE — Messages normalization (TST-INT-APPLE-004).
//!
//! CONTRACT.
//!
//! ```text
//! Apple Messages export rows are normalized into neutral identity evidence: emails are lowercased
//! and shape-checked, phones are reduced to E.164 (US/PR) or quarantined, group-chat GUIDs are
//! never one person's relationship, timestamps land on the instant the exporter reports, and the
//! bounded preview is collapsed, capped, and never fabricates prose.
//! ```
//!
//! The rules live in `model::apple_messages` (pure, no I/O). The test exercises the same boundary
//! production uses: the normalization functions the intake calls, the evidence builder that
//! aggregates messages per identity, and the reconciliation decision that never guesses.
//!
//! The negative case is the payload itself: a non-email handle produces no email evidence, an
//! ambiguous phone is quarantined (never guessed), a group chat is counted for nobody, and an
//! over-long preview is truncated with an ellipsis.
//!
//! Level: L1 Component — the model crate's normalization boundary with deterministic collaborators.
//! No database, no socket, deterministic.

use model::apple_messages::{
    apple_nanos_to_iso, apple_service_to_channel, bounded_preview, build_handle_evidence,
    decide_apple_handle, derive_source_account, effective_date_iso, fingerprint,
    handle_to_identities, is_group_chat_guid, normalize_email, normalize_phone,
    AppleHandleLookup, AppleMessagesHandle, AppleMessagesMessage, APPLE_LOCAL_SOURCE_ACCOUNT,
    APPLE_MESSAGES_SOURCE, APPLE_PREVIEW_MAX_LENGTH,
};

fn handle(rowid: i64, id: &str, service: Option<&str>) -> AppleMessagesHandle {
    AppleMessagesHandle {
        rowid: Some(rowid),
        id: Some(id.to_owned()),
        service: service.map(str::to_owned),
        ..Default::default()
    }
}

fn message(
    guid: &str,
    handle_id: i64,
    chat_guid: Option<&str>,
    date_iso: Option<&str>,
    is_from_me: i64,
    service: Option<&str>,
    text: Option<&str>,
) -> AppleMessagesMessage {
    AppleMessagesMessage {
        guid: Some(guid.to_owned()),
        chat_guid: chat_guid.map(str::to_owned),
        handle_id: Some(handle_id),
        service: service.map(str::to_owned),
        account: Some("owner@example.com".to_owned()),
        date_iso: date_iso.map(str::to_owned),
        is_from_me: Some(is_from_me),
        text: text.map(str::to_owned),
        ..Default::default()
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-INT-APPLE-004); the file and the assay use it.
fn int_apple_004__messages_normalization() {
    // 1. Emails normalize to lowercase and are shape-checked. The original value is retained
    //    by the caller; the normalized value is what matching uses.
    assert_eq!(normalize_email("  A@B.COM ").as_deref(), Some("a@b.com"));
    assert_eq!(normalize_email("not-an-email"), None);
    assert_eq!(normalize_email("two@at@example.com"), None);
    assert_eq!(normalize_email("no@domain"), None);
    assert_eq!(normalize_email(""), None);

    // 2. Phones normalize to E.164 (US/PR) or are quarantined. An ambiguous phone is never
    //    guessed at — `None` means "not reliably matchable".
    assert_eq!(normalize_phone("787-555-1234").as_deref(), Some("+17875551234"));
    assert_eq!(normalize_phone("+1 (787) 555-1234").as_deref(), Some("+17875551234"));
    assert_eq!(normalize_phone("5551234"), None, "7 digits is not a US phone");
    assert_eq!(normalize_phone("44 20 7946 0958"), None, "non-US is quarantined");
    assert_eq!(normalize_phone("no digits here"), None);

    // 3. A handle splits into email OR phone identity evidence, never both, never a guess.
    let (emails, phones) = handle_to_identities("Client@Example.com");
    assert_eq!(emails.len(), 1);
    assert!(phones.is_empty());
    assert_eq!(emails[0].normalized, "client@example.com");
    assert_eq!(emails[0].value, "Client@Example.com", "the original is retained");

    let (emails, phones) = handle_to_identities("(787) 555-1234");
    assert!(emails.is_empty());
    assert_eq!(phones.len(), 1);
    assert_eq!(phones[0].normalized, "+17875551234");

    let (emails, phones) = handle_to_identities("not-an-identity");
    assert!(emails.is_empty() && phones.is_empty(), "an unparseable handle yields nothing");

    // 4. Group-chat GUIDs are never one person's relationship.
    assert!(is_group_chat_guid(Some("any;+;abc123")));
    assert!(is_group_chat_guid(Some("iMessage;+;group-chat-42")));
    assert!(!is_group_chat_guid(Some("any;-;+17875551234")));
    assert!(!is_group_chat_guid(None));

    // 5. Apple nanosecond timestamps land on the instant the exporter reports.
    assert_eq!(apple_nanos_to_iso(0.0).as_deref(), Some("2001-01-01T00:00:00.000Z"));
    assert_eq!(apple_nanos_to_iso(1_000_000_000.0).as_deref(), Some("2001-01-01T00:00:01.000Z"));
    assert_eq!(apple_nanos_to_iso(f64::NAN), None);
    assert_eq!(apple_nanos_to_iso(f64::INFINITY), None);

    // 6. The observed timestamp prefers the exporter's ISO value, falling back to the raw
    //    Apple timestamp when the exporter could not render one.
    let with_iso = message("m1", 1, None, Some("2026-01-02T03:04:05.000Z"), 0, None, None);
    assert_eq!(effective_date_iso(&with_iso).as_deref(), Some("2026-01-02T03:04:05.000Z"));
    let without_iso = AppleMessagesMessage {
        date_iso: None,
        date: Some(722_000_000_000_000_000.0),
        ..with_iso.clone()
    };
    assert_eq!(effective_date_iso(&without_iso).as_deref(), Some("2023-11-18T11:33:20.000Z"));

    // 7. The bounded preview collapses whitespace, caps at 160 characters, and never fabricates
    //    prose from nothing.
    assert_eq!(bounded_preview(Some("  hello \n world  ")).as_deref(), Some("hello world"));
    assert_eq!(bounded_preview(Some("   ")), None);
    assert_eq!(bounded_preview(None), None);
    let long = bounded_preview(Some(&"x".repeat(500))).expect("a long preview is bounded");
    assert_eq!(long.encode_utf16().count(), APPLE_PREVIEW_MAX_LENGTH);
    assert!(long.ends_with('…'), "an over-long preview is truncated with an ellipsis");

    // 8. The service maps to the canonical channel: SMS is sms, everything else is imessage.
    assert_eq!(apple_service_to_channel(Some("SMS")), "sms");
    assert_eq!(apple_service_to_channel(Some("iMessage")), "imessage");
    assert_eq!(apple_service_to_channel(None), "imessage");

    // 9. The source account comes from the messages, then falls back to the local account.
    let messages = vec![message("m1", 1, None, None, 0, None, None)];
    assert_eq!(derive_source_account(&messages), "owner@example.com");
    assert_eq!(derive_source_account(&[]), APPLE_LOCAL_SOURCE_ACCOUNT);

    // 10. The fingerprint is deterministic and matches the TypeScript hash bit for bit.
    assert_eq!(fingerprint(""), "488bdcb81aee8d83");
    assert_eq!(fingerprint("apple_messages|local|+17875551234|3|1"), "9b36555c50d880a7");

    // 11. Evidence aggregates every rowid for one identity and excludes group chats.
    let handles = vec![
        handle(1, "+17875551234", Some("iMessage")),
        handle(2, "+17875551234", Some("SMS")),
        handle(3, "+17870001111", Some("iMessage")),
    ];
    let messages = vec![
        message("m1", 1, Some("any;-;+17875551234"), Some("2024-01-01T00:00:00.000Z"), 0, Some("iMessage"), Some("hi")),
        message("m2", 2, Some("any;-;+17875551234"), Some("2024-03-01T00:00:00.000Z"), 1, Some("SMS"), Some("reply")),
        message("m3", 2, Some("any;+;group-7"), Some("2024-04-01T00:00:00.000Z"), 0, Some("SMS"), Some("group noise")),
    ];
    let evidence = build_handle_evidence("owner@example.com", &handles, &messages);
    assert_eq!(evidence.len(), 2, "one identity per textual handle");

    let two_way = evidence
        .iter()
        .find(|row| row.source_identity_key == "+17875551234")
        .expect("two-way row");
    assert_eq!(two_way.inbound_count, 1);
    assert_eq!(two_way.outbound_count, 1);
    assert_eq!(two_way.is_two_way, Some(true));
    assert_eq!(two_way.source, APPLE_MESSAGES_SOURCE);
    assert_eq!(two_way.handle_rowids, vec![1, 2], "every rowid is aggregated");

    let silent = evidence
        .iter()
        .find(|row| row.source_identity_key == "+17870001111")
        .expect("silent row");
    assert_eq!(silent.total_count() + silent.inbound_count + silent.outbound_count, 0);
    assert_eq!(silent.is_two_way, None);

    // 12. The reconciliation decision never guesses: an exact match links, a multi-match is a
    //    conflict, and a two-way without an exact match is reviewable.
    let row = two_way;
    let linked = decide_apple_handle(
        row,
        &AppleHandleLookup {
            phone_owners: vec!["person-1".to_owned()],
            ..Default::default()
        },
    );
    assert_eq!(linked.review_state, "exact_linked");
    assert_eq!(linked.canonical_person_id.as_deref(), Some("person-1"));

    let conflicting = decide_apple_handle(
        row,
        &AppleHandleLookup {
            phone_owners: vec!["person-1".to_owned()],
            phone_multi_match: true,
            ..Default::default()
        },
    );
    assert_eq!(conflicting.review_state, "ambiguous");
    assert_eq!(conflicting.canonical_person_id, None);

    let review = decide_apple_handle(row, &AppleHandleLookup::default());
    assert_eq!(review.review_state, "review_required");
    assert_eq!(review.canonical_person_id, None);
}
