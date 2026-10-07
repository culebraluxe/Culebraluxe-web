//! INT.APPLE — Apple Mail normalization (TST-INT-APPLE-006).
//!
//! CONTRACT.
//!
//! ```text
//! Apple Mail envelope rows are normalized into neutral observations: the replay identity is the
//! RFC Message-ID (or mailbox + local id), the sender is parsed into a display name and a
//! normalized address, the subject is NFKC-normalized and bounded, and the direction is decided
//! by the mailbox and the sender — an inbox copy is inbound, a sent copy is outbound with exactly
//! one external recipient, an archive copy lets the sender decide, and a second landing of the
//! same source message is a duplicate.
//! ```
//!
//! The rules live in `model::applemail` (pure, no I/O). The test exercises the same boundary
//! production uses: the normalization functions, the replay-id builder, the evidence builder,
//! and the landed-mail normalizer that decides direction.
//!
//! The negative case is the payload itself: an unaddressed row is skipped, a sent message to
//! multiple external recipients is ambiguous (never a guess), an internal-only message is
//! skipped, and a duplicate source message is counted once.
//!
//! Level: L1 Component — the model crate's mail-normalization boundary with deterministic
//! collaborators. No database, no socket, deterministic.

use model::applemail::{
    apple_mail_replay_id, bounded_email_subject, build_mail_evidence, mail_observation_interaction,
    normalize_landed_mail, normalize_mailbox, parse_sender_address, LandedAppleMail,
    MailAddress, MailObservation, ICLOUD_MAIL_SOURCE,
};
use std::collections::BTreeSet;

