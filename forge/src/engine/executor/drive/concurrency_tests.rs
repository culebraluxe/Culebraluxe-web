// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;
use crate::engine::facts::ForgeGateEvidence;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;
use workflow::WorkflowError;

#[derive(Default)]
struct OverlapGate {
    started: Mutex<usize>,
    changed: Condvar,
    active: AtomicUsize,
    maximum_active: AtomicUsize,
    both_entered: AtomicUsize,
}

struct BlockingRunner {
    gate: Arc<OverlapGate>,
    fail: bool,
}

impl ForgeRoleRunner for BlockingRunner {
    fn run(&self, _node_id: &str, _task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
        let active = self.gate.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.gate.maximum_active.fetch_max(active, Ordering::SeqCst);
        let mut started = self.gate.started.lock().unwrap();
        *started += 1;
        self.gate.changed.notify_all();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while *started < 2 {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            let (next, timeout) = self.gate.changed.wait_timeout(started, remaining).unwrap();
            started = next;
            if timeout.timed_out() {
                break;
            }
        }
        if *started >= 2 {
            self.gate.both_entered.fetch_add(1, Ordering::SeqCst);
        }
        drop(started);
        self.gate.active.fetch_sub(1, Ordering::SeqCst);
        if self.fail {
            return Err(WorkflowError::generic("planned lane failure"));
        }
        Ok(ForgeRoleOutcome {
            transition_name: Some("complete".into()),
            evidence: ForgeGateEvidence::default(),
        })
    }

    fn run_scoped(
        &self,
        _execution_id: &str,
        node_id: &str,
        task: &ActiveForgeRoleTask,
    ) -> Result<ForgeRoleOutcome> {
        self.run(node_id, task)
    }
}

/// A lane that panics instead of reporting (`WO-121` §3): the drive wrapper has to turn that into a
/// recorded per-lane failure instead of letting the thread take the batch with it.
struct PanicRunner;

impl ForgeRoleRunner for PanicRunner {
    fn run(&self, _node_id: &str, _task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
        panic!("lane runner panicked");
    }
}

struct NoopJobs;

impl JobService for NoopJobs {
    fn enqueue(&self, _request: &crate::engine::job::ForgeJobRequest) -> Result<String> {
        unreachable!()
    }
    fn claim(&self, _worker_id: &str, _limit: usize) -> Result<Vec<ForgeJobLease>> {
        unreachable!()
    }
    fn claim_one(&self, _job_id: &str, _worker_id: &str) -> Result<ForgeJobLease> {
        unreachable!()
    }
    fn heartbeat(&self, _job_id: &str, _worker_id: &str) -> Result<i64> {
        Ok(1)
    }
    fn inspect(&self, _job_id: &str) -> Result<crate::engine::job::ForgeJobState> {
        Err(WorkflowError::generic("unused"))
    }
    fn complete(&self, _job_id: &str, _worker_id: &str) -> Result<()> {
        Ok(())
    }
    fn fail(&self, _job_id: &str, _worker_id: &str, _error: &str, _permanent: bool) -> Result<()> {
        Ok(())
    }
    fn cancel(&self, _job_id: &str, _actor: &str) -> Result<()> {
        Ok(())
    }
    fn requeue(&self, _job_id: &str, _actor: &str) -> Result<()> {
        Ok(())
    }
    fn recover_stale(&self, _batch: usize) -> Result<usize> {
        Ok(0)
    }
}

/// Jobs that refuse to settle a claim. `WO-121` is about exactly this: the failure that happens while
/// another failure is being handled, which used to be dropped by a `let _ =`.
struct RefusingFailJobs;

