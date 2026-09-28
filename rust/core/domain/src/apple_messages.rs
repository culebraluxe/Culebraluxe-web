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

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppleMessagesHandle {
    pub rowid: Option<i64>,
    pub id: Option<String>,
    pub country: Option<String>,
    pub service: Option<String>,
    pub uncanonicalized_id: Option<String>,
    pub person_centric_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppleMessagesMessage {
    pub rowid: Option<i64>,
    pub guid: Option<String>,
    pub chat_guid: Option<String>,
    pub handle_id: Option<i64>,
    pub handle_value: Option<String>,
    pub service: Option<String>,
    pub account: Option<String>,
    pub date: Option<f64>,
    /// The exporter spells this `dateISO` (serde's camelCase would produce `dateIso`).
    #[serde(default, rename = "dateISO")]
    pub date_iso: Option<String>,
    pub is_from_me: Option<i64>,
    pub text: Option<String>,
    pub has_attachments: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct AppleMessagesExport {
    pub source_account: String,
    pub handles: Vec<AppleMessagesHandle>,
    pub messages: Vec<AppleMessagesMessage>,
}

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

/// A 1:1 vs group chat is decided by the Apple chat GUID, never by participant count.
pub fn is_group_chat_guid(chat_guid: Option<&str>) -> bool {
    let Some(guid) = chat_guid else {
        return false;
    };
    let lower = guid.to_lowercase();
    lower.contains(GROUP_CHAT_GUID_MARKER) || lower.starts_with(GROUP_CHAT_GUID_PLUS_PREFIX)
}

/// Convert an Apple INTEGER-nanoseconds timestamp to an ISO-8601 string.
pub fn apple_nanos_to_iso(raw: f64) -> Option<String> {
    if !raw.is_finite() {
        return None;
    }
    let unix = raw / 1_000_000_000.0 + APPLE_EPOCH_UNIX_OFFSET_SECONDS as f64;
    if !(unix > 0.0) {
        return None;
    }
    let seconds = unix.trunc() as i64;
    let nanos = (((unix - unix.trunc()) * 1_000_000_000.0).round() as i64).clamp(0, 999_999_999);
    let stamp = Utc.timestamp_opt(seconds, nanos as u32).single()?;
    Some(stamp.to_rfc3339_opts(SecondsFormat::Millis, true))
}

/// The observed timestamp of a message: the exporter's ISO value, or the raw Apple timestamp when
/// the exporter could not render one.
pub fn effective_date_iso(message: &AppleMessagesMessage) -> Option<String> {
    match message.date_iso.as_ref().map(|value| value.trim()) {
        Some(value) if !value.is_empty() => Some(value.to_owned()),
        _ => message.date.and_then(apple_nanos_to_iso),
    }
}

/// Normalize an email for matching. The original value is always retained by the caller.
pub fn normalize_email(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.encode_utf16().count() > 320 {
        return None;
    }
    if !is_email_shaped(trimmed) {
        return None;
    }
    Some(trimmed.to_lowercase())
}

/// Shared with `crate::applemail::normalize_mailbox`: one email shape rule for the whole crate.
pub(crate) fn is_email_shaped(value: &str) -> bool {
    if value.chars().any(char::is_whitespace) || value.matches('@').count() != 1 {
        return false;
    }
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    if local.is_empty() || domain.is_empty() || domain.contains('@') {
        return false;
    }
    match domain.split_once('.') {
        Some((left, right)) => !left.is_empty() && !right.is_empty(),
        None => false,
    }
}

/// Normalize a phone number for matching (US/Puerto Rico, canonical E.164).
/// `None` means "not reliably matchable" — quarantined, never guessed at.
pub fn normalize_phone(input: &str) -> Option<String> {
    let digits: String = input.chars().filter(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    if digits.len() == 11 && digits.starts_with('1') {
        return Some(format!("+{digits}"));
    }
    if digits.len() == 10 {
        return Some(format!("+1{digits}"));
    }
    None
}

#[derive(Debug, Clone, Serialize)]
pub struct IdentityEvidence {
    pub value: String,
    pub normalized: String,
    pub label: Option<String>,
}

/// Split an Apple handle id into neutral email/phone identity evidence.
pub fn handle_to_identities(handle_id: &str) -> (Vec<IdentityEvidence>, Vec<IdentityEvidence>) {
    let raw = handle_id.trim();
    if raw.is_empty() {
        return (Vec::new(), Vec::new());
    }
    if let Some(normalized) = normalize_email(raw) {
        return (
            vec![IdentityEvidence {
                value: raw.to_owned(),
                normalized,
                label: None,
            }],
            Vec::new(),
        );
    }
    if let Some(normalized) = normalize_phone(raw) {
        return (
            Vec::new(),
            vec![IdentityEvidence {
                value: raw.to_owned(),
                normalized,
                label: None,
            }],
        );
    }
    (Vec::new(), Vec::new())
}

/// Stable deterministic fingerprint for replay/dedup. Not a cryptographic hash: it is the same
/// algorithm the TypeScript engine used (UTF-16 code units, `cyrb53`-style), so fingerprints written
/// before the port keep their shape and width.
pub fn fingerprint(input: &str) -> String {
    let mut h1: u32 = 0xdeadbeef;
    let mut h2: u32 = 0x41c6ce57;
    for unit in input.encode_utf16() {
        h1 = (h1 ^ unit as u32).wrapping_mul(2_654_435_761);
        h2 = (h2 ^ unit as u32).wrapping_mul(1_597_334_677);
    }
    h1 = (h1 ^ (h1 >> 16)).wrapping_mul(2_246_822_507)
        ^ (h2 ^ (h2 >> 13)).wrapping_mul(3_266_489_909);
    h2 = (h2 ^ (h2 >> 16)).wrapping_mul(2_246_822_507)
        ^ (h1 ^ (h1 >> 13)).wrapping_mul(3_266_489_909);
    format!("{h2:08x}{h1:08x}")
}

/// Bounded one-line memory cue: whitespace collapsed, capped at 160 characters, `None` when there is
/// no usable text (never fabricate prose).
pub fn bounded_preview(text: Option<&str>) -> Option<String> {
    let raw = text?;
    let one_line = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.is_empty() {
        return None;
    }
    let units: Vec<u16> = one_line.encode_utf16().collect();
    if units.len() <= APPLE_PREVIEW_MAX_LENGTH {
        return Some(one_line);
    }
    let truncated = String::from_utf16_lossy(&units[..APPLE_PREVIEW_MAX_LENGTH - 1]);
    Some(format!("{}…", truncated.trim_end()))
}

/// Map an Apple service value to the canonical interaction channel.
pub fn apple_service_to_channel(service: Option<&str>) -> &'static str {
    match service.map(str::to_lowercase) {
        Some(value) if value.contains("sms") => "sms",
        _ => "imessage",
    }
}

/// Deterministic source account derived from the export messages.
pub fn derive_source_account(messages: &[AppleMessagesMessage]) -> String {
    for message in messages {
        if let Some(account) = message.account.as_deref() {
            if account.contains('@') && !account.trim().is_empty() {
                return account.to_owned();
            }
        }
    }
    APPLE_LOCAL_SOURCE_ACCOUNT.to_owned()
}

// ---------------------------------------------------------------------------
// Evidence: one source-neutral row per Apple handle identity.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct AppleHandleEvidence {
    pub source: String,
    pub source_account: String,
    pub source_identity_key: String,
    pub source_label: Option<String>,
    pub display_name: Option<String>,
    pub organization: Option<String>,
    pub emails: Vec<IdentityEvidence>,
    pub phones: Vec<IdentityEvidence>,
    pub first_observed_at: Option<String>,
    pub last_observed_at: Option<String>,
    pub last_inbound_at: Option<String>,
    pub last_outbound_at: Option<String>,
    pub inbound_count: i64,
    pub outbound_count: i64,
    pub is_two_way: Option<bool>,
    pub is_owner_initiated: Option<bool>,
    pub is_automated_or_bulk: Option<bool>,
    pub is_organization_or_service: Option<bool>,
    pub known_apple_contact: Option<bool>,
    pub has_email: bool,
    pub has_phone: bool,
    pub coverage_note: Option<String>,
    pub evidence_fingerprint: String,
    /// Apple `handle` rowids this identity resolves to; used to materialize events.
    pub handle_rowids: Vec<i64>,
}

impl AppleHandleEvidence {
    /// Every 1:1 message counted for this identity (inbound + outbound).
    pub fn total_count(&self) -> i64 {
        self.inbound_count + self.outbound_count
    }
}

/// Build the evidence rows for one export package.
///
/// Group chats are excluded from the counts (a group is never one person's relationship) and every
/// `handle` rowid sharing one textual identity is aggregated, because the evidence row is keyed by
/// that textual value: taking only the last rowid would silently overwrite the counts of the earlier
/// ones.
pub fn build_handle_evidence(
    source_account: &str,
    handles: &[AppleMessagesHandle],
    messages: &[AppleMessagesMessage],
) -> Vec<AppleHandleEvidence> {
    let mut by_rowid: BTreeMap<i64, Vec<&AppleMessagesMessage>> = BTreeMap::new();
    for message in messages {
        if let Some(rowid) = message.handle_id {
            by_rowid.entry(rowid).or_default().push(message);
        }
    }

    let mut grouped: BTreeMap<&str, Vec<&AppleMessagesHandle>> = BTreeMap::new();
    for handle in handles {
        let Some(identity) = handle
            .id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        grouped.entry(identity).or_default().push(handle);
    }

    let mut out = Vec::with_capacity(grouped.len());
    for (identity, group) in grouped {
        let mut first_observed_at: Option<String> = None;
        let mut last_observed_at: Option<String> = None;
        let mut last_inbound_at: Option<String> = None;
        let mut last_outbound_at: Option<String> = None;
        let mut inbound_count = 0i64;
        let mut outbound_count = 0i64;
        let mut handle_rowids = Vec::new();

        for handle in &group {
            let Some(rowid) = handle.rowid else {
                continue;
            };
            handle_rowids.push(rowid);
            let Some(items) = by_rowid.get(&rowid) else {
                continue;
            };
            for message in items {
                // Group chats are never silently attributed to an individual person.
                if is_group_chat_guid(message.chat_guid.as_deref()) {
                    continue;
                }
                // Direction is independent of a usable timestamp; only the windows use dates.
                let outbound = message.is_from_me == Some(1);
                if outbound {
                    outbound_count += 1;
                } else {
                    inbound_count += 1;
                }
                let Some(iso) = effective_date_iso(message) else {
                    continue;
                };
                if first_observed_at
                    .as_deref()
                    .map_or(true, |current| iso.as_str() < current)
                {
                    first_observed_at = Some(iso.clone());
                }
                if last_observed_at
                    .as_deref()
                    .map_or(true, |current| iso.as_str() > current)
                {
                    last_observed_at = Some(iso.clone());
                }
                if outbound {
                    if last_outbound_at
                        .as_deref()
                        .map_or(true, |current| iso.as_str() > current)
                    {
                        last_outbound_at = Some(iso);
                    }
                } else if last_inbound_at
                    .as_deref()
                    .map_or(true, |current| iso.as_str() > current)
                {
                    last_inbound_at = Some(iso);
                }
            }
        }

        let (emails, phones) = handle_to_identities(identity);
        let source_label = group.iter().find_map(|handle| handle.service.clone());
        let is_two_way = if inbound_count > 0 && outbound_count > 0 {
            Some(true)
        } else if inbound_count + outbound_count > 0 {
            Some(false)
        } else {
            None
        };
        let evidence_fingerprint = fingerprint(&format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            APPLE_MESSAGES_SOURCE,
            source_account,
            identity,
            emails
                .iter()
                .map(|item| item.normalized.as_str())
                .collect::<Vec<_>>()
                .join(","),
            phones
                .iter()
                .map(|item| item.normalized.as_str())
                .collect::<Vec<_>>()
                .join(","),
            first_observed_at.as_deref().unwrap_or(""),
            last_observed_at.as_deref().unwrap_or(""),
            last_inbound_at.as_deref().unwrap_or(""),
            last_outbound_at.as_deref().unwrap_or(""),
            inbound_count,
            outbound_count,
        ));

        out.push(AppleHandleEvidence {
            source: APPLE_MESSAGES_SOURCE.to_owned(),
            source_account: source_account.to_owned(),
            source_identity_key: identity.to_owned(),
            source_label,
            display_name: None,
            organization: None,
            has_email: !emails.is_empty(),
            has_phone: !phones.is_empty(),
            emails,
            phones,
            first_observed_at,
            last_observed_at,
            last_inbound_at,
            last_outbound_at,
            inbound_count,
            outbound_count,
            is_two_way,
            is_owner_initiated: None,
            is_automated_or_bulk: None,
            is_organization_or_service: None,
            known_apple_contact: Some(false),
            coverage_note: None,
            evidence_fingerprint,
            handle_rowids,
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Reconciliation: deterministic, explainable, and it never guesses.
// ---------------------------------------------------------------------------

/// Everything the reconciliation decision needs, fetched in bulk by the caller so the pass stays
/// two reads rather than two reads per handle.
#[derive(Debug, Clone, Default)]
pub struct AppleHandleLookup {
    /// An explicit, durable source link already recorded for this identity.
    pub explicit_link: Option<String>,
    /// Distinct canonical owners of any normalized email on the evidence.
    pub email_owners: Vec<String>,
    /// Distinct canonical owners of any normalized phone on the evidence.
    pub phone_owners: Vec<String>,
    /// At least one email matched more than one person (a conflict, never a choice).
    pub email_multi_match: bool,
    /// At least one phone matched more than one person.
    pub phone_multi_match: bool,
}

fn decision(
    review_state: &str,
    match_method: &str,
    match_confidence: &str,
    reason: &str,
    canonical_person_id: Option<&str>,
) -> RelationshipDecision {
    RelationshipDecision {
        review_state: review_state.to_owned(),
        match_method: match_method.to_owned(),
        match_confidence: match_confidence.to_owned(),
        canonical_person_id: canonical_person_id.map(str::to_owned),
        reason: reason.to_owned(),
        rule_version: REL_INTEL_RULE_VERSION.to_owned(),
    }
}

/// Decide what an Apple handle's evidence means. Weak fuzzy-name similarity is never an automatic
/// match: only an exact normalized email/phone or an explicit source link auto-links, and an identity
/// that matches more than one person is a reviewable conflict rather than a silent choice.
pub fn decide_apple_handle(
    evidence: &AppleHandleEvidence,
    lookup: &AppleHandleLookup,
) -> RelationshipDecision {
    if evidence.is_automated_or_bulk == Some(true) || evidence.is_organization_or_service == Some(true) {
        return if evidence.is_organization_or_service == Some(true) {
            decision(
                "non_person",
                "rejected",
                "none",
                "automated_service_or_organization",
                None,
            )
        } else {
            decision(
                "rejected",
                "rejected",
                "none",
                "automated_or_bulk_evidence",
                None,
            )
        };
    }

    if let Some(person_id) = lookup.explicit_link.as_deref() {
        return decision(
            "exact_linked",
            "source_link",
            "exact",
            "explicit_source_link",
            Some(person_id),
        );
    }

    let matched_method = if !lookup.email_owners.is_empty() {
        Some("exact_email")
    } else if !lookup.phone_owners.is_empty() {
        Some("exact_phone")
    } else {
        None
    };

    let mut owners: Vec<&str> = lookup
        .email_owners
        .iter()
        .chain(lookup.phone_owners.iter())
        .map(String::as_str)
        .collect();
    owners.sort_unstable();
    owners.dedup();

    if lookup.email_multi_match || lookup.phone_multi_match {
        return decision(
            "ambiguous",
            matched_method.unwrap_or("exact_email"),
            "ambiguous",
            "identity_matches_multiple_people",
            None,
        );
    }
    if owners.len() == 1 && matched_method.is_some() {
        return decision(
            "exact_linked",
            matched_method.unwrap_or("exact_email"),
            "exact",
            "exact_normalized_identity",
            Some(owners[0]),
        );
    }
    if owners.len() > 1 {
        return decision(
            "ambiguous",
            matched_method.unwrap_or("exact_email"),
            "ambiguous",
            "cross_identity_conflict",
            None,
        );
    }

    if !evidence.has_email && !evidence.has_phone {
        return decision(
            "deferred",
            "unmatched",
            "none",
            "insufficient_identity_evidence",
            None,
        );
    }
    let meaningful = evidence.is_two_way == Some(true)
        || evidence.is_owner_initiated == Some(true)
        || evidence.outbound_count > 0;
    if meaningful {
        return decision(
            "review_required",
            "review_candidate",
            "probable",
            "two_way_or_owner_initiated_without_exact_match",
            None,
        );
    }
    decision("unmatched", "unmatched", "none", "no_exact_match", None)
}

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
