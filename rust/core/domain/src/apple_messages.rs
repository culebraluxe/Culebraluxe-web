// ---------------------------------------------------------------------------
// Apple Messages — intake rules, now in Rust.
//
// Source of truth: `lib/relationship-intel/apple-messages.ts`,
// `apple-message-materializer.ts`, `normalize.ts` and `reconcile.ts`. Those files were deleted with
// the TypeScript engine in the 2026-09 port (commit 4cf98110) and recovered from that commit, so the
// rules below are a translation and not a re-invention: the same identity grouping, the same
// inbound/outbound counting, the same refusal to guess an ambiguous phone number, and the same
// "only an authoritative link may be materialized" rule.
//
// Pure: no database, no filesystem, no clock. `cli` parses the export package into these types and
// `db` owns the SQL.
// ---------------------------------------------------------------------------
use chrono::{SecondsFormat, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::relationship_evidence::RelationshipDecision;

mod types;
mod normalize;
mod evidence;
#[allow(unused_imports)]
pub use types::*;
#[allow(unused_imports)]
pub use normalize::*;
#[allow(unused_imports)]
pub use evidence::*;

pub const APPLE_MESSAGES_SOURCE: &str = "apple_messages";

/// Deterministic fallback account when no message carries a usable account address.
pub const APPLE_LOCAL_SOURCE_ACCOUNT: &str = "apple_messages_local";

/// Maximum length of the bounded one-line memory cue persisted on an interaction.
pub const APPLE_PREVIEW_MAX_LENGTH: usize = 160;

pub const REL_INTEL_RULE_VERSION: &str = "rel-intel/v1";

/// Apple Messages timestamps are nanoseconds since 2001-01-01T00:00:00Z.
const APPLE_EPOCH_UNIX_OFFSET_SECONDS: i64 = 978_307_200;

/// Group chats carry a group marker in the Apple chat GUID.
const GROUP_CHAT_GUID_MARKER: &str = "group";

/// Apple group chats use the `any;+;` GUID form (style 43); 1:1 uses `any;-;`.
const GROUP_CHAT_GUID_PLUS_PREFIX: &str = "any;+;";

// ---------------------------------------------------------------------------
// Export package shapes (identities.jsonl / messages.jsonl, written by the
// Swift `apple-messages-export` package — its key names are the contract).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn handle(rowid: i64, id: &str, service: Option<&str>) -> AppleMessagesHandle {
        AppleMessagesHandle {
            rowid: Some(rowid),
            id: Some(id.to_owned()),
            service: service.map(str::to_owned),
            ..Default::default()
        }
    }

    #[allow(clippy::too_many_arguments)]
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
    fn group_chat_guids_are_never_one_persons_relationship() {
        assert!(is_group_chat_guid(Some("any;+;abc123")));
        assert!(is_group_chat_guid(Some("iMessage;+;group-chat-42")));
        assert!(!is_group_chat_guid(Some("any;-;+17875551234")));
        assert!(!is_group_chat_guid(None));
    }

    #[test]
    fn apple_timestamps_land_on_the_instant_the_exporter_reports() {
        // 0 nanoseconds after the Apple reference date is 2001-01-01T00:00:00Z.
        assert_eq!(
            apple_nanos_to_iso(0.0).as_deref(),
            Some("2001-01-01T00:00:00.000Z")
        );
        assert_eq!(
            apple_nanos_to_iso(1_000_000_000.0).as_deref(),
            Some("2001-01-01T00:00:01.000Z")
        );
        assert_eq!(apple_nanos_to_iso(f64::NAN), None);
    }

    #[test]
    fn ambiguous_phone_numbers_are_quarantined_not_guessed() {
        assert_eq!(
            normalize_phone("787-555-1234").as_deref(),
            Some("+17875551234")
        );
        assert_eq!(
            normalize_phone("+1 (787) 555-1234").as_deref(),
            Some("+17875551234")
        );
        assert_eq!(normalize_phone("5551234"), None);
        assert_eq!(normalize_phone("44 20 7946 0958"), None);
        assert_eq!(normalize_phone("no digits here"), None);
    }

    #[test]
    fn emails_normalize_to_lowercase_and_reject_non_addresses() {
        assert_eq!(normalize_email("  A@B.COM ").as_deref(), Some("a@b.com"));
        assert_eq!(normalize_email("not-an-email"), None);
        assert_eq!(normalize_email("two@at@example.com"), None);
        assert_eq!(normalize_email("no@domain"), None);
    }

    #[test]
    fn handle_identities_split_the_same_way_the_typescript_did() {
        let (emails, phones) = handle_to_identities("Client@Example.com");
        assert_eq!(emails.len(), 1);
        assert!(phones.is_empty());
        assert_eq!(emails[0].normalized, "client@example.com");

        let (emails, phones) = handle_to_identities("(787) 555-1234");
        assert!(emails.is_empty());
        assert_eq!(phones.len(), 1);
        assert_eq!(phones[0].value, "(787) 555-1234");
    }

    #[test]
    fn the_fingerprint_matches_the_typescript_hash_bit_for_bit() {
        // Reference values produced by the deleted `normalize.ts` fingerprint, run under node.
        assert_eq!(fingerprint(""), "488bdcb81aee8d83");
        assert_eq!(
            fingerprint("apple_messages|local|+17875551234|3|1"),
            "9b36555c50d880a7"
        );
    }

    #[test]
    fn the_preview_is_bounded_and_never_fabricates_prose() {
        assert_eq!(
            bounded_preview(Some("  hello \n world  ")).as_deref(),
            Some("hello world")
        );
        assert_eq!(bounded_preview(Some("   ")), None);
        assert_eq!(bounded_preview(None), None);
        let preview = bounded_preview(Some(&"x".repeat(500))).expect("preview");
        assert_eq!(preview.encode_utf16().count(), APPLE_PREVIEW_MAX_LENGTH);
        assert!(preview.ends_with('…'));
    }

    #[test]
    fn the_exporter_jsonl_keys_deserialize_exactly() {
        // The live check caught this: serde's camelCase turns `date_iso` into `dateIso`, but the
        // Swift exporter writes `dateISO`, so every message silently lost its timestamp.
        let line = r#"{"rowid":1,"guid":"ABC","chatGuid":"any;-;+17875551234","handleId":7,"handleValue":"+17875551234","service":"iMessage","account":"owner@example.com","date":722000000000000000,"dateISO":"2026-01-02T03:04:05.000Z","isFromMe":0,"text":"hello","hasAttachments":0}"#;
        let message: AppleMessagesMessage = serde_json::from_str(line).expect("message parses");
        assert_eq!(
            message.date_iso.as_deref(),
            Some("2026-01-02T03:04:05.000Z")
        );
        assert_eq!(
            effective_date_iso(&message).as_deref(),
            Some("2026-01-02T03:04:05.000Z")
        );
        assert_eq!(message.handle_id, Some(7));
        assert_eq!(message.is_from_me, Some(0));
        assert_eq!(message.has_attachments, Some(0));

        let handle_line = r#"{"rowid":7,"id":"+17875551234","country":"US","service":"iMessage","uncanonicalizedId":null,"personCentricId":null}"#;
        let handle: AppleMessagesHandle = serde_json::from_str(handle_line).expect("handle parses");
        assert_eq!(handle.id.as_deref(), Some("+17875551234"));
        assert_eq!(handle.service.as_deref(), Some("iMessage"));
    }

    #[test]
    fn every_rowid_for_one_identity_is_aggregated_and_group_chats_are_excluded() {
        let handles = vec![
            handle(1, "+17875551234", Some("iMessage")),
            handle(2, "+17875551234", Some("SMS")),
            handle(3, "+17870001111", Some("iMessage")),
        ];
        let messages = vec![
            message(
                "m1",
                1,
                Some("any;-;+17875551234"),
                Some("2024-01-01T00:00:00.000Z"),
                0,
                Some("iMessage"),
                Some("hi"),
            ),
            message(
                "m2",
                2,
                Some("any;-;+17875551234"),
                Some("2024-03-01T00:00:00.000Z"),
                1,
                Some("SMS"),
                Some("reply"),
            ),
            // Same handle, but a group chat: counted for nobody.
            message(
                "m3",
                2,
                Some("any;+;group-7"),
                Some("2024-04-01T00:00:00.000Z"),
                0,
                Some("SMS"),
                Some("group noise"),
            ),
        ];

        let evidence = build_handle_evidence("owner@example.com", &handles, &messages);
        assert_eq!(evidence.len(), 2);

        let two_way = evidence
            .iter()
            .find(|row| row.source_identity_key == "+17875551234")
            .expect("two-way row");
        assert_eq!(two_way.inbound_count, 1);
        assert_eq!(two_way.outbound_count, 1);
        assert_eq!(two_way.is_two_way, Some(true));
        assert_eq!(two_way.source_label.as_deref(), Some("iMessage"));
        assert_eq!(
            two_way.first_observed_at.as_deref(),
            Some("2024-01-01T00:00:00.000Z")
        );
        assert_eq!(
            two_way.last_outbound_at.as_deref(),
            Some("2024-03-01T00:00:00.000Z")
        );
        assert_eq!(two_way.handle_rowids, vec![1, 2]);

        let silent = evidence
            .iter()
            .find(|row| row.source_identity_key == "+17870001111")
            .expect("silent row");
        assert_eq!(silent.total_count(), 0);
        assert_eq!(silent.is_two_way, None);
    }

    #[test]
    fn the_source_account_comes_from_the_messages_then_falls_back() {
        let messages = vec![message(
            "m1",
            1,
            Some("any;-;+17875551234"),
            Some("2024-01-01T00:00:00.000Z"),
            0,
            Some("iMessage"),
            None,
        )];
        assert_eq!(derive_source_account(&messages), "owner@example.com");
        assert_eq!(derive_source_account(&[]), APPLE_LOCAL_SOURCE_ACCOUNT);
    }

    #[test]
    fn only_an_authoritative_link_materializes() {
        let evidence = build_handle_evidence(
            "owner@example.com",
            &[handle(1, "+17875551234", Some("iMessage"))],
            &[message(
                "m1",
                1,
                Some("any;-;+17875551234"),
                Some("2024-01-01T00:00:00.000Z"),
                1,
                Some("iMessage"),
                Some("hello"),
            )],
        );
        let row = &evidence[0];

        let linked = decide_apple_handle(
            row,
            &AppleHandleLookup {
                phone_owners: vec!["person-1".to_owned()],
                ..Default::default()
            },
        );
        assert_eq!(linked.review_state, "exact_linked");
        assert_eq!(linked.canonical_person_id.as_deref(), Some("person-1"));
        assert_eq!(linked.match_method, "exact_phone");

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
        assert_eq!(conflicting.reason, "identity_matches_multiple_people");

        // Nothing exact, but the owner wrote to it: reviewable, never auto-linked.
        let review = decide_apple_handle(row, &AppleHandleLookup::default());
        assert_eq!(review.review_state, "review_required");
        assert_eq!(review.canonical_person_id, None);

        // An explicit link outranks every guess.
        let explicit = decide_apple_handle(
            row,
            &AppleHandleLookup {
                explicit_link: Some("person-9".to_owned()),
                phone_multi_match: true,
                ..Default::default()
            },
        );
        assert_eq!(explicit.canonical_person_id.as_deref(), Some("person-9"));

        let mut unknown = row.clone();
        unknown.phones = Vec::new();
        unknown.has_phone = false;
        assert_eq!(
            decide_apple_handle(&unknown, &AppleHandleLookup::default()).review_state,
            "deferred"
        );
    }
}
