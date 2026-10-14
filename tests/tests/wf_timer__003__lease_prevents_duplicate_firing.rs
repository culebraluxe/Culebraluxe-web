//! WF.TIMER — lease prevents duplicate firing (TST-WF-TIMER-003).
//!
//! Contract: a timer job's **lease** is what stops it firing twice. Claiming is not a read — it is a durable
//! transition: `claim_due_jobs_by_type` only offers jobs that are still `Pending`, and it writes `Locked`,
//! `locked_by`, `locked_until = now + JOB_LEASE_MS` and `attempts += 1`
//! (`middle/workflow/src/memory.rs:505-539`; the engine supplies the lease from `JOB_LEASE_MS`,
//! `middle/workflow/src/engine/engine_options.rs:545-558`, `middle/workflow/src/types.rs:4`). So the lease, not the
//! due time, is what makes a job invisible to the next worker: a due job that is already leased is not merely
//! *claimed twice*, it is not offered at all.
//!
//! The second half of the contract is the one a naive implementation gets wrong. Being unable to *claim* a job is not
//! the same as being unable to *fire* it — a worker that reaches the fire path with an id it never won can still fire
//! it. `fire_timer_job` therefore refuses twice over: a job that is not `Locked` is `TIMER_NOT_LOCKED`, and a job
//! locked by somebody else is `TIMER_LOCK_OWNER` (`middle/workflow/src/engine/fire_timer_job.rs:21-35`). And once the
//! owner has fired it, the durable state itself refuses a replay with `TIMER_ALREADY_FIRED` (`:15-20`).
//!
//! Everything here drives the production `WorkflowEngine<MemoryStore>`: the timer job is created by production graph
//! execution (`handle_timer`, `middle/workflow/src/engine/handle_join.rs:121-180`), claimed by production
//! `claim_jobs_by_type`, and fired by production `fire_timer_job`. Nothing re-declares a claim or a fire.
//!
//! Level: L4 Adversarial, harness `DeterministicClockHarness`. The contention is modelled deterministically: the
//! clock is frozen, so lease windows and due times are exact fixtures rather than coincidences, and the second
//! worker is driven into every failure mode a racy one could reach. Deterministic and isolated: no database, no
//! network, no filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_timer__003__lease_prevents_duplicate_firing

use std::collections::BTreeMap;

use test_harness::EngineHarness;
use workflow::{
    DefinitionStatus, FireTimerParams, Job, JobStatus, NodeDefinition, ProcessDefinition,
    ProcessEvent, ProcessGraph, ProcessOutcome, StartProcessParams, TimerSpec, Token, TokenStatus,
    TransitionDefinition, Value, JOB_LEASE_MS,
};

/// The canonical harness label for this level.
const HARNESS: &str = "DeterministicClockHarness/L4 Adversarial";

/// The definition key the engine registers.
const DEFINITION_KEY: &str = "TST-WF-TIMER-003";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";

/// The graph: `start -> wait (timer) -> done (end)`. The timer node's transition resumes into the end node.
const START_NODE: &str = "start";
const WAIT_NODE: &str = "wait";
const DONE_NODE: &str = "done";
const BEGIN: &str = "begin";
const RESUME: &str = "resume";

/// The frozen instant every lease and due time is measured from.
const NOW: i64 = 1_700_000_000_000;

