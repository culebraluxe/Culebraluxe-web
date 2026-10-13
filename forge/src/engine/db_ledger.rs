//! The durable completion ledger: the same unit `MemoryLedger` applies, held in rows.
//!
//! WHY THIS EXISTS. `MemoryLedger` is process-local, and the Forge engine is a *child process per
//! dispatch* (`bin/forge.rs`), so the "exactly once" the unit promises held only inside one run. The next
//! dispatch of the same story began with an empty ledger, `reconcile_completions` found every
//! `task.completed` in the instance history unknown (`has_final` = false) and re-applied the whole unit:
//! the evidence merge was replayed and `forge_repair_attempts` / `forge_replan_attempts` were incremented
//! once per process that ever looked. The receipt exists in the schema for exactly this
//! (`workflow_command_receipt`, `forge.completion:{taskId}`) and was unused.
//!
//! ONE WRITE, NOT FOUR (FORGE-B1 slice 2). The claim, the evidence merge, the budget spend and the
//! finalize used to be four separate calls, and a process that died between them left spent counters and
//! merged evidence with no receipt — which every later process then re-applied. They are now one call to
//! `ForgeEngineDao::apply_completion`: one transaction, so a receipt that exists is a unit that happened,
//! and one that does not is the crash window the resume reconciles.
//!
//! FAILURE IS NOT ABSENCE. Every method returns `Result`: a database that cannot answer must stop the
//! caller, because "someone already applied this unit" is an answer only a committed receipt may give, and
//! reporting a lost connection that way would silently drop the evidence and the counters. A conflicting
//! or in-flight receipt is an error too, never `AlreadyApplied`: the caller must not treat a unit it did
//! not apply as its own.

use std::sync::Arc;

use db::{CompletionApply as DbApply, CompletionSpend as DbSpend, CompletionUnit, ForgeEngineDao};
use workflow::{Result, WorkflowError};

use crate::engine::completion::{
    CompletionApply, CompletionLedger, CompletionRecord, CompletionSpend,
};
use crate::engine::evidence_store::evidence_patch;
use crate::engine::neon_sql::RECEIPT_PREFIX;
use crate::engine::runtime::completion_receipt_id;
use crate::engine::vendor_session::with_shared;

/// The ledger the engine binary installs (`ForgeRuntime::durable`). No fields: the state is the rows.
#[derive(Debug, Default)]
pub struct DbCompletionLedger;

pub fn durable_completion_ledger() -> Arc<dyn CompletionLedger> {
    Arc::new(DbCompletionLedger)
}

fn failed(operation: &str, error: impl std::fmt::Display) -> WorkflowError {
    WorkflowError::generic(format!("completion ledger {operation}: {error}"))
}

/// The one story a process instance names — the provenance policy both recovery paths read.
///
/// A business key is a *second spelling* of the subject id, never a replacement for it: the discovery query
/// already accepts either (`pi.subject_id = $1 or pi.business_key = $1`). So the question a row has to
/// answer is whether a key that *is* there agrees, not whether one is there at all. `subject_type = 'story'`
/// plus a non-blank `subject_id` is the identity; a key that is present and different refuses, because that
/// ambiguity is a human's to resolve and applying the unit to either story would be a guess.
///
/// WHY AN ABSENT KEY IS NOT A CONFLICT (2026-10-12). Forge only began recording the key when it started a
/// story run (`runtime.rs` `start_process`), so every completion accepted before that carries NULL. Read
/// strictly — `business_key != Some(story_id)` — that refused *every* row in the ledger: 557 unfinished
/// completions, the sweep dead since 2026-10-03, and the worker exiting non-zero every three minutes before
/// it could claim anything. A column nobody wrote held the whole queue hostage.
pub(crate) fn story_identity(
    subject_type: Option<&str>,
    subject_id: Option<&str>,
    business_key: Option<&str>,
) -> std::result::Result<String, &'static str> {
    if subject_type != Some("story") {
        return Err("process subject is not a story");
    }
    let story_id = subject_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("story subject id is missing")?;
    if let Some(key) = business_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if key != story_id {
            return Err("business key disagrees with story subject id");
        }
    }
    Ok(story_id.to_string())
}

/// The one budget a completion may spend, in the durable vocabulary.
fn db_spend(spend: CompletionSpend) -> DbSpend {
    match spend {
        CompletionSpend::Repair => DbSpend::Repair,
        CompletionSpend::Replan => DbSpend::Replan,
    }
}