impl JobService for RefusingFailJobs {
    fn enqueue(&self, _request: &crate::engine::job::ForgeJobRequest) -> Result<String> {
        unreachable!()
    }
    fn claim(&self, _worker_id: &str, _limit: usize) -> Result<Vec<ForgeJobLease>> {
        unreachable!()
    }
    fn claim_one(&self, _job_id: &str, _worker_id: &str) -> Result<ForgeJobLease> {
        unreachable!()
    }
    fn heartbeat(&self, _job_id: &str, _worker_id: &str) -> Result<i64> {
        Ok(1)
    }
    fn inspect(&self, _job_id: &str) -> Result<crate::engine::job::ForgeJobState> {
        Err(WorkflowError::generic("unused"))
    }
    fn complete(&self, _job_id: &str, _worker_id: &str) -> Result<()> {
        Ok(())
    }
    fn fail(&self, _job_id: &str, _worker_id: &str, _error: &str, _permanent: bool) -> Result<()> {
        Err(WorkflowError::generic("claim already lost"))
    }
    fn cancel(&self, _job_id: &str, _actor: &str) -> Result<()> {
        Ok(())
    }
    fn requeue(&self, _job_id: &str, _actor: &str) -> Result<()> {
        Ok(())
    }
    fn recover_stale(&self, _batch: usize) -> Result<usize> {
        Ok(0)
    }
}

fn prepared_lane(task_id: &str, gate: &Arc<OverlapGate>, fail: bool) -> PreparedDurableLane {
    prepared_lane_with_runner(
        task_id,
        Box::new(BlockingRunner {
            gate: Arc::clone(gate),
            fail,
        }),
    )
}

fn prepared_lane_with_runner(
    task_id: &str,
    runner: Box<dyn ForgeRoleRunner>,
) -> PreparedDurableLane {
    let task = ActiveForgeRoleTask {
        task_id: task_id.into(),
        process_instance_id: "instance-1".into(),
        story_id: "story-1".into(),
        token_id: Some(format!("token-{task_id}")),
        node_id: Some("smith".into()),
        status: TaskStatus::Ready,
        assignee: None,
        candidates: vec![],
        write_surface: Some(vec![format!("src/{task_id}.rs")]),
    };
    let lease = ForgeJobLease {
        job_id: format!("job-{task_id}"),
        service_key: "forge.smith".into(),
        node_id: "smith".into(),
        task_id: task_id.into(),
        process_instance_id: "instance-1".into(),
        story_id: "story-1".into(),
        token_id: task.token_id.clone(),
        attempts: 1,
        max_attempts: 5,
        locked_until: None,
    };
    PreparedDurableLane {
        claimed: ClaimedDurableLane {
            task,
            actor: "worker-1".into(),
            node: "smith".into(),
            lease,
        },
        runner,
        worktree_path: PathBuf::from(format!("/tmp/{task_id}")),
        branch_name: format!("agent/story/{task_id}"),
        parent_worktree: PathBuf::from("/tmp/story"),
        base_commit: "0".repeat(40),
        // A remover that removes nothing: a unit test must not shell out to git, and the guard's own
        // behaviour is what these tests are about (`WO-121`).
        worktrees: LaneWorktree::with_remover(
            PathBuf::from("/tmp"),
            PathBuf::from(format!("/tmp/{task_id}")),
            format!("agent/story/{task_id}"),
            "0".repeat(40),
            Box::new(|_, _, _, _| Ok(())),
        ),
    }
}

#[test]
fn unsupported_overlap_is_rejected_before_claiming_work() {
    assert!(validate_story_concurrency(1, false).is_ok());
    assert!(validate_story_concurrency(2, true).is_ok());
    let error = validate_story_concurrency(2, false).unwrap_err();
    assert_eq!(error.code(), "FORGE_PARALLEL_LANES_UNAVAILABLE");
}

#[test]
fn cap_refusal_and_interruption_are_stops_without_a_role_verdict() {
    let capped = WorkflowError::conflict(
        turn_budget::MODEL_TURN_CAP_CODE,
        "generation exhausted its model-attempt allowance",
    );
    assert_eq!(lane_stop_diagnostic(&capped), ("model_attempt_cap", false));
    assert!(
        lane_failure_reason("smith", "task-1", &capped).contains("no role verdict was accepted")
    );
    assert!(!lane_failure_reason("smith", "task-1", &capped).contains("role smith failed"));

    let interrupted = WorkflowError::conflict(
        crate::engine::harness::TURN_INTERRUPTED_CODE,
        "the supervised turn was interrupted",
    );
    assert_eq!(
        lane_stop_diagnostic(&interrupted),
        ("turn_interrupted", false)
    );
    assert!(lane_failure_reason("smith", "task-2", &interrupted)
        .contains("no role verdict was accepted"));
}

