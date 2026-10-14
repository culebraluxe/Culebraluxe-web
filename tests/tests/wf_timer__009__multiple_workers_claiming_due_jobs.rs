//! WF.TIMER — multiple workers claiming due jobs (TST-WF-TIMER-009).
//!
//! Contract: when several workers reach the same set of due timers at once, the work is distributed **exactly once**
//! between them. `claim_due_jobs_by_type` selects the due `Pending` jobs whose `attempts` are below `max_attempts`,
//! orders them by `due_at`, takes at most `limit`, and durably marks each one `Locked` with an owner, a lease window
//! and `attempts += 1` (`middle/workflow/src/memory.rs:505-539`). Because the durable write sets the status before the
//! next worker looks, two workers cannot both come away holding the same timer — the loser is offered fewer jobs, not
//! a second copy of one.
//!
//! This is a concurrency contract, so it is tested with a **real race**, not a call graph drawn to look parallel: each
//! round spawns N OS threads against one shared engine, releases them together from a `Barrier` so none can finish
//! before the others have started, and then asserts the invariant on committed state afterwards. The rounds are
//! repeated so an interleaving that hands out a duplicate has many chances to appear rather than one.
//!
//! The invariant is convergence, not order — which worker wins is not defined and is not asserted, because nothing in
//! production promises it. What is asserted is that the claims partition the work: no id claimed twice by anyone,
//! every claimed job durably owned by exactly one worker at `attempts == 1`, and a later pass finds nothing to hand
//! out. `MemoryStore` serializes each step behind one mutex (`middle/workflow/src/memory.rs:26-28`), which is the
//! same per-transaction atomicity Neon gives the claim's `FOR UPDATE SKIP LOCKED` predicate
//! (`middle/workflow/src/neon/new_id.rs:581`); the Neon predicate itself is covered by the workflow crate's own store
//! tests, and the contract under test here is the engine's distribution of work, not the SQL.
//!
//! Level: L4 Adversarial, harness `DeterministicClockHarness`. The clock is frozen so the due set is an exact
//! fixture; the contention is real threads, and the race is repeated rather than hoped for.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_timer__009__multiple_workers_claiming_due_jobs

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Barrier;

use test_harness::EngineHarness;
use workflow::{
    DefinitionStatus, Job, JobStatus, NodeDefinition, ProcessDefinition, ProcessGraph,
    ProcessOutcome, StartProcessParams, TimerSpec, TransitionDefinition, Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "DeterministicClockHarness/L4 Adversarial";

/// The definition key the engine registers.
const DEFINITION_KEY: &str = "TST-WF-TIMER-009";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";

/// The graph: `start -> wait (timer) -> done (end)`.
const START_NODE: &str = "start";
const WAIT_NODE: &str = "wait";
const DONE_NODE: &str = "done";
const BEGIN: &str = "begin";
const RESUME: &str = "resume";

/// The frozen instant every timer is due at.
const NOW: i64 = 1_700_000_000_000;

/// The competing workers. Four workers against three jobs means contention is guaranteed, not incidental.
const WORKERS: [&str; 4] = ["worker-a", "worker-b", "worker-c", "worker-d"];

/// Due timers per round.
const JOBS: usize = 3;

/// How many times the whole race is repeated, so a duplicate-claim interleaving has many chances to surface.
const ROUNDS: usize = 100;

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

/// A fresh engine holding `count` due timers, one per started process, and the ids of those timers.
fn engine_with_due_timers(count: usize) -> (EngineHarness, Vec<String>) {
    let harness = EngineHarness::at_unix_millis(NOW);
    harness
        .engine()
        .seed_definition(timer_definition())
        .expect("the timer definition registers with the engine");
    let mut ids = Vec::with_capacity(count);
    for _ in 0..count {
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
            "{HARNESS}: each started process parks on exactly one timer"
        );
        ids.push(jobs.into_iter().next().expect("the scheduled job").id);
    }
    (harness, ids)
}

