// ---------------------------------------------------------------------------
// Apple Call History intake — the Rust replacement for `scripts/apple-calls-intake.ts`.
//
//   calls.jsonl (read-only Swift export of CallHistoryDB)
//     -> l_call landing rows                       (every call, replay-safe on the source id)
//     -> relationship evidence per counterparty    (phone and FaceTime are SEPARATE sources)
//     -> deterministic reconciliation to a Person
//     -> canonical interaction (one row per Person x channel: the NEWEST call)
//     -> client read-model refresh
//
// The rules live in `model::apple_calls`; the target is resolved from the declared environment and
// named in the tally, so a run can never be silent about which database it wrote to.
// ---------------------------------------------------------------------------
use db::{
    CallLanding, DbTarget, EvidenceUpsert, LandingDao, LatestInteraction, LatestInteractionOutcome,
    RelationshipEvidenceDao,
};
use model::apple_calls::flag;
use model::{
    build_call_evidence, call_date_iso, call_duration_raw, call_landing_direction,
    call_latest_interaction, call_source, call_unique_id, decide_apple_handle, is_facetime_call,
    AppleCallInteraction, AppleCallRecord, APPLE_CALLS_SOURCE, APPLE_CALL_HISTORY_ACCOUNT,
    APPLE_FACETIME_SOURCE,
};
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Rows per landing statement — one round trip per batch, not one per call.
const LANDING_BATCH: usize = 500;

const DEFAULT_FILE: &str = "public/upload/data/apple-messages-export/calls.jsonl";

fn resolve_file(args: &[String]) -> PathBuf {
    match crate::apple_mail::option(args, "--file") {
        Some(path) => PathBuf::from(path),
        None => crate::apple_sync::repo_root().join(DEFAULT_FILE),
    }
}

/// Read the JSONL export. An unreadable or empty export is a refusal: a silent no-op sync that
/// reports success is the failure mode this whole chain exists to avoid.
fn read_calls(path: &Path) -> Result<Vec<AppleCallRecord>, Box<dyn Error>> {
    let raw = fs::read_to_string(path).map_err(|error| {
        io::Error::other(format!(
            "CallHistory export unreadable at {}: {error}",
            path.display()
        ))
    })?;
    let mut calls = Vec::new();
    for (index, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let record: AppleCallRecord = serde_json::from_str(trimmed).map_err(|error| {
            io::Error::other(format!(
                "{}:{} is not a call record: {error}",
                path.display(),
                index + 1
            ))
        })?;
        calls.push(record);
    }
    if calls.is_empty() {
        return Err(io::Error::other(format!(
            "{} contains no calls; refusing to intake",
            path.display()
        ))
        .into());
    }
    Ok(calls)
}

#[derive(Debug, Default)]
struct Tally {
    calls: usize,
    normal_calls: usize,
    facetime_calls: usize,
    landed: usize,
    replayed_landing: usize,
    skipped_no_source_id: usize,
    evidence_rows: usize,
    inserted: usize,
    updated: usize,
    current: usize,
    skipped_unlinked: usize,
    skipped_no_date: usize,
    skipped_no_address: usize,
    reconcile: BTreeMap<String, i64>,
}

impl Tally {
    fn count_decision(&mut self, review_state: &str) {
        *self.reconcile.entry(review_state.to_owned()).or_insert(0) += 1;
    }
}

/// The newest interaction per Person x channel, decided in memory. The canonical write is keyed
/// `latest:<person>:<channel>`, so only the newest call per person can ever be the stored row —
/// sending every call to the database would be thousands of round trips proving the same thing.
fn remember_newest(
    latest: &mut HashMap<String, AppleCallInteraction>,
    interaction: AppleCallInteraction,
) {
    match latest.get(&interaction.person_id) {
        Some(current) if current.occurred_at >= interaction.occurred_at => {}
        _ => {
            latest.insert(interaction.person_id.clone(), interaction);
        }
    }
}

