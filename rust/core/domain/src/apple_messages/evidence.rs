//! Moved from `apple_messages.rs` (move only): build_handle_evidence, decide_apple_handle, and the private decision helper.

#[allow(unused_imports)]
use super::*;

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
