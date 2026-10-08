//! WF.DECISION — preferred transition override (TST-WF-DECISION-009).
//!
//! Contract: an explicitly preferred transition beats EVERY arm — the
//! decision takes the named transition without evaluating any condition,
//! even when the facts match a different arm, even when the facts match no
//! arm at all. A preferred name that matches no transition is refused, and
//! with no preferred transition the arms decide exactly as before.
//!
//! This file exercises the production boundary, not a re-declaration of it:
//! the real `WorkflowEngine<MemoryStore>` runs the real `evaluate_decision`
//! with `preferred = Some(..)` (`middle/workflow/src/engine/
//! execute_node_leave.rs:351-358`, which returns before any arm is read)
//! through the public `signal_token` API, and the winner is read back from
//! the durable `token.moved` transition and the resting token through the
//! production `Store`. It is L0 Pure on `WorkflowHarness`: no database, no
//! provider.
//!
//! REACHABILITY NOTE (honest, load-bearing): `start_process` and the leave
//! path of every node type pass `preferred = None` — a decision reached by
//! flow always evaluates its arms. The preferred path is the operator
//! override: `signal_token` (and `complete_task`) forward
//! `transition_name` into `execute_node_leave`
//! (`engine_options.rs:517-525`), and they run against whichever node the
//! addressed token is parked on. A token is therefore positioned on the
//! decision node first — through the production store's own CAS `move_token`,
//! the same op the engine uses — and then driven with the real public
//! `signal_token`. No engine code is re-declared and no private seam is
//! opened: the boundary under test is `signal_token` → `execute_node_leave`
//! → `evaluate_decision(preferred)`, all production. (The parked human task
//! that positioned the token is left behind by the repositioning; the
//! assertions read the routing — transition, resting node, instance state —
//! never that leftover row.)
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__009__preferred_transition_override

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::value::obj;
use workflow::{
    DecisionArm, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessGraph, ProcessOutcome,
    ProcessStatus, SignalTokenParams, StartProcessParams, TokenStatus, TransitionDefinition, Value,
};

