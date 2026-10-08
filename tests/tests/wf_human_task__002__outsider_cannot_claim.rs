//! WF.HUMAN_TASK — outsider cannot claim (TST-WF-HUMAN-TASK-002).
//!
//! Contract: a user with no relationship to a human task — neither its assignee nor one of its candidates —
//! cannot take it over. Against a task owned by someone else the production boundary refuses the outsider on
//! **every** mutating operation, each with the code that names why:
//!
//! - `claim_task` (`middle/workflow/src/engine/engine_options.rs:194-256`): the candidate gate runs first, so
//!   an outsider who is not a candidate is refused with `TASK_CANDIDATE_ONLY` (`engine_options.rs:214-222`)
//!   even when the task is already owned — the gate stops strangers before ownership is even consulted;
//! - `complete_task` (`engine_options.rs:364-463`): a task assigned to someone else refuses the outsider with
//!   `TASK_ASSIGNEE_ONLY` (`engine_options.rs:381-388`);
//! - `release_task` (`engine_options.rs:258-305`): only the assignee may release, so the outsider is refused
//!   with `TASK_ASSIGNEE_ONLY` (`engine_options.rs:269-274`);
//! - `reassign_task` (`engine_options.rs:307-362`): the new assignee must be a candidate when the task names
//!   candidates, so reassigning to the outsider is refused with `TASK_CANDIDATE_ONLY`
//!   (`engine_options.rs:330-335`).
//!
//! Every refusal commits nothing (the whole durable row is byte-identical, no event is emitted), and the owner
//! can still complete the task afterwards — proving the refusals stop the outsider, not the task. A candidate
//! who is not the owner is refused differently (`TASK_ALREADY_ASSIGNED`), which pins that the outsider's
//! refusal is about being outside the roster, not merely about the task being taken.
//!
//! This exercises the production boundary, not a re-declaration of it. The real `WorkflowEngine<MemoryStore>`
//! is driven through `seed_definition`, `start_process`, `claim_task`, `complete_task`, `release_task` and
//! `reassign_task`; every refusal and the final completion are read back through the production `Store` and
//! the durable event log. No provider is touched: a human task has no external adapter.
//!
//! Level: L3 Composition, harness `WorkflowHarness`. Deterministic and isolated: a fixed `TestClock`, an
//! in-memory store, and no database — nothing is written outside the process.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_human_task__002__outsider_cannot_claim

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessEvent,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, Task, TaskStatus,
    TransitionDefinition, Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L3 Composition";
/// The candidate task graph the outsider presses against.
const TASK_KEY: &str = "TST-WF-HUMAN-TASK-002";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
/// The task node the process parks on.
const TASK_NODE: &str = "review";
const APPROVE: &str = "approve";
const END_NODE: &str = "end";
/// The two candidates the definition names; Alice will own the task.
const ALICE: &str = "alice";
const BOB: &str = "bob";
/// A user who is neither the assignee nor a candidate: the outsider.
const CAROL: &str = "carol";

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

