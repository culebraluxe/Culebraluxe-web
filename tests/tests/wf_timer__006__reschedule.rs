//! WF.TIMER — reschedule (TST-WF-TIMER-006).
//!
//! Contract: rescheduling a timer **moves its deadline**, and that move is the whole point — the job becomes claimable
//! at the new time and unclaimable away from it. `WorkflowEngine::reschedule_timer` sets `due_at` to the caller's
//! instant, returns the job to `Pending`, and clears `locked_by`/`locked_until`
//! (`middle/workflow/src/engine/fire_timer_job.rs:284-322`, `:300-303`), recording `job.rescheduled` with the new
//! deadline (`:305-319`). Because the claim path filters on `due_at <= now`
//! (`middle/workflow/src/memory.rs:517-522`), the moved deadline is observable as claimability: that is how this test
//! proves the reschedule happened, rather than trusting the field it just wrote.
//!
//! Two refusals complete the contract, and they are the ones a permissive implementation gets wrong:
//!
//! - A **settled** job cannot be rescheduled. `is_settled` covers `Cancelled | Completed | Failed`
//!   (`middle/workflow/src/types.rs:65-67`), and rescheduling one is `JOB_SETTLED` (`fire_timer_job.rs:291-299`).
//!   Without the guard, reschedule would be a way to resurrect a cancelled or already-fired timer — the exact
//!   "revive settled work" bug the durable status exists to prevent.
//! - A settled job **stays** settled after the refusal. A refusal that mutated the row on its way out would be worse
//!   than no refusal at all.
//!
//! Level: L4 Adversarial, harness `DeterministicClockHarness`. The deadline is moved to exact instants on a frozen
//! clock, so "claimable at the new time, unclaimable one millisecond before it" is a fixture rather than a wait.
//! Deterministic and isolated: no database, no network, no filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_timer__006__reschedule

use std::collections::BTreeMap;

