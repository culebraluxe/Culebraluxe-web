// ---------------------------------------------------------------------------
// Apple Messages intake — the Rust replacement for `scripts/apple-messages-intake.ts`.
//
// The chain is unchanged from the TypeScript it replaces:
//
//   local Apple chat.db -> export package (identities.jsonl + messages.jsonl + manifest.json)
//     -> ODS relationship-evidence upsert   (integration_relationship_evidence)
//     -> deterministic reconciliation to a canonical Person
//     -> canonical interaction materialization (exact links only)
//     -> client read-model refresh
//
// Two things are deliberately different:
//   * the rules live in `domain::apple_messages` (pure, unit tested against the deleted TypeScript's
//     own reference values) instead of a deleted library, and
//   * the target is resolved from the environment the same way the rest of the Rust stack resolves
//     it, and refuses to run when no target is declared — there is no silent fallback to another
//     database. The resolved target is printed with the tally.
//
// Replay-safe: evidence upserts on `(source, source_account, source_identity_key)`, landing inserts
// `do nothing` on the source message identity, and the per-Person interaction key is
// `latest:<person>:<channel>`.
// ---------------------------------------------------------------------------
use db::{
    Database, EvidenceUpsert, ImessageLanding, LandingDao, LatestInteraction,
    LatestInteractionOutcome, RelationshipEvidenceDao, resolve_declared_target,
};
use domain::{
    APPLE_MESSAGES_SOURCE, AppleHandleLookup, AppleMessagesExport, AppleMessagesHandle,
    AppleMessagesMessage, build_handle_evidence, bounded_preview, decide_apple_handle,
    derive_source_account, effective_date_iso, is_group_chat_guid,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fs;
use std::io;
use std::path::Path;

/// Rows per landing statement. The single-row form costs one round trip per message; the measured
/// Apple backfill is ~93,000 messages, so this is the difference between minutes and hours.
const LANDING_BATCH: usize = 500;

/// Counters printed as one JSON line so the shell wrapper can log them without parsing prose.
#[derive(Debug, Default)]
struct Tally {
    handles: usize,
    messages: usize,
    dated_messages: usize,
    messages_with_bounded_text: usize,
    group_chat_messages: usize,
    evidence_rows: usize,
    exact_linked_handles: usize,
    unmatched_or_review_handles: usize,
    events_seen: usize,
    landed: usize,
    source_rows_inserted: usize,
    source_rows_updated: usize,
    source_rows_current: usize,
    skipped_no_timestamp: usize,
    skipped_group_chat: usize,
    skipped_no_guid: usize,
    errors: usize,
    reconcile_tally: BTreeMap<String, i64>,
}

impl Tally {
    fn count_decision(&mut self, review_state: &str) {
        *self
            .reconcile_tally
            .entry(review_state.to_owned())
            .or_insert(0) += 1;
    }
}

fn read_jsonl<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, Box<dyn Error>> {
    let raw = fs::read_to_string(path).map_err(|error| {
        io::Error::other(format!("export package unreadable at {}: {error}", path.display()))
    })?;
    let mut out = Vec::new();
    for (index, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: T = serde_json::from_str(trimmed).map_err(|error| {
            io::Error::other(format!(
                "{} line {} is not valid JSON: {error}",
                path.display(),
                index + 1
            ))
        })?;
        out.push(value);
    }
    Ok(out)
}

/// Load and validate the export package. A package missing its manifest, identities or messages is a
/// refusal, not an empty run: an empty intake that reports success is how a sync silently stops
/// working.
fn load_export_package(dir: &Path) -> Result<AppleMessagesExport, Box<dyn Error>> {
    if !dir.join("manifest.json").is_file() {
        return Err(io::Error::other(format!(
            "manifest.json missing at {}; refusing to intake",
            dir.display()
        ))
        .into());
    }
    let identities = dir.join("identities.jsonl");
    let messages = dir.join("messages.jsonl");
    if !identities.is_file() || !messages.is_file() {
        return Err(io::Error::other(format!(
            "export package incomplete at {} (need identities.jsonl + messages.jsonl)",
            dir.display()
        ))
        .into());
    }
    let handles: Vec<AppleMessagesHandle> = read_jsonl(&identities)?;
    let messages: Vec<AppleMessagesMessage> = read_jsonl(&messages)?;
    if messages.is_empty() {
        return Err(io::Error::other(format!(
            "messages.jsonl is empty at {}; refusing to intake",
            dir.display()
        ))
        .into());
    }
    let source_account = derive_source_account(&messages);
    Ok(AppleMessagesExport {
        source_account,
        handles,
        messages,
    })
}

/// Canonical identity owners, read once and matched in memory.
#[derive(Debug, Default)]
pub(crate) struct OwnerIndex {
    emails: HashMap<String, Vec<String>>,
    phones: HashMap<String, Vec<String>>,
}

impl OwnerIndex {
    pub(crate) async fn load(dao: &RelationshipEvidenceDao) -> Result<Self, Box<dyn Error>> {
        let mut index = Self::default();
        for owner in dao.identity_owners().await? {
            match owner.identity_type.trim().to_lowercase().as_str() {
                "email" => push_unique(
                    &mut index.emails,
                    owner.identity_value.trim().to_lowercase(),
                    &owner.person_id,
                ),
                "phone" => {
                    if let Some(key) = phone_match_key(&owner.identity_value) {
                        push_unique(&mut index.phones, key, &owner.person_id);
                    }
                }
                _ => {}
            }
        }
        Ok(index)
    }
}

fn push_unique(map: &mut HashMap<String, Vec<String>>, key: String, person_id: &str) {
    if key.is_empty() {
        return;
    }
    let entry = map.entry(key).or_default();
    if !entry.iter().any(|existing| existing == person_id) {
        entry.push(person_id.to_owned());
    }
}

fn push_unique_value(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_owned());
    }
}