#[test]
fn durable_compatible_lanes_overlap_and_both_report_outcomes() {
    let gate = Arc::new(OverlapGate::default());
    let results = execute_durable_batch(
        vec![
            prepared_lane("task-a", &gate, false),
            prepared_lane("task-b", &gate, false),
        ],
        &NoopJobs,
        "worker-1",
        None,
        "story-1",
        1,
    );
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|result| result.outcome.is_ok()));
    assert_eq!(gate.maximum_active.load(Ordering::SeqCst), 2);
    assert_eq!(gate.both_entered.load(Ordering::SeqCst), 2);
}

#[test]
fn one_lane_failure_does_not_drop_its_completed_sibling_result() {
    let gate = Arc::new(OverlapGate::default());
    let results = execute_durable_batch(
        vec![
            prepared_lane("task-a", &gate, true),
            prepared_lane("task-b", &gate, false),
        ],
        &NoopJobs,
        "worker-1",
        None,
        "story-1",
        2,
    );
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|result| result.outcome.is_err()));
    assert!(results.iter().any(|result| result.outcome.is_ok()));
    assert_eq!(gate.maximum_active.load(Ordering::SeqCst), 2);
}

#[test]
fn a_secondary_failure_is_reported_beside_the_primary_one() {
    let note = with_secondary_failure("the lane failed", "worktree discard", "git said no");
    assert!(note.starts_with("the lane failed"), "{note}");
    assert!(note.contains("worktree discard"), "{note}");
    assert!(note.contains("git said no"), "{note}");
}

#[test]
fn a_panicking_lane_is_a_recorded_failure_and_keeps_its_sibling_result() {
    let gate = Arc::new(OverlapGate::default());
    let results = execute_durable_batch(
        vec![
            prepared_lane("task-a", &gate, false),
            prepared_lane_with_runner("task-b", Box::new(PanicRunner)),
        ],
        &NoopJobs,
        "worker-1",
        None,
        "story-1",
        2,
    );
    assert_eq!(results.len(), 2);
    let panicked = results
        .iter()
        .find(|result| result.claimed.task.task_id == "task-b")
        .expect("the panicking lane has a result");
    assert!(panicked.outcome.is_err());
    assert!(panicked
        .outcome
        .as_ref()
        .err()
        .expect("the panic is this lane's failure")
        .to_string()
        .contains("panicked"));
    // The unwind dispositioned this lane's tree through its own guard, and the guard said so.
    assert_eq!(
        panicked.worktrees.disposition(),
        LaneWorktreeDisposition::Retained
    );
    assert!(results
        .iter()
        .any(|result| result.claimed.task.task_id == "task-a" && result.outcome.is_ok()));
}

#[test]
fn a_panicking_lane_reports_the_claim_it_could_not_settle() {
    // `WO-121`: a panic bypasses the settle inside `execute_claimed_job_unsettled`, so the drive's own
    // `jobs.fail` is the only settlement this lane gets. It used to be a `let _ =`.
    let results = execute_durable_batch(
        vec![prepared_lane_with_runner("task-b", Box::new(PanicRunner))],
        &RefusingFailJobs,
        "worker-1",
        None,
        "story-1",
        1,
    );
    assert_eq!(results.len(), 1);
    assert!(results[0].outcome.is_err());
    assert_eq!(
        results[0].settlement_failure.as_deref(),
        Some("claim already lost")
    );
}

