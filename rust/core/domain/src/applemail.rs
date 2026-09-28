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
use crate::apple_messages::{AppleHandleEvidence, IdentityEvidence, is_email_shaped};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use unicode_normalization::UnicodeNormalization;

/// The evidence source for Apple-hosted mail. Google mail is a different source.
pub const ICLOUD_MAIL_SOURCE: &str = "icloud_mail";
/// Subjects are bounded at the source's own limit; a longer one is truncated, never dropped.
pub const EMAIL_SUBJECT_MAX_LENGTH: usize = 500;
/// Source uid validity marker: Apple Mail has no IMAP UIDVALIDITY, so the landed row's own
/// account/generation is named explicitly rather than left to look like a provider value.
pub const MAIL_UID_VALIDITY: &str = "apple-mail-local";

/// An address as the envelope index reports it: `{"address": "...", "name": "..."}`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MailAddress {
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

/// One `l_applemail` row as promotion reads it. The repository normalizes driver-native
/// values on the way out, so `occurred_at` arrives as an ISO-8601 UTC string.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LandedAppleMail {
    pub source_account: String,
    pub source_message_id: String,
    pub mailbox_kind: Option<String>,
    pub mailbox_name: Option<String>,
    pub local_id: Option<i64>,
    pub message_id: Option<String>,
    pub occurred_at: Option<String>,
    pub sender: Option<String>,
    #[serde(default)]
    pub to_recipients: Vec<MailAddress>,
    #[serde(default)]
    pub cc_recipients: Vec<MailAddress>,
    #[serde(default)]
    pub bcc_recipients: Vec<MailAddress>,
    pub subject: Option<String>,
}

/// Normalize a mailbox address for matching: trimmed, lowercased, and only when it is
/// shaped like an address. The original value is never lost — the caller keeps it.
pub fn normalize_mailbox(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed.to_lowercase();
    if is_email_shaped(&normalized) {
        Some(normalized)
    } else {
        None
    }
}

/// Bound a subject for storage and display: NFKC, control characters to spaces, whitespace
/// collapsed, and anything past the limit truncated with an ellipsis. An over-long subject
/// is shortened, never dropped.
pub fn bounded_email_subject(value: Option<&str>) -> Option<String> {
    let value = value?;
    if value.is_empty() {
        return None;
    }
    let mut normalized = String::with_capacity(value.len());
    for character in value.nfkc() {
        if character.is_control() {
            normalized.push(' ');
        } else {
            normalized.push(character);
        }
    }
    let collapsed = normalized
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if collapsed.is_empty() {
        return None;
    }
    // The limit is counted in characters, not bytes: a truncation must never split one.
    let length = collapsed.chars().count();
    if length <= EMAIL_SUBJECT_MAX_LENGTH {
        return Some(collapsed);
    }
    let head: String = collapsed
        .chars()
        .take(EMAIL_SUBJECT_MAX_LENGTH - 1)
        .collect::<String>()
        .trim_end()
        .to_owned();
    Some(format!("{head}\u{2026}"))
}

/// `"Dana <dana@example.com>"` -> the address and the display name. A bare address also
/// works. The address is normalized; the name is trimmed and unquoted, or absent.
pub fn parse_sender_address(value: Option<&str>) -> Option<(String, Option<String>)> {
    let value = value?;
    if value.trim().is_empty() {
        return None;
    }
    // Bracketed form: everything before the LAST `<...>` is the display name. When the
    // bracketed address is unusable the row is unaddressed — it is never re-parsed as a
    // bare token, because that would attribute a malformed sender to whoever came first.
    let bracketed = value.rfind('<').and_then(|open| {
        let tail = value[open + 1..].trim_end();
        let inner = tail.strip_suffix('>')?;
        if inner.is_empty() || inner.contains('<') || inner.contains('>') {
            return None;
        }
        Some((&value[..open], inner))
    });

    let (address, raw_name) = match bracketed {
        Some((name, inner)) => (normalize_mailbox(inner)?, Some(name)),
        None => (normalize_mailbox(&first_address_token(value)?)?, None),
    };

    let name = raw_name
        .map(str::trim)
        .map(|name| name.trim_matches(|c| c == '"' || c == '\'').trim())
        .filter(|name| !name.is_empty())
        .map(str::to_owned);
    Some((address, name))
}

