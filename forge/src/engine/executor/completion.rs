//! Completion write retry logic.

use workflow::{Result, TxStore, WorkflowError};

use crate::engine::engine_fault::is_engine_fault_error;
use crate::engine::executor::drive::ForgeRoleOutcome;
use crate::engine::runtime::ForgeRuntime;

/// How many times a Workflow completion write is repeated while the failure
/// is the engine's own plumbing.
///
/// THE ROLE TURN HAS ALREADY BEEN PAID FOR when this runs, so the only thing
/// worth repeating is the WRITE (2026-10-02). A completion that died
/// mid-transaction is also safe to repeat: the engine answers
/// `TASK_ALREADY_COMPLETED` when the earlier attempt actually committed, and
/// the caller already reads that as "the Workflow settled — close the job".
/// Repeating the TURN instead is precisely what the completion-before-completion
/// ordering exists to prevent, so it is not on the table.
const COMPLETION_WRITE_ATTEMPTS: u32 = 3;

/// Settle the Workflow task the role just finished, repeating only the write.
///
/// The ordering invariant is untouched: Workflow state still wins before the
/// durable execution receipt closes. What this adds is what happens when the
/// WRITE itself fails — a database that went away mid-commit is the engine's
/// failure and says nothing about the story, so it is retried while the lease
/// is still held rather than being recorded on the first attempt. Anything
/// that is not plumbing goes back at once, unwrapped and unattempted.
pub fn complete_role_task_with_transient_retry<S: TxStore>(
    rt: &ForgeRuntime<S>,
    task_id: &str,
    actor: &str,
    outcome: &ForgeRoleOutcome,
) -> Result<()> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        match rt.complete_role_task(
            task_id,
            actor,
            outcome.transition_name.as_deref(),
            outcome.evidence.clone(),
        ) {
            Ok(_) => return Ok(()),
            Err(error) if should_repeat_completion_write(attempt, &error) => {
                eprintln!(
                    "forge: Workflow completion for task {task_id} hit an engine fault (attempt \
                     {attempt}/{COMPLETION_WRITE_ATTEMPTS}); repeating the write, not the turn: {error}"
                );
            }
            Err(error) => return Err(error),
        }
    }
}

/// Whether a completion failure is repeated — the whole rule, as a function of
/// the attempt and the error.
///
/// Stated separately so it can be railed as a table rather than inferred from a
/// run: the repeat is bounded by the budget, and only the engine's own
/// plumbing is repeated at all.
fn should_repeat_completion_write(attempt: u32, error: &WorkflowError) -> bool {
    attempt < COMPLETION_WRITE_ATTEMPTS && is_engine_fault_error(error)
}

/// The durable reason a completion write that never landed leaves behind.
///
/// It names the one thing an operator must not do — pay for the turn again —
/// because this row is the only evidence that the model work was already spent:
/// the Workflow task is still open, so a later run of the same story would
/// otherwise find READY work for a turn that has already been executed once.
pub fn completion_failure_reason(error: &WorkflowError) -> String {
    if is_engine_fault_error(error) {
        format!(
            "PAID_TURN_NOT_REDISPATCHED: the role turn for this task ran, but the Workflow completion write failed on \
             every attempt ({error}). Reconcile the Workflow task, then requeue this job; do not run the turn again."
        )
    } else {
        format!("Workflow task completion failed after role execution: {error}")
    }
}

#[cfg(test)]
mod completion_fault_tests {
    use super::*;
    use workflow::WorkflowError;

    /// The rule, as a table: only the engine's plumbing is repeated, and only
    /// while the write budget lasts.
    ///
    /// The turn has already been paid for when this runs, so the difference
    /// between "repeat" and "hand back" is the difference between one lost
    /// write and a second model turn — which is why it is pinned rather than
    /// read.
    #[test]
    fn only_a_plumbing_failure_is_repeated_and_only_while_the_budget_lasts() {
        let plumbing = WorkflowError::unavailable("db: sqlstate 25P03 idle-in-transaction");
        let sqlstate_only = WorkflowError::generic("db: DatabaseUnavailable during workflow.step");
        let verdict = WorkflowError::conflict("PROCESS_NOT_ACTIVE", "the instance is not active");

        assert!(should_repeat_completion_write(1, &plumbing));
        assert!(should_repeat_completion_write(1, &sqlstate_only));
        assert!(should_repeat_completion_write(
            COMPLETION_WRITE_ATTEMPTS - 1,
            &plumbing
        ));
        assert!(
            !should_repeat_completion_write(COMPLETION_WRITE_ATTEMPTS, &plumbing),
            "the write is repeated, not repeated forever"
        );
        for attempt in 1..=COMPLETION_WRITE_ATTEMPTS {
            assert!(
                !should_repeat_completion_write(attempt, &verdict),
                "attempt {attempt} repeated a decision the engine already made"
            );
        }
    }

    /// The durable row an operator reads has to say the one thing that must not
    /// happen next.
    ///
    /// The Workflow task is still open when this reason is written, so the row
    /// is the only evidence that the model work was already spent: a reason
    /// that does not say so invites a second paid turn.
    #[test]
    fn a_completion_that_never_landed_names_the_paid_turn() {
        let plumbing = WorkflowError::unavailable("db: DatabaseUnavailable (sqlstate 25P03)");
        let reason = completion_failure_reason(&plumbing);
        assert!(reason.contains("PAID_TURN_NOT_REDISPATCHED"), "{reason}");
        assert!(reason.contains("do not run the turn again"), "{reason}");
        assert!(reason.contains("sqlstate 25P03"), "{reason}");

        let verdict = WorkflowError::conflict("PROCESS_NOT_ACTIVE", "the instance is not active");
        let reason = completion_failure_reason(&verdict);
        assert!(
            !reason.contains("PAID_TURN_NOT_REDISPATCHED"),
            "an engine decision is not a plumbing failure: {reason}"
        );
        assert!(
            reason.contains("Workflow task completion failed after role execution"),
            "{reason}"
        );
    }
}