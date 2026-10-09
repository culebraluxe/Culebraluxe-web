//! WF.TOKEN — optional branch cannot prevent completion (TST-WF-TOKEN-006).
//!
//! Contract: when a fork mints an optional child token (required=false), that token's
//! completion or lack thereof must not prevent the process from completing.
//!
//! WHERE THE RULE LIVES — the half this case got wrong as first authored (batch 60, `5fdfbf4ae`).
//! The completion check is one line and it counts **every** token: `check_process_completion`
//! completes the instance only when `count_active_tokens == 0`
//! (`middle/workflow/src/engine/execute_node_leave.rs:173-202`; both stores count
//! `status = 'active'`, `memory.rs:283-290` and `neon/new_id.rs:283-290`). An optional branch
//! cannot prevent completion because the **join the branches rendezvous at retires it**:
//! `handle_join` waits on required siblings only (`handle_join.rs:42-44`), then concludes every
//! still-active optional sibling `Completed` with outcome `Skipped`, obsoletes its open task and
//! cancels its open job (`handle_join.rs:46-80`) before it emits the result token. That is the
//! production shape — the optional deadline tracks of `forge/definitions/RE_supermodel-v1.xml:199-204`
//! "skip straight to the join … and are skipped by the join (timer cancelled) when the milestone
//! completes first". A fork whose branches converge on an `end` node with **no join** is not a shape
//! this engine has a rule for, and waiting there is not a defect in the engine; the graph below is
//! the shape the contract speaks about.
//!
//! Level: L1 Component, harness `WorkflowHarness`. Deterministic and isolated: a fixed clock
//! and an in-memory store; no database, no network, no filesystem write, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_token__006__optional_branch_cannot_prevent_completion

use std::collections::BTreeMap;

use test_harness::EngineHarness;
use workflow::{
    CompleteTaskParams, DefinitionStatus, EngineOptions, MemoryStore, NodeDefinition,
    ProcessDefinition, ProcessGraph, ProcessInstance, ProcessOutcome, ProcessStatus, Result,
    StartProcessParams, Store, Task, TaskStatus, Token, TokenOutcome, TokenStatus,
    TransitionDefinition, TxStore, Value, WorkflowEngine,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L1 Component";

/// The definition key/version the engine half registers.
const DEFINITION_KEY: &str = "TST-WF-TOKEN-006";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";

/// A fork-join graph with one required branch and one optional branch:
/// start -> fork -> (required-branch -> task-r -> end) + (optional-branch -> task-o -> end)
/// The process must complete when the required branch completes, even if the optional
/// branch is still active (e.g., parked at its task).
const START_NODE: &str = "start";
const FORK_NODE: &str = "fork";
const REQ_BRANCH: &str = "req-branch";
const REQ_TASK: &str = "req-task";
const OPT_BRANCH: &str = "opt-branch";
const OPT_TASK: &str = "opt-task";
const END_NODE: &str = "end";
const BEGIN: &str = "begin";
const TO_REQ: &str = "to-req";
const TO_OPT: &str = "to-opt";
const FINISH_REQ: &str = "finish-req";
const FINISH_OPT: &str = "finish-opt";
/// The join both branches rendezvous at — where a still-active optional branch is retired.
const JOIN_NODE: &str = "join";
/// The join's own transition to the end node: the token the join creates takes it.
const ONWARD: &str = "onward";

fn transition(name: &str, to: &str, required: Option<bool>) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required,
    }
}