/// One neutral observation: what one landed message means, with no Person attached.
#[derive(Debug, Clone, PartialEq)]
pub struct MailObservation {
    pub source_external_id: String,
    pub source_account: String,
    pub mailbox: String,
    /// The landed row's own local id. Never an index into a page — only a source value.
    pub uid: Option<i64>,
    pub uid_validity: String,
    pub occurred_at: String,
    pub direction: &'static str,
    pub external_email: String,
    pub display_name: Option<String>,
    pub subject: Option<String>,
}

/// What a normalization pass read, and what it refused to read. A skip is a decision, so it
/// is counted and reported rather than quietly dropped.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MailNormalization {
    pub observations: Vec<MailObservation>,
    pub skipped: BTreeMap<String, i64>,
}

impl MailNormalization {
    fn skip(&mut self, reason: &str) {
        *self.skipped.entry(reason.to_owned()).or_insert(0) += 1;
    }
}

/// ISO-8601 UTC with milliseconds — the shape the deleted TypeScript wrote with
/// `toISOString()`, so stored provenance stays comparable across the port.
fn to_iso(value: Option<&str>) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed = chrono::DateTime::parse_from_rfc3339(trimmed).ok().or_else(|| {
        chrono::DateTime::parse_from_str(
            &trimmed.replacen(' ', "T", 1),
            "%Y-%m-%dT%H:%M:%S%.f%:z",
        )
        .ok()
    })?;
    Some(
        parsed
            .with_timezone(&chrono::Utc)
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string(),
    )
}

/// The addresses on a sent message that are not ours, in the order they appear across
/// To, Cc and Bcc, de-duplicated. More than one is an AMBIGUITY, never a guess: a sent
/// message to three people does not belong to any one of them.
fn external_recipients(
    row: &LandedAppleMail,
    internal: &std::collections::BTreeSet<String>,
) -> Vec<(String, Option<String>)> {
    let mut external: Vec<(String, Option<String>)> = Vec::new();
    for recipient in row
        .to_recipients
        .iter()
        .chain(row.cc_recipients.iter())
        .chain(row.bcc_recipients.iter())
    {
        let Some(address) = recipient
            .address
            .as_deref()
            .and_then(normalize_mailbox)
        else {
            continue;
        };
        if internal.contains(&address) || external.iter().any(|(known, _)| known == &address) {
            continue;
        }
        let name = recipient
            .name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned);
        external.push((address, name));
    }
    external
}

/// Landed rows -> neutral observations. Pure: the same rows always produce the same
/// observations, so a re-run of the promotion changes nothing.
///
/// The rules, unchanged from the deleted TypeScript:
///   * inbox (or `received`) -> inbound; the external sender is the counterparty
///   * sent                   -> outbound; exactly ONE external recipient or it is ambiguous
///   * archive (Gmail's All Mail, an IMAP Archive) -> the mailbox holds both directions, so
///     the sender decides: external sender = inbound, our own address = outbound
///   * internal-only, unaddressed and timestamp-less rows are skipped, never invented
///   * a second landing of the same `source_message_id` is a duplicate, not a second event
pub fn normalize_landed_mail(
    rows: &[LandedAppleMail],
    internal: &std::collections::BTreeSet<String>,
) -> MailNormalization {
    let mut out = MailNormalization::default();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for row in rows {
        let Some(occurred_at) = to_iso(row.occurred_at.as_deref()) else {
            out.skip("no_timestamp");
            continue;
        };

        let kind = row
            .mailbox_kind
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_lowercase();

        let sender = parse_sender_address(row.sender.as_deref());
        // An archive copy (Gmail's "All Mail", an IMAP Archive) holds BOTH directions, so the
        // mailbox cannot say which one this is: the sender does. Everything else keeps the
        // original rule - an inbox copy is inbound, anything else is outbound.
        let is_archive = kind == "archive" || kind == "all mail";
        let sender_address = sender.as_ref().map(|(address, _)| address.clone());
        let sender_is_external = sender_address
            .as_deref()
            .is_some_and(|address| !internal.contains(address));
        let inbound_sender = if kind == "inbox" || kind == "received" || is_archive {
            if sender_is_external {
                sender.clone()
            } else if is_archive {
                // An archive copy from one of our own addresses is mail we sent, which the
                // recipient rule below decides; a missing sender has only that rule left.
                None
            } else if sender_address.is_none() {
                out.skip("unaddressed");
                continue;
            } else {
                out.skip("internal_only");
                continue;
            }
        } else {
            None
        };

        let (direction, external_email, display_name) = match inbound_sender {
            Some((address, name)) => ("inbound", address, name),
            None => {
                let external = external_recipients(row, internal);
                if external.is_empty() {
                    out.skip("internal_only");
                    continue;
                }
                if external.len() > 1 {
                    out.skip("ambiguous");
                    continue;
                }
                let (address, name) = external.into_iter().next().unwrap_or_default();
                ("outbound", address, name)
            }
        };

        // One message can surface in two mailboxes (an inbox copy and a sent copy); the
        // landed identity is the source_message_id, so the second surfacing is a duplicate.
        if !seen.insert(row.source_message_id.clone()) {
            out.skip("duplicate");
            continue;
        }

        out.observations.push(MailObservation {
            source_external_id: row.source_message_id.clone(),
            source_account: row.source_account.clone(),
            mailbox: row
                .mailbox_name
                .clone()
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| kind.clone()),
            uid: row.local_id,
            uid_validity: MAIL_UID_VALIDITY.to_owned(),
            occurred_at,
            direction,
            external_email,
            display_name,
            subject: bounded_email_subject(row.subject.as_deref()),
        });
    }

    out
}

