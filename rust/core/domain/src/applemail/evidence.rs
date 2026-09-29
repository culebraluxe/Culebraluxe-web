//! Moved from `applemail.rs` (move only): apple_mail_replay_id, build_mail_evidence, mail_evidence_fingerprint.

#[allow(unused_imports)]
use super::*;

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