/// The two workers contending for the one timer.
const WORKER_A: &str = "worker-a";
const WORKER_B: &str = "worker-b";

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// `start -> wait (timer) -> done (end)`.
fn timer_definition() -> ProcessDefinition {
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
                due_at: None,
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

fn read_token(harness: &EngineHarness, id: &str) -> Token {
    harness
        .store()
        .with_tx(|tx| tx.get_token(id))
        .expect("the token is readable")
}

/// Start the timer graph and return the parked token id plus the one timer job the graph scheduled.
fn started_timer(harness: &EngineHarness) -> (String, Job) {
    harness
        .engine()
        .seed_definition(timer_definition())
        .expect("the timer definition registers with the engine");
    let started = harness
        .engine()
        .start_process(start_params())
        .expect("the process starts and parks on the timer node");
    let token_id = started.root_token_id.clone();
    let jobs: Vec<Job> = harness
        .store()
        .with_tx(|tx| tx.open_jobs_for_token(&token_id))
        .expect("the token's jobs read");
    assert_eq!(
        jobs.len(),
        1,
        "{HARNESS}: the timer node schedules exactly one job"
    );
    assert_eq!(
        jobs[0].job_type, "timer",
        "{HARNESS}: the scheduled job is a timer job, which is the only type this executor claims"
    );
    (token_id, jobs[0].clone())
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

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TIMER-003); the file and the assay use it.
fn wf_timer_003__lease_prevents_duplicate_firing() {
    // ── SCENARIO 1: the claim is a durable lease, not a read ─────────────────────────────────────────────────────
    // Frozen clock, so the lease window is an exact fixture: `now + JOB_LEASE_MS`, never "about five minutes".
    let harness = EngineHarness::at_unix_millis(NOW);
    let (token_id, job) = started_timer(&harness);
    let instance_id = harness
        .store()
        .with_tx(|tx| tx.get_token(&token_id))
        .expect("the token reads")
        .process_instance_id;
    assert_eq!(
        read_token(&harness, &token_id).node_id,
        WAIT_NODE,
        "{HARNESS}: the token parks on the timer node while the job waits"
    );

    let claimed = harness
        .engine()
        .claim_jobs_by_type(WORKER_A, "timer", 10)
        .expect("worker A's claim step commits");
    assert_eq!(
        claimed.len(),
        1,
        "{HARNESS}: the due timer is claimable exactly once by the first worker"
    );
    let leased = read_job(&harness, &job.id);
    assert_eq!(
        leased.status,
        JobStatus::Locked,
        "{HARNESS}: a claim durably locks the job — that is what makes the lease, not the read, the contract"
    );
    assert_eq!(
        leased.locked_by.as_deref(),
        Some(WORKER_A),
        "{HARNESS}: the lease names its owner"
    );
    assert_eq!(
        leased.locked_until,
        Some(NOW + JOB_LEASE_MS),
        "{HARNESS}: the lease runs from the claim instant to now + JOB_LEASE_MS"
    );
    assert_eq!(
        leased.attempts, 1,
        "{HARNESS}: claiming counts one attempt, so a replay would show up here"
    );
    assert_eq!(
        leased.due_at, NOW,
        "{HARNESS}: the timer was due at the frozen instant, so nothing but the lease keeps B out"
    );

    // ── SCENARIO 2 (negative): a live lease keeps the second worker out of the batch ────────────────────────────
    // The job is due right now. The only reason B receives nothing is that A holds the lease. A claim that filtered
    // on due time alone would hand B a duplicate of work already in flight.
    let second = harness
        .engine()
        .claim_jobs_by_type(WORKER_B, "timer", 10)
        .expect("worker B's claim step commits");
    assert!(
        second.is_empty(),
        "{HARNESS}: a job already leased is not offered to a second worker, however due it is: {:?}",
        second.iter().map(|j| j.id.as_str()).collect::<Vec<_>>()
    );
    // The exact claim refuses for the same reason.
    assert!(
        harness
            .engine()
            .claim_job(&job.id, WORKER_B)
            .expect("worker B's exact claim step commits")
            .is_none(),
        "{HARNESS}: the exact claim must also refuse a job another worker holds"
    );
    assert_eq!(
        read_job(&harness, &job.id).locked_by.as_deref(),
        Some(WORKER_A),
        "{HARNESS}: the refusals left the lease with its original owner"
    );
    assert_eq!(
        read_job(&harness, &job.id).attempts,
        1,
        "{HARNESS}: a refused claim costs the job no attempt"
    );

    // ── SCENARIO 3 (negative, load-bearing): a worker without the lease cannot fire ──────────────────────────────
    // Being unable to *claim* is not the same as being unable to *fire*. B has a job id — it watched A take it — and
    // B reaches the fire path directly. Only the lease stops it: `fire_timer_job` checks the lock owner
    // (`fire_timer_job.rs:30-35`), and this is the assertion that fails if that check is ever dropped.
    let theft = harness
        .engine()
        .fire_timer_job(FireTimerParams {
            job_id: job.id.clone(),
            worker_id: WORKER_B.to_string(),
            variables: Value::object(),
        })
        .expect_err("a worker that does not hold the lease must not fire the timer");
    assert_eq!(
        theft.code(),
        "TIMER_LOCK_OWNER",
        "{HARNESS}: firing a leased timer without the lease is refused as a lock-owner conflict"
    );

    // Nothing the attempted duplicate could have touched moved.
    let after_theft = read_job(&harness, &job.id);
    assert_eq!(
        after_theft.status,
        JobStatus::Locked,
        "{HARNESS}: a refused fire leaves the job leased, not completed"
    );
    assert_eq!(
        read_token(&harness, &token_id).node_id,
        WAIT_NODE,
        "{HARNESS}: a refused duplicate fire did not advance the token"
    );
    assert!(
        events_of_type(&harness, &instance_id, "timer.fired").is_empty(),
        "{HARNESS}: no timer.fired may be recorded for a fire that was refused"
    );

    // ── SCENARIO 4: the owner fires once, and the token advances exactly once ───────────────────────────────────
    harness
        .engine()
        .fire_timer_job(FireTimerParams {
            job_id: job.id.clone(),
            worker_id: WORKER_A.to_string(),
            variables: Value::object(),
        })
        .expect("the lease owner fires the timer");

    let fired = read_job(&harness, &job.id);
    assert_eq!(
        fired.status,
        JobStatus::Completed,
        "{HARNESS}: firing the timer settles the job"
    );
    assert_eq!(
        fired.locked_by, None,
        "{HARNESS}: firing releases the lease"
    );
    assert_eq!(
        fired.locked_until, None,
        "{HARNESS}: firing clears the lease window"
    );
    assert_eq!(
        fired.completed_at,
        Some(NOW),
        "{HARNESS}: firing stamps the completion from the engine's clock, not the system clock"
    );
    assert_eq!(
        read_token(&harness, &token_id).status,
        TokenStatus::Completed,
        "{HARNESS}: the resumed token ran into the end node and completed"
    );
    assert_eq!(
        read_token(&harness, &token_id).node_id,
        DONE_NODE,
        "{HARNESS}: the timer resumed the token onto the end node"
    );

    // ── SCENARIO 5 (negative): the duplicate firing is refused at the durable state ──────────────────────────────
    // The lease is gone now, so this is no longer about ownership — it is the durable refusal that survives the
    // lease's release. A replay after success is `TIMER_ALREADY_FIRED`, not a second resume.
    let replay_by_owner = harness
        .engine()
        .fire_timer_job(FireTimerParams {
            job_id: job.id.clone(),
            worker_id: WORKER_A.to_string(),
            variables: Value::object(),
        })
        .expect_err("a timer that already fired must not fire again");
    assert_eq!(
        replay_by_owner.code(),
        "TIMER_ALREADY_FIRED",
        "{HARNESS}: replaying the owner's own fire is refused by the completed status"
    );

    let replay_by_other = harness
        .engine()
        .fire_timer_job(FireTimerParams {
            job_id: job.id.clone(),
            worker_id: WORKER_B.to_string(),
            variables: Value::object(),
        })
        .expect_err("a timer that already fired must not fire for another worker either");
    assert_eq!(
        replay_by_other.code(),
        "TIMER_ALREADY_FIRED",
        "{HARNESS}: a late worker is refused exactly as the owner was"
    );

    // ── SCENARIO 6: the proof that no duplicate happened — one fire, one resume ────────────────────────────────
    // Counted in the durable history rather than inferred from statuses, so a duplicate that fired and were later
    // corrected would still be visible here.
    assert_eq!(
        events_of_type(&harness, &instance_id, "timer.fired").len(),
        1,
        "{HARNESS}: exactly one timer.fired is recorded for exactly one firing"
    );
    let moves: Vec<ProcessEvent> = events_of_type(&harness, &instance_id, "token.moved");
    assert_eq!(
        moves.len(),
        2,
        "{HARNESS}: the token moved twice in total — onto the timer, then the resume — never a third time"
    );
    assert_eq!(
        moves
            .iter()
            .filter(|event| event.data.get("transition").and_then(Value::as_str) == Some(RESUME))
            .count(),
        1,
        "{HARNESS}: the resume transition was taken exactly once"
    );

    // And nothing is left claimable: a settled job is invisible to every executor.
    assert!(
        harness
            .engine()
            .claim_jobs_by_type(WORKER_A, "timer", 10)
            .expect("the post-fire claim step commits")
            .is_empty(),
        "{HARNESS}: a fired timer must not be claimable again"
    );
    assert!(
        harness
            .engine()
            .claim_jobs_by_type(WORKER_B, "timer", 10)
            .expect("the post-fire claim step commits")
            .is_empty(),
        "{HARNESS}: nor by a different worker"
    );
}
