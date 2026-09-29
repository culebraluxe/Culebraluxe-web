//! Moved from `applemail.rs` (move only): normalize_landed_mail, with its private helpers to_iso and external_recipients.

#[allow(unused_imports)]
use super::*;

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