/// `l_applemail.source_message_id`: the stable replay identity for one landed message.
///
/// The RFC Message-ID is the identity when the source has one. When it does not, the mailbox
/// and Mail's own local row id are used — never the row's index, and never
/// `messages.message_id`, which migration 163 records as an integer shared by the same mail
/// in different mailboxes and therefore unusable as an identity.
pub fn apple_mail_replay_id(
    message_id: Option<&str>,
    mailbox_kind: &str,
    local_id: i64,
) -> Option<String> {
    if let Some(rfc) = message_id.map(str::trim).filter(|value| !value.is_empty()) {
        return Some(format!("message-id:{rfc}"));
    }
    let kind = mailbox_kind.trim().to_lowercase();
    if kind.is_empty() || local_id <= 0 {
        return None;
    }
    Some(format!("mail-local:{kind}:{local_id}"))
}

/// One evidence row per counterparty email address.
///
/// The evidence is built in the shape `apple_messages::decide_apple_handle` already takes (an
/// identity with emails and no phones) so the reconciliation rules — explicit link, exact
/// email match, a multi-match is a conflict — exist exactly once in this codebase.
pub fn build_mail_evidence(observations: &[MailObservation]) -> Vec<AppleHandleEvidence> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: BTreeMap<String, Vec<&MailObservation>> = BTreeMap::new();
    for observation in observations {
        if !groups.contains_key(&observation.external_email) {
            order.push(observation.external_email.clone());
        }
        groups
            .entry(observation.external_email.clone())
            .or_default()
            .push(observation);
    }

    order
        .into_iter()
        .filter_map(|email| {
            let mut rows = groups.remove(&email)?;
            // Oldest first: the windows below are "the first and the last time we saw them".
            rows.sort_by(|left, right| left.occurred_at.cmp(&right.occurred_at));
            let inbound = rows
                .iter()
                .filter(|row| row.direction == "inbound")
                .collect::<Vec<_>>();
            let outbound = rows
                .iter()
                .filter(|row| row.direction == "outbound")
                .collect::<Vec<_>>();
            let first = rows.first()?;
            let last = rows.last()?;
            let display_name = rows
                .iter()
                .find_map(|row| row.display_name.clone().filter(|name| !name.is_empty()));

            let fingerprint = mail_evidence_fingerprint(
                &first.source_account,
                &email,
                rows.iter().map(|row| row.source_external_id.as_str()),
            );

            Some(AppleHandleEvidence {
                source: ICLOUD_MAIL_SOURCE.to_owned(),
                source_account: first.source_account.clone(),
                source_identity_key: email.clone(),
                source_label: Some("Apple-hosted work email".to_owned()),
                display_name,
                organization: None,
                emails: vec![IdentityEvidence {
                    value: email.clone(),
                    normalized: email.clone(),
                    label: Some("Email".to_owned()),
                }],
                phones: Vec::new(),
                first_observed_at: Some(first.occurred_at.clone()),
                last_observed_at: Some(last.occurred_at.clone()),
                last_inbound_at: inbound.last().map(|row| row.occurred_at.clone()),
                last_outbound_at: outbound.last().map(|row| row.occurred_at.clone()),
                inbound_count: inbound.len() as i64,
                outbound_count: outbound.len() as i64,
                is_two_way: Some(!inbound.is_empty() && !outbound.is_empty()),
                is_owner_initiated: Some(!outbound.is_empty()),
                is_automated_or_bulk: None,
                is_organization_or_service: None,
                known_apple_contact: None,
                has_email: true,
                has_phone: false,
                coverage_note: Some(
                    "Apple iCloud Mail envelope metadata only; bodies, snippets, attachments, and raw MIME omitted."
                        .to_owned(),
                ),
                evidence_fingerprint: fingerprint,
                handle_rowids: Vec::new(),
            })
        })
        .collect()
}

