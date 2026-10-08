//! WF.JOIN — only one downstream token created (TST-WF-JOIN-004).
//!
//! Contract: a fired join produces **exactly one** continuation token for the process. Under no circumstance — a
//! single arrival of the last required child, a replayed branch arrival, or a roster with several branches — is a
//! second token minted past the join. The one token that does exist is *the* join's: it is named by the
//! `token.joined` event's own `token_id` and its `resultTokenId` datum, and it belongs to the fork (never to a
//! branch). Every branch token that fed the join is concluded.
//!
//! This is asserted against the production `WorkflowEngine<MemoryStore>` and the production `Store`: the fork
//! mints its children, the last required arrival fires `handle_join`, and the resulting token inventory is read
//! back from the store. A join that minted two continuations (for example, one per racing branch arrival), or one
//! attributed to the wrong parent, fails here.
//!
//! ```text
//! start -> fan (fork, two required branches)
//!            |-- approve (task) -> converge (join) -> settle (end)
//!            '-- review  (task) -> converge
//! ```
//!
//! Level: L4 Adversarial, harness `WorkflowHarness`. Deterministic and isolated: fixed `TestClock`, in-memory
//! store, no database.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_join__004__only_one_downstream_token_created

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessEvent,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, Token, TokenStatus,
    TransitionDefinition, Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L4 Adversarial";
const DEFINITION_KEY: &str = "TST-WF-JOIN-004";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const FORK_NODE: &str = "fan";
const JOIN_NODE: &str = "converge";
const MAIN_NODE: &str = "approve";
const REVIEW_NODE: &str = "review";
const END_NODE: &str = "settle";
const BRANCH_TRANSITION: &str = "go";

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// `start -> fan (2 required branches) -> two task branches -> converge (join) -> settle (end)`.
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
                transition("review", REVIEW_NODE),
            ]),
            ..Default::default()
        },
    );
    for (id, name) in [(MAIN_NODE, "Approve"), (REVIEW_NODE, "Review")] {
        nodes.insert(
            id.to_string(),
            NodeDefinition {
                id: id.to_string(),
                node_type: "task".to_string(),
                name: Some(name.to_string()),
                transitions: Some(vec![transition(BRANCH_TRANSITION, JOIN_NODE)]),
                ..Default::default()
            },
        );
    }
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
        id: "tst-wf-join-004".to_string(),
        tenant_id: None,
        key: DEFINITION_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: "TST WF.JOIN 004".to_string(),
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

fn tasks(harness: &EngineHarness, instance: &str) -> Vec<workflow::Task> {
    harness
        .store()
        .with_tx(|tx| tx.tasks_for_instance(instance))
        .expect("the instance tasks are readable")
}

fn task_at(harness: &EngineHarness, instance: &str, node: &str) -> workflow::Task {
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

fn complete(harness: &EngineHarness, task_id: &str) -> workflow::Result<()> {
    harness.engine().complete_task(CompleteTaskParams {
        task_id: task_id.to_string(),
        user_id: STARTED_BY.to_string(),
        form_data: Value::object(),
        transition_name: Some(BRANCH_TRANSITION.to_string()),
    })
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-JOIN-004); the file and the assay use it.
fn wf_join_004__only_one_downstream_token_created() {
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

    let main_task = task_at(&harness, &instance, MAIN_NODE);
    let review_task = task_at(&harness, &instance, REVIEW_NODE);
    complete(&harness, &main_task.id).expect("the first required branch completes");
    complete(&harness, &review_task.id).expect("the second required branch completes the join");

    // Exactly one token.joined event, and exactly one token past the join.
    let joined = events_of_type(&harness, &instance, "token.joined");
    assert_eq!(
        joined.len(),
        1,
        "{HARNESS}: one join, one join event"
    );
    let tokens_after = tokens(&harness, &instance);
    let downstream: Vec<&Token> = tokens_after
        .iter()
        .filter(|t| t.node_id == END_NODE)
        .collect();
    assert_eq!(
        downstream.len(),
        1,
        "{HARNESS}: exactly one downstream token exists"
    );
    let result = downstream[0];

    // The join's own event names this one token, twice-over: the durable `token_id` column and the
    // `resultTokenId` datum must agree, or a reader of either side is misled.
    assert_eq!(
        joined[0].token_id.as_deref(),
        Some(result.id.as_str()),
        "{HARNESS}: the token.joined event is attributed to the downstream token"
    );
    assert_eq!(
        joined[0].data.get("resultTokenId").and_then(Value::as_str),
        Some(result.id.as_str()),
        "{HARNESS}: the token.joined payload names the same downstream token"
    );

    // The downstream token is the fork's continuation, not a branch's ghost: it is required, it belongs to the
    // fork token, and it is the process's only survivor — every branch token is concluded.
    let fork = tokens(&harness, &instance)
        .into_iter()
        .find(|t| t.node_id == FORK_NODE)
        .expect("the fork token is present");
    assert_eq!(
        result.parent_token_id.as_deref(),
        Some(fork.id.as_str()),
        "{HARNESS}: the downstream token descends from the fork"
    );
    assert!(
        result.required,
        "{HARNESS}: the continuation token is required, so an enclosing join can rely on it"
    );
    assert_eq!(
        result.status,
        TokenStatus::Completed,
        "{HARNESS}: the downstream token ran to its end"
    );
    let still_active: Vec<&Token> = tokens_after
        .iter()
        .filter(|t| t.status == TokenStatus::Active)
        .collect();
    assert_eq!(
        still_active.len(),
        0,
        "{HARNESS}: no token leaks active — there is no second live continuation"
    );
    let concluded_branches = tokens(&harness, &instance)
        .into_iter()
        .filter(|t| t.parent_token_id.as_deref() == Some(fork.id.as_str()) && t.id != result.id)
        .count();
    assert_eq!(
        concluded_branches, 2,
        "{HARNESS}: both branch tokens exist and are concluded; together with the result they are the fork's children"
    );
    assert_eq!(
        harness
            .store()
            .with_tx(|tx| tx.get_instance(&instance))
            .expect("the instance is readable")
            .status,
        ProcessStatus::Completed,
        "{HARNESS}: the single downstream token completed the process"
    );
}
