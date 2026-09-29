//! Forge agent-work orchestration. Persistence lives in db::ForgeEngineDao.

use crate::engine::vendor_session::with_shared;
use db::ForgeEngineDao;

pub const AGENT_CLAIM_LOCK: i64 = 9_000_212;

#[derive(Debug, Clone)]
pub struct AgentWorkItem {
    pub id: String,
    pub story_id: String,
    pub state: String,
    pub claimed_by: Option<String>,
    pub role: Option<String>,
    pub kind: Option<String>,
}

fn map(row: db::ForgeAgentWorkRow) -> AgentWorkItem {
    AgentWorkItem {
        id: row.id,
        story_id: row.story_id,
        state: row.state,
        claimed_by: row.claimed_by,
        role: row.role,
        kind: row.kind,
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

/// `Claimed → Running`. `Ok(false)` means the row was not `Claimed`, so this process does not own the run it is
/// about to start and must not drive the story. It is a fence, not a formality: the update is a CAS, and the engine
/// is only ever launched behind it.
pub fn begin_agent_work_run(work_item_id: &str) -> Result<bool, String> {
    with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.begin_agent_work_run(work_item_id)
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