fn task_definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "start".to_string(),
        NodeDefinition {
            id: "start".to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition("begin", TASK_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        TASK_NODE.to_string(),
        NodeDefinition {
            id: TASK_NODE.to_string(),
            node_type: "task".to_string(),
            name: Some("Review".to_string()),
            candidate_groups: Some(vec![ALICE.to_string(), BOB.to_string()]),
            transitions: Some(vec![transition(APPROVE, END_NODE)]),
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
        id: format!("{TASK_KEY}-def"),
        tenant_id: None,
        key: TASK_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: TASK_KEY.to_string(),
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
        definition_key: TASK_KEY.to_string(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: Value::object(),
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    }
}

fn task_by_id(harness: &EngineHarness, task_id: &str) -> Task {
    harness
        .store()
        .with_tx(|tx| tx.get_task(task_id))
        .expect("the task is readable")
}

fn history(harness: &EngineHarness, instance: &str) -> Vec<ProcessEvent> {
    harness
        .store()
        .with_tx(|tx| tx.history(instance, 512))
        .expect("the instance history reads")
}

fn events_of_type(harness: &EngineHarness, instance: &str, event_type: &str) -> Vec<ProcessEvent> {
    history(harness, instance)
        .into_iter()
        .filter(|event| event.event_type == event_type)
        .collect()
}

/// The durable shape an outsider decision must leave untouched — `(status, assignee, claimed_at, version)`.
/// A refused operation must leave every field identical, not merely leave the status alone.
fn fingerprint(task: &Task) -> (TaskStatus, Option<String>, Option<i64>, i32) {
    (
        task.status,
        task.assignee.clone(),
        task.claimed_at,
        task.version,
    )
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-HUMAN-TASK-002); the file and the assay use it.
fn wf_human_task_002__outsider_cannot_claim() {
    let clock = TestClock::at_unix_millis(1_650_000_000_000);
    let harness = EngineHarness::new(clock);
    harness
        .engine()
        .seed_definition(task_definition())
        .expect("the candidate task definition registers with the engine");

    // ── 1. THE TASK IS OWNED — Alice, a candidate, claims first ───────────────────────────────────────────────
    // The outsider presses against an owned task, so ownership is established first through the production
    // claim path.
    let started = harness
        .engine()
        .start_process(start_params())
        .expect("the process starts and parks on its human task");
    let instance = started.process_instance_id.clone();
    let parked_id = harness
        .store()
        .with_tx(|tx| tx.tasks_for_instance(&instance))
        .expect("the instance tasks are readable")
        .into_iter()
        .find(|task| task.node_id.as_deref() == Some(TASK_NODE))
        .expect("a task is parked at the review node")
        .id;
    harness
        .engine()
        .claim_task(&parked_id, ALICE)
        .expect("a listed candidate claims the task");
    let owned = task_by_id(&harness, &parked_id);
    assert_eq!(
        owned.status,
        TaskStatus::Reserved,
        "{HARNESS}: the candidate's claim reserves the task"
    );
    assert_eq!(
        owned.assignee.as_deref(),
        Some(ALICE),
        "{HARNESS}: the claim records Alice as assignee"
    );

    // ── 2. NEGATIVE — THE OUTSIDER CANNOT CLAIM THE OWNED TASK ────────────────────────────────────────────────
    // Carol is neither the assignee nor a candidate. The candidate gate runs before ownership is consulted, so
    // she is refused with `TASK_CANDIDATE_ONLY` — the gate stops strangers, not merely second candidates — and
    // the refusal commits nothing and emits nothing.
    let before_claim = fingerprint(&task_by_id(&harness, &parked_id));
    let refused_claim = harness
        .engine()
        .claim_task(&parked_id, CAROL)
        .expect_err("an outsider must not be allowed to claim an owned candidate task");
    assert_eq!(
        refused_claim.code(),
        "TASK_CANDIDATE_ONLY",
        "{HARNESS}: the outsider is refused at the candidate gate: {refused_claim}"
    );
    assert!(
        refused_claim.to_string().contains(CAROL),
        "{HARNESS}: the refusal names the outsider it turned away: {refused_claim}"
    );
    assert_eq!(
        fingerprint(&task_by_id(&harness, &parked_id)),
        before_claim,
        "{HARNESS}: the refused outsider claim commits nothing"
    );
    assert!(
        events_of_type(&harness, &instance, "task.claimed").len() == 1,
        "{HARNESS}: the refused outsider claim emits no further task.claimed event"
    );

    // ── 3. NEGATIVE — THE OUTSIDER CANNOT COMPLETE OR RELEASE THE OWNED TASK ─────────────────────────────────
    // Completion and release are assignee-only on an owned task, so the outsider is refused with
    // `TASK_ASSIGNEE_ONLY` on both — and both refusals commit nothing.
    let before_complete = fingerprint(&task_by_id(&harness, &parked_id));
    let refused_complete = harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: parked_id.clone(),
            user_id: CAROL.to_string(),
            form_data: Value::object(),
            transition_name: Some(APPROVE.to_string()),
        })
        .expect_err("an outsider must not complete someone else's task");
    assert_eq!(
        refused_complete.code(),
        "TASK_ASSIGNEE_ONLY",
        "{HARNESS}: the outsider is refused completion as a non-assignee: {refused_complete}"
    );
    assert_eq!(
        fingerprint(&task_by_id(&harness, &parked_id)),
        before_complete,
        "{HARNESS}: the refused outsider completion commits nothing"
    );
    let refused_release = harness
        .engine()
        .release_task(&parked_id, CAROL)
        .expect_err("an outsider must not release someone else's task");
    assert_eq!(
        refused_release.code(),
        "TASK_ASSIGNEE_ONLY",
        "{HARNESS}: the outsider is refused release as a non-assignee: {refused_release}"
    );
    assert_eq!(
        fingerprint(&task_by_id(&harness, &parked_id)),
        before_complete,
        "{HARNESS}: the refused outsider release commits nothing"
    );
    assert!(
        events_of_type(&harness, &instance, "task.completed").is_empty(),
        "{HARNESS}: the refused outsider completion emits no task.completed event"
    );
    assert!(
        events_of_type(&harness, &instance, "task.released").is_empty(),
        "{HARNESS}: the refused outsider release emits no task.released event"
    );

    // ── 4. NEGATIVE — THE TASK CANNOT BE REASSIGNED TO THE OUTSIDER ──────────────────────────────────────────
    // Reassignment to a non-candidate is refused with `TASK_CANDIDATE_ONLY`: the roster cannot be bypassed by
    // handing the task over, and the refusal commits nothing.
    let before_reassign = fingerprint(&task_by_id(&harness, &parked_id));
    let refused_reassign = harness
        .engine()
        .reassign_task(&parked_id, CAROL, ALICE)
        .expect_err("a candidate task must not be reassigned to an outsider");
    assert_eq!(
        refused_reassign.code(),
        "TASK_CANDIDATE_ONLY",
        "{HARNESS}: reassigning to an outsider is refused at the candidate gate: {refused_reassign}"
    );
    assert_eq!(
        fingerprint(&task_by_id(&harness, &parked_id)),
        before_reassign,
        "{HARNESS}: the refused reassignment commits nothing"
    );
    assert!(
        events_of_type(&harness, &instance, "task.reassigned").is_empty(),
        "{HARNESS}: the refused reassignment emits no task.reassigned event"
    );

    // ── 5. DISCRIMINATION — a candidate non-owner is refused differently ──────────────────────────────────────
    // Bob IS a candidate but not the owner: he passes the candidate gate and is refused at ownership with
    // `TASK_ALREADY_ASSIGNED`. The outsider's `TASK_CANDIDATE_ONLY` is therefore about being outside the
    // roster, not merely about the task being taken.
    let refused_bob = harness
        .engine()
        .claim_task(&parked_id, BOB)
        .expect_err("a candidate must not claim a task another candidate owns");
    assert_eq!(
        refused_bob.code(),
        "TASK_ALREADY_ASSIGNED",
        "{HARNESS}: a candidate non-owner is refused at ownership, not at the roster: {refused_bob}"
    );

    // ── 6. POSITIVE CONTROL — the owner still completes ───────────────────────────────────────────────────────
    // Alice completes through the approved transition: the boundary works for the owner, so the five refusals
    // above stopped the outsider rather than a broken task. The process completes behind her.
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: parked_id.clone(),
            user_id: ALICE.to_string(),
            form_data: Value::object(),
            transition_name: Some(APPROVE.to_string()),
        })
        .expect("the owner completes her own task");
    let completed = task_by_id(&harness, &parked_id);
    assert_eq!(
        completed.status,
        TaskStatus::Completed,
        "{HARNESS}: the owner's completion lands"
    );
    assert_eq!(
        completed.completed_by.as_deref(),
        Some(ALICE),
        "{HARNESS}: the completion is attributed to the owner"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "task.completed").len(),
        1,
        "{HARNESS}: the owner's single completion is announced exactly once"
    );
    let ended = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance))
        .expect("the instance is readable");
    assert_eq!(
        ended.status,
        ProcessStatus::Completed,
        "{HARNESS}: the process completes behind the owner's completion"
    );
}