/// One claim pass: every worker is released together from the barrier, then claims through the production engine.
///
/// The barrier is what makes this a race rather than a sequence — none of the workers can proceed until all of them
/// have started, so each one is inside the claim window while the others are too.
fn claim_pass(harness: &EngineHarness, limit: usize) -> Vec<(String, Vec<Job>)> {
    let barrier = Barrier::new(WORKERS.len());
    std::thread::scope(|scope| {
        let handles: Vec<_> = WORKERS
            .iter()
            .map(|worker| {
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    let claimed = harness
                        .engine()
                        .claim_jobs_by_type(worker, "timer", limit)
                        .expect("the claim step commits");
                    ((*worker).to_string(), claimed)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("a claiming worker did not panic"))
            .collect()
    })
}

/// The ids claimed across a pass, and the ones claimed more than once — which must always be empty.
fn partition(results: &[(String, Vec<Job>)]) -> (BTreeSet<String>, Vec<String>) {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut duplicates: Vec<String> = Vec::new();
    for (worker, jobs) in results {
        for job in jobs {
            if !seen.insert(job.id.clone()) {
                duplicates.push(format!("{} handed to {worker} twice", job.id));
            }
        }
    }
    (seen, duplicates)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-TIMER-009); the file and the assay use it.
fn wf_timer_009__multiple_workers_claiming_due_jobs() {
    // ── SCENARIO 1: the race — every due timer is distributed, and none of them twice ───────────────────────────
    // Four workers released together against three due timers. Fewer jobs than workers is deliberate: it guarantees
    // that at least two workers come away empty-handed, so "one of them got everything" cannot pass as a
    // distribution.
    for round in 0..ROUNDS {
        let (harness, ids) = engine_with_due_timers(JOBS);
        let expected: BTreeSet<String> = ids.iter().cloned().collect();

        let results = claim_pass(&harness, JOBS);
        let (claimed, duplicates) = partition(&results);
        assert!(
            duplicates.is_empty(),
            "{HARNESS} round {round}: a due timer was claimed by two workers — {duplicates:?}"
        );
        assert_eq!(
            claimed, expected,
            "{HARNESS} round {round}: the workers must take the due set between them, losing none of it"
        );

        // At least one worker came away empty, so the partition above really was a partition.
        let empty_workers = results.iter().filter(|(_, jobs)| jobs.is_empty()).count();
        assert!(
            empty_workers > 0,
            "{HARNESS} round {round}: with fewer timers than workers at least one worker must get nothing"
        );

        // The durable state agrees with what each worker was told, which is what makes this a committed partition
        // rather than two views of one row.
        for (worker, jobs) in &results {
            for job in jobs {
                let row = read_job(&harness, &job.id);
                assert_eq!(
                    row.status,
                    JobStatus::Locked,
                    "{HARNESS} round {round}: a claimed timer is durably locked"
                );
                assert_eq!(
                    row.locked_by.as_deref(),
                    Some(worker.as_str()),
                    "{HARNESS} round {round}: the durable owner is the worker that was handed the job"
                );
                assert_eq!(
                    row.attempts, 1,
                    "{HARNESS} round {round}: winning a claim costs exactly one attempt — two workers claiming would show two"
                );
            }
        }
    }

    // ── SCENARIO 2 (negative, load-bearing): claimed work is never offered again ───────────────────────────────
    // A duplicate is not only possible inside one pass; it is possible across passes if the first pass failed to
    // record ownership. Re-running the whole race against the same engine is what catches that.
    let (harness, ids) = engine_with_due_timers(JOBS);
    let expected: BTreeSet<String> = ids.iter().cloned().collect();

    let first = claim_pass(&harness, JOBS);
    let (first_claimed, first_duplicates) = partition(&first);
    assert!(
        first_duplicates.is_empty(),
        "{HARNESS}: one timer, one worker"
    );
    assert_eq!(
        first_claimed, expected,
        "{HARNESS}: the first pass drains the due set"
    );

    let second = claim_pass(&harness, JOBS);
    let (second_claimed, second_duplicates) = partition(&second);
    assert!(
        second_duplicates.is_empty(),
        "{HARNESS}: a second pass must not duplicate the first pass's work"
    );
    assert!(
        second_claimed.is_empty(),
        "{HARNESS}: a later pass must find nothing to hand out — leased work is invisible to the next worker: {:?}",
        second_claimed
    );
    // Every job still records exactly one attempt across both passes.
    for id in &expected {
        assert_eq!(
            read_job(&harness, id).attempts,
            1,
            "{HARNESS}: attempts must not accumulate across passes for work that was claimed once"
        );
    }

    // ── SCENARIO 3 (negative): the exact claim is exclusive too, under the same race ───────────────────────────
    // The batch claim is the one a scheduler uses, but `claim_job` is the same boundary narrowed to one row. If the
    // exact claim were not exclusive, a worker addressing a known job id would double-book it even though the batch
    // claim is safe.
    for round in 0..ROUNDS {
        let (harness, ids) = engine_with_due_timers(1);
        let target = ids[0].clone();
        let barrier = Barrier::new(WORKERS.len());
        let engine = &harness;
        let job_id = &target;
        let results: Vec<(String, Option<Job>)> = std::thread::scope(|scope| {
            let handles: Vec<_> = WORKERS
                .iter()
                .map(|worker| {
                    let barrier = &barrier;
                    scope.spawn(move || {
                        barrier.wait();
                        (
                            (*worker).to_string(),
                            engine
                                .engine()
                                .claim_job(job_id, worker)
                                .expect("the exact claim step commits"),
                        )
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("a claiming worker did not panic"))
                .collect()
        });

        let winners: Vec<&(String, Option<Job>)> =
            results.iter().filter(|(_, job)| job.is_some()).collect();
        assert_eq!(
            winners.len(),
            1,
            "{HARNESS} round {round}: exactly one worker may claim one exact job — got {}",
            winners.len()
        );
        assert_eq!(
            read_job(&harness, &target).attempts,
            1,
            "{HARNESS} round {round}: the loser's attempt must not be counted against the job"
        );
        assert_eq!(
            read_job(&harness, &target).locked_by.as_deref(),
            Some(winners[0].0.as_str()),
            "{HARNESS} round {round}: the durable owner is the single winner"
        );
    }

    // ── SCENARIO 4 (negative): a bounded claim never exceeds its limit, and draining never duplicates ───────────
    // `limit` is part of the production contract: a worker asked for two jobs must not walk away with six, and a
    // round that respects the limit must still converge — the remainder is claimable later, by somebody, once each.
    // Both properties are interleaving-independent, which is what makes them safe to assert over a real race.
    let bounded_limit = 2;
    let (harness, ids) = engine_with_due_timers(6);
    let expected: BTreeSet<String> = ids.iter().cloned().collect();

    let mut distributed: BTreeSet<String> = BTreeSet::new();
    for pass in 0..WORKERS.len() + 1 {
        let results = claim_pass(&harness, bounded_limit);
        let (claimed, duplicates) = partition(&results);
        assert!(
            duplicates.is_empty(),
            "{HARNESS} pass {pass}: a bounded claim still must not double-book a timer — {duplicates:?}"
        );
        for (worker, jobs) in &results {
            assert!(
                jobs.len() <= bounded_limit,
                "{HARNESS} pass {pass}: {worker} was asked for {bounded_limit} jobs and took {}",
                jobs.len()
            );
        }
        for id in &claimed {
            assert!(
                distributed.insert(id.clone()),
                "{HARNESS} pass {pass}: {id} was handed out again after an earlier pass took it"
            );
        }
        if claimed.is_empty() {
            break;
        }
    }
    assert_eq!(
        distributed, expected,
        "{HARNESS}: draining bounded passes hands out every due timer exactly once"
    );
    for id in &expected {
        assert_eq!(
            read_job(&harness, id).attempts,
            1,
            "{HARNESS}: {id} was claimed once across every pass, so it carries one attempt"
        );
    }
}