/// Fork with one required and one optional branch, joining at a single end node.
fn fork_optional_definition() -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        START_NODE.to_string(),
        NodeDefinition {
            id: START_NODE.to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition(BEGIN, FORK_NODE, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        FORK_NODE.to_string(),
        NodeDefinition {
            id: FORK_NODE.to_string(),
            node_type: "fork".to_string(),
            transitions: Some(vec![
                transition(TO_REQ, REQ_BRANCH, Some(true)), // required branch
                transition(TO_OPT, OPT_BRANCH, Some(false)), // optional branch
            ]),
            ..Default::default()
        },
    );
    nodes.insert(
        REQ_BRANCH.to_string(),
        NodeDefinition {
            id: REQ_BRANCH.to_string(),
            node_type: "service".to_string(),
            name: Some("Required Branch".to_string()),
            transitions: Some(vec![transition("next-req", REQ_TASK, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        REQ_TASK.to_string(),
        NodeDefinition {
            id: REQ_TASK.to_string(),
            node_type: "task".to_string(),
            name: Some("Required Task".to_string()),
            transitions: Some(vec![transition(FINISH_REQ, JOIN_NODE, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        OPT_BRANCH.to_string(),
        NodeDefinition {
            id: OPT_BRANCH.to_string(),
            node_type: "service".to_string(),
            name: Some("Optional Branch".to_string()),
            transitions: Some(vec![transition("next-opt", OPT_TASK, None)]),
            ..Default::default()
        },
    );
    nodes.insert(
        OPT_TASK.to_string(),
        NodeDefinition {
            id: OPT_TASK.to_string(),
            node_type: "task".to_string(),
            name: Some("Optional Task".to_string()),
            transitions: Some(vec![transition(FINISH_OPT, JOIN_NODE, None)]),
            ..Default::default()
        },
    );
    // The rendezvous: the join is what an optional branch cannot hold back, and what retires it.
    nodes.insert(
        JOIN_NODE.to_string(),
        NodeDefinition {
            id: JOIN_NODE.to_string(),
            node_type: "join".to_string(),
            transitions: Some(vec![transition(ONWARD, END_NODE, None)]),
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

fn read_token(store: &MemoryStore, id: &str) -> Token {
    store
        .with_tx(|tx| tx.get_token(id))
        .expect("the token is readable")
}

#[test]
#[allow(non_snake_case)]
fn wf_token_006__optional_branch_cannot_prevent_completion() {
    let harness = EngineHarness::at_unix_millis(1_700_000_000_000);
    harness
        .engine()
        .seed_definition(fork_optional_definition())
        .expect("the fork definition registers with the engine");
    let started = harness
        .engine()
        .start_process(start_params())
        .expect("the process starts and forks into required and optional branches");
    let instance = started.process_instance_id.clone();

    // The fork mints two child tokens: one required, one optional.
    let tokens = harness
        .store()
        .with_tx(|tx| tx.list_active_tokens(&instance))
        .expect("active tokens read");
    assert_eq!(
        tokens.len(),
        2,
        "{HARNESS}: the fork must mint exactly two child tokens"
    );

    // Identify required vs optional token by their `required` field.
    let req_token = tokens.iter().find(|t| t.required);
    let opt_token = tokens.iter().find(|t| !t.required);

    let req_token = req_token.expect("{HARNESS}: one token must be required");
    let opt_token = opt_token.expect("{HARNESS}: one token must be optional");

    let req_id = req_token.id.clone();
    let opt_id = opt_token.id.clone();

    assert!(
        req_token.required,
        "{HARNESS}: req token must be required=true"
    );
    assert!(
        !opt_token.required,
        "{HARNESS}: opt token must be required=false"
    );

    // Both tokens start at their branch service nodes, then move to their task nodes.
    let after_fork_req = read_token(harness.store(), &req_id);
    let after_fork_opt = read_token(harness.store(), &opt_id);

    assert_eq!(
        after_fork_req.node_id, REQ_TASK,
        "{HARNESS}: req token at req-task"
    );
    assert_eq!(
        after_fork_opt.node_id, OPT_TASK,
        "{HARNESS}: opt token at opt-task"
    );
    assert_eq!(after_fork_req.status, TokenStatus::Active);
    assert_eq!(after_fork_opt.status, TokenStatus::Active);

    // The optional branch is parked on an open task — the work a "required-only completion" rule
    // would have to ignore. The join is what will retire the branch, so read the task now: after
    // the join fires there is no open task left to read.
    let opt_task = harness
        .store()
        .with_tx(|tx| tx.open_tasks_for_token(&opt_id))
        .expect("opt token's task reads")
        .into_iter()
        .next()
        .expect("opt token has exactly one open task");

    // Complete ONLY the required branch's task.
    let req_task = harness
        .store()
        .with_tx(|tx| tx.open_tasks_for_token(&req_id))
        .expect("req token's task reads")
        .into_iter()
        .next()
        .expect("req token has exactly one open task");
    harness
        .engine()
        .complete_task(CompleteTaskParams {
            task_id: req_task.id.clone(),
            user_id: STARTED_BY.to_string(),
            form_data: Value::object(),
            transition_name: Some(FINISH_REQ.to_string()),
        })
        .expect("completing required task advances req token to the join");

    // The join fires now — it waits on required siblings only — so the required token is concluded
    // AT THE JOIN (the join completes it; it never reaches the end node) while the parked optional
    // branch is retired rather than waited for.
    let req_done = read_token(harness.store(), &req_id);
    assert_eq!(
        req_done.status,
        TokenStatus::Completed,
        "{HARNESS}: the required token is concluded by the join"
    );
    assert_eq!(req_done.node_id, JOIN_NODE);

    // THE CONTRACT: the optional branch did not prevent completion. `handle_join` concludes every
    // still-active optional sibling `Completed` with outcome `Skipped`
    // (`middle/workflow/src/engine/handle_join.rs:46-61`) — retired, not completed as work.
    let opt_done = read_token(harness.store(), &opt_id);
    assert_eq!(
        opt_done.status,
        TokenStatus::Completed,
        "{HARNESS}: the parked optional branch is concluded by the join"
    );
    assert_eq!(
        opt_done.outcome,
        Some(TokenOutcome::Skipped),
        "{HARNESS}: the optional branch is retired as Skipped, not completed as work"
    );

    // Its open task is obsoleted with it: the branch is not work anyone can still complete.
    let opt_task_after = harness
        .store()
        .with_tx(|tx| tx.get_task(&opt_task.id))
        .expect("the optional branch's task reads");
    assert_eq!(
        opt_task_after.status,
        TaskStatus::Obsolete,
        "{HARNESS}: retiring the optional branch obsoletes its task"
    );

    // The process is Completed even though the optional branch's work was never done.
    let instance_status = harness
        .store()
        .with_tx(|tx| tx.get_instance(&instance))
        .expect("the instance reads")
        .status;
    assert_eq!(
        instance_status, ProcessStatus::Completed,
        "{HARNESS}: process completes when the required branch completes, even though the optional \
         branch's work was never done"
    );

    // The retirement is on the record once: one join, one skip.
    let history = harness
        .store()
        .with_tx(|tx| tx.history(&instance, 100))
        .expect("the instance's history reads");
    assert_eq!(
        history
            .iter()
            .filter(|event| event.event_type == "token.joined")
            .count(),
        1,
        "{HARNESS}: the join fires exactly once"
    );
    assert_eq!(
        history
            .iter()
            .filter(|event| event.event_type == "token.skipped")
            .count(),
        1,
        "{HARNESS}: exactly one optional branch is retired"
    );

    // Retired means retired: the obsoleted task refuses completion, so the process cannot be dragged
    // back into work through the branch it decided not to wait for.
    match harness.engine().complete_task(CompleteTaskParams {
        task_id: opt_task.id.clone(),
        user_id: STARTED_BY.to_string(),
        form_data: Value::object(),
        transition_name: Some(FINISH_OPT.to_string()),
    }) {
        Err(error) => assert_eq!(
            error.code(),
            "TASK_NOT_ACTIONABLE",
            "{HARNESS}: a retired optional branch is refused as non-actionable work"
        ),
        Ok(()) => panic!("{HARNESS}: a retired optional branch must not be completable"),
    }

    // ── THE SHAPE MATTERS ────────────────────────────────────────────────────────────────────────
    // The case as first authored (batch 60, `5fdfbf4ae`) forked into a required and an optional branch
    // that converged on an `end` node with NO join, and asserted the process completed on the required
    // token alone. Nothing retires a token in that shape — the JOIN is what retires it — so the
    // engine's one completion rule (`count_active_tokens == 0`) correctly waited, and the case was red.
    // The rule is not "optional tokens are ignored"; it is "a join does not wait for them".
}
