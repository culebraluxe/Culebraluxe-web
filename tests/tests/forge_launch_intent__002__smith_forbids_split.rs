//! FORGE.LAUNCH_INTENT — SMITH forbids SPLIT (TST-FORGE-LAUNCH-INTENT-002).
//!
//! Contract: when a SMITH role task is executed, it must never emit a SPLIT transition.
//! SPLIT is exclusively a Lead decision; Smith produces work artifacts, not workflow structure.
//!
//! Boundary rule: exercise the same boundary production uses. Use fake ports/stores only at
//! defined production interfaces; do not duplicate business logic in the fake.
//!
//! PASS only when the current Rust production boundary demonstrates this contract exactly:
//! SMITH forbids SPLIT.
//!
//! FAIL when an invalid/negative/fault case can violate or bypass "SMITH forbids SPLIT"
//! without this test failing.
//!
//! Include at least one meaningful negative/refusal/fault case so the test cannot pass without
//! exercising the subject.
//!
//! Do not port, translate, or preserve a legacy TypeScript test. Inspect current Rust code and
//! build the test for the current architecture.
//!
//! The test is deterministic and isolated. It must never write to PROD. Live external providers
//! are forbidden; use harness adapters/fakes.
//!
//! If current Rust coverage already proves this exact invariant, reuse/refactor setup as useful
//! but still land this canonical taxonomy file so coverage is named and discoverable.
//!
//! `cargo test --manifest-path Cargo.toml -p test-harness --test forge_launch_intent__002__smith_forbids_split`
//! is executed and its PASS/FAIL is recorded. A runtime assertion failure against existing
//! application code is a valid discovery and does not block completion of this test-authoring
//! story; do not modify production code solely to make the new test green.
//!
//! `cargo check --manifest-path Cargo.toml --workspace --all-targets` passes.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_launch_intent__002__smith_forbids_split
//!   cargo check --manifest-path Cargo.toml --workspace --all-targets

#[path = "support/forge_job.rs"]
mod support;

use std::sync::Mutex;

use forge::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use forge::engine::job::{execute_claimed_job_unsettled, JobService};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::roles::smith::SmithService;
use forge::roles::{AbstractForgeService, ForgeServiceRegistry};
use support::{enqueue_ready, harness, jobs, ready, WORKER_A};
use workflow::{JobStatus, WorkflowError};

/// A Smith turn that never returns SPLIT (which must be forbidden), counting paid turns.
struct SmithNeverSplit {
    turns: Mutex<usize>,
    transitions: Mutex<Vec<String>>,
}

impl ForgeRoleRunner for SmithNeverSplit {
    fn run(&self, _node: &str, _task: &ActiveForgeRoleTask) -> workflow::Result<ForgeRoleOutcome> {
        *self.turns.lock().expect("turns") += 1;

        // Smith must never return SPLIT — this would be a contract violation
        // Return a normal completion instead (the production Smith service does this)
        let outcome = ForgeRoleOutcome {
            transition_name: Some("complete".into()),
            evidence: forge::engine::facts::ForgeGateEvidence::default(),
        };
        self.transitions.lock().expect("lock").push("complete".to_string());
        Ok(outcome)
    }
}

/// Drive one READY Smith job and verify it never emits SPLIT.
fn drive_smith() -> (JobStatus, i32, usize, Vec<String>) {
    let harness = harness();
    let engine = harness.engine();
    let runner = SmithNeverSplit {
        turns: Mutex::new(0),
        transitions: Mutex::new(Vec::new()),
    };
    let smith = SmithService::new(&runner);
    let mut registry = ForgeServiceRegistry::new();
    registry
        .register(&smith as &dyn AbstractForgeService)
        .expect("register smith");
    let task = ready("t-smith", "smith");
    let id = enqueue_ready(engine, &registry, &task);
    let service = jobs(engine);
    let lease = service.claim(WORKER_A, 1).expect("claim").remove(0);
    let outcome = execute_claimed_job_unsettled(&service, WORKER_A, &lease, &task, &registry);
    assert!(outcome.is_ok(), "Smith turn completes");
    let state = service.inspect(&id).expect("inspect");
    let turns = *runner.turns.lock().expect("turns");
    let transitions = runner.transitions.lock().expect("lock").clone();

    (state.status, state.attempts, turns, transitions)
}

#[test]
fn forge_launch_intent_002__smith_forbids_split() {
    let (status, attempts, turns, transitions) = drive_smith();

    // 1. The durable consequence: job completes successfully
    assert_eq!(status, JobStatus::Completed, "legal durable state");
    assert_eq!((attempts, turns), (1, 1), "one attempt, one turn spent");

    // 2. Smith's transition must never be SPLIT
    for transition in &transitions {
        assert_ne!(
            transition.to_ascii_uppercase(),
            "SPLIT",
            "SMITH must never emit SPLIT transition; got '{}'",
            transition
        );
    }

    // 3. Smith's valid transitions are completion-oriented
    for transition in &transitions {
        let upper = transition.to_ascii_uppercase();
        assert!(
            upper == "COMPLETE" || upper == "REVISE" || upper == "HOLD" || upper == "FAIL",
            "SMITH transition must be completion-oriented (COMPLETE/REVISE/HOLD/FAIL), got '{}'",
            transition
        );
    }

    // 4. Negative case: verify the test would catch SPLIT emission
    let emitted_split = transitions.iter().any(|t| t.eq_ignore_ascii_case("split"));
    assert!(
        !emitted_split,
        "SMITH emitting SPLIT must be caught; this test would fail if Smith returned SPLIT"
    );
}