impl CompletionLedger for DbCompletionLedger {
    fn apply(&self, rec: &CompletionRecord) -> Result<CompletionApply> {
        // One mapping, one writer: the same `evidence_patch` the workflow-evidence port uses, so the
        // ledger and the port cannot disagree about which column a field lands in.
        let patch = evidence_patch(&rec.evidence);
        let command_id = completion_receipt_id(&rec.task_id);
        let fingerprint = rec.fingerprint();
        let assay_receipt = rec
            .assay_receipt_link()
            .map_err(|error| failed("assay receipt link", error))?;
        let db_assay_receipt = assay_receipt
            .as_ref()
            .map(|receipt| db::CompletionAssayReceipt {
                artifact_id: &receipt.artifact_id,
                story_run_id: &receipt.story_run_id,
                idempotency_key: &receipt.idempotency_key,
                measurement_node: &receipt.measurement_node,
                gate_verdict: &receipt.gate_verdict,
                plan_sha256: receipt.plan_sha256.as_deref(),
                candidate_sha: receipt.candidate_sha.as_deref(),
            });
        // `apply_completion` owns the transaction: the receipt, the story-locked budget spend and the
        // evidence merge commit together, or none of them do.
        let unit = CompletionUnit {
            command_id: &command_id,
            process_instance_id: &rec.process_instance_id,
            story_id: &rec.story_id,
            node_id: rec.node_id.as_deref(),
            evidence: &patch,
            spend: rec.spend().map(db_spend),
            fingerprint: &fingerprint,
            assay_receipt: db_assay_receipt.as_ref(),
        };
        let outcome = with_shared(|db, rt| {
            let dao = ForgeEngineDao::new(db.clone());
            rt.block_on(async {
                dao.apply_completion(&unit)
                    .await
                    .map_err(|error| error.to_string())
            })
        })
        .map_err(|error| failed("apply", error))?
        .map_err(|error| failed(&command_id, error))?;
        match outcome {
            DbApply::Applied => Ok(CompletionApply::Applied),
            DbApply::AlreadyApplied => Ok(CompletionApply::AlreadyApplied),
            // Neither of these is "applied": a receipt left by another story, or a peer mid-write, is an
            // answer only a human can act on — and the caller must not replay its unit over it.
            DbApply::Conflict { stored } => Err(failed(
                &command_id,
                format!("receipt holds {stored}, not {fingerprint}"),
            )),
            DbApply::UnverifiableLegacy { command_id } => Err(failed(
                &command_id,
                "finalized legacy receipt has no full-unit fingerprint; effects were preserved and not replayed",
            )),
            DbApply::Busy => Err(failed(
                &command_id,
                "another process holds this unit mid-write",
            )),
            DbApply::LegacyAmbiguous { command_id } => Err(failed(
                &command_id,
                format!("legacy pending completion was quarantined; fingerprint={fingerprint}; inspect the accepted event and durable counter/evidence effects before resolving it"),
            )),
        }
    }

    fn has_final(&self, receipt_id: &str) -> Result<bool> {
        let command_id = receipt_id.to_string();
        with_shared(|db, rt| {
            let dao = ForgeEngineDao::new(db.clone());
            rt.block_on(async {
                let outcome = dao
                    .read_workflow_receipt_outcome(&command_id)
                    .await
                    .map_err(|error| error.to_string())
                    ?;
                match outcome.as_deref() {
                    Some("success") => Ok(true),
                    Some("unverifiable_legacy") => Err(
                        "finalized legacy receipt has no full-unit fingerprint; effects were preserved and not replayed".to_string(),
                    ),
                    _ => Ok(false),
                }
            })
        })
        .map_err(|error| failed("has_final", error))?
        .map_err(|error| failed(&command_id, error))
    }

    fn unfinished(
        &self,
        story_id: Option<&str>,
        limit: usize,
    ) -> Result<Option<Vec<CompletionRecord>>> {
        unfinished_rows(story_id, limit)?
            .into_iter()
            .map(completion_record_from_event)
            .collect::<Result<Vec<_>>>()
            .map(Some)
    }

    fn reset_budget(&self, story_id: &str) -> Result<()> {
        let id = story_id.to_string();
        with_shared(|db, rt| {
            let dao = ForgeEngineDao::new(db.clone());
            rt.block_on(async {
                dao.reset_forge_attempts(&id)
                    .await
                    .map_err(|error| error.to_string())
            })
        })
        .map_err(|error| failed("reset_budget", error))?
        .map_err(|error| failed(&format!("reset_budget({story_id})"), error))
    }