#[test]
fn a_lane_that_never_reports_names_the_panic_and_settles_its_claim() {
    let gate = Arc::new(OverlapGate::default());
    let lane = prepared_lane("task-a", &gate, false);
    let fallback = LaneFallback::of(&lane);
    let reason = "lane worker thread panicked before it could report an outcome";
    let result = lane_that_never_reported(&RefusingFailJobs, "worker-1", fallback, reason);
    assert!(result.outcome.is_err());
    assert!(result
        .outcome
        .as_ref()
        .err()
        .unwrap()
        .to_string()
        .contains("panicked"));
    // Both facts survive: the panic, and the claim that could not be settled.
    assert_eq!(
        result.settlement_failure.as_deref(),
        Some("claim already lost")
    );
    assert_eq!(
        result.worktrees.disposition(),
        LaneWorktreeDisposition::Retained
    );
}

#[test]
fn a_release_that_fails_is_returned_and_attempted_only_once() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&attempts);
    let mut tree = LaneWorktree::with_remover(
        PathBuf::from("/tmp"),
        PathBuf::from("/tmp/task-a"),
        "agent/story/task-a".into(),
        "0".repeat(40),
        Box::new(move |_, _, _, _| {
            seen.fetch_add(1, Ordering::SeqCst);
            Err("git worktree remove failed".to_string())
        }),
    );
    assert_eq!(
        tree.release("task-a").as_deref(),
        Some("git worktree remove failed")
    );
    // One lane, one disposition: a second release is not a second cleanup.
    assert!(tree.release("task-a").is_none());
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    assert_eq!(tree.disposition(), LaneWorktreeDisposition::Released);
}