/// `apple-sync calls-intake [dev|prod] [--file <calls.jsonl>]`
pub async fn calls_intake(args: &[String]) -> Result<(), Box<dyn Error>> {
    crate::apple_sync::load_env();
    let target: DbTarget = crate::apple_mail::target_arg(args)?;
    let file = resolve_file(args);
    let calls = read_calls(&file)?;

    let database = crate::apple_mail::connect(target).await?;
    let landing = LandingDao::new(database.clone());
    let evidence_dao = RelationshipEvidenceDao::new(database);

    let mut tally = Tally {
        calls: calls.len(),
        normal_calls: calls
            .iter()
            .filter(|call| call_source(call) == APPLE_CALLS_SOURCE)
            .count(),
        facetime_calls: calls
            .iter()
            .filter(|call| call_source(call) == APPLE_FACETIME_SOURCE)
            .count(),
        ..Default::default()
    };

    // --- 1. Land every call, before anything interprets it ---------------------
    let mut pending: Vec<CallLanding> = Vec::with_capacity(LANDING_BATCH);
    let mut landable = 0usize;
    for call in &calls {
        let Some(source_message_id) = call_unique_id(call) else {
            // Without a source identity the row is not replay-safe, and a landing table that cannot
            // dedup is a landing table that grows on every run.
            tally.skipped_no_source_id += 1;
            continue;
        };
        landable += 1;
        pending.push(CallLanding {
            source_account: Some(APPLE_CALL_HISTORY_ACCOUNT.to_owned()),
            source_message_id,
            handle: call.address.clone(),
            direction: Some(call_landing_direction(call).to_owned()),
            // Video is FaceTime — the landing table records the modality, not the channel.
            call_type: Some(
                if is_facetime_call(call) {
                    "video"
                } else {
                    "audio"
                }
                .to_owned(),
            ),
            answered: flag(call.answered.as_ref()),
            duration_seconds: call_duration_raw(call),
            started_at: call_date_iso(call),
            raw: serde_json::to_value(call).unwrap_or(serde_json::Value::Null),
        });
        if pending.len() >= LANDING_BATCH {
            tally.landed += landing.land_call_batch(&pending).await?;
            pending.clear();
        }
    }
    if !pending.is_empty() {
        tally.landed += landing.land_call_batch(&pending).await?;
    }
    tally.replayed_landing = landable.saturating_sub(tally.landed);

    // --- 2. Evidence, reconciled to a canonical Person -------------------------
    let builds = build_call_evidence(&calls, APPLE_CALL_HISTORY_ACCOUNT);
    tally.evidence_rows = builds.len();
    let owners = crate::apple_messages::OwnerIndex::load(&evidence_dao).await?;

    // Explicit, durable source links — what reconciliation treats as the highest-trust match.
    let mut links: HashMap<(String, String), String> = HashMap::new();
    for source in [APPLE_CALLS_SOURCE, APPLE_FACETIME_SOURCE] {
        for link in evidence_dao.source_links(source).await? {
            links.insert(
                (link.source_account, link.source_identity_key),
                link.canonical_person_id,
            );
        }
    }

    for row in &builds {
        let lookup = crate::apple_messages::lookup_for(row, &owners, &links);
        let decision = decide_apple_handle(row, &lookup);
        tally.count_decision(&decision.review_state);
        let id = evidence_dao
            .upsert_evidence(&EvidenceUpsert::from(row))
            .await?;
        evidence_dao.record_decision(&id, &decision).await?;
    }

    // Only evidence the rows themselves say is `exact_linked` can become an interaction, read back
    // AFTER this run's decisions: the row is the one writer of a link, and an established link
    // survives a later ambiguous pass.
    let mut linked_by_source: HashMap<(String, String), String> = HashMap::new();
    for source in [APPLE_CALLS_SOURCE, APPLE_FACETIME_SOURCE] {
        for row in evidence_dao
            .candidates(Some(source), Some("exact_linked"), &[], 10_000)
            .await?
        {
            if let Some(person_id) = row.canonical_person_id {
                linked_by_source
                    .entry((row.source.clone(), row.source_identity_key.clone()))
                    .or_insert(person_id);
            }
        }
    }

    // --- 3. The canonical interaction: one row per Person x call channel -------
    let mut latest: HashMap<String, AppleCallInteraction> = HashMap::new();
    for call in &calls {
        let Some(address) = call
            .address
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            tally.skipped_no_address += 1;
            continue;
        };
        if call_date_iso(call).is_none() {
            tally.skipped_no_date += 1;
            continue;
        }
        let source = call_source(call);
        let Some(person_id) = linked_by_source.get(&(source.to_owned(), address.to_owned())) else {
            // Evidence stays staged for the unlinked: a decision, not a failure.
            tally.skipped_unlinked += 1;
            continue;
        };
        if let Some(interaction) = call_latest_interaction(call, person_id) {
            remember_newest(&mut latest, interaction);
        }
    }

    for interaction in latest.values() {
        let draft = LatestInteraction {
            person_id: interaction.person_id.clone(),
            channel: interaction.channel.to_owned(),
            event_type: interaction.event_type.to_owned(),
            direction: Some(interaction.direction.to_owned()),
            occurred_at: interaction.occurred_at.clone(),
            summary: None,
            duration_seconds: interaction.duration_seconds,
            source_system: interaction.source_system.to_owned(),
            source_external_id: interaction.source_external_id.clone(),
            source_metadata: interaction.source_metadata.clone(),
        };
        match landing.upsert_latest_interaction(&draft).await? {
            LatestInteractionOutcome::Inserted => tally.inserted += 1,
            LatestInteractionOutcome::Updated => tally.updated += 1,
            LatestInteractionOutcome::Ignored => tally.current += 1,
        }
    }

    let refreshed = tally.inserted > 0 || tally.updated > 0 || tally.evidence_rows > 0;
    if refreshed {
        landing.refresh_client_read_models().await?;
    }

    println!(
        "{}",
        json!({
            "source": APPLE_CALLS_SOURCE,
            "target": target.as_str(),
            "file": file.display().to_string(),
            "sourceAccount": APPLE_CALL_HISTORY_ACCOUNT,
            "refreshed": refreshed,
            "calls": tally.calls,
            "normalCalls": tally.normal_calls,
            "facetimeCalls": tally.facetime_calls,
            "lCallLanded": tally.landed,
            "lCallReplayed": tally.replayed_landing,
            "skippedNoSourceId": tally.skipped_no_source_id,
            "evidenceRows": tally.evidence_rows,
            "reconcileTally": tally.reconcile,
            "interactionsInserted": tally.inserted,
            "interactionsUpdated": tally.updated,
            "interactionsCurrent": tally.current,
            "skippedUnlinked": tally.skipped_unlinked,
            "skippedNoDate": tally.skipped_no_date,
            "skippedNoAddress": tally.skipped_no_address,
        })
    );
    Ok(())
}