    fn watermark(&self, prefix: &str) -> Result<Option<i64>> {
        let prefix = prefix.to_string();
        with_shared(|db, rt| {
            let dao = ForgeEngineDao::new(db.clone());
            rt.block_on(async {
                dao.receipt_watermark_ms(&prefix)
                    .await
                    .map_err(|error| error.to_string())
            })
        })
        .map_err(|error| failed("watermark", error))?
        .map_err(|error| failed(&format!("watermark({prefix})"), error))
    }
}

/// The four parts a completion record is built from, or the fixed sentence naming the one the row lacks.
///
/// ONE SHAPE CHECK, TWO READERS. The sweep counts these sentences (a page that cannot advance must not
/// reprint a page of ids on every pass), and the ledger's mapping reports them as the conflict that stopped
/// the unit — so a row the sweep calls unreadable is exactly one the mapping would have refused.
fn completion_parts(
    row: &db::UnfinishedForgeCompletion,
) -> std::result::Result<(String, String, String, serde_json::Value), &'static str> {
    let story_id = story_identity(
        row.subject_type.as_deref(),
        row.subject_id.as_deref(),
        row.business_key.as_deref(),
    )?;
    let task_id = non_blank(row.task_id.as_deref()).ok_or("task id is missing")?;
    let node_id = non_blank(row.node_id.as_deref()).ok_or("workflow node is missing")?;
    let form_data = row
        .form_data
        .clone()
        .ok_or("accepted form data is missing")?;
    if !form_data.is_object() {
        return Err("accepted form data is not an object");
    }
    Ok((story_id, task_id, node_id, form_data))
}

/// A row names its task or its node when the column is there and is not blank: both reach recovery as
/// nullable text, and a row written before the writer set them is the row this module has to survive.
fn non_blank(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn completion_record_from_event(row: db::UnfinishedForgeCompletion) -> Result<CompletionRecord> {
    let conflict = |reason: &str| {
        failed(
            &format!("recovery event {} provenance", row.event_id),
            reason,
        )
    };
    let (story_id, task_id, node_id, form_data) = completion_parts(&row).map_err(conflict)?;
    let form = workflow::json::parse(&form_data.to_string())
        .map_err(|error| conflict(&format!("accepted form data is invalid JSON: {error}")))?;
    if form.as_object().is_none() {
        return Err(conflict("accepted form data did not decode to an object"));
    }
    Ok(CompletionRecord {
        task_id,
        process_instance_id: row.process_instance_id,
        story_id,
        node_id: Some(node_id),
        evidence: crate::engine::facts::evidence_from_value(&form),
    })
}

/// The one discovery read the two recovery paths share: a bounded page of rows by event id. The ledger
/// method maps the page strictly; the worker's sweep maps it row by row (see below).
fn unfinished_rows(
    story_id: Option<&str>,
    limit: usize,
) -> Result<Vec<db::UnfinishedForgeCompletion>> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.unfinished_forge_completions(story_id, limit)
                .await
                .map_err(|error| error.to_string())
        })
    })
    .map_err(|error| failed("discover unfinished completions", error))?
    .map_err(|error| failed("discover unfinished completions", error))
}

/// Recover a bounded page of durable accepted completions at the worker service boundary.
/// A successful apply removes that event from the next page; failure stays visible and is retried
/// on the next worker pass. No model, process, or workflow action is repeated.
///
/// ONE UNREADABLE ROW IS NOT A STOPPED QUEUE (2026-10-12). The page is ordered by event id, so the first row
/// the ledger cannot *map* is the first row of every later page too: while mapping one row was fatal, the
/// worker exited non-zero before it claimed anything, every three minutes, and the rows behind the unreadable
/// one were never looked at. A row whose shape cannot describe a completion is now named and skipped — the
/// rows behind it are still recovered, and it stays unfinished (and keeps being named here) until its row is
/// repaired — while a failed `apply` (a database that cannot answer, a receipt that holds another unit)
/// stays fatal exactly as it was: calling those "skipped" would silently drop effects the ledger never
/// applied, which is the one thing this pass must never do.
pub fn reconcile_unfinished_completion_batch(limit: usize) -> Result<usize> {
    let ledger = DbCompletionLedger;
    let mut applied = 0;
    let mut shapes: Vec<&'static str> = Vec::new();
    let mut details = Vec::new();
    for row in unfinished_rows(None, limit.max(1))? {
        let event_id = row.event_id;
        if let Err(reason) = completion_parts(&row) {
            shapes.push(reason);
            continue;
        }
        match completion_record_from_event(row) {
            Ok(record) => {
                if ledger.apply(&record)?.applied() {
                    applied += 1;
                }
            }
            Err(error) => details.push(format!("event {event_id}: {error}")),
        }
    }
    if !shapes.is_empty() {
        // Counted, not reprinted: the page is ordered by event id and re-read on every pass, so a row that
        // cannot describe a completion sits in every later page as well. One line per reason is the whole
        // signal a human needs, and it stays one line for as long as that row is there.
        let quarantined = shapes.len();
        let mut counted: Vec<(&str, usize)> = Vec::new();
        for reason in shapes {
            match counted.iter_mut().find(|(named, _)| *named == reason) {
                Some((_, count)) => *count += 1,
                None => counted.push((reason, 1)),
            }
        }
        let summary = counted
            .iter()
            .map(|(reason, count)| format!("{count} x {reason}"))
            .collect::<Vec<_>>()
            .join("; ");
        eprintln!(
            "forge-worker: completion recovery quarantined {quarantined} row(s) this pass — {summary}. They stay unfinished (the next pass names them again) until the row is repaired; the rest of the page is still recovered."
        );
    }
    for detail in &details {
        eprintln!("forge-worker: completion recovery quarantined {detail}");
    }
    Ok(applied)
}

