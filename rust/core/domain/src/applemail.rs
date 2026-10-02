// ---------------------------------------------------------------------------
// Apple Mail (iCloud Mail) intake and promotion rules.
//
// The Rust replacement for `lib/relationship-intel/icloud-mail.ts` and
// `lib/relationship-intel/applemail.ts`, deleted with the TypeScript application in
// commit 4cf98110. The behaviour is the one those two files described; the cases they
// encoded are unit-tested at the bottom of this file.
//
// The chain this file serves:
//
//   local Mail.app Envelope Index -> l_applemail        (source evidence, no judgment)
//     -> normalize_landed_mail                          (neutral observations)
//     -> build_mail_evidence                            (one row per counterparty)
//     -> reconcile + interaction                        (the CRM pane's comms event)
//
// Everything here is pure: no database, no clock, no filesystem. Promotion decides
// nothing about a Person — that stays in `apple_messages::decide_apple_handle`, and mail
// evidence is deliberately built in the shape that function already takes, so the
// reconciliation rules exist exactly once instead of twice.
//
// PRIVACY: envelope metadata only. No body, snippet, attachment or raw MIME reaches these
// rules, and none of it is stored.
// ---------------------------------------------------------------------------
use crate::apple_messages::{is_email_shaped, AppleHandleEvidence, IdentityEvidence};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use unicode_normalization::UnicodeNormalization;

mod evidence;
mod interaction;
mod landing;
mod normalize;
mod types;
#[allow(unused_imports)]
pub use evidence::*;
#[allow(unused_imports)]
pub use interaction::*;
#[allow(unused_imports)]
pub use landing::*;
#[allow(unused_imports)]
pub use normalize::*;
#[allow(unused_imports)]
pub use types::*;

/// The evidence source for Apple-hosted mail. Google mail is a different source.
pub const ICLOUD_MAIL_SOURCE: &str = "icloud_mail";

/// Subjects are bounded at the source's own limit; a longer one is truncated, never dropped.
pub const EMAIL_SUBJECT_MAX_LENGTH: usize = 500;