#[test]
fn an_undecided_lane_worktree_is_retained_by_its_drop_not_deleted() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&attempts);
    {
        let _tree = LaneWorktree::with_remover(
            PathBuf::from("/tmp"),
            PathBuf::from("/tmp/task-a"),
            "agent/story/task-a".into(),
            "0".repeat(40),
            Box::new(move |_, _, _, _| {
                seen.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
        );
    }
    // `WO-121` §3: the backstop disposition is retention, because a lane that exited without deciding
    // is not evidence that its tree is empty. What it must never be is silent.
    assert_eq!(attempts.load(Ordering::SeqCst), 0);
}

#[test]
fn a_released_lane_worktree_is_removed_once_and_a_retained_one_is_not_removed() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&attempts);
    {
        let mut released = LaneWorktree::with_remover(
            PathBuf::from("/tmp"),
            PathBuf::from("/tmp/task-a"),
            "agent/story/task-a".into(),
            "0".repeat(40),
            Box::new(move |_, _, _, _| {
                seen.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
        );
        assert!(released.release("task-a").is_none());
        assert_eq!(released.disposition(), LaneWorktreeDisposition::Released);
    }
    // The drop after the release must not remove the same tree a second time.
    assert_eq!(attempts.load(Ordering::SeqCst), 1);

    let retained_attempts = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&retained_attempts);
    {
        let mut retained = LaneWorktree::with_remover(
            PathBuf::from("/tmp"),
            PathBuf::from("/tmp/task-b"),
            "agent/story/task-b".into(),
            "0".repeat(40),
            Box::new(move |_, _, _, _| {
                seen.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
        );
        retained.retain("task-b", "execution_failed=planned lane failure");
        assert_eq!(retained.disposition(), LaneWorktreeDisposition::Retained);
    }
    assert_eq!(retained_attempts.load(Ordering::SeqCst), 0);
}
/// What a scripted lane fails with (`WO-302` Part A chaos).
enum ScriptedFailure {
    /// A genuine business verdict: says nothing about the engine.
    Hold,
    /// An internal engine fault: says nothing about the story.
    EngineFault,
}

/// A lane runner scripted to fail one fixed way.
struct FailingRunner {
    failure: ScriptedFailure,
}

impl ForgeRoleRunner for FailingRunner {
    fn run(&self, _node_id: &str, _task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
        match self.failure {
            // A genuine business Hold verdict: a role refusal that says nothing about the
            // engine, so `is_engine_fault_error` must not claim it.
            ScriptedFailure::Hold => Err(WorkflowError::generic(
                "human gate requires operator decision before this lane may proceed",
            )),
            // A sibling engine fault: the database went away mid-turn. Says nothing about
            // the story.
            ScriptedFailure::EngineFault => Err(WorkflowError::unavailable(
                "db: DatabaseUnavailable during workflow.step",
            )),
        }
    }
}

/// Fold one wave's lane executions exactly the way the drive loop does: first Hold wins, first
/// engine fault wins, then `wave_disposition` weighs them. Mirrors the loop so the chaos test
/// exercises the real precedence rather than restating it.
fn fold_wave(executions: Vec<LaneExecutionResult>) -> WaveDisposition {
    let mut hold_reason: Option<String> = None;
    let mut engine_fault: Option<WorkflowError> = None;
    for execution in executions {
        let channels = classify_lane_failure(&execution);
        if let Err(error) = execution.outcome {
            if channels.is_engine_fault {
                if engine_fault.is_none() {
                    engine_fault = Some(error);
                }
            } else if hold_reason.is_none() {
                hold_reason = Some(channels.reason);
            }
        }
    }
    wave_disposition(hold_reason, engine_fault)
}

fn hold_lane(task_id: &str) -> PreparedDurableLane {
    prepared_lane_with_runner(
        task_id,
        Box::new(FailingRunner {
            failure: ScriptedFailure::Hold,
        }),
    )
}

fn fault_lane(task_id: &str) -> PreparedDurableLane {
    prepared_lane_with_runner(
        task_id,
        Box::new(FailingRunner {
            failure: ScriptedFailure::EngineFault,
        }),
    )
}

#[test]
fn a_real_hold_survives_a_sibling_engine_fault() {
    // `WO-302` Part A chaos: sibling 1 returns a genuine Hold verdict while sibling 2 hits an
    // internal engine fault on the same wave.
    let results = execute_durable_batch(
        vec![hold_lane("task-hold"), fault_lane("task-fault")],
        &NoopJobs,
        "worker-1",
        None,
        "story-1",
        2,
    );
    assert_eq!(results.len(), 2);
    assert!(
        results.iter().all(|result| result.outcome.is_err()),
        "both lanes failed, one with a verdict and one with a fault"
    );
    match fold_wave(results) {
        WaveDisposition::Hold(reason) => {
            assert!(
                reason.contains("operator decision"),
                "the Hold verdict is the disposition: {reason}"
            );
            assert!(
                reason.contains("sibling engine fault"),
                "the masked fault stays observable on the Hold: {reason}"
            );
            assert!(
                reason.contains("DatabaseUnavailable"),
                "the fault text rides along, not just its existence: {reason}"
            );
        }
        WaveDisposition::EngineFault(error) => {
            panic!("the engine fault masked the genuine Hold: {error}")
        }
        WaveDisposition::Clear => panic!("a Hold and a fault cannot clear a wave"),
    }
}

#[test]
fn an_engine_fault_alone_propagates_without_inventing_a_hold() {
    // The other direction: with no business verdict anywhere, the fault must still propagate —
    // Hold priority must not become Hold invention.
    let results = execute_durable_batch(
        vec![fault_lane("task-fault")],
        &NoopJobs,
        "worker-1",
        None,
        "story-1",
        1,
    );
    match fold_wave(results) {
        WaveDisposition::EngineFault(error) => {
            assert!(is_engine_fault_error(&error), "{error}");
        }
        WaveDisposition::Hold(reason) => {
            panic!("a fault alone must never invent a Hold: {reason}")
        }
        WaveDisposition::Clear => panic!("a faulted wave is not clear"),
    }
}

#[test]
fn a_hold_alone_carries_no_fault_attachment() {
    let results = execute_durable_batch(
        vec![hold_lane("task-hold")],
        &NoopJobs,
        "worker-1",
        None,
        "story-1",
        1,
    );
    match fold_wave(results) {
        WaveDisposition::Hold(reason) => {
            assert!(reason.contains("operator decision"), "{reason}");
            assert!(
                !reason.contains("sibling engine fault"),
                "no fault happened, so none may be attached: {reason}"
            );
        }
        other => panic!(
            "a lone Hold must dispose as Hold, got {}",
            match other {
                WaveDisposition::EngineFault(error) => format!("EngineFault({error})"),
                WaveDisposition::Clear => "Clear".to_string(),
                WaveDisposition::Hold(_) => unreachable!(),
            }
        ),
    }
}

/// A `JobService` whose `complete` fails transiently while recording every call (`WO-302` Part B
/// chaos). `fail` panics when called: the settle path under test must never fail a job it just
/// completed, so a call there is the defect, not a fixture gap.
struct FlakyCompleteJobs {
    complete_calls: Mutex<usize>,
    fail_calls: Mutex<usize>,
    fail_complete: bool,
}

impl JobService for FlakyCompleteJobs {
    fn enqueue(&self, _request: &crate::engine::job::ForgeJobRequest) -> Result<String> {
        unreachable!()
    }
    fn claim(&self, _worker_id: &str, _limit: usize) -> Result<Vec<ForgeJobLease>> {
        unreachable!()
    }
    fn claim_one(&self, _job_id: &str, _worker_id: &str) -> Result<ForgeJobLease> {
        unreachable!()
    }
    fn heartbeat(&self, _job_id: &str, _worker_id: &str) -> Result<i64> {
        Ok(1)
    }
    fn inspect(&self, _job_id: &str) -> Result<crate::engine::job::ForgeJobState> {
        Err(WorkflowError::generic("unused"))
    }
    fn complete(&self, _job_id: &str, _worker_id: &str) -> Result<()> {
        *self.complete_calls.lock().unwrap() += 1;
        if self.fail_complete {
            return Err(WorkflowError::unavailable(
                "db: DatabaseUnavailable during jobs.complete",
            ));
        }
        Ok(())
    }
    fn fail(&self, _job_id: &str, _worker_id: &str, _error: &str, _permanent: bool) -> Result<()> {
        *self.fail_calls.lock().unwrap() += 1;
        panic!("settle_completed_lane must never fail a job it just completed");
    }
    fn cancel(&self, _job_id: &str, _actor: &str) -> Result<()> {
        Ok(())
    }
    fn requeue(&self, _job_id: &str, _actor: &str) -> Result<()> {
        Ok(())
    }
    fn recover_stale(&self, _batch: usize) -> Result<usize> {
        Ok(0)
    }
}

#[test]
fn a_settle_blip_holds_with_reconciliation_instead_of_orphaning_the_job() {
    // `WO-302` Part B chaos: the lane completed, then `jobs.complete` died transiently. The
    // settle must hold with the failure named (not propagate, not swallow), must never fail the
    // just-completed job, and must leave the row for reconciliation instead of orphaning it.
    let jobs = FlakyCompleteJobs {
        complete_calls: Mutex::new(0),
        fail_calls: Mutex::new(0),
        fail_complete: true,
    };
    let reason = settle_completed_lane(&jobs, "worker-1", "job-x", "task-x", false)
        .expect("a failed settle must hold, not vanish");
    assert!(reason.contains("task-x"), "{reason}");
    assert!(
        reason.contains("reconciliation"),
        "the Hold must name the reconciliation path, not just the failure: {reason}"
    );
    assert_eq!(
        *jobs.complete_calls.lock().unwrap(),
        1,
        "one settle attempt"
    );
    assert_eq!(
        *jobs.fail_calls.lock().unwrap(),
        0,
        "the completed job is never failed: failing it would orphan paid work"
    );
}

#[test]
fn a_clean_settle_holds_nothing() {
    let jobs = FlakyCompleteJobs {
        complete_calls: Mutex::new(0),
        fail_calls: Mutex::new(0),
        fail_complete: false,
    };
    assert!(
        settle_completed_lane(&jobs, "worker-1", "job-x", "task-x", false).is_none(),
        "a settled job holds nothing"
    );
    assert!(
        settle_completed_lane(&jobs, "worker-1", "job-x", "task-x", true).is_none(),
        "an already-completed settle holds nothing either"
    );
    assert_eq!(*jobs.complete_calls.lock().unwrap(), 2);
}
