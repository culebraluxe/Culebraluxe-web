//! WF.DECISION — first-match semantics (TST-WF-DECISION-008).
//!
//! Contract: among the decision's arms, the WINNER is the first arm in
//! declaration order whose condition holds — even when several arms match.
//! Arm declaration order decides, not transition table order, not
//! "specificity", not position of the matching fact. The scan still continues
//! past false (and fault) arms to the first true one; it simply never looks
//! past the first true one.
//!
//! This file exercises the production boundary, not a re-declaration of it:
//! the real `WorkflowEngine<MemoryStore>` and the real condition parser are
//! driven through `start_process`, and the winner is read back from the
//! durable `token.moved` transition and the resting token through the
//! production `Store`. It is L0 Pure on `WorkflowHarness`: no database, no
//! provider.
//!
//! The production lines this contract pins:
//! `middle/workflow/src/engine/execute_node_leave.rs:359-367` — arms are
//! scanned in declaration order and the FIRST true arm returns its
//! transition. The companion file `wf_decision__006__precedence` owns the
//! precedence CHAIN (arm-order > otherwise > refusal); this file owns the
//! WITHIN-arms selection: which of several matching arms wins, and why only
//! declaration order can explain it.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__008__first_match_semantics

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::value::obj;
use workflow::{
    DecisionArm, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessGraph, ProcessOutcome,
    ProcessStatus, StartProcessParams, StartProcessResult, TransitionDefinition, Value,
};