use test_harness::EngineHarness;
use workflow::{
    DefinitionStatus, Job, JobStatus, NodeDefinition, ProcessDefinition, ProcessEvent,
    ProcessGraph, ProcessOutcome, StartProcessParams, TimerSpec, Token, TransitionDefinition,
    Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "DeterministicClockHarness/L4 Adversarial";

/// The definition key the engine registers.
const DEFINITION_KEY: &str = "TST-WF-TIMER-006";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const ACTOR: &str = "scheduler";

/// The graph: `start -> wait (timer) -> done (end)`.
const START_NODE: &str = "start";
const WAIT_NODE: &str = "wait";
const DONE_NODE: &str = "done";
const BEGIN: &str = "begin";
const RESUME: &str = "resume";

/// The frozen instant the graph runs at.
const NOW: i64 = 1_700_000_000_000;

/// A deadline far outside any claim window, so "unclaimable" is unambiguous.
const FAR_FUTURE: i64 = 1_900_000_000_000;

/// The worker that claims the rescheduled timer.
const WORKER: &str = "worker-a";

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// A timer node that comes due at `due_at` rather than at the engine's clock.
fn timer_definition(due_at: &str) -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition(BEGIN, WAIT_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        WAIT_NODE.to_string(),
        NodeDefinition {
            id: WAIT_NODE.to_string(),
            node_type: "timer".to_string(),
            name: Some("Wait".to_string()),
            transitions: Some(vec![transition(RESUME, DONE_NODE)]),
            timer: Some(TimerSpec {
                due_at: Some(due_at.to_string()),
                due_at_variable: None,
                transition: Some(RESUME.to_string()),
            }),
            ..Default::default()
        },
    );
    nodes.insert(
        DONE_NODE.to_string(),
        NodeDefinition {
            id: DONE_NODE.to_string(),
            node_type: "end".to_string(),
            outcome: Some(ProcessOutcome::Completed),
            ..Default::default()
        },
    );
    ProcessDefinition {
        id: format!("{DEFINITION_KEY}-def"),
        tenant_id: None,
        key: DEFINITION_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: DEFINITION_KEY.to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: START_NODE.to_string(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

fn start_params() -> StartProcessParams {
    StartProcessParams {
        definition_key: DEFINITION_KEY.to_string(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: Value::object(),
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    }
}

fn read_job(harness: &EngineHarness, id: &str) -> Job {
    harness
        .store()
        .with_tx(|tx| tx.get_job(id))
        .expect("the job is readable")
}

fn events_of_type(
    harness: &EngineHarness,
    instance_id: &str,
    event_type: &str,
) -> Vec<ProcessEvent> {
    harness
        .store()
        .with_tx(|tx| tx.history(instance_id, 128))
        .expect("the instance history reads")
        .into_iter()
        .filter(|event| event.event_type == event_type)
        .collect()
}

/// Start the timer graph with a node due at `due_at`, returning the scheduled job, its token and its instance.
fn scheduled_timer(harness: &EngineHarness, due_at: &str) -> (Job, String, String) {
    harness
        .engine()
        .seed_definition(timer_definition(due_at))
        .expect("the timer definition registers with the engine");
    let started = harness
        .engine()
        .start_process(start_params())
        .expect("the process starts and parks on the timer node");
    let token_id = started.root_token_id.clone();
    let instance_id = harness
        .store()
        .with_tx(|tx| tx.get_token(&token_id))
        .expect("the token reads")
        .process_instance_id;
    let jobs: Vec<Job> = harness
        .store()
        .with_tx(|tx| tx.open_jobs_for_token(&token_id))
        .expect("the token's jobs read");
    assert_eq!(
        jobs.len(),
        1,
        "{HARNESS}: the timer node schedules exactly one job"
    );
    (
        jobs.into_iter().next().expect("the scheduled job"),
        token_id,
        instance_id,
    )
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TIMER-006); the file and the assay use it.
fn wf_timer_006__reschedule() {
    // ── SCENARIO 1: the scheduled deadline holds the job out of the executor ──────────────────────────────────
    // The node is scheduled far in the future, so the job is genuinely not due. Every claimability assertion below is
    // therefore measuring the deadline, and only the deadline.
    let harness = EngineHarness::at_unix_millis(NOW);
    let (job, _token_id, instance_id) = scheduled_timer(&harness, &FAR_FUTURE.to_string());
    let job_id = job.id.clone();
    assert_eq!(
        job.due_at, FAR_FUTURE,
        "{HARNESS}: the scheduled deadline is the node's own due_at"
    );
    assert_eq!(
        job.status,
        JobStatus::Pending,
        "{HARNESS}: a scheduled timer is pending, not claimed"
    );
    assert!(
        harness
            .engine()
            .claim_jobs_by_type(WORKER, "timer", 10)
            .expect("the early claim step commits")
            .is_empty(),
        "{HARNESS}: a timer due in the future must not be claimable"
    );
    assert!(
        harness
            .engine()
            .list_overdue_jobs(10)
            .expect("the early overdue listing commits")
            .is_empty(),
        "{HARNESS}: a timer due in the future is not overdue"
    );

    // ── SCENARIO 2 (positive): rescheduling onto the current instant makes it claimable ────────────────────────
    // This is the proof the deadline moved. Unclaimable before the call, claimable after it, with nothing else
    // changed: a reschedule that only rewrote `due_at` in the row would show up here as a claim either way.
    harness
        .engine()
        .reschedule_timer(&job_id, NOW, ACTOR)
        .expect("the reschedule onto now commits");
    let moved = read_job(&harness, &job_id);
    assert_eq!(
        moved.due_at, NOW,
        "{HARNESS}: reschedule writes the caller's deadline"
    );
    assert_eq!(
        moved.status,
        JobStatus::Pending,
        "{HARNESS}: a rescheduled timer is claimable again"
    );
    assert_eq!(
        moved.locked_by, None,
        "{HARNESS}: a reschedule leaves no owner"
    );
    assert_eq!(
        moved.locked_until, None,
        "{HARNESS}: a reschedule leaves no lease window"
    );

    let reschedules = events_of_type(&harness, &instance_id, "job.rescheduled");
    assert_eq!(
        reschedules.len(),
        1,
        "{HARNESS}: reschedule records job.rescheduled exactly once"
    );
    assert_eq!(
        reschedules[0].data.get("dueAt").and_then(Value::as_i64),
        Some(NOW),
        "{HARNESS}: the recorded deadline is the new one, so the history explains the move"
    );
    assert_eq!(
        reschedules[0].actor, ACTOR,
        "{HARNESS}: the reschedule is attributed to the actor that asked for it"
    );

    let claimed = harness
        .engine()
        .claim_jobs_by_type(WORKER, "timer", 10)
        .expect("the post-reschedule claim step commits");
    assert_eq!(
        claimed.len(),
        1,
        "{HARNESS}: the moved deadline put the timer into the executor's hands at the new instant"
    );

    // ── SCENARIO 3 (positive): rescheduling away removes it again, and releases the lease ───────────────────────
    // Reschedule has to work in both directions, and it has to work on a job that is currently leased — otherwise a
    // held timer could never be re-planned and would fire at the old time anyway.
    harness
        .engine()
        .reschedule_timer(&job_id, FAR_FUTURE, ACTOR)
        .expect("the reschedule away commits");
    let pushed = read_job(&harness, &job_id);
    assert_eq!(
        pushed.due_at, FAR_FUTURE,
        "{HARNESS}: reschedule moves the deadline in both directions"
    );
    assert_eq!(
        pushed.status,
        JobStatus::Pending,
        "{HARNESS}: rescheduling a leased timer returns it to the claimable state"
    );
    assert_eq!(
        pushed.locked_by, None,
        "{HARNESS}: reschedule releases the lease it took over"
    );
    assert_eq!(
        pushed.locked_until, None,
        "{HARNESS}: reschedule releases the lease window it took over"
    );
    assert!(
        harness
            .engine()
            .claim_jobs_by_type(WORKER, "timer", 10)
            .expect("the post-push claim step commits")
            .is_empty(),
        "{HARNESS}: a timer pushed into the future is not claimable at the old instant any more"
    );
    assert_eq!(
        events_of_type(&harness, &instance_id, "job.rescheduled").len(),
        2,
        "{HARNESS}: each reschedule records its own event"
    );

    // ── SCENARIO 4 (negative, load-bearing): a CANCELLED timer cannot be rescheduled ───────────────────────────
    // Cancellation is one of the settled states, and reviving it is the bug this guard exists to prevent: a
    // cancelled timer that a reschedule could resurrect would fire for a process that had already given it up.
    harness
        .engine()
        .cancel_timer(&job_id, ACTOR)
        .expect("the cancellation commits");
    assert_eq!(
        read_job(&harness, &job_id).status,
        JobStatus::Cancelled,
        "{HARNESS}: the precondition of this scenario — the timer is cancelled"
    );

    let revive = harness
        .engine()
        .reschedule_timer(&job_id, NOW, ACTOR)
        .expect_err("a cancelled timer must not be rescheduled");
    assert_eq!(
        revive.code(),
        "JOB_SETTLED",
        "{HARNESS}: rescheduling a settled timer is refused as JOB_SETTLED"
    );
    let still_cancelled = read_job(&harness, &job_id);
    assert_eq!(
        still_cancelled.status,
        JobStatus::Cancelled,
        "{HARNESS}: the refused reschedule must not change the settled status"
    );
    assert_eq!(
        still_cancelled.due_at, FAR_FUTURE,
        "{HARNESS}: the refused reschedule must not move the deadline of a settled job"
    );
    assert!(
        harness
            .engine()
            .claim_jobs_by_type(WORKER, "timer", 10)
            .expect("the post-revival claim step commits")
            .is_empty(),
        "{HARNESS}: the refused reschedule left nothing claimable — a cancelled timer stayed dead"
    );
    assert_eq!(
        events_of_type(&harness, &instance_id, "job.rescheduled").len(),
        2,
        "{HARNESS}: a refused reschedule records no event"
    );

    // ── SCENARIO 5 (negative): a COMPLETED timer cannot be rescheduled either ──────────────────────────────────
    // The guard is `is_settled`, not "is cancelled". A timer that already fired resumed its token and completed the
    // process; rescheduling it would schedule a second resume for work already done.
    let harness = EngineHarness::at_unix_millis(NOW);
    let (job, token_id, _instance_id) = scheduled_timer(&harness, &NOW.to_string());
    let job_id = job.id.clone();
    let claimed = harness
        .engine()
        .claim_jobs_by_type(WORKER, "timer", 10)
        .expect("the claim step commits");
    assert_eq!(claimed.len(), 1, "{HARNESS}: a timer due now is claimable");
    harness
        .engine()
        .fire_timer_job(workflow::FireTimerParams {
            job_id: job_id.clone(),
            worker_id: WORKER.to_string(),
            variables: Value::object(),
        })
        .expect("the timer fires and resumes the token");
    assert_eq!(
        read_job(&harness, &job_id).status,
        JobStatus::Completed,
        "{HARNESS}: the precondition of this scenario — the timer already fired"
    );
    assert_eq!(
        harness
            .store()
            .with_tx(|tx| tx.get_token(&token_id))
            .expect("the resumed token reads")
            .status,
        workflow::TokenStatus::Completed,
        "{HARNESS}: firing really did resume the process, so this is settled work and not a pending job"
    );

    let replay = harness
        .engine()
        .reschedule_timer(&job_id, FAR_FUTURE, ACTOR)
        .expect_err("a completed timer must not be rescheduled");
    assert_eq!(
        replay.code(),
        "JOB_SETTLED",
        "{HARNESS}: the settled guard covers a completed timer as well as a cancelled one"
    );
    assert_eq!(
        read_job(&harness, &job_id).status,
        JobStatus::Completed,
        "{HARNESS}: the refused reschedule left the completed status alone"
    );
    assert_eq!(
        harness
            .engine()
            .list_overdue_jobs(10)
            .expect("the post-replay overdue listing commits")
            .is_empty(),
        true,
        "{HARNESS}: a completed timer is neither claimable nor overdue"
    );
}
