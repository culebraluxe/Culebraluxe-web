//! WF.HUMAN_TASK — candidate claim (TST-WF-HUMAN-TASK-001).
//!
//! Contract: a human task whose definition names **candidates** may be claimed by a candidate, and by no one
//! else. `WorkflowEngine::claim_task` at `rust/core/workflow/src/engine/engine_options.rs:188` is the production
//! boundary: it locks the instance and the task, admits only a `Ready`/`Reserved` task
//! (`engine_options.rs:202-207`), and then applies the candidate gate —
//!
//! ```text
//! let can_claim = task.assignee.as_deref() == Some(user_id)
//!     || task.candidates.iter().any(|c| c == user_id)
//!     || task.candidates.is_empty();   // engine_options.rs:208-210
//! ```
//!
//! A user who is neither the (absent) assignee nor listed in `candidates` — and where `candidates` is not empty —
//! is refused with `TASK_CANDIDATE_ONLY` (`engine_options.rs:211-216`), and the refusal must commit nothing. A
//! candidate who claims is moved onto the task: `status = Reserved`, `assignee = user`, `claimed_at = now`,
//! `version += 1`, with a durable `task.claimed` event (`engine_options.rs:225-248`). A candidate is not a blank
//! cheque: once one candidate owns the `Reserved` task, a second candidate is refused with
//! `TASK_ALREADY_ASSIGNED` (`engine_options.rs:217-224`). An empty candidate set is the open-task path the same
//! gate declares (`candidates.is_empty()`), so the refusal above is shown to be candidate-specific rather than a
//! boundary that refuses every claim.
//!
//! This exercises the production boundary, not a re-declaration of it. The real `WorkflowEngine<MemoryStore>` is
//! driven through `seed_definition`, `start_process` and `claim_task`; the parked task, its candidate roster and
//! every claim decision are read back through the production `Store` and the durable event log on `MemoryStore`,
//! production's in-memory `TxStore`. No provider is touched: a human task has no external adapter, so the only
//! seam this test fakes is none — the engine, the store and the candidate rule are all production code.
//!
//! Level: L3 Composition, harness `WorkflowHarness`. Deterministic and isolated: a fixed `TestClock`, an in-memory
//! store, and no database — nothing is written outside the process.
//!
//! Run with:
//!   cargo test --manifest-path rust/Cargo.toml -p test-harness --test wf_human_task__001__candidate_claim

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessEvent, ProcessGraph,
    ProcessOutcome, ProcessStatus, StartProcessParams, Task, TaskStatus, TransitionDefinition,
    Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L3 Composition";
