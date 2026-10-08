//! WF.JOIN — terminated process cannot reactivate (TST-WF-JOIN-006).
//!
//! Contract: once a process has been terminated (cancelled, failed, or conflicted) it never comes back. Every
//! work artifact it parked — open tokens, open tasks, open timer jobs — is closed by `terminate_process`
//! (`execute_node_leave.rs:204-277`): tokens concluded `Cancelled`, tasks obsoleted, jobs cancelled and their
//! leases cleared. Afterwards the production entry points refuse to wake it:
//!
//! - `complete_task` on its obsolete task — `TASK_NOT_ACTIONABLE`;
//! - `run_due_jobs` over its parked timer job — the job is no longer pending, so zero are claimed and zero fire;
//! - `cancel_process` again — idempotent `Ok`, but the instance row and the event log do not move; and no event of
//!   type `timer.fired` or `token.joined` is ever produced for the terminated instance.
//!
//! ```text
//! start -> fan (fork)
//!            |-- approve (required, task) -> converge (join) -> settle (end)
//!            '-- sla     (required, timer) -> converge
//! ```
//!
//! The process is cancelled while both children are parked: the task is obsolete and the timer job is cancelled
//! before either can fire, so nothing left to reactivate the instance survives the cancellation.
//!
//! Level: L4 Adversarial, harness `WorkflowHarness`. Deterministic and isolated: fixed `TestClock`, in-memory
//! store, no database.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_join__006__terminated_process_cannot_reactivate

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    CancelProcessParams, CompleteTaskParams, DefinitionStatus, Job, JobStatus, NodeDefinition,
    ProcessDefinition, ProcessEvent, ProcessGraph, ProcessOutcome, ProcessStatus,
    StartProcessParams, Task, TaskStatus, TimerSpec, Token, TokenStatus, TransitionDefinition,
    Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L4 Adversarial";
