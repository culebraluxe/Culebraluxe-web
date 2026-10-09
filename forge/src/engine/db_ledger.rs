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
            DbApply::Busy => Err(failed(
                &command_id,
                "another process holds this unit mid-write",
            )),
        }
    }

    fn has_final(&self, receipt_id: &str) -> Result<bool> {
        let command_id = receipt_id.to_string();
        with_shared(|db, rt| {
            let dao = ForgeEngineDao::new(db.clone());
            rt.block_on(async {
                dao.read_workflow_receipt_outcome(&command_id)
                    .await
                    .map(|outcome| outcome.is_some())
                    .map_err(|error| error.to_string())
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
        let rows = with_shared(|db, rt| {
            let dao = ForgeEngineDao::new(db.clone());
            rt.block_on(async {
                dao.unfinished_forge_completions(story_id, limit)
                    .await
                    .map_err(|error| error.to_string())
            })
        })
        .map_err(|error| failed("discover unfinished completions", error))?
        .map_err(|error| failed("discover unfinished completions", error))?;
        rows.into_iter()
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

fn completion_record_from_event(row: db::UnfinishedForgeCompletion) -> Result<CompletionRecord> {
    let conflict = |reason: &str| {
        failed(
            &format!("recovery event {} provenance", row.event_id),
            reason,
        )
    };
    if row.subject_type.as_deref() != Some("story") {
        return Err(conflict("process subject is not a story"));
    }
    let story_id = row
        .subject_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| conflict("story subject id is missing"))?;
    if row.business_key.as_deref() != Some(story_id) {
        return Err(conflict("business key disagrees with story subject id"));
    }
    let task_id = row
        .task_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| conflict("task id is missing"))?;
    let node_id = row
        .node_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| conflict("workflow node is missing"))?;
    let form_data = row
        .form_data
        .ok_or_else(|| conflict("accepted form data is missing"))?;
    if !form_data.is_object() {
        return Err(conflict("accepted form data is not an object"));
    }
    let form = workflow::json::parse(&form_data.to_string())
        .map_err(|error| conflict(&format!("accepted form data is invalid JSON: {error}")))?;
    if form.as_object().is_none() {
        return Err(conflict("accepted form data did not decode to an object"));
    }
    Ok(CompletionRecord {
        task_id,
        process_instance_id: row.process_instance_id,
        story_id: story_id.to_string(),
        node_id: Some(node_id),
        evidence: crate::engine::facts::evidence_from_value(&form),
    })
}

/// Recover a bounded page of durable accepted completions at the worker service boundary.
/// A successful apply removes that event from the next page; failure stays visible and is retried
/// on the next worker pass. No model, process, or workflow action is repeated.
pub fn reconcile_unfinished_completion_batch(limit: usize) -> Result<usize> {
    let ledger = DbCompletionLedger;
    let records = ledger.unfinished(None, limit.max(1))?.unwrap_or_default();
    let mut applied = 0;
    for record in records {
        if ledger.apply(&record)?.applied() {
            applied += 1;
        }
    }
    Ok(applied)
}

/// The prefix the watermark is read under, exposed so the runtime and the ledger spell it once
/// (`neon_sql::RECEIPT_PREFIX`) instead of each holding its own literal.
pub fn completion_receipt_prefix() -> &'static str {
    RECEIPT_PREFIX
}
