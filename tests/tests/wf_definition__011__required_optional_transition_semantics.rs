//! WF.DEFINITION — required/optional transition semantics (TST-WF-DEFINITION-011).
//!
//! Contract: a fork transition decides whether its child branch is **required**. The flag is declared on the
//! transition (`TransitionDefinition.required`), defaults to required when absent, and the engine enforces it in
//! exactly two places — the join gate and the end-state resolution:
//!
//! - spawn (`handle_fork`, `middle/workflow/src/engine/execute_node_leave.rs:375-422`): the child token inherits
//!   `transition.required.unwrap_or(true)` (`execute_node_leave.rs:391`), so a transition that says nothing is a
//!   required branch and only `required="false"` opts out;
//! - join (`handle_join`, `middle/workflow/src/engine/handle_join.rs:7-119`): the join waits while any **required**
//!   sibling is still active (`handle_join.rs:42-44`); when the last required branch arrives, still-active
//!   **optional** siblings are skipped (`token.skipped`, their open tasks obsoleted as `"branch skipped"`,
//!   `handle_join.rs:46-80`);
//! - end (`resolve_process_after_token`, `execute_node_leave.rs:280-298`): a token that reaches a non-`Completed`
//!   end terminates the whole process **only when the token is required**; an optional token merely runs the
//!   completion check, so an optional branch may fail without failing the process.
//!
//! The contract is demonstrated in both directions on the production engine (`WorkflowEngine<MemoryStore>`,
//! deterministic `TestClock`, no database, no network — nothing leaves the process):
//!
//! - the fork's children carry the declared flags (absent means required `true`, explicit `false` means optional);
//! - completing the optional branch first parks at the join with no `token.joined` — the join waits for required;
//! - completing the required branch first fires the join and **skips** the still-running optional branch (its task
//!   becomes `Obsolete`, and completing it afterwards is refused with `TASK_NOT_ACTIONABLE`);
//! - an optional branch that ends `Failed` leaves the process `Active`; a required branch that ends `Failed`
//!   terminates it (`Error`/`Failed` with a `process.failed` event).
//!
//! Level L0 Pure, harness `WorkflowHarness`. Deterministic and isolated: a fixed `TestClock`, an in-memory store,
//! and no database — nothing is written outside the process.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_definition__011__required_optional_transition_semantics

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessEvent,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, Task, TaskStatus,
    TransitionDefinition, Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L0 Pure";
/// The fork/join graph: one required branch, one optional branch, rendezvous at a join.
const JOIN_KEY: &str = "TST-WF-DEFINITION-011-JOIN";
/// The end-resolution graph: each branch ends Failed; only the required one may terminate the process.
const END_KEY: &str = "TST-WF-DEFINITION-011-END";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const START_NODE: &str = "start";
const FORK_NODE: &str = "fork";
const JOIN_NODE: &str = "join";
const END_NODE: &str = "end";
/// The required branch task (its fork transition declares no `required`, so the default applies).
const REQ_TASK: &str = "t_req";
/// The optional branch task (its fork transition declares `required: false`).
const OPT_TASK: &str = "t_opt";
const ACTOR: &str = "tst-actor";

fn transition(name: &str, to: &str, required: Option<bool>) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required,
    }
}

fn start_node(next: &str) -> NodeDefinition {
    NodeDefinition {
        id: START_NODE.to_string(),
        node_type: "start".to_string(),
        transitions: Some(vec![transition("begin", next, None)]),
        ..Default::default()
    }
}

fn task_node(id: &str, next: &str) -> NodeDefinition {
    NodeDefinition {
        id: id.to_string(),
        node_type: "task".to_string(),
        name: Some(id.to_string()),
        transitions: Some(vec![transition("done", next, None)]),
        ..Default::default()
    }
}

fn end_node(id: &str, outcome: ProcessOutcome) -> NodeDefinition {
    NodeDefinition {
        id: id.to_string(),
        node_type: "end".to_string(),
        outcome: Some(outcome),
        ..Default::default()
    }
}