const DEFINITION_KEY: &str = "TST-WF-JOIN-006";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const FORK_NODE: &str = "fan";
const JOIN_NODE: &str = "converge";
const MAIN_NODE: &str = "approve";
const WAIT_NODE: &str = "sla";
const END_NODE: &str = "settle";
const BRANCH_TRANSITION: &str = "go";
/// Far-future due date so the parked timer never fires on its own at this clock instant.
const FAR_FUTURE_MS: &str = "4102444800000";
const WORKER: &str = "worker-1";

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// `start -> fan (2 required branches) -> approve (task) + sla (timer) -> converge (join) -> settle (end)`.
fn definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "start".to_string(),
        NodeDefinition {
            id: "start".to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition("begin", FORK_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        FORK_NODE.to_string(),
        NodeDefinition {
            id: FORK_NODE.to_string(),
            node_type: "fork".to_string(),
            name: Some("Fan".to_string()),
            transitions: Some(vec![
                transition("main", MAIN_NODE),
                transition("wait", WAIT_NODE),
            ]),
            ..Default::default()
        },
    );
    nodes.insert(
        MAIN_NODE.to_string(),
        NodeDefinition {
            id: MAIN_NODE.to_string(),
            node_type: "task".to_string(),
            name: Some("Approve".to_string()),
            transitions: Some(vec![transition(BRANCH_TRANSITION, JOIN_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        WAIT_NODE.to_string(),
        NodeDefinition {
            id: WAIT_NODE.to_string(),
            node_type: "timer".to_string(),
            name: Some("Sla".to_string()),
            timer: Some(TimerSpec {
                due_at: Some(FAR_FUTURE_MS.to_string()),
                due_at_variable: None,
                transition: Some(BRANCH_TRANSITION.to_string()),
            }),
            transitions: Some(vec![transition(BRANCH_TRANSITION, JOIN_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        JOIN_NODE.to_string(),
        NodeDefinition {
            id: JOIN_NODE.to_string(),
            node_type: "join".to_string(),
            name: Some("Converge".to_string()),
            transitions: Some(vec![transition("next", END_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        END_NODE.to_string(),
        NodeDefinition {
            id: END_NODE.to_string(),
            node_type: "end".to_string(),
            outcome: Some(ProcessOutcome::Completed),
            ..Default::default()
        },
    );
    ProcessDefinition {
        id: "tst-wf-join-006".to_string(),
        tenant_id: None,
        key: DEFINITION_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: "TST WF.JOIN 006".to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: "start".to_string(),
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

fn tasks(harness: &EngineHarness, instance: &str) -> Vec<Task> {
    harness
        .store()
        .with_tx(|tx| tx.tasks_for_instance(instance))
        .expect("the instance tasks are readable")
}

fn task_at(harness: &EngineHarness, instance: &str, node: &str) -> Task {
    tasks(harness, instance)
        .into_iter()
        .find(|task| task.node_id.as_deref() == Some(node))
        .unwrap_or_else(|| panic!("a task exists at {node}"))
}

fn tokens(harness: &EngineHarness, instance: &str) -> Vec<Token> {
    harness
        .store()
        .with_tx(|tx| tx.tokens_for_instance(instance))
        .expect("the instance tokens are readable")
}

fn jobs(harness: &EngineHarness, instance: &str) -> Vec<Job> {
    harness
        .store()
        .with_tx(|tx| tx.open_jobs_for_instance(instance))
        .expect("the instance jobs are readable")
}

fn history(harness: &EngineHarness, instance: &str) -> Vec<ProcessEvent> {
    harness
        .store()
        .with_tx(|tx| tx.history(instance, 512))
        .expect("the instance history reads")
}

fn events_of_type(
    harness: &EngineHarness,
    instance: &str,
    event_type: &str,
) -> Vec<ProcessEvent> {
    history(harness, instance)
        .into_iter()
        .filter(|event| event.event_type == event_type)
        .collect()
}

fn instance_status(harness: &EngineHarness, instance: &str) -> ProcessStatus {
    harness
        .store()
        .with_tx(|tx| tx.get_instance(instance))
        .expect("the instance is readable")
        .status
}

fn instance_outcome(harness: &EngineHarness, instance: &str) -> Option<ProcessOutcome> {
    harness
        .store()
        .with_tx(|tx| tx.get_instance(instance))
        .expect("the instance is readable")
        .outcome
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-JOIN-006); the file and the assay use it.
fn wf_join_006__terminated_process_cannot_reactivate() {
    let clock = TestClock::at_unix_millis(1_700_000_000_000);
    let harness = EngineHarness::new(clock);
    harness
        .engine()
        .seed_definition(definition())
        .expect("the definition registers with the engine");
    let started = harness
        .engine()
        .start_process(start_params())
        .expect("the fork process starts and parks its branches");
    let instance = started.process_instance_id.clone();

    // Both branches parked: a ready task and a pending timer job.
    let main_task = task_at(&harness, &instance, MAIN_NODE);
    assert_eq!(main_task.status, TaskStatus::Ready);
    let parked_jobs: Vec<Job> = {
        // The timer job's due date is far in the future, so it is Pending but not yet due.
        let all: Vec<Job> = harness
            .store()
            .with_tx(|tx| tx.open_jobs_for_instance(&instance))
            .expect("the instance jobs are readable");
        all
    };
    assert_eq!(parked_jobs.len(), 1, "{HARNESS}: the timer branch parked one pending job");
    assert_eq!(parked_jobs[0].status, JobStatus::Pending);
    let parked_job_id = parked_jobs[0].id.clone();

    // Terminate the process while both branches are parked.
    harness
        .engine()
        .cancel_process(CancelProcessParams {
            process_instance_id: instance.clone(),
            actor: STARTED_BY.to_string(),
            reason: Some("operator cancelled".to_string()),
        })
        .expect("the active process cancels");
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Aborted,
        "{HARNESS}: a cancelled process is Aborted"
    );
    assert_eq!(
        instance_outcome(&harness, &instance),
        Some(ProcessOutcome::Cancelled),
        "{HARNESS}: the cancellation outcome is recorded"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "process.cancelled").len(),
        1,
        "{HARNESS}: the cancellation is announced exactly once"
    );
    // Every artifact the cancellation closed: tokens cancelled, the task obsoleted, the job cancelled.
    assert!(
        tokens(&harness, &instance)
            .iter()
            .all(|t| t.status == TokenStatus::Completed
                && matches!(t.outcome, Some(workflow::TokenOutcome::Cancelled) | Some(workflow::TokenOutcome::Completed))),
        "{HARNESS}: every token of the terminated process is concluded"
    );
    assert_eq!(
        task_at(&harness, &instance, MAIN_NODE).status,
        TaskStatus::Obsolete,
        "{HARNESS}: the parked task is obsoleted by the termination"
    );
    let parked_job = harness
        .store()
        .with_tx(|tx| tx.get_job(&parked_job_id))
        .expect("the job is readable");
    assert_eq!(
        parked_job.status,
        JobStatus::Cancelled,
        "{HARNESS}: the parked timer job is cancelled by the termination"
    );
    assert_eq!(
        jobs(&harness, &instance).len(),
        0,
        "{HARNESS}: no job left open after termination"
    );

    // ── NEGATIVE/REFUSAL: completing the obsolete task cannot reactivate anything ─────────────────────────────
    let completed = harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: main_task.id.clone(),
            user_id: STARTED_BY.to_string(),
            form_data: Value::object(),
            transition_name: Some(BRANCH_TRANSITION.to_string()),
        })
        .expect_err("the terminated process's task cannot be completed");
    assert_eq!(
        completed.code(),
        "TASK_NOT_ACTIONABLE",
        "{HARNESS}: the refusal names the not-actionable rule"
    );
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Aborted,
        "{HARNESS}: the refused completion left the process terminated"
    );

    // ── NEGATIVE/REFUSAL: the parked timer cannot fire its way back ─────────────────────────────────────────────
    // The clock is moved past the parked job's due date, then the timer executor runs: nothing may be claimed or
    // fired, because the cancellation removed the job from the claimable set.
    harness.clock().advance_millis(4_223_366_400_000);
    let report = harness
        .engine()
        .run_due_jobs(WORKER, 8)
        .expect("the timer executor runs against the terminated process");
    assert_eq!(report.claimed.len(), 0, "{HARNESS}: nothing is claimable");
    assert_eq!(report.fired, 0, "{HARNESS}: the terminated timer does not fire");
    assert_eq!(
        events_of_type(&harness, &instance, "timer.fired").len(),
        0,
        "{HARNESS}: no timer.fired is ever produced for the terminated instance"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "token.joined").len(),
        0,
        "{HARNESS}: no join ever fires for the terminated instance"
    );
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Aborted,
        "{HARNESS}: running due jobs did not reactivate the process"
    );

    // ── NEGATIVE/CONTROL: cancelling again is idempotent and changes nothing ────────────────────────────────────
    harness
        .engine()
        .cancel_process(CancelProcessParams {
            process_instance_id: instance.clone(),
            actor: STARTED_BY.to_string(),
            reason: None,
        })
        .expect("a repeat cancel of a cancelled process is Ok");
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Aborted,
        "{HARNESS}: the process is still Aborted"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "process.cancelled").len(),
        1,
        "{HARNESS}: the repeat cancel emits no second process.cancelled"
    );
}