/// The prefix the watermark is read under, exposed so the runtime and the ledger spell it once
/// (`neon_sql::RECEIPT_PREFIX`) instead of each holding its own literal.
pub fn completion_receipt_prefix() -> &'static str {
    RECEIPT_PREFIX
}

#[cfg(test)]
mod tests {
    use super::{completion_parts, story_identity};

    /// The answer this policy has to give: an absent key is not a disagreement. The middle cases are the
    /// ones that stood between every worker pass and its queue — a month of completions accepted before
    /// Forge wrote the key at all.
    #[test]
    fn story_identity_reads_an_absent_business_key_as_agreement() {
        assert_eq!(
            story_identity(Some("story"), Some("ENG-1"), None).as_deref(),
            Ok("ENG-1")
        );
        assert_eq!(
            story_identity(Some("story"), Some("ENG-1"), Some("")).as_deref(),
            Ok("ENG-1")
        );
        assert_eq!(
            story_identity(Some("story"), Some("ENG-1"), Some("   ")).as_deref(),
            Ok("ENG-1")
        );
        assert_eq!(
            story_identity(Some("story"), Some(" ENG-1 "), Some("ENG-1")).as_deref(),
            Ok("ENG-1")
        );
    }

    /// And the answers it must never give: a key that is there and disagrees, or a row that names no story.
    #[test]
    fn story_identity_refuses_a_key_that_disagrees_and_a_row_that_names_no_story() {
        assert_eq!(
            story_identity(Some("story"), Some("ENG-1"), Some("ENG-2")),
            Err("business key disagrees with story subject id")
        );
        assert_eq!(
            story_identity(None, Some("ENG-1"), None),
            Err("process subject is not a story")
        );
        assert_eq!(
            story_identity(Some("ticket"), Some("ENG-1"), None),
            Err("process subject is not a story")
        );
        assert_eq!(
            story_identity(Some("story"), None, None),
            Err("story subject id is missing")
        );
        assert_eq!(
            story_identity(Some("story"), Some("  "), None),
            Err("story subject id is missing")
        );
    }

    /// The production row. Event 1173 and the 556 behind it arrived with no business key and no node id —
    /// both written before Forge recorded them. The missing key must not be what refuses the row (doing that
    /// was the outage); the missing node must be, because the node is what decides which budget a completion
    /// spends, and spending the wrong budget is worse than reporting the row unreadable.
    #[test]
    fn completion_parts_refuses_a_legacy_row_for_its_missing_node_not_its_missing_key() {
        let row =
            |business_key: Option<&str>, node_id: Option<&str>| db::UnfinishedForgeCompletion {
                event_id: 1173,
                process_instance_id: "f5cbf444-5661-45af-976f-4735d505c743".to_string(),
                subject_type: Some("story".to_string()),
                subject_id: Some("ENG-FORGE-V13".to_string()),
                business_key: business_key.map(str::to_string),
                task_id: Some("2b2d3527-8eb4-46b2-92e6-23e11c1f7b3e".to_string()),
                node_id: node_id.map(str::to_string),
                form_data: Some(serde_json::json!({"candidateSha": "a4f0"})),
            };
        assert_eq!(
            completion_parts(&row(None, None)),
            Err("workflow node is missing")
        );
        let named = completion_parts(&row(None, Some("smith")))
            .map(|(story, task, node, form)| (story, task, node, form.is_object()));
        assert_eq!(
            named,
            Ok((
                "ENG-FORGE-V13".to_string(),
                "2b2d3527-8eb4-46b2-92e6-23e11c1f7b3e".to_string(),
                "smith".to_string(),
                true
            ))
        );
    }
}