fn landed(
    source_message_id: &str,
    mailbox_kind: &str,
    sender: Option<&str>,
    to: Vec<(&str, Option<&str>)>,
    subject: Option<&str>,
) -> LandedAppleMail {
    LandedAppleMail {
        source_account: "owner@example.com".to_owned(),
        source_message_id: source_message_id.to_owned(),
        mailbox_kind: Some(mailbox_kind.to_owned()),
        mailbox_name: Some(mailbox_kind.to_owned()),
        local_id: Some(1),
        message_id: Some(source_message_id.to_owned()),
        occurred_at: Some("2026-01-01T10:00:00.000Z".to_owned()),
        sender: sender.map(str::to_owned),
        to_recipients: to
            .into_iter()
            .map(|(address, name)| MailAddress {
                address: Some(address.to_owned()),
                name: name.map(str::to_owned),
            })
            .collect(),
        cc_recipients: Vec::new(),
        bcc_recipients: Vec::new(),
        subject: subject.map(str::to_owned),
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-INT-APPLE-006); the file and the assay use it.
fn int_apple_006__apple_mail_normalization() {
    let internal: BTreeSet<String> = ["owner@example.com".to_owned()].into_iter().collect();

    // 1. The replay identity is the RFC Message-ID when the source has one; otherwise the
    //    mailbox and Mail's own local row id. Never the row's index, never `messages.message_id`.
    assert_eq!(
        apple_mail_replay_id(Some("<msg-1@apple.com>"), "inbox", 1).as_deref(),
        Some("message-id:<msg-1@apple.com>")
    );
    assert_eq!(
        apple_mail_replay_id(None, "inbox", 42).as_deref(),
        Some("mail-local:inbox:42")
    );
    assert_eq!(apple_mail_replay_id(None, "", 42), None, "an empty mailbox kind is not an identity");
    assert_eq!(apple_mail_replay_id(None, "inbox", 0), None, "a non-positive local id is not an identity");

    // 2. A mailbox address is normalized (trimmed, lowercased) only when shaped like an address.
    assert_eq!(normalize_mailbox("  Dana@Example.com ").as_deref(), Some("dana@example.com"));
    assert_eq!(normalize_mailbox("not-an-address"), None);
    assert_eq!(normalize_mailbox(""), None);

    // 3. A subject is NFKC-normalized, control characters become spaces, whitespace is collapsed,
    //    and anything past the limit is truncated with an ellipsis (never dropped).
    assert_eq!(bounded_email_subject(Some("  Hello \n World  ")).as_deref(), Some("Hello World"));
    assert_eq!(bounded_email_subject(Some("")), None);
    assert_eq!(bounded_email_subject(None), None);
    let long_subject = bounded_email_subject(Some(&"x".repeat(501))).expect("a long subject is bounded");
    assert!(long_subject.ends_with('…'), "an over-long subject is truncated with an ellipsis");
    assert!(long_subject.chars().count() <= 500, "the subject is bounded");

    // 4. The sender is parsed into a display name and a normalized address. The bracketed form
    //    takes the last `<...>`; a bare address also works; the name is trimmed and unquoted.
    let (address, name) = parse_sender_address(Some("Dana <dana@example.com>")).expect("a bracketed sender parses");
    assert_eq!(address, "dana@example.com");
    assert_eq!(name.as_deref(), Some("Dana"));

    let (address, name) = parse_sender_address(Some("dana@example.com")).expect("a bare sender parses");
    assert_eq!(address, "dana@example.com");
    assert_eq!(name, None, "a bare address has no display name");

    let (address, name) = parse_sender_address(Some("\"Dana\" <dana@example.com>")).expect("a quoted sender parses");
    assert_eq!(address, "dana@example.com");
    assert_eq!(name.as_deref(), Some("Dana"), "the name is unquoted");

    assert_eq!(parse_sender_address(None), None);
    assert_eq!(parse_sender_address(Some("")), None);
    assert_eq!(parse_sender_address(Some("   ")), None);

    // 5. An inbox copy from an external sender is inbound.
    let inbound = landed("m1", "inbox", Some("dana@example.com"), vec![], Some("hello"));
    let result = normalize_landed_mail(&[inbound], &internal);
    assert_eq!(result.observations.len(), 1);
    assert_eq!(result.observations[0].direction, "inbound");
    assert_eq!(result.observations[0].external_email, "dana@example.com");
    assert_eq!(result.observations[0].display_name, None);
    assert_eq!(result.observations[0].source_external_id, "m1");

    // 6. A sent copy with exactly one external recipient is outbound.
    let outbound = landed(
        "m2",
        "sent",
        Some("owner@example.com"),
        vec![("client@example.com", Some("Client"))],
        Some("reply"),
    );
    let result = normalize_landed_mail(&[outbound], &internal);
    assert_eq!(result.observations.len(), 1);
    assert_eq!(result.observations[0].direction, "outbound");
    assert_eq!(result.observations[0].external_email, "client@example.com");
    assert_eq!(result.observations[0].display_name.as_deref(), Some("Client"));

    // 7. NEGATIVE: a sent message to multiple external recipients is ambiguous, never a guess.
    let ambiguous = landed(
        "m3",
        "sent",
        Some("owner@example.com"),
        vec![("a@example.com", None), ("b@example.com", None)],
        Some("to many"),
    );
    let result = normalize_landed_mail(&[ambiguous], &internal);
    assert_eq!(result.observations.len(), 0);
    assert_eq!(result.skipped.get("ambiguous"), Some(&1));

    // 8. NEGATIVE: an internal-only message is skipped, never invented.
    let internal_only = landed(
        "m4",
        "inbox",
        Some("owner@example.com"),
        vec![("owner@example.com", None)],
        Some("to self"),
    );
    let result = normalize_landed_mail(&[internal_only], &internal);
    assert_eq!(result.observations.len(), 0);
    assert_eq!(result.skipped.get("internal_only"), Some(&1));

    // 9. NEGATIVE: an unaddressed inbox copy is skipped.
    let unaddressed = landed("m5", "inbox", None, vec![], Some("no sender"));
    let result = normalize_landed_mail(&[unaddressed], &internal);
    assert_eq!(result.observations.len(), 0);
    assert_eq!(result.skipped.get("unaddressed"), Some(&1));

    // 10. NEGATIVE: a second landing of the same source message is a duplicate, not a second event.
    let dup1 = landed("m6", "inbox", Some("dana@example.com"), vec![], Some("first"));
    let dup2 = landed("m6", "sent", Some("owner@example.com"), vec![("dana@example.com", None)], Some("second"));
    let result = normalize_landed_mail(&[dup1, dup2], &internal);
    assert_eq!(result.observations.len(), 1, "the second surfacing is a duplicate");
    assert_eq!(result.skipped.get("duplicate"), Some(&1));

    // 11. An archive copy lets the sender decide: an external sender is inbound, our own address
    //     is outbound (the recipient rule decides).
    let archive_inbound = landed("m7", "archive", Some("dana@example.com"), vec![], Some("archived inbound"));
    let result = normalize_landed_mail(&[archive_inbound], &internal);
    assert_eq!(result.observations.len(), 1);
    assert_eq!(result.observations[0].direction, "inbound");

    let archive_outbound = landed(
        "m8",
        "archive",
        Some("owner@example.com"),
        vec![("dana@example.com", None)],
        Some("archived outbound"),
    );
    let result = normalize_landed_mail(&[archive_outbound], &internal);
    assert_eq!(result.observations.len(), 1);
    assert_eq!(result.observations[0].direction, "outbound");

    // 12. Evidence is one row per counterparty email, with the observation windows.
    let observations = vec![
        MailObservation {
            source_external_id: "m1".to_owned(),
            source_account: "owner@example.com".to_owned(),
            mailbox: "inbox".to_owned(),
            uid: Some(1),
            uid_validity: "uid-validity".to_owned(),
            occurred_at: "2026-01-01T10:00:00.000Z".to_owned(),
            direction: "inbound",
            external_email: "dana@example.com".to_owned(),
            display_name: None,
            subject: Some("hello".to_owned()),
        },
        MailObservation {
            source_external_id: "m2".to_owned(),
            source_account: "owner@example.com".to_owned(),
            mailbox: "sent".to_owned(),
            uid: Some(2),
            uid_validity: "uid-validity".to_owned(),
            occurred_at: "2026-01-02T10:00:00.000Z".to_owned(),
            direction: "outbound",
            external_email: "dana@example.com".to_owned(),
            display_name: None,
            subject: Some("reply".to_owned()),
        },
    ];
    let evidence = build_mail_evidence(&observations);
    assert_eq!(evidence.len(), 1, "one row per counterparty email");
    assert_eq!(evidence[0].source, ICLOUD_MAIL_SOURCE);
    assert_eq!(evidence[0].source_identity_key, "dana@example.com");
    assert_eq!(evidence[0].inbound_count, 1);
    assert_eq!(evidence[0].outbound_count, 1);
    assert_eq!(evidence[0].is_two_way, Some(true));
    assert_eq!(evidence[0].first_observed_at.as_deref(), Some("2026-01-01T10:00:00.000Z"));
    assert_eq!(evidence[0].last_observed_at.as_deref(), Some("2026-01-02T10:00:00.000Z"));

    // 13. The interaction a observation materializes into is keyed on the source, with the subject
    //     as the title and envelope metadata as provenance (never the body).
    let interaction = mail_observation_interaction(&observations[0], "person-1");
    assert_eq!(interaction.channel, "email");
    assert_eq!(interaction.event_type, "email_received");
    assert_eq!(interaction.direction, "inbound");
    assert_eq!(interaction.source_system, ICLOUD_MAIL_SOURCE);
    assert_eq!(interaction.source_external_id, "m1");
    assert_eq!(interaction.title.as_deref(), Some("hello"));
    assert_eq!(interaction.person_id, "person-1");
}