/// The evidence fingerprint: sha256 over the identity plus the sorted source ids it was built
/// from — the same payload the deleted TypeScript hashed, so evidence written before the port
/// keeps the same fingerprint after it.
fn mail_evidence_fingerprint<'a>(
    source_account: &str,
    email: &str,
    ids: impl Iterator<Item = &'a str>,
) -> String {
    let json_string =
        |value: &str| serde_json::to_string(value).unwrap_or_else(|_| format!("\"{value}\""));
    let mut sorted: Vec<&str> = ids.collect();
    sorted.sort_unstable();
    let ids = sorted
        .into_iter()
        .map(json_string)
        .collect::<Vec<_>>()
        .join(",");
    let payload = format!(
        "{{\"source\":{},\"sourceAccount\":{},\"email\":{},\"ids\":[{}]}}",
        json_string(ICLOUD_MAIL_SOURCE),
        json_string(source_account),
        json_string(email),
        ids
    );
    let mut hasher = Sha256::new();
    hasher.update(payload.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// The canonical interaction one observation materializes into, once its evidence is linked to
/// a Person. Envelope metadata as provenance; the message itself is never stored.
#[derive(Debug, Clone, PartialEq)]
pub struct MailInteraction {
    pub person_id: String,
    pub channel: &'static str,
    pub event_type: &'static str,
    pub direction: &'static str,
    pub occurred_at: String,
    pub title: Option<String>,
    pub source_system: &'static str,
    pub source_external_id: String,
    pub source_metadata: Value,
}

pub fn mail_observation_interaction(
    observation: &MailObservation,
    canonical_person_id: &str,
) -> MailInteraction {
    MailInteraction {
        person_id: canonical_person_id.to_owned(),
        channel: "email",
        event_type: if observation.direction == "inbound" {
            "email_received"
        } else {
            "email_sent"
        },
        direction: observation.direction,
        occurred_at: observation.occurred_at.clone(),
        title: observation.subject.clone(),
        source_system: ICLOUD_MAIL_SOURCE,
        source_external_id: observation.source_external_id.clone(),
        source_metadata: json!({
            "sourceAccount": observation.source_account,
            "mailbox": observation.mailbox,
            "uid": observation.uid,
            "uidValidity": observation.uid_validity,
            "metadataOnly": true,
        }),
    }
}

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

        let mut internal_only =
            inbound_row("in-2", "Lisa <lisa@culebraluxe.com>", "2026-09-01T11:00:00Z");
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
        let unaddressed = vec![inbound_row("in-5", "not an address", "2026-09-01T09:00:00Z")];
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
        assert_eq!(dana.source_label.as_deref(), Some("Apple-hosted work email"));
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

/// The first token that looks like an address (`[^\s<>]+@[^\s<>]+`, the rule the deleted
/// TypeScript used before it fell back to `null`).
fn first_address_token(value: &str) -> Option<String> {
    let at = value.find('@')?;
    let is_token_char = |c: char| !c.is_whitespace() && c != '<' && c != '>';
    let start = value[..at]
        .char_indices()
        .rev()
        .find(|(_, c)| !is_token_char(*c))
        .map(|(index, c)| index + c.len_utf8())
        .unwrap_or(0);
    let end = value[at + 1..]
        .char_indices()
        .find(|(_, c)| !is_token_char(*c))
        .map(|(index, _)| at + 1 + index)
        .unwrap_or(value.len());
    Some(value[start..end].to_owned())
}