/// The definition key/version the candidate task graph registers under.
const CANDIDATE_KEY: &str = "TST-WF-HUMAN-TASK-001";
/// A second graph whose task names no candidates: the open-task control.
const OPEN_KEY: &str = "TST-WF-HUMAN-TASK-001-OPEN";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
/// The candidate-bearing task node.
const TASK_NODE: &str = "review";
/// The transition that leaves the task (unused once a claim parks on it, but required for a well-formed graph).
const APPROVE: &str = "approve";
const END_NODE: &str = "end";
/// The two candidates the definition names.
const ALICE: &str = "alice";
const BOB: &str = "bob";
/// A user who is deliberately not a candidate.
const CAROL: &str = "carol";
/// An arbitrary user for the open (candidate-less) task control.
const DAVE: &str = "dave";

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// `start -> review (task) -> end`. The task's `candidate_groups` are the candidate roster; `None` (or an empty
/// list) means the candidate gate's open branch.
fn task_definition(key: &str, candidates: Option<Vec<String>>) -> ProcessDefinition {
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
            candidate_groups: candidates,
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
        id: format!("{key}-def"),
        tenant_id: None,
        key: key.to_string(),
        version: DEFINITION_VERSION,
        name: key.to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: "start".to_string(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

fn start_params(key: &str) -> StartProcessParams {
    StartProcessParams {
        definition_key: key.to_string(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: Value::object(),
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    }
}

/// Start a graph, let it commit, and hand back `(instance_id, the parked task)`.
fn start_and_park(harness: &EngineHarness, key: &str) -> (String, Task) {
    let started = harness
        .engine()
        .start_process(start_params(key))
        .expect("the process starts and parks on its human task");
    let instance = started.process_instance_id;
    let task = task_at(harness, &instance, TASK_NODE);
    (instance, task)
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
        .unwrap_or_else(|| panic!("a task is parked at {node}"))
}

/// The task read back by its durable id, so the assertions always name the exact row the claim touched.
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

fn instance_status(harness: &EngineHarness, instance: &str) -> ProcessStatus {
    harness
        .store()
        .with_tx(|tx| tx.get_instance(instance))
        .expect("the instance is readable")
        .status
}

/// The durable shape a claim decision moves — `(status, assignee, claimed_at, version)`. A refused claim must
/// leave every field identical, not merely leave the status `Ready`.
fn fingerprint(task: &Task) -> (TaskStatus, Option<String>, Option<i64>, i32) {
    (
        task.status,
        task.assignee.clone(),
        task.claimed_at,
        task.version,
    )
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-HUMAN-TASK-001); the file and the assay use it.
fn wf_human_task_001__candidate_claim() {
    let t0 = 1_650_000_000_000;
    let clock = TestClock::at_unix_millis(t0);
    let harness = EngineHarness::new(clock.clone());
    harness
        .engine()
        .seed_definition(task_definition(
            CANDIDATE_KEY,
            Some(vec![ALICE.to_string(), BOB.to_string()]),
        ))
        .expect("the candidate task definition registers with the engine");
    harness
        .engine()
        .seed_definition(task_definition(OPEN_KEY, None))
        .expect("the open task definition registers with the engine");

    // ── 1. THE PARKED TASK CARRIES ITS CANDIDATE ROSTER ────────────────────────────────────────────────────────
    // The gate below reads `task.candidates`, so the roster must actually reach the durable task row, unassigned.
    let (instance, parked) = start_and_park(&harness, CANDIDATE_KEY);
    assert_eq!(
        parked.status,
        TaskStatus::Ready,
        "{HARNESS}: a fresh human task parks Ready for a candidate to claim"
    );
    assert_eq!(
        parked.assignee, None,
        "{HARNESS}: a fresh human task is owned by no one"
    );
    assert_eq!(
        parked.candidates,
        vec![ALICE.to_string(), BOB.to_string()],
        "{HARNESS}: the definition's candidates reach the durable task row"
    );
    assert_eq!(
        parked.claimed_at, None,
        "{HARNESS}: a fresh human task has never been claimed"
    );
    assert_eq!(
        parked.version, 1,
        "{HARNESS}: a fresh human task is at its initial version"
    );
    // The candidates are auditable: the `task.created` event carries the same roster the gate reads, so a task
    // created without candidates could not silently turn the gate into the open-task path.
    let created = events_of_type(&harness, &instance, "task.created");
    assert_eq!(
        created.len(),
        1,
        "{HARNESS}: the parked task is announced exactly once"
    );
    let created_candidates: Vec<String> = created[0]
        .data
        .get("candidates")
        .and_then(Value::as_array)
        .expect("the task.created event carries its candidate roster")
        .iter()
        .map(|candidate| {
            candidate
                .as_str()
                .expect("every candidate is named")
                .to_string()
        })
        .collect();
    assert_eq!(
        created_candidates,
        vec![ALICE.to_string(), BOB.to_string()],
        "{HARNESS}: the durable task.created event names the same candidates the claim gate will enforce"
    );

    // ── 2. NEGATIVE / REFUSAL — A NON-CANDIDATE CANNOT CLAIM ───────────────────────────────────────────────────
    // Carol is not in the roster and there is no assignee to fall back on, so the candidate gate must refuse her
    // with `TASK_CANDIDATE_ONLY` and commit *nothing*: the whole durable row is byte-identical afterwards.
    let before_carol = fingerprint(&task_by_id(&harness, &parked.id));
    let refused = harness
        .engine()
        .claim_task(&parked.id, CAROL)
        .expect_err("a non-candidate must not be allowed to claim a candidate-only task");
    assert_eq!(
        refused.code(),
        "TASK_CANDIDATE_ONLY",
        "{HARNESS}: the refusal names the candidate rule, not a generic conflict"
    );
    assert!(
        refused.to_string().contains(CAROL),
        "{HARNESS}: the refusal names the user it turned away: {refused}"
    );
    assert_eq!(
        fingerprint(&task_by_id(&harness, &parked.id)),
        before_carol,
        "{HARNESS}: a refused non-candidate claim must commit nothing — status, assignee, claimed_at and version are unchanged"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "task.claimed").len(),
        0,
        "{HARNESS}: a refused claim emits no task.claimed event"
    );
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Active,
        "{HARNESS}: a refused claim leaves the process active on its task"
    );

    // ── 3. POSITIVE — A CANDIDATE CLAIMS ───────────────────────────────────────────────────────────────────────
    // The claim happens at a later instant than the park, so `claimed_at` is proven to be the claim time and not
    // the creation time.
    harness.clock().advance_millis(3_600_000);
    let claim_instant = harness.now_millis();
    harness
        .engine()
        .claim_task(&parked.id, ALICE)
        .expect("a listed candidate claims the task");

    let claimed = task_by_id(&harness, &parked.id);
    assert_eq!(
        claimed.status,
        TaskStatus::Reserved,
        "{HARNESS}: a candidate's claim reserves the task"
    );
    assert_eq!(
        claimed.assignee.as_deref(),
        Some(ALICE),
        "{HARNESS}: the claim records the candidate as assignee"
    );
    assert_eq!(
        claimed.claimed_at,
        Some(claim_instant),
        "{HARNESS}: claimed_at is the instant of the claim, read from the engine clock"
    );
    assert_eq!(
        claimed.version,
        parked.version + 1,
        "{HARNESS}: the claim bumps the task version exactly once"
    );
    assert_eq!(
        claimed.candidates,
        vec![ALICE.to_string(), BOB.to_string()],
        "{HARNESS}: claiming does not rewrite the candidate roster"
    );
    // The claim is durable and attributed: one `task.claimed` event, naming this task, this actor, and the
    // status the task left (`ready`).
    let claimed_events = events_of_type(&harness, &instance, "task.claimed");
    assert_eq!(
        claimed_events.len(),
        1,
        "{HARNESS}: the candidate's single claim emits exactly one task.claimed event"
    );
    assert_eq!(
        claimed_events[0].task_id.as_deref(),
        Some(parked.id.as_str()),
        "{HARNESS}: the claim event names the task that was claimed"
    );
    assert_eq!(
        claimed_events[0].actor, ALICE,
        "{HARNESS}: the claim event is attributed to the claiming candidate"
    );
    assert_eq!(
        claimed_events[0]
            .data
            .get("previousStatus")
            .and_then(Value::as_str),
        Some("ready"),
        "{HARNESS}: the claim event records the status the task left behind"
    );

    // ── 4. NEGATIVE — A SECOND CANDIDATE CANNOT STEAL THE CLAIM ────────────────────────────────────────────────
    // Bob is a candidate, but the task is no longer open: it is reserved to Alice. The gate must refuse him with
    // `TASK_ALREADY_ASSIGNED` and commit nothing — being a candidate is permission to claim at most one task.
    let before_bob = fingerprint(&task_by_id(&harness, &parked.id));
    let stolen = harness.engine().claim_task(&parked.id, BOB).expect_err(
        "a candidate must not be able to claim a task another candidate already reserved",
    );
    assert_eq!(
        stolen.code(),
        "TASK_ALREADY_ASSIGNED",
        "{HARNESS}: the second candidate is refused because the task is already owned"
    );
    assert_eq!(
        fingerprint(&task_by_id(&harness, &parked.id)),
        before_bob,
        "{HARNESS}: the refused second claim must commit nothing — Alice keeps the task at the same version"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "task.claimed").len(),
        1,
        "{HARNESS}: the refused second claim emits no further task.claimed event"
    );

    // ── 5. POSITIVE CONTROL — AN EMPTY CANDIDATE SET IS CLAIMABLE BY ANYONE ─────────────────────────────────────
    // The same boundary admits a task that names no candidates (`candidates.is_empty()`); this is the open branch
    // of the identical gate. It proves the refusal in step 2 turned Carol away *because she is not a candidate*,
    // not because this engine refuses every claim, and it keeps the candidate gate honest about its declared rule.
    let (open_instance, open_task) = start_and_park(&harness, OPEN_KEY);
    assert!(
        open_task.candidates.is_empty(),
        "{HARNESS}: the control task names no candidates"
    );
    harness
        .engine()
        .claim_task(&open_task.id, DAVE)
        .expect("a task with an empty candidate set is claimable by any user");
    let open_claimed = task_by_id(&harness, &open_task.id);
    assert_eq!(
        open_claimed.status,
        TaskStatus::Reserved,
        "{HARNESS}: the open task is reserved by the arbitrary claimant"
    );
    assert_eq!(
        open_claimed.assignee.as_deref(),
        Some(DAVE),
        "{HARNESS}: the open task records the arbitrary claimant as assignee"
    );
    assert_eq!(
        events_of_type(&harness, &open_instance, "task.claimed").len(),
        1,
        "{HARNESS}: the open-task claim is announced exactly once"
    );
}
