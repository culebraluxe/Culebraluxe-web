//! Forge agent-work orchestration. Persistence lives in db::ForgeEngineDao.

use crate::engine::vendor_session::with_shared;
use db::ForgeEngineDao;

pub const AGENT_CLAIM_LOCK: i64 = 9_000_212;

/// The one policy the unattended poller may run (migration 029: "only 'Unattended OK' work may be claimed by the
/// unattended poller"). The other three values are `Daytime Only`, `Human Gate` and `Manual Only`.
pub const UNATTENDED_OK: &str = "Unattended OK";

/// May this item be executed with nobody watching?
///
/// The column is NOT NULL and CHECK-constrained to four values, but an unrecognised value is **not** allowed
/// through: this is a rail, and a rail that opens on a value it does not know is not a rail.
pub fn execution_policy_allows_unattended(policy: &str) -> bool {
    policy.trim() == UNATTENDED_OK
}

#[derive(Debug, Clone)]
pub struct AgentWorkItem {
    pub id: String,
    pub story_id: String,
    pub state: String,
    pub claimed_by: Option<String>,
    pub role: Option<String>,
    pub kind: Option<String>,
    /// The durable dispatch envelope (migrations 029 and 167), carried on the claim so the run is configured by the
    /// row rather than by argv. `None`/`null` means "not set", which has a meaning of its own: a null `stop_after`
    /// is the full chain, a null `launch_intent` leaves the decision to the Lead.
    pub execution_policy: String,
    pub model_policy: Option<String>,
    pub stop_after: Option<String>,
    pub launch_intent: Option<String>,
}

fn map(row: db::ForgeAgentWorkRow) -> AgentWorkItem {
    AgentWorkItem {
        id: row.id,
        story_id: row.story_id,
        state: row.state,
        claimed_by: row.claimed_by,
        role: row.role,
        kind: row.kind,
        execution_policy: row.execution_policy,
        model_policy: row.model_policy,
        stop_after: row.stop_after,
        launch_intent: row.launch_intent,
    }
}

pub fn claim_specific_agent_work(
    work_item_id: &str,
    worker_id: &str,
) -> Result<Option<AgentWorkItem>, String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.claim_specific_agent_work(work_item_id, worker_id)
                .await
                .map(|row| row.map(map))
                .map_err(|error| error.to_string())
        })
    })?
}

pub fn claim_next_agent_work(worker_id: &str) -> Result<Option<AgentWorkItem>, String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.claim_next_agent_work(worker_id)
                .await
                .map(|row| row.map(map))
                .map_err(|error| error.to_string())
        })
    })?
}

/// `Claimed → Running`, returning the row's durable envelope **and the Story Run it opened**. `Ok(None)` means the
/// row was not `Claimed`, so this process does not own the run it is about to start and must not drive the story.
/// It is a fence, not a formality: the update is a CAS, and the engine is only ever launched behind it.
///
/// The policy rides the fence because it decides whether the run may happen at all — see
/// `execution_policy_allows_unattended`. The run id rides it because every durable artifact the lane produces hangs
/// off `storyboard_story_run` (migration 025 §2: the spec is "snapshotted into `storyboard_story_run` when execution
/// begins"), so a caller that cannot name its run cannot record what it did.
/// `Claimed → Running`, opening the run this claim executes, with the specification snapshotted into it by the
/// insert itself (`begin_agent_work_run` copies `storyboard_story`'s twelve specification columns — migration 024
/// §2: "snapshotted into `storyboard_story_run` when execution begins").
pub fn begin_agent_work_run(work_item_id: &str) -> Result<Option<db::BeginAgentWorkRun>, String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.begin_agent_work_run(work_item_id)
                .await
                .map_err(|error| error.to_string())
        })
    })?
}

/// Stamp the run's base commit once provisioning knows it. False = nothing written (blank, unknown run, or a base
/// already recorded, which is never overwritten).
pub fn stamp_run_base_commit(run_id: &str, base_commit: &str) -> Result<bool, String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.stamp_run_base_commit(run_id, base_commit)
                .await
                .map_err(|error| error.to_string())
        })
    })?
}

pub fn reject_agent_work_configuration(work_item_id: &str, evidence: &str) -> Result<(), String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.reject_agent_work_configuration(work_item_id, evidence)
                .await
                .map_err(|error| error.to_string())
        })
    })?
}

/// The run's own terminal write — the item **and its story**, decided together inside one transaction.
///
/// `Ok(None)` means the row was no longer claimable: a settle that raced another settle and lost, which is reported,
/// not retried. `Ok(Some(settled))` carries the pair that was actually written, including the case where a `Done`
/// was refused in favour of `Error` because the board never confirmed completion — the caller must not report a
/// `Done` the database did not accept.
pub fn finish_agent_work_run(
    work_item_id: &str,
    outcome: db::AgentWorkOutcome,
    error_text: Option<&str>,
) -> Result<Option<db::AgentWorkSettlement>, String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.finish_agent_work_run(work_item_id, outcome, error_text)
                .await
                .map_err(|error| error.to_string())
        })
    })?
}

/// Clean the control plane before a claim is taken: the queue and the board are put back into agreement, and work
/// that no longer exists is cleared rather than left to be read as queue state. Called at the top of every worker
/// pass, so it runs before each run — see `db::ForgeEngineDao::reconcile_dispatch_queue`.
pub fn reconcile_dispatch_queue() -> Result<db::DispatchReconcile, String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.reconcile_dispatch_queue()
                .await
                .map_err(|error| error.to_string())
        })
    })?
}

#[cfg(test)]
mod execution_policy_tests {
    use super::*;

    /// Migration 029's rule, as a test: only `Unattended OK` may run with nobody watching.
    ///
    /// `Human Gate` is the one that matters most — the column is NOT NULL, the engine's own vocabulary has a value
    /// that says "needs a human", and nothing in the Rust engine read it, so work the rail marked as human-gated was
    /// dispatched to a model exactly like unattended work.
    #[test]
    fn only_unattended_ok_may_run_unattended() {
        assert!(execution_policy_allows_unattended("Unattended OK"));
        assert!(execution_policy_allows_unattended("  Unattended OK  "));
        for policy in ["Daytime Only", "Human Gate", "Manual Only"] {
            assert!(!execution_policy_allows_unattended(policy), "{policy}");
        }
        // A value nobody defined is refused, not admitted: this is a rail.
        for unknown in ["", "unattended ok", "UNATTENDED OK", "anything", "null"] {
            assert!(!execution_policy_allows_unattended(unknown), "{unknown}");
        }
    }
}

/// Touch the claim so `stale_agent_work` does not requeue a run that is still alive. False = no longer claimable.
pub fn heartbeat_agent_work(work_item_id: &str) -> Result<bool, String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.heartbeat_agent_work(work_item_id)
                .await
                .map_err(|error| error.to_string())
        })
    })?
}
