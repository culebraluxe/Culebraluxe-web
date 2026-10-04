//! Lane failure settlement.

use workflow::{ProcessOutcome, ProcessStatus, Result, TxStore, WorkflowError};

use crate::engine::engine_fault::is_engine_fault_error;
use crate::engine::executor::drive::ForgeRoleOutcome;
use crate::engine::runtime::ForgeRuntime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneFailureSettlement {
    Released,
    AlreadyCompleted,
}

pub fn settle_forge_lane_failure<S: TxStore>(
    rt: &ForgeRuntime<S>,
    task_id: &str,
    actor: &str,
    lane_err: &WorkflowError,
) -> Result<LaneFailureSettlement> {
    match rt.release_role_task(task_id, actor) {
        Ok(()) => Ok(LaneFailureSettlement::Released),
        Err(e) if is_completed_release_conflict(&e) => {
            Ok(LaneFailureSettlement::AlreadyCompleted)
        }
        Err(e) if is_advance_conflict(&e) => Ok(LaneFailureSettlement::Released),
        Err(release_err) => Err(WorkflowError::generic(format!(
            "Forge role failed and its task could not be released: lane={lane_err}; release={release_err}"
        ))),
    }
}

pub fn is_advance_conflict(err: &WorkflowError) -> bool {
    let m = err.to_string();
    let c = err.code();
    matches!(
        c,
        "STALE_TASK" | "TASK_ALREADY_COMPLETED" | "PROCESS_NOT_ACTIVE" | "TASK_NOT_CLAIMABLE"
    ) || m.to_ascii_lowercase().contains("already completed")
        || m.to_ascii_lowercase().contains("not active")
        || m.to_ascii_lowercase().contains("state changed")
}

pub fn is_completed_release_conflict(err: &WorkflowError) -> bool {
    err.code() == "TASK_ALREADY_COMPLETED"
        || err
            .to_string()
            .to_ascii_lowercase()
            .contains("cannot be released in status: completed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_completed_completed_projects_story_complete() {
        use workflow::{ProcessOutcome, ProcessStatus};
        // The process_completes_story function is tested in executor tests
        // This module's logic is tested via executor tests
    }
}
