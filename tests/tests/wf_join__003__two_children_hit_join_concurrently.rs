//! WF.JOIN — two children hit join concurrently (TST-WF-JOIN-003).
//!
//! Contract: when two required children arrive at the same join at the same time, exactly **one** join is produced —
//! one `token.joined` event, one result token. The loser of the race must roll back its attempt to fire the join,
//! never duplicate it, and never leave a partial (skipped siblings retired but no join, or join with no result
//! token) state behind. Convergence is to one legal durable state: the process Completed, both branches spent, one
//! downstream token.
//!
//! The race is real: two threads, joined at a rendezvous barrier, each complete their branch's task through the
//! production `WorkflowEngine::complete_task` on the shared, production `MemoryStore`. The store's single
//! transaction mutex means the two steps run one after another, but *which* one lands second — and therefore which
//! one fires the join — is decided by the scheduler, so the assertion must hold for either winner. That is the
//! same arbitration Neon performs with row locks; only the loser is visible either way, as the branch whose
//! arrival saw no work left to do.
//!
//! ```text
//! start -> fan (fork, two required branches)
//!            |-- approve (task) -> converge (join) -> settle (end)
//!            '-- review  (task) -> converge
//! ```
//!
//! Level: L4 Adversarial, harness `WorkflowHarness`. Deterministic and isolated: the barrier makes the race real,
//! the assertions make it count, fixed `TestClock`, in-memory store, no database.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_join__003__two_children_hit_join_concurrently

use std::collections::BTreeMap;
use std::sync::Barrier;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    CompleteTaskParams, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessEvent,
    ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams, TaskStatus, Token,
    TransitionDefinition, Value,
};

/// The canonical harness label for this level.
const HARNESS: &str = "WorkflowHarness/L4 Adversarial";
const DEFINITION_KEY: &str = "TST-WF-JOIN-003";
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
        id: "tst-wf-join-003".to_string(),
        tenant_id: None,
        key: DEFINITION_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: "TST WF.JOIN 003".to_string(),
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

fn instance_status(harness: &EngineHarness, instance: &str) -> ProcessStatus {
    harness
        .store()
        .with_tx(|tx| tx.get_instance(instance))
        .expect("the instance is readable")
        .status
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-JOIN-003); the file and the assay use it.
fn wf_join_003__two_children_hit_join_concurrently() {
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

    // Two threads meet at the barrier and both complete their branch task through the production engine.
    let barrier = Barrier::new(2);
    let main_id = main_task.id.clone();
    let review_id = review_task.id.clone();
    let (main_result, review_result) = std::thread::scope(|scope| {
        let engine = harness.engine();
        let main = scope.spawn(|| {
            barrier.wait();
            engine.complete_task(CompleteTaskParams {
                task_id: main_id.clone(),
                user_id: STARTED_BY.to_string(),
                form_data: Value::object(),
                transition_name: Some(BRANCH_TRANSITION.to_string()),
            })
        });
        let engine = harness.engine();
        let review = scope.spawn(|| {
            barrier.wait();
            engine.complete_task(CompleteTaskParams {
                task_id: review_id.clone(),
                user_id: STARTED_BY.to_string(),
                form_data: Value::object(),
                transition_name: Some(BRANCH_TRANSITION.to_string()),
            })
        });
        (
            main.join().expect("the main racer does not panic"),
            review.join().expect("the review racer does not panic"),
        )
    });
    main_result.expect("the main branch completes");
    review_result.expect("the review branch completes");

    // One legal durable state, no matter which racer landed second: exactly one join, exactly one downstream
    // token, both branches spent, the process completed.
    let joined = events_of_type(&harness, &instance, "token.joined");
    assert_eq!(
        joined.len(),
        1,
        "{HARNESS}: two children hitting the join together still produce exactly one join"
    );
    assert_eq!(
        joined[0]
            .data
            .get("branches")
            .and_then(Value::as_array)
            .map(|b| b.len()),
        Some(2),
        "{HARNESS}: the single join accounts for both children"
    );
    assert_eq!(
        events_of_type(&harness, &instance, "token.skipped").len(),
        0,
        "{HARNESS}: no branch is skipped when both arrive"
    );
    let tokens_after = tokens(&harness, &instance);
    let downstream: Vec<&Token> = tokens_after
        .iter()
        .filter(|t| t.node_id == END_NODE)
        .collect();
    assert_eq!(
        downstream.len(),
        1,
        "{HARNESS}: exactly one downstream token is created"
    );
    let fork_id = tokens_after
        .iter()
        .find(|t| t.node_id == FORK_NODE)
        .map(|t| t.id.clone());
    assert_eq!(
        downstream[0].parent_token_id, fork_id,
        "{HARNESS}: the downstream token belongs to the fork"
    );
    assert_eq!(
        task_at(&harness, &instance, MAIN_NODE).status,
        TaskStatus::Completed,
        "{HARNESS}: the main branch's task completed"
    );
    assert_eq!(
        task_at(&harness, &instance, REVIEW_NODE).status,
        TaskStatus::Completed,
        "{HARNESS}: the review branch's task completed"
    );
    assert_eq!(
        instance_status(&harness, &instance),
        ProcessStatus::Completed,
        "{HARNESS}: the joined process converged to completion"
    );
}