/// Source uid validity marker: Apple Mail has no IMAP UIDVALIDITY, so the landed row's own
/// account/generation is named explicitly rather than left to look like a provider value.
pub const MAIL_UID_VALIDITY: &str = "apple-mail-local";

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn internal() -> BTreeSet<String> {
        ["lisa@culebraluxe.com", "penfield33@gmail.com"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    fn address(value: &str) -> MailAddress {
        MailAddress {
            address: Some(value.to_owned()),
            name: None,
        }
    }

    fn inbound_row(id: &str, sender: &str, occurred_at: &str) -> LandedAppleMail {
        LandedAppleMail {
            source_account: "lisa@culebraluxe.com".into(),
            source_message_id: id.into(),
            mailbox_kind: Some("inbox".into()),
            mailbox_name: Some("INBOX".into()),
            local_id: Some(7),
            message_id: Some(format!("<{id}@mail.example>")),
            occurred_at: Some(occurred_at.into()),
            sender: Some(sender.into()),
            subject: Some("Hello there".into()),
            ..Default::default()
        }
    }

    #[test]
    fn replay_identity_prefers_the_rfc_message_id_and_never_invents_one() {
        assert_eq!(
            apple_mail_replay_id(Some(" <abc@mail.example> "), "inbox", 12).as_deref(),
            Some("message-id:<abc@mail.example>")
        );
        assert_eq!(
            apple_mail_replay_id(None, "Sent", 12).as_deref(),
            Some("mail-local:sent:12")
        );
        // No RFC, no usable local id: refusing beats landing under a made-up key.
        assert_eq!(apple_mail_replay_id(None, "inbox", 0), None);
        assert_eq!(apple_mail_replay_id(None, "  ", 12), None);
    }

    #[test]
    fn mailbox_normalization_keeps_only_address_shaped_values() {
        assert_eq!(
            normalize_mailbox("  Dana@Example.COM ").as_deref(),
            Some("dana@example.com")
        );
        assert_eq!(normalize_mailbox("Dana Q"), None);
        assert_eq!(normalize_mailbox("dana@example"), None);
        assert_eq!(normalize_mailbox(""), None);
    }

    #[test]
    fn sender_parsing_handles_brackets_quotes_and_bare_addresses() {
        assert_eq!(
            parse_sender_address(Some("Dana <Dana@Example.com>")),
            Some(("dana@example.com".to_owned(), Some("Dana".to_owned())))
        );
        // Trim first, then strip the quotes: a quoted name can carry trailing space.
        assert_eq!(
            parse_sender_address(Some("\"Dana Q\" <dana@example.com>")),
            Some(("dana@example.com".to_owned(), Some("Dana Q".to_owned())))
        );
        assert_eq!(
            parse_sender_address(Some("dana@example.com")),
            Some(("dana@example.com".to_owned(), None))
        );
        // Multi-address senders are not produced by the bridge, and the rule the deleted
        // TypeScript encoded was "the last bracketed address wins" (its non-greedy `.*?<`
        // backtracks to the final `<...>` that ends the string). Kept as-is, so a row that
        // somehow carries two addresses resolves exactly as it did before the port.
        assert_eq!(
            parse_sender_address(Some("Dana Q <dana@example.com>, Bob <bob@example.com>")),
            Some((
                "bob@example.com".to_owned(),
                Some("Dana Q <dana@example.com>, Bob".to_owned())
            ))
        );
        // A malformed bracket is unaddressed, not a reason to grab the first token.
        assert_eq!(parse_sender_address(Some("Dana <not-an-address>")), None);
        assert_eq!(parse_sender_address(Some("")), None);
        assert_eq!(parse_sender_address(None), None);
    }

    #[test]
    fn subjects_are_collapsed_and_bounded_never_dropped() {
        assert_eq!(
            bounded_email_subject(Some("  Invoice\n\tfor   Unit 7 ")).as_deref(),
            Some("Invoice for Unit 7")
        );
        assert_eq!(bounded_email_subject(Some("   \n  ")), None);
        assert_eq!(bounded_email_subject(None), None);

        let long = "a".repeat(EMAIL_SUBJECT_MAX_LENGTH + 25);
        let bounded = bounded_email_subject(Some(&long)).unwrap_or_default();
        assert_eq!(bounded.chars().count(), EMAIL_SUBJECT_MAX_LENGTH);
        assert!(bounded.ends_with('\u{2026}'));
    }

    #[test]
    fn normalization_decides_direction_and_refuses_to_guess() {
        let mut sent = inbound_row("sent-1", "Dana <dana@example.com>", "2026-09-01T10:00:00Z");
        sent.mailbox_kind = Some("sent".into());
        sent.sender = Some("lisa@culebraluxe.com".into());
        sent.to_recipients = vec![address("Dana@Example.com")];

        let mut internal_only = inbound_row(
            "in-2",
            "Lisa <lisa@culebraluxe.com>",
            "2026-09-01T11:00:00Z",
        );
        internal_only.mailbox_kind = Some("sent".into());
        internal_only.to_recipients = vec![address("lisa@culebraluxe.com")];

        let mut ambiguous = inbound_row("in-3", "Dana <dana@example.com>", "2026-09-01T12:00:00Z");
        ambiguous.mailbox_kind = Some("sent".into());
        ambiguous.to_recipients = vec![address("dana@example.com"), address("bob@example.com")];

        let no_timestamp = LandedAppleMail {
            source_message_id: "in-4".into(),
            mailbox_kind: Some("inbox".into()),
            occurred_at: None,
            sender: Some("dana@example.com".into()),
            ..Default::default()
        };

        let rows = vec![
            inbound_row("in-1", "Dana <dana@example.com>", "2026-09-01T09:00:00Z"),
            sent,
            internal_only,
            ambiguous,
            no_timestamp,
            // The same landed identity surfacing twice is a duplicate, not a second event.
            inbound_row("in-1", "Dana <dana@example.com>", "2026-09-01T09:00:00Z"),
        ];

        let out = normalize_landed_mail(&rows, &internal());
        assert_eq!(out.observations.len(), 2);
        assert_eq!(out.observations[0].direction, "inbound");
        assert_eq!(out.observations[0].external_email, "dana@example.com");
        assert_eq!(out.observations[0].mailbox, "INBOX");
        assert_eq!(out.observations[0].uid, Some(7));
        assert_eq!(out.observations[0].uid_validity, MAIL_UID_VALIDITY);
        assert_eq!(out.observations[1].direction, "outbound");
        assert_eq!(out.skipped.get("internal_only"), Some(&1));
        assert_eq!(out.skipped.get("ambiguous"), Some(&1));
        assert_eq!(out.skipped.get("no_timestamp"), Some(&1));
        assert_eq!(out.skipped.get("duplicate"), Some(&1));
        assert_eq!(out.skipped.get("unaddressed"), None);

        // An unaddressed inbox row is skipped, never attributed to something else on the row.
        let unaddressed = vec![inbound_row(
            "in-5",
            "not an address",
            "2026-09-01T09:00:00Z",
        )];
        let out = normalize_landed_mail(&unaddressed, &internal());
        assert!(out.observations.is_empty());
        assert_eq!(out.skipped.get("unaddressed"), Some(&1));
    }

    #[test]
    fn an_archive_copy_takes_its_direction_from_the_sender() {
        // Gmail's "[Gmail]/All Mail" holds both directions, so the mailbox cannot decide: the
        // sender does. Without this, every archive row was read as outbound — a Gmail account's
        // inbound business mail became outbound mail addressed to a stranger.
        let mut inbound = inbound_row("arc-1", "Dana <dana@example.com>", "2026-09-01T09:00:00Z");
        inbound.mailbox_kind = Some("archive".into());
        inbound.mailbox_name = Some("[Gmail]/All Mail".into());

        let mut outbound = inbound_row("arc-2", "lisa@culebraluxe.com", "2026-09-01T10:00:00Z");
        outbound.mailbox_kind = Some("all mail".into());
        outbound.mailbox_name = Some("[Gmail]/All Mail".into());
        outbound.to_recipients = vec![address("Dana@Example.com")];

        let mut self_mail = inbound_row("arc-3", "lisa@culebraluxe.com", "2026-09-01T11:00:00Z");
        self_mail.mailbox_kind = Some("archive".into());
        self_mail.to_recipients = vec![address("lisa@culebraluxe.com")];

        let mut ambiguous = inbound_row("arc-4", "lisa@culebraluxe.com", "2026-09-01T12:00:00Z");
        ambiguous.mailbox_kind = Some("archive".into());
        ambiguous.to_recipients = vec![address("dana@example.com"), address("bob@example.com")];

        let out = normalize_landed_mail(&[inbound, outbound, self_mail, ambiguous], &internal());

        assert_eq!(out.observations.len(), 2);
        assert_eq!(out.observations[0].direction, "inbound");
        assert_eq!(out.observations[0].external_email, "dana@example.com");
        assert_eq!(out.observations[0].mailbox, "[Gmail]/All Mail");
        assert_eq!(out.observations[1].direction, "outbound");
        assert_eq!(out.observations[1].external_email, "dana@example.com");
        // Mail we sent to ourselves, and mail with two possible counterparties, is not guessed.
        assert_eq!(out.skipped.get("internal_only"), Some(&1));
        assert_eq!(out.skipped.get("ambiguous"), Some(&1));
    }

    #[test]
    fn an_inbox_copy_and_its_archive_copy_are_one_message() {
        // Mail keeps the same message in INBOX and in All Mail. The landing identity is the RFC
        // Message-ID, so the second surfacing is a duplicate, never a second event.
        let inbox = inbound_row("dup-1", "Dana <dana@example.com>", "2026-09-01T09:00:00Z");
        let mut archive = inbound_row("dup-1", "Dana <dana@example.com>", "2026-09-01T09:00:00Z");
        archive.mailbox_kind = Some("archive".into());
        archive.mailbox_name = Some("[Gmail]/All Mail".into());

        let out = normalize_landed_mail(&[inbox, archive], &internal());
        assert_eq!(out.observations.len(), 1);
        assert_eq!(out.observations[0].direction, "inbound");
        assert_eq!(out.skipped.get("duplicate"), Some(&1));
    }

    fn mail_observations() -> Vec<MailObservation> {
        let rows = vec![
            inbound_row("in-1", "Dana <dana@example.com>", "2026-09-03T09:00:00Z"),
            inbound_row("in-2", "Dana <dana@example.com>", "2026-09-01T09:00:00Z"),
            inbound_row("in-3", "Sam <sam@example.com>", "2026-09-02T09:00:00Z"),
        ];
        let mut observations = normalize_landed_mail(&rows, &internal()).observations;
        let mut sent = inbound_row("out-1", "lisa@culebraluxe.com", "2026-09-04T09:00:00Z");
        sent.mailbox_kind = Some("sent".into());
        sent.to_recipients = vec![MailAddress {
            address: Some("dana@example.com".into()),
            name: Some("Dana Q".into()),
        }];
        observations.extend(normalize_landed_mail(&[sent], &internal()).observations);
        observations
    }

    #[test]
    fn evidence_groups_by_counterparty_and_counts_both_directions() {
        let evidence = build_mail_evidence(&mail_observations());
        assert_eq!(evidence.len(), 2);

        let dana = evidence
            .iter()
            .find(|row| row.source_identity_key == "dana@example.com")
            .expect("dana evidence");
        assert_eq!(dana.source, ICLOUD_MAIL_SOURCE);
        assert_eq!(dana.source_account, "lisa@culebraluxe.com");
        assert_eq!(
            dana.source_label.as_deref(),
            Some("Apple-hosted work email")
        );
        assert_eq!(dana.inbound_count, 2);
        assert_eq!(dana.outbound_count, 1);
        assert_eq!(dana.is_two_way, Some(true));
        assert_eq!(dana.is_owner_initiated, Some(true));
        assert!(dana.has_email);
        assert!(!dana.has_phone);
        assert_eq!(
            dana.first_observed_at.as_deref(),
            Some("2026-09-01T09:00:00.000Z"),
            "windows are ordered by the source's own timestamps, not by read order"
        );
        assert_eq!(
            dana.last_observed_at.as_deref(),
            Some("2026-09-04T09:00:00.000Z")
        );
        assert_eq!(
            dana.last_inbound_at.as_deref(),
            Some("2026-09-03T09:00:00.000Z")
        );
        assert_eq!(
            dana.last_outbound_at.as_deref(),
            Some("2026-09-04T09:00:00.000Z")
        );
        assert_eq!(dana.display_name.as_deref(), Some("Dana"));
        assert_eq!(dana.emails.len(), 1);
        assert_eq!(dana.emails[0].normalized, "dana@example.com");
        assert!(dana.phones.is_empty());
        assert!(dana.handle_rowids.is_empty());

        let sam = evidence
            .iter()
            .find(|row| row.source_identity_key == "sam@example.com")
            .expect("sam evidence");
        assert_eq!(sam.inbound_count, 1);
        assert_eq!(sam.outbound_count, 0);
        assert_eq!(sam.is_two_way, Some(false));
        assert_eq!(sam.is_owner_initiated, Some(false));
        assert_ne!(dana.evidence_fingerprint, sam.evidence_fingerprint);
    }

    #[test]
    fn evidence_fingerprint_is_stable_for_the_same_identity() {
        let first = build_mail_evidence(&mail_observations());
        let second = build_mail_evidence(&mail_observations());
        for index in 0..first.len() {
            assert_eq!(
                first[index].evidence_fingerprint, second[index].evidence_fingerprint,
                "the same observation set always hashes the same"
            );
            assert_eq!(
                first[index].evidence_fingerprint.len(),
                64,
                "sha256 hex, the shape the evidence table already holds"
            );
        }
    }

    #[test]
    fn an_observation_becomes_the_interaction_the_crm_pane_reads() {
        let observations = mail_observations();
        let observation = observations
            .iter()
            .find(|row| row.source_external_id == "in-1")
            .expect("inbound observation");
        let interaction =
            mail_observation_interaction(observation, "00000000-0000-0000-0000-000000000001");
        assert_eq!(interaction.channel, "email");
        assert_eq!(interaction.event_type, "email_received");
        assert_eq!(interaction.direction, "inbound");
        assert_eq!(interaction.source_system, ICLOUD_MAIL_SOURCE);
        assert_eq!(interaction.source_external_id, "in-1");
        assert_eq!(interaction.title.as_deref(), Some("Hello there"));
        assert_eq!(interaction.source_metadata["metadataOnly"], json!(true));
        assert_eq!(
            interaction.source_metadata["mailbox"],
            json!("INBOX"),
            "the pane's provenance is the mailbox name, not the kind"
        );
        assert_eq!(
            interaction.source_metadata["uidValidity"],
            json!(MAIL_UID_VALIDITY)
        );

        let sent = observations
            .iter()
            .find(|row| row.source_external_id == "out-1")
            .expect("outbound observation");
        let interaction =
            mail_observation_interaction(sent, "00000000-0000-0000-0000-000000000001");
        assert_eq!(interaction.event_type, "email_sent");
        assert_eq!(interaction.direction, "outbound");
    }
}