/// Phone match key: digits only with a leading US/PR country code stripped — the same key the
/// `person_identity` phone lookup uses, so an Apple handle and a stored identity agree.
fn phone_match_key(value: &str) -> Option<String> {
    let digits: String = value.chars().filter(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    if digits.len() == 11 && digits.starts_with('1') {
        return Some(digits[1..].to_owned());
    }
    Some(digits)
}

/// Assemble the reconciliation input for one identity from the in-memory owners and durable
/// links. Shared with the mail promotion, which builds its evidence in the same shape: the
/// reconciliation rules exist once, so the two feeds cannot drift apart.
pub(crate) fn lookup_for(
    evidence: &domain::AppleHandleEvidence,
    index: &OwnerIndex,
    links: &HashMap<(String, String), String>,
) -> AppleHandleLookup {
    let mut lookup = AppleHandleLookup {
        explicit_link: links
            .get(&(
                evidence.source_account.clone(),
                evidence.source_identity_key.clone(),
            ))
            .cloned(),
        ..Default::default()
    };

    for identity in &evidence.emails {
        let Some(owners) = index.emails.get(&identity.normalized.to_lowercase()) else {
            continue;
        };
        match owners.len() {
            0 => {}
            1 => push_unique_value(&mut lookup.email_owners, &owners[0]),
            _ => lookup.email_multi_match = true,
        }
    }
    for identity in &evidence.phones {
        let Some(key) = phone_match_key(&identity.normalized) else {
            continue;
        };
        let Some(owners) = index.phones.get(&key) else {
            continue;
        };
        match owners.len() {
            0 => {}
            1 => push_unique_value(&mut lookup.phone_owners, &owners[0]),
            _ => lookup.phone_multi_match = true,
        }
    }
    lookup
}

/// How one intake pass should behave.
#[derive(Debug, Clone, Copy, Default)]
pub struct IntakeOptions {
    /// Evidence and reconciliation only — no interaction materialization. This is the repair path.
    pub evidence_only: bool,
    /// Rebuild the client read models even when nothing new was materialized. The evidence pass feeds
    /// `mv_client_relationship_channels`, so a repair run needs the refresh that a normal run earns by
    /// materializing.
    pub refresh: bool,
}

/// One full intake pass over an export package: evidence, reconciliation, materialization, refresh.
///
/// A database failure fails the run. It is never counted and swallowed: a sync that reports success
/// while writing nothing is the failure mode this command exists to end.
pub async fn intake_messages(dir: &Path, options: IntakeOptions) -> Result<(), Box<dyn Error>> {
    let target = resolve_declared_target(
        std::env::var("VERCEL_ENV").ok().as_deref(),
        std::env::var("APP_ENV").ok().as_deref(),
    )?;
    let export = load_export_package(dir)?;
    let mut tally = Tally {
        handles: export.handles.len(),
        messages: export.messages.len(),
        ..Default::default()
    };
    for message in &export.messages {
        if effective_date_iso(message).is_some() {
            tally.dated_messages += 1;
        }
        if bounded_preview(message.text.as_deref()).is_some() {
            tally.messages_with_bounded_text += 1;
        }
        if is_group_chat_guid(message.chat_guid.as_deref()) {
            tally.group_chat_messages += 1;
        }
    }

    let database = Database::connect_from_env().await?;
    let evidence_dao = RelationshipEvidenceDao::new(database.clone());
    let landing_dao = LandingDao::new(database);

    let evidence = build_handle_evidence(&export.source_account, &export.handles, &export.messages);
    tally.evidence_rows = evidence.len();

    let owners = OwnerIndex::load(&evidence_dao).await?;
    let links: HashMap<(String, String), String> = evidence_dao
        .source_links(APPLE_MESSAGES_SOURCE)
        .await?
        .into_iter()
        .map(|link| {
            (
                (link.source_account, link.source_identity_key),
                link.canonical_person_id,
            )
        })
        .collect();

    let mut by_rowid: BTreeMap<i64, Vec<&AppleMessagesMessage>> = BTreeMap::new();
    for message in &export.messages {
        if let Some(rowid) = message.handle_id {
            by_rowid.entry(rowid).or_default().push(message);
        }
    }

    let mut pending: Vec<ImessageLanding> = Vec::with_capacity(LANDING_BATCH);
    let mut latest: HashMap<String, LatestInteraction> = HashMap::new();

    for row in &evidence {
        let lookup = lookup_for(row, &owners, &links);
        let decision = decide_apple_handle(row, &lookup);
        tally.count_decision(&decision.review_state);

        let id = evidence_dao.upsert_evidence(&EvidenceUpsert::from(row)).await?;
        evidence_dao.record_decision(&id, &decision).await?;

        let linked =
            decision.review_state == "exact_linked" && decision.canonical_person_id.is_some();
        if linked {
            tally.exact_linked_handles += 1;
        } else {
            tally.unmatched_or_review_handles += 1;
        }
        let Some(person_id) = decision.canonical_person_id.clone().filter(|_| linked) else {
            continue;
        };
        if options.evidence_only {
            continue;
        }

        for rowid in &row.handle_rowids {
            let Some(messages) = by_rowid.get(rowid) else {
                continue;
            };
            for message in messages {
                tally.events_seen += 1;
                // Group chats are never silently attributed to an individual person.
                if is_group_chat_guid(message.chat_guid.as_deref()) {
                    tally.skipped_group_chat += 1;
                    continue;
                }
                // The Apple GUID is the replay key; without one there is nothing to be idempotent about.
                let Some(guid) = message
                    .guid
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                else {
                    tally.skipped_no_guid += 1;
                    continue;
                };
                let Some(occurred_at) = effective_date_iso(message) else {
                    tally.skipped_no_timestamp += 1;
                    continue;
                };
                let outbound = message.is_from_me == Some(1);
                let channel = domain::apple_service_to_channel(message.service.as_deref());

                pending.push(ImessageLanding {
                    source_account: Some(export.source_account.clone()),
                    source_message_id: guid.to_owned(),
                    conversation_id: message.chat_guid.clone(),
                    handle: message.handle_value.clone(),
                    direction: Some(if outbound { "outgoing" } else { "incoming" }.to_owned()),
                    service: message.service.clone(),
                    sent_at: Some(occurred_at.clone()),
                    text: message.text.clone(),
                    raw: serde_json::to_value(message).unwrap_or(Value::Null),
                });
                if pending.len() >= LANDING_BATCH {
                    tally.landed += landing_dao.land_imessage_batch(&pending).await?;
                    pending.clear();
                }

                let candidate = LatestInteraction {
                    person_id: person_id.clone(),
                    channel: channel.to_owned(),
                    event_type: "message".to_owned(),
                    direction: Some(if outbound { "outbound" } else { "inbound" }.to_owned()),
                    occurred_at,
                    summary: bounded_preview(message.text.as_deref()).or_else(|| {
                        Some(
                            if message.has_attachments == Some(1) {
                                "Attachment"
                            } else {
                                "Message"
                            }
                            .to_owned(),
                        )
                    }),
                    source_system: APPLE_MESSAGES_SOURCE.to_owned(),
                    // A message has no duration; the call channel is where that column is used.
                    duration_seconds: None,
                    source_external_id: format!("latest:{person_id}:{channel}"),
                    // Provenance only. The CRM remembers the moment, never the transcript.
                    source_metadata: json!({
                        "sourceAccount": export.source_account,
                        "handleId": message.handle_id,
                        "service": message.service,
                        "hasAttachments": message.has_attachments == Some(1),
                    }),
                };
                let key = format!("{person_id}\u{0}{channel}");
                match latest.get(&key) {
                    Some(current) if current.occurred_at >= candidate.occurred_at => {}
                    _ => {
                        latest.insert(key, candidate);
                    }
                }
            }
        }
    }

    if !pending.is_empty() {
        tally.landed += landing_dao.land_imessage_batch(&pending).await?;
    }

    for interaction in latest.values() {
        match landing_dao.upsert_latest_interaction(interaction).await? {
            LatestInteractionOutcome::Inserted => tally.source_rows_inserted += 1,
            LatestInteractionOutcome::Updated => tally.source_rows_updated += 1,
            LatestInteractionOutcome::Ignored => tally.source_rows_current += 1,
        }
    }

    if options.refresh
        || (!options.evidence_only
            && (tally.source_rows_inserted > 0 || tally.source_rows_updated > 0))
    {
        landing_dao.refresh_client_read_models().await?;
    }

    println!(
        "{}",
        json!({
            "source": APPLE_MESSAGES_SOURCE,
            "target": if target == db::DbTarget::Prod { "prod" } else { "dev" },
            "exportDir": dir.display().to_string(),
            "sourceAccount": export.source_account,
            "evidenceOnly": options.evidence_only,
            "refreshed": options.refresh
                || (!options.evidence_only
                    && (tally.source_rows_inserted > 0 || tally.source_rows_updated > 0)),
            "handles": tally.handles,
            "messages": tally.messages,
            "datedMessages": tally.dated_messages,
            "messagesWithBoundedText": tally.messages_with_bounded_text,
            "groupChatMessages": tally.group_chat_messages,
            "evidenceRows": tally.evidence_rows,
            "eventsSeen": tally.events_seen,
            "exactLinkedHandles": tally.exact_linked_handles,
            "unmatchedOrReviewHandles": tally.unmatched_or_review_handles,
            "landed": tally.landed,
            "sourceRowsInserted": tally.source_rows_inserted,
            "sourceRowsUpdated": tally.source_rows_updated,
            "sourceRowsCurrent": tally.source_rows_current,
            "skippedNoTimestamp": tally.skipped_no_timestamp,
            "skippedGroupChat": tally.skipped_group_chat,
            "skippedNoGuid": tally.skipped_no_guid,
            "errors": tally.errors,
            "reconcileTally": tally.reconcile_tally,
        })
    );
    Ok(())
}