const HARNESS: &str = "WorkflowHarness/L0 Pure";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const TASK_NODE: &str = "wait";
const TASK_TRANSITION: &str = "submit";
const DECIDE: &str = "decide";
const KEY: &str = "TST-WF-DECISION-009";

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// `start -> wait (task, submit -> decide) -> decide -> end_*`. The task parks
/// the run so a token can be addressed; the decision's arms are `go` (facts
/// below match it) and `other`.
fn definition() -> ProcessDefinition {
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
            name: Some("Wait".to_string()),
            transitions: Some(vec![transition(TASK_TRANSITION, DECIDE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        DECIDE.to_string(),
        NodeDefinition {
            id: DECIDE.to_string(),
            node_type: "decision".to_string(),
            decisions: Some(vec![
                DecisionArm {
                    condition: "status == \"open\"".to_string(),
                    transition: "go".to_string(),
                },
                DecisionArm {
                    condition: "status == \"closed\"".to_string(),
                    transition: "other".to_string(),
                },
            ]),
            transitions: Some(vec![
                transition("go", "end_go"),
                transition("other", "end_other"),
            ]),
            ..Default::default()
        },
    );
    for end in ["end_go", "end_other"] {
        nodes.insert(
            end.to_string(),
            NodeDefinition {
                id: end.to_string(),
                node_type: "end".to_string(),
                name: Some(end.to_string()),
                outcome: Some(ProcessOutcome::Completed),
                ..Default::default()
            },
        );
    }
    ProcessDefinition {
        id: format!("{KEY}-def"),
        tenant_id: None,
        key: KEY.to_string(),
        version: DEFINITION_VERSION,
        name: KEY.to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: "start".to_string(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

fn seeded() -> EngineHarness {
    let harness = EngineHarness::new(TestClock::at_unix_millis(1_700_000_090_000));
    harness
        .engine()
        .seed_definition(definition())
        .expect("definition registers");
    harness
}

/// Start one run (it parks on the task), position its active token on the
/// decision node through the store's own CAS move, and hand back the
/// instance id and the repositioned token id.
fn park_on_decision(harness: &EngineHarness) -> (String, String) {
    let started = harness
        .engine()
        .start_process(StartProcessParams {
            definition_key: KEY.to_string(),
            version: Some(DEFINITION_VERSION),
            business_key: None,
            variables: Value::object(),
            started_by: STARTED_BY.to_string(),
            tenant_id: None,
            subject: None,
        })
        .expect("the run starts and parks on its task");
    let instance_id = started.process_instance_id;
    let token = harness
        .store()
        .with_tx(|tx| tx.tokens_for_instance(&instance_id))
        .expect("the run's tokens are readable")
        .into_iter()
        .find(|token| token.node_id == TASK_NODE && matches!(token.status, TokenStatus::Active))
        .expect("the run parks an active token on the task");
    let moved = harness
        .store()
        .with_tx(|tx| tx.move_token(&token.id, token.version, DECIDE))
        .expect("the store positions the token on the decision");
    assert!(
        moved,
        "{HARNESS}: the CAS move to the decision node must apply"
    );
    (instance_id, token.id)
}

/// Drive the positioned token through the public override API.
fn signal(
    harness: &EngineHarness,
    token_id: &str,
    preferred: Option<&str>,
    vars: Value,
) -> workflow::Result<()> {
    harness.engine().signal_token(SignalTokenParams {
        token_id: token_id.to_string(),
        transition_name: preferred.map(str::to_string),
        variables: vars,
        actor: STARTED_BY.to_string(),
    })
}

/// The transition the decision actually took, read from the newest
/// `token.moved` event.
fn decision_transition(harness: &EngineHarness, instance_id: &str) -> Option<String> {
    harness
        .engine()
        .history(instance_id, 200)
        .expect("history readable")
        .into_iter()
        .filter(|e| e.event_type == "token.moved")
        .max_by_key(|e| e.id)
        .and_then(|e| {
            e.data
                .get("transition")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
}

/// The node the token came to rest on.
fn resting_node(harness: &EngineHarness, instance_id: &str) -> Vec<String> {
    harness
        .store()
        .with_tx(|tx| tx.tokens_for_instance(instance_id))
        .expect("tokens readable")
        .into_iter()
        .map(|t| t.node_id)
        .collect()
}

#[test]
#[allow(non_snake_case)]
fn wf_decision_009__preferred_transition_override() {
    let harness = seeded();
    // Facts matching the `go` arm — the override must beat them.
    let open = obj([("status", Value::from("open"))]);

    // 1. THE OVERRIDE BEATS A MATCHING ARM — preferred `other` wins although
    //    the facts satisfy the `go` arm. The token rests on the overridden
    //    destination and the run completes through it.
    let (overridden_id, overridden_token) = park_on_decision(&harness);
    signal(&harness, &overridden_token, Some("other"), open.clone())
        .expect("the preferred transition drives the parked decision");
    assert_eq!(
        decision_transition(&harness, &overridden_id).as_deref(),
        Some("other"),
        "{HARNESS}: the preferred transition wins over the matching arm"
    );
    assert!(
        resting_node(&harness, &overridden_id)
            .iter()
            .any(|n| n == "end_other"),
        "{HARNESS}: the override routed to end_other"
    );
    let overridden = harness
        .store()
        .with_tx(|tx| tx.get_instance(&overridden_id))
        .expect("the overridden instance is readable");
    assert_eq!(
        overridden.status,
        ProcessStatus::Completed,
        "{HARNESS}: the overridden run completes through the preferred transition"
    );

    // 2. THE OVERRIDE BEATS NO-MATCH — facts satisfy no arm and every
    //    transition is guarded (no otherwise exists), yet preferred `go`
    //    still routes instead of refusing. The override outranks arms AND the
    //    refusal.
    let (nomatch_id, nomatch_token) = park_on_decision(&harness);
    signal(
        &harness,
        &nomatch_token,
        Some("go"),
        obj([("status", Value::from("stuck"))]),
    )
    .expect("the preferred transition answers an otherwise-answerless decision");
    assert_eq!(
        decision_transition(&harness, &nomatch_id).as_deref(),
        Some("go"),
        "{HARNESS}: the preferred transition wins with no matching arm"
    );
    assert!(
        resting_node(&harness, &nomatch_id)
            .iter()
            .any(|n| n == "end_go"),
        "{HARNESS}: the override routed to end_go"
    );

    // 3. NEGATIVE — a preferred name that matches NO transition is refused by
    //    name, and nothing moves: the token stays active on the decision and
    //    the instance stays active. The override cannot invent a destination.
    let (refused_id, refused_token) = park_on_decision(&harness);
    let error = signal(&harness, &refused_token, Some("sideways"), open.clone())
        .expect_err("an unknown preferred transition must be refused");
    assert!(
        error
            .to_string()
            .contains("No valid transition from decision node"),
        "{HARNESS}: the refusal names the decision, got {error}"
    );
    let stuck: Vec<workflow::Token> = harness
        .store()
        .with_tx(|tx| tx.tokens_for_instance(&refused_id))
        .expect("the refused run's tokens are readable");
    assert!(
        stuck
            .iter()
            .any(|token| token.node_id == DECIDE && matches!(token.status, TokenStatus::Active)),
        "{HARNESS}: the refused override left the token active on the decision"
    );
    let refused = harness
        .store()
        .with_tx(|tx| tx.get_instance(&refused_id))
        .expect("the refused instance is readable");
    assert_eq!(
        refused.status,
        ProcessStatus::Active,
        "{HARNESS}: the refused override left the run active"
    );

    // 4. CONTROL — with no preferred transition the arms decide exactly as
    //    before: status=open takes `go`. The override is what changed the
    //    outcomes above, not the graph.
    let (control_id, control_token) = park_on_decision(&harness);
    signal(&harness, &control_token, None, open.clone()).expect("the arms still decide");
    assert_eq!(
        decision_transition(&harness, &control_id).as_deref(),
        Some("go"),
        "{HARNESS}: without a preferred transition the matching arm wins"
    );
    assert!(
        resting_node(&harness, &control_id)
            .iter()
            .any(|n| n == "end_go"),
        "{HARNESS}: the control routed to end_go"
    );
}