/// `start -> fork(req -> t_req, opt -> t_opt) -> join -> end`. The `req` transition carries no `required` flag
/// (the default must apply); `opt` carries `Some(false)`.
fn join_definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(START_NODE.to_string(), start_node(FORK_NODE));
    nodes.insert(
        FORK_NODE.to_string(),
        NodeDefinition {
            id: FORK_NODE.to_string(),
            node_type: "fork".to_string(),
            transitions: Some(vec![
                transition("req", REQ_TASK, None),
                transition("opt", OPT_TASK, Some(false)),
            ]),
            ..Default::default()
        },
    );
    nodes.insert(REQ_TASK.to_string(), task_node(REQ_TASK, JOIN_NODE));
    nodes.insert(OPT_TASK.to_string(), task_node(OPT_TASK, JOIN_NODE));
    nodes.insert(
        JOIN_NODE.to_string(),
        NodeDefinition {
            id: JOIN_NODE.to_string(),
            node_type: "join".to_string(),
            transitions: Some(vec![transition("onward", END_NODE, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        END_NODE.to_string(),
        end_node(END_NODE, ProcessOutcome::Completed),
    );
    ProcessDefinition {
        id: format!("{JOIN_KEY}-def"),
        tenant_id: None,
        key: JOIN_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: JOIN_KEY.to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: START_NODE.to_string(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

/// `start -> fork(a -> fail_req, b -> fail_opt)` where each task ends `Failed`. The `a` transition is required
/// (default); `b` is optional. Completing `fail_opt` must not terminate the process; completing `fail_req` must.
fn end_definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(START_NODE.to_string(), start_node(FORK_NODE));
    nodes.insert(
        FORK_NODE.to_string(),
        NodeDefinition {
            id: FORK_NODE.to_string(),
            node_type: "fork".to_string(),
            transitions: Some(vec![
                transition("a", "fail_req", None),
                transition("b", "fail_opt", Some(false)),
            ]),
            ..Default::default()
        },
    );
    nodes.insert("fail_req".to_string(), task_node("fail_req", "end_req"));
    nodes.insert("fail_opt".to_string(), task_node("fail_opt", "end_opt"));
    nodes.insert(
        "end_req".to_string(),
        end_node("end_req", ProcessOutcome::Failed),
    );
    nodes.insert(
        "end_opt".to_string(),
        end_node("end_opt", ProcessOutcome::Failed),
    );
    ProcessDefinition {
        id: format!("{END_KEY}-def"),
        tenant_id: None,
        key: END_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: END_KEY.to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: START_NODE.to_string(),
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

fn complete_params(task_id: &str, transition: &str) -> CompleteTaskParams {
    CompleteTaskParams {
        task_id: task_id.to_string(),
        user_id: ACTOR.to_string(),
        form_data: Value::object(),
        transition_name: Some(transition.to_string()),
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
        .unwrap_or_else(|| panic!("a task is parked at {node}"))
}

fn task_by_id(harness: &EngineHarness, task_id: &str) -> Task {
    harness
        .store()
        .with_tx(|tx| tx.get_task(task_id))
        .expect("the task is readable")
}

fn events_of_type(harness: &EngineHarness, instance: &str, event_type: &str) -> Vec<ProcessEvent> {
    harness
        .store()
        .with_tx(|tx| tx.history(instance, 512))
        .expect("the instance history reads")
        .into_iter()
        .filter(|event| event.event_type == event_type)
        .collect()
}

fn instance_of(harness: &EngineHarness, instance: &str) -> workflow::ProcessInstance {
    harness
        .store()
        .with_tx(|tx| tx.get_instance(instance))
        .expect("the instance is readable")
}

fn active_token_count(harness: &EngineHarness, instance: &str) -> i32 {
    harness
        .store()
        .with_tx(|tx| tx.count_active_tokens(instance))
        .expect("the active token count reads")
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-DEFINITION-011); the file and the assay use it.
fn wf_definition_011__required_optional_transition_semantics() {
    let clock = TestClock::at_unix_millis(1_650_000_000_000);
    let harness = EngineHarness::new(clock);
    harness
        .engine()
        .seed_definition(join_definition())
        .expect("the fork/join definition registers");
    harness
        .engine()
        .seed_definition(end_definition())
        .expect("the end-resolution definition registers");

    // ── 1. SPAWN — the children carry the declared flags, absent means required ────────────────────────────────
    // The `req` transition declares no `required`, so its child must be required (`unwrap_or(true)`); `opt`
    // declares `false`. The flags are read back from the durable token rows, not inferred from later behaviour.
    let probe = harness
        .engine()
        .start_process(start_params(JOIN_KEY))
        .expect("the probe process starts and parks both branches");
    let children = harness
        .store()
        .with_tx(|tx| tx.list_children(&probe.root_token_id))
        .expect("the fork children are readable");
    assert_eq!(
        children.len(),
        2,
        "{HARNESS}: the two-transition fork spawns exactly two children"
    );
    let flag = |node: &str| {
        children
            .iter()
            .find(|child| child.node_id == node)
            .unwrap_or_else(|| panic!("a child is parked at {node}"))
            .required
    };
    assert!(
        flag(REQ_TASK),
        "{HARNESS}: a transition that declares no required flag spawns a required child"
    );
    assert!(
        !flag(OPT_TASK),
        "{HARNESS}: a transition that declares required=false spawns an optional child"
    );

    // ── 2. JOIN WAITS FOR REQUIRED — the optional branch alone cannot fire the join ────────────────────────────
    // Completing the optional task moves its token to the join, but the required sibling is still active, so the
    // join must wait: no `token.joined`, and the process stays active.
    let waiting = harness
        .engine()
        .start_process(start_params(JOIN_KEY))
        .expect("the waiting process starts");
    let waiting_id = waiting.process_instance_id.clone();
    let waiting_opt = task_at(&harness, &waiting_id, OPT_TASK);
    harness
        .engine()
        .complete_task(complete_params(&waiting_opt.id, "done"))
        .expect("the optional branch completes to the join");
    assert_eq!(
        events_of_type(&harness, &waiting_id, "token.joined").len(),
        0,
        "{HARNESS}: the join must not fire while a required sibling is still active"
    );
    assert_eq!(
        instance_of(&harness, &waiting_id).status,
        ProcessStatus::Active,
        "{HARNESS}: the process waits at the join for its required branch"
    );
    // The required branch then completes and the waiting join fires exactly once, completing the process.
    let waiting_req = task_at(&harness, &waiting_id, REQ_TASK);
    harness
        .engine()
        .complete_task(complete_params(&waiting_req.id, "done"))
        .expect("the required branch completes to the join");
    assert_eq!(
        events_of_type(&harness, &waiting_id, "token.joined").len(),
        1,
        "{HARNESS}: the join fires once the last required branch arrives"
    );
    assert_eq!(
        instance_of(&harness, &waiting_id).status,
        ProcessStatus::Completed,
        "{HARNESS}: the joined process completes through its Completed end"
    );

    // ── 3. REQUIRED FIRES FIRST — the still-running optional branch is skipped, not left behind ────────────────
    // Completing the required branch first fires the join while the optional token is still active. The join must
    // skip it (`token.skipped`) and obsolete its open task; the skip is enforced, so completing that task
    // afterwards is refused with `TASK_NOT_ACTIONABLE`.
    let skipping = harness
        .engine()
        .start_process(start_params(JOIN_KEY))
        .expect("the skipping process starts");
    let skipping_id = skipping.process_instance_id.clone();
    let skipping_req = task_at(&harness, &skipping_id, REQ_TASK);
    let skipping_opt = task_at(&harness, &skipping_id, OPT_TASK);
    harness
        .engine()
        .complete_task(complete_params(&skipping_req.id, "done"))
        .expect("the required branch completes first");
    let skipped = events_of_type(&harness, &skipping_id, "token.skipped");
    assert_eq!(
        skipped.len(),
        1,
        "{HARNESS}: firing the join with an active optional sibling skips exactly that sibling"
    );
    let obsoleted = task_by_id(&harness, &skipping_opt.id);
    assert_eq!(
        obsoleted.status,
        TaskStatus::Obsolete,
        "{HARNESS}: the skipped branch's open task is obsoleted, not left Ready"
    );
    assert_eq!(
        instance_of(&harness, &skipping_id).status,
        ProcessStatus::Completed,
        "{HARNESS}: the process completes after the join fires past its optional branch"
    );
    let refused = harness
        .engine()
        .complete_task(complete_params(&skipping_opt.id, "done"))
        .expect_err("a skipped branch's task must not be completable");
    assert_eq!(
        refused.code(),
        "TASK_NOT_ACTIONABLE",
        "{HARNESS}: the skipped task refuses completion: {refused}"
    );

    // ── 4. END RESOLUTION — an optional failure does not fail the process; a required one does ────────────────
    // Both branches end `Failed`. Completing the optional branch first must leave the process active with the
    // required branch still alive; completing the required branch then terminates the process as Failed.
    let ending = harness
        .engine()
        .start_process(start_params(END_KEY))
        .expect("the end-resolution process starts");
    let ending_id = ending.process_instance_id.clone();
    let ending_opt = task_at(&harness, &ending_id, "fail_opt");
    let ending_req = task_at(&harness, &ending_id, "fail_req");
    harness
        .engine()
        .complete_task(complete_params(&ending_opt.id, "done"))
        .expect("the optional branch reaches its Failed end");
    let mid = instance_of(&harness, &ending_id);
    assert_eq!(
        mid.status,
        ProcessStatus::Active,
        "{HARNESS}: an optional branch that ends Failed must not terminate the process"
    );
    assert_eq!(
        mid.outcome, None,
        "{HARNESS}: an optional failure sets no process outcome"
    );
    assert_eq!(
        active_token_count(&harness, &ending_id),
        1,
        "{HARNESS}: the required branch is still alive after the optional failure"
    );
    harness
        .engine()
        .complete_task(complete_params(&ending_req.id, "done"))
        .expect("the required branch reaches its Failed end");
    let end = instance_of(&harness, &ending_id);
    assert_eq!(
        end.status,
        ProcessStatus::Error,
        "{HARNESS}: a required branch that ends Failed terminates the process"
    );
    assert_eq!(
        end.outcome,
        Some(ProcessOutcome::Failed),
        "{HARNESS}: the termination carries the Failed outcome"
    );
    assert_eq!(
        events_of_type(&harness, &ending_id, "process.failed").len(),
        1,
        "{HARNESS}: the required failure is announced exactly once"
    );
}
