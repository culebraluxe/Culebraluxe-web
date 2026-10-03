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
//! FAILURE IS NOT ABSENCE. Every method returns `Result`: a database that cannot answer must stop the
//! caller, because `false` from `claim` means "someone already applied this unit" and reporting a lost
//! connection that way would silently drop the evidence and the counters.

use std::sync::Arc;

use db::{ForgeEngineDao, WorkflowReceiptClaim};
use workflow::{Result, WorkflowError};

use crate::engine::completion::{CompletionLedger, CompletionRecord};
use crate::engine::evidence_store::evidence_patch;
use crate::engine::neon_sql::RECEIPT_PREFIX;
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

impl CompletionLedger for DbCompletionLedger {
    fn claim(&self, receipt_id: &str) -> Result<bool> {
        let command_id = receipt_id.to_string();
        // The receipt's actor column is a uuid (`actor_app_user_id`); a role task has no app-user
        // identity and inventing one would be a second identity for the same fact.
        let actor: Option<&str> = None;
        with_shared(|db, rt| {
            let dao = ForgeEngineDao::new(db.clone());
            rt.block_on(async {
                dao.claim_workflow_receipt(&command_id, actor)
                    .await
                    .map(|claim| matches!(claim, WorkflowReceiptClaim::Acquired))
                    .map_err(|error| error.to_string())
            })
        })
        .map_err(|error| failed("claim", error))?
        .map_err(|error| failed(&command_id, error))
    }

    fn finalize(&self, receipt_id: &str) -> Result<()> {
        let command_id = receipt_id.to_string();
        with_shared(|db, rt| {
            let dao = ForgeEngineDao::new(db.clone());
            rt.block_on(async {
                dao.finalize_workflow_receipt(&command_id, "success", None, None)
                    .await
                    .map_err(|error| error.to_string())
            })
        })
        .map_err(|error| failed("finalize", error))?
        .map_err(|error| failed(&command_id, error))
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

    fn merge_evidence(&self, rec: &CompletionRecord) -> Result<()> {
        self.merge(rec)
    }

    fn increment_repair(&self, story_id: &str) -> Result<()> {
        self.increment(story_id, "repair")
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

    fn increment_replan(&self, story_id: &str) -> Result<()> {
        self.increment(story_id, "replan")
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

/// The prefix the watermark is read under, exposed so the runtime and the ledger spell it once
/// (`neon_sql::RECEIPT_PREFIX`) instead of each holding its own literal.
pub fn completion_receipt_prefix() -> &'static str {
    RECEIPT_PREFIX
}

impl DbCompletionLedger {
    fn merge(&self, rec: &CompletionRecord) -> Result<()> {
        // One mapping, one writer: the same `evidence_patch` the workflow-evidence port uses, so the
        // ledger and the port cannot disagree about which column a field lands in.
        let patch = evidence_patch(&rec.evidence);
        let instance_id = rec.process_instance_id.clone();
        let story_id = rec.story_id.clone();
        with_shared(|db, rt| {
            let dao = ForgeEngineDao::new(db.clone());
            rt.block_on(async {
                dao.merge_workflow_evidence(&instance_id, &story_id, &patch, false)
                    .await
                    .map_err(|error| error.to_string())
            })
        })
        .map_err(|error| failed("merge_evidence", error))?
        .map_err(|error| failed(&format!("merge_evidence({story_id})"), error))
    }

    fn increment(&self, story_id: &str, which: &str) -> Result<()> {
        let story_id = story_id.to_string();
        with_shared(|db, rt| {
            let dao = ForgeEngineDao::new(db.clone());
            rt.block_on(async {
                let result = if which == "repair" {
                    dao.increment_forge_repair_attempts(&story_id).await
                } else {
                    dao.increment_forge_replan_attempts(&story_id).await
                };
                result.map_err(|error| error.to_string())
            })
        })
        .map_err(|error| failed(which, error))?
        .map_err(|error| failed(&format!("{which}({story_id})"), error))
    }
}
