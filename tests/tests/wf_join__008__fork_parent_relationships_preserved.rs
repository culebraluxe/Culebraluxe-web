//! WF.JOIN — fork parent relationships preserved (TST-WF-JOIN-008).
//!
//! Contract: every token minted by a fork points back at the fork's own token, and the join's continuation token
//! points back at the same fork token. Parent identity is the join's sibling inventory: `handle_join` finds the
//! fork through `token.parent_token_id` (`middle/workflow/src/engine/handle_join.rs:11-44`), counts required
//! active siblings and lists optional ones against it (`middle/workflow/src/memory.rs:283-308`), and mints the
//! result token with `parent_token_id` set to that same fork token (`handle_join.rs:94-108`). A child whose
//! `parent_token_id` were dropped, or a result token parented to a branch token or to nothing, would silently
//! detach the join from its sibling set — so each of the three linkages below is asserted, and a `token.forked`
//! event must name the same parent the row does.
//!
//! ```text
//! start -> fan (fork)
//!            |-- approve (required, task) -> converge (join) -> settle (end)
//!            '-- review  (required, task) -> converge
//! ```
//!
//! Level: L4 Adversarial, harness `WorkflowHarness`. Deterministic and isolated: fixed `TestClock`, in-memory
//! store, no database.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_join__008__fork_parent_relationships_preserved

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessEvent,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, Token, TransitionDefinition,
    Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L4 Adversarial";
const DEFINITION_KEY: &str = "TST-WF-JOIN-008";
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
        id: "tst-wf-join-008".to_string(),
        tenant_id: None,
        key: DEFINITION_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: "TST WF.JOIN 008".to_string(),
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

fn events_of_type(harness: &EngineHarness, instance: &str, event_type: &str) -> Vec<ProcessEvent> {
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-JOIN-008); the file and the assay use it.
fn wf_join_008__fork_parent_relationships_preserved() {
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
    let root_token_id = started.root_token_id.clone();

    // The token that arrived at the fork completes there and becomes the parent of every branch token it minted.
    let fork = tokens(&harness, &instance)
        .into_iter()
        .find(|t| t.node_id == FORK_NODE)
        .expect("the fork token exists");
    assert_eq!(
        fork.id, root_token_id,
        "{HARNESS}: the fork work means the root token itself"
    );
    assert_eq!(
        fork.status,
        workflow::TokenStatus::Completed,
        "{HARNESS}: the fork token is concluded once it has minted its children"
    );
    assert_eq!(
        fork.parent_token_id, None,
        "{HARNESS}: the fork token is the root's continuation, not the root's child"
    );
    let mut branches = Vec::new();
    for node in [MAIN_NODE, REVIEW_NODE] {
        let token = tokens(&harness, &instance)
            .into_iter()
            .find(|t| t.node_id == node)
            .unwrap_or_else(|| panic!("a token is parked at {node}"));
        assert_eq!(
            token.parent_token_id.as_deref(),
            Some(fork.id.as_str()),
            "{HARNESS}: the child token at {node} points back at the fork token"
        );
        assert!(
            token.required,
            "{HARNESS}: the sibling default makes the branch required"
        );
        branches.push(token);
    }
    // Each branch task is linked back to its own token, so the task graph and the token graph cannot disagree.
    for node in [MAIN_NODE, REVIEW_NODE] {
        let task = task_at(&harness, &instance, node);
        let token = tokens(&harness, &instance)
            .into_iter()
            .find(|t| t.node_id == node)
            .expect("the token exists");
        assert_eq!(
            task.token_id.as_deref(),
            Some(token.id.as_str()),
            "{HARNESS}: the task at {node} is attributed to its own branch token"
        );
    }
    // The durable record agrees with the rows: each `token.forked` event names the same fork parent.
    let forked = events_of_type(&harness, &instance, "token.forked");
    assert_eq!(forked.len(), 2, "{HARNESS}: one forked event per branch");
    for event in &forked {
        assert_eq!(
            event.data.get("parentTokenId").and_then(Value::as_str),
            Some(fork.id.as_str()),
            "{HARNESS}: the durable forked event names the fork token as parent"
        );
    }

    // Joining preserves the same lineage: the continuation descends from the fork, not from a branch.
    let main_task = task_at(&harness, &instance, MAIN_NODE);
    let review_task = task_at(&harness, &instance, REVIEW_NODE);
    complete(&harness, &main_task.id).expect("the first branch completes");
    complete(&harness, &review_task.id).expect("the second branch completes the join");
    let result = tokens(&harness, &instance)
        .into_iter()
        .find(|t| t.node_id == END_NODE)
        .expect("the join's result token exists");
    assert_eq!(
        result.parent_token_id.as_deref(),
        Some(fork.id.as_str()),
        "{HARNESS}: the join's continuation descends from the fork it joined over"
    );
    // The fork-grandchildren relationship is intact: result + two branches are exactly the fork's children.
    let mut children_of_fork: Vec<String> = tokens(&harness, &instance)
        .into_iter()
        .filter(|t| t.parent_token_id.as_deref() == Some(fork.id.as_str()))
        .map(|t| t.id)
        .collect();
    children_of_fork.sort();
    let mut expected: Vec<String> = branches.iter().map(|t| t.id.clone()).collect();
    expected.push(result.id.clone());
    expected.sort();
    assert_eq!(
        children_of_fork, expected,
        "{HARNESS}: the fork's children are exactly its two branches and the join's continuation"
    );
    assert_eq!(
        harness
            .store()
            .with_tx(|tx| tx.get_instance(&instance))
            .expect("the instance is readable")
            .status,
        ProcessStatus::Completed,
        "{HARNESS}: the preserved lineage does not impede convergence"
    );
}