const HARNESS: &str = "WorkflowHarness/L0 Pure";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const DECIDE: &str = "decide";

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// `start -> decide(arms, transitions) -> end_*`. Every transition target
/// becomes a completed end node, so the node the token rests on names the
/// winning arm.
fn definition_for(
    key: &str,
    arms: &[(&str, &str)],
    transitions: &[(&str, &str)],
) -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "start".to_string(),
        NodeDefinition {
            id: "start".to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition("begin", DECIDE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        DECIDE.to_string(),
        NodeDefinition {
            id: DECIDE.to_string(),
            node_type: "decision".to_string(),
            decisions: Some(
                arms.iter()
                    .map(|(condition, to)| DecisionArm {
                        condition: (*condition).to_string(),
                        transition: (*to).to_string(),
                    })
                    .collect(),
            ),
            transitions: Some(
                transitions
                    .iter()
                    .map(|(name, to)| transition(name, to))
                    .collect(),
            ),
            ..Default::default()
        },
    );
    for (_, to) in transitions {
        nodes
            .entry((*to).to_string())
            .or_insert_with(|| NodeDefinition {
                id: (*to).to_string(),
                node_type: "end".to_string(),
                name: Some((*to).to_string()),
                outcome: Some(ProcessOutcome::Completed),
                ..Default::default()
            });
    }
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

fn start_with(
    harness: &EngineHarness,
    key: &str,
    vars: Value,
) -> workflow::Result<StartProcessResult> {
    harness.engine().start_process(StartProcessParams {
        definition_key: key.to_string(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: vars,
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    })
}

fn seeded(key: &str, arms: &[(&str, &str)], transitions: &[(&str, &str)]) -> EngineHarness {
    let harness = EngineHarness::new(TestClock::at_unix_millis(1_700_000_080_000));
    harness
        .engine()
        .seed_definition(definition_for(key, arms, transitions))
        .expect("definition registers");
    harness
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
fn wf_decision_008__first_match_semantics() {
    // Facts under which ALL THREE arms below hold at once.
    let open = obj([("status", Value::from("open"))]);

    // 1. THREE MATCHING ARMS — the first declared wins, and the later two
    //    lose even though they match. The transitions are deliberately listed
    //    in the OPPOSITE order to the arms, so a boundary that read the
    //    transition table instead of the arm order would take `third` here.
    const TRIPLE_KEY: &str = "TST-WF-DECISION-008-TRIPLE";
    let triple = seeded(
        TRIPLE_KEY,
        &[
            ("status == \"open\"", "first"),
            ("status != \"closed\"", "second"),
            ("status != \"archived\"", "third"),
        ],
        &[
            ("third", "end_third"),
            ("second", "end_second"),
            ("first", "end_first"),
        ],
    );
    let all_match = start_with(&triple, TRIPLE_KEY, open.clone()).expect("three arms match");
    assert_eq!(
        decision_transition(&triple, &all_match.process_instance_id).as_deref(),
        Some("first"),
        "{HARNESS}: the first of three matching arms wins"
    );
    let nodes = resting_node(&triple, &all_match.process_instance_id);
    assert!(
        nodes.iter().any(|n| n == "end_first"),
        "{HARNESS}: routed to end_first, got {nodes:?}"
    );
    assert!(
        !nodes.iter().any(|n| n == "end_second" || n == "end_third"),
        "{HARNESS}: the later matching arms took nothing, got {nodes:?}"
    );
    let instance = triple
        .store()
        .with_tx(|tx| tx.get_instance(&all_match.process_instance_id))
        .expect("the triple instance is readable");
    assert_eq!(
        instance.status,
        ProcessStatus::Completed,
        "{HARNESS}: the first-match run completes"
    );

    // 2. THE SAME ARMS REORDERED flip the winner: arm order is the ONLY
    //    difference, so arm order is what decides.
    const REORDERED_KEY: &str = "TST-WF-DECISION-008-REORDERED";
    let reordered = seeded(
        REORDERED_KEY,
        &[
            ("status != \"archived\"", "third"),
            ("status != \"closed\"", "second"),
            ("status == \"open\"", "first"),
        ],
        &[
            ("third", "end_third"),
            ("second", "end_second"),
            ("first", "end_first"),
        ],
    );
    let flipped = start_with(&reordered, REORDERED_KEY, open.clone())
        .expect("the reordered arms still all match");
    assert_eq!(
        decision_transition(&reordered, &flipped.process_instance_id).as_deref(),
        Some("third"),
        "{HARNESS}: reordering arms flips the winner to the new first match"
    );

    // 3. THE SCAN CONTINUES PAST FALSE ARMS — first-match is not
    //    first-arm-always: with the first two arms false, the third (only)
    //    match wins regardless of its late position.
    const SCAN_KEY: &str = "TST-WF-DECISION-008-SCAN";
    let scan = seeded(
        SCAN_KEY,
        &[
            ("status == \"closed\"", "first"),
            ("status == \"archived\"", "second"),
            ("status == \"open\"", "third"),
        ],
        &[
            ("first", "end_first"),
            ("second", "end_second"),
            ("third", "end_third"),
        ],
    );
    let late = start_with(&scan, SCAN_KEY, open.clone()).expect("the late arm matches alone");
    assert_eq!(
        decision_transition(&scan, &late.process_instance_id).as_deref(),
        Some("third"),
        "{HARNESS}: false arms are passed over; the first TRUE arm wins from any position"
    );
    assert!(
        resting_node(&scan, &late.process_instance_id)
            .iter()
            .any(|n| n == "end_third"),
        "{HARNESS}: the late match routed to end_third"
    );

    // 4. FAULT — the scan continues past an arm that does not parse, and the
    //    first WELL-FORMED match still wins: a fault arm is skipped, never
    //    selected, and never aborts the selection.
    const FAULT_KEY: &str = "TST-WF-DECISION-008-FAULT";
    let fault = seeded(
        FAULT_KEY,
        &[
            ("status == open", "broken"), // bareword RHS: parse fails
            ("status == \"open\"", "first"),
            ("status != \"closed\"", "second"),
        ],
        &[
            ("broken", "end_broken"),
            ("first", "end_first"),
            ("second", "end_second"),
        ],
    );
    let past_fault =
        start_with(&fault, FAULT_KEY, open.clone()).expect("the scan survives the fault arm");
    assert_eq!(
        decision_transition(&fault, &past_fault.process_instance_id).as_deref(),
        Some("first"),
        "{HARNESS}: the fault arm is skipped and the first well-formed match wins"
    );
    assert!(
        !resting_node(&fault, &past_fault.process_instance_id)
            .iter()
            .any(|n| n == "end_broken"),
        "{HARNESS}: the fault arm's transition is never taken"
    );

    // 5. NON-VACUITY — the facts, not the graph shape, pick the winner: the
    //    same triple definition routes `first` for status=open and `third`
    //    for status=closed (only the third arm holds there).
    let closed = start_with(
        &triple,
        TRIPLE_KEY,
        obj([("status", Value::from("closed"))]),
    )
    .expect("status=closed matches only the third arm");
    assert_eq!(
        decision_transition(&triple, &closed.process_instance_id).as_deref(),
        Some("third"),
        "{HARNESS}: a fact matching only the third arm takes it"
    );
    assert_ne!(
        decision_transition(&triple, &all_match.process_instance_id),
        decision_transition(&triple, &closed.process_instance_id),
        "{HARNESS}: the fact value flips the winner between arms"
    );
}
