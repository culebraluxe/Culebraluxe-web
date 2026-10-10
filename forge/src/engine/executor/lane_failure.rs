//! Lane failure settlement.

use workflow::{Result, TxStore, WorkflowError};

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
    matches!(
        err.code(),
        "STALE_TASK" | "TASK_ALREADY_COMPLETED" | "PROCESS_NOT_ACTIVE" | "TASK_NOT_CLAIMABLE"
    )
}

pub fn is_completed_release_conflict(err: &WorkflowError) -> bool {
    err.code() == "TASK_ALREADY_COMPLETED"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_conflicts_are_recognized_by_stable_code() {
        for code in [
            "STALE_TASK",
            "TASK_ALREADY_COMPLETED",
            "PROCESS_NOT_ACTIVE",
            "TASK_NOT_CLAIMABLE",
        ] {
            assert!(is_advance_conflict(&WorkflowError::conflict(
                code, "detail"
            )));
        }
        assert!(!is_advance_conflict(&WorkflowError::generic(
            "task state changed concurrently"
        )));
    }

    #[test]
    fn unrelated_conflict_text_does_not_masquerade_as_an_advance_conflict() {
        let error = WorkflowError::conflict("UNRELATED", "process is not active");
        assert!(!is_advance_conflict(&error));
    }

    #[test]
    fn completed_release_conflict_uses_its_code() {
        assert!(is_completed_release_conflict(&WorkflowError::conflict(
            "TASK_ALREADY_COMPLETED",
            "completion already committed"
        )));
        assert!(!is_completed_release_conflict(&WorkflowError::generic(
            "cannot be released in status: completed"
        )));
    }
}
