//! WF.DECISION — malformed condition (TST-WF-DECISION-007).
//!
//! Contract: an arm whose condition does not parse is NEVER a winner — it is
//! skipped like a false arm (`evaluate_condition` fails, `unwrap_or(false)`)
//! and the scan continues. When no well-formed arm matches, the decision
//! falls to its otherwise; when every transition is guarded and nothing
//! matched, the engine REFUSES instead of inventing an answer. A broken
//! expression can neither route nor abort the decision.
//!
//! This file exercises the production boundary, not a re-declaration of it:
//! the real `WorkflowEngine<MemoryStore>` and the real condition parser
//! (`middle/workflow/src/expr.rs`, whose grammar is `identifier WS (==|!=) WS
//! literal`) are driven through `start_process` with malformed arms, and the
//! routing is read back through the production `Store`. It is L0 Pure on
//! `WorkflowHarness` (`EngineHarness` + the production engine): no database,
//! no provider.
//!
//! The production lines this contract pins:
//! `middle/workflow/src/engine/execute_node_leave.rs:359-370` —
//! `evaluate_condition(...).unwrap_or(false)` skips the fault arm, the scan
//! continues, otherwise answers when nothing matched;
//! `:51-58` — a decision with no answer is refused with "No valid transition
//! from decision node {id}" inside the same transaction, leaving no run
//! behind. The companion file `wf_decision__006__precedence` owns the
//! precedence chain; this file owns the malformation rule across every shape
//! the grammar rejects.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__007__malformed_condition

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

/// Every shape the grammar rejects, each broken in a different place: empty,
/// bareword literal, missing literal, missing identifier, unbalanced quote,
/// and a single `=` that is no operator at all.
const MALFORMED: &[&str] = &[
    "",
    "status == open",
    "status ==",
    "== \"open\"",
    "status == \"open",
    "status = \"open\"",
];

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
    let harness = EngineHarness::new(TestClock::at_unix_millis(1_700_000_070_000));
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
fn wf_decision_007__malformed_condition() {
    let open = obj([("status", Value::from("open"))]);

    // 1. NO MALFORMED ARM EVER WINS — each rejected shape, alone as the only
    //    arm, loses to the otherwise. The otherwise here is `fallback`, the
    //    one transition no arm names; facts would satisfy the arm's APPARENT
    //    intent (status=open), so a boundary that routed on intent rather than
    //    grammar would take `go` and fail here.
    for (index, shape) in MALFORMED.iter().enumerate() {
        let key = format!("TST-WF-DECISION-007-SHAPE-{index}");
        let harness = seeded(
            &key,
            &[(shape, "go")],
            &[("go", "end_go"), ("fallback", "end_fallback")],
        );
        let started = start_with(&harness, &key, open.clone())
            .expect("a malformed arm must not abort the decision");
        assert_eq!(
            decision_transition(&harness, &started.process_instance_id).as_deref(),
            Some("fallback"),
            "{HARNESS}: malformed arm {shape:?} never wins; otherwise answers"
        );
        assert!(
            resting_node(&harness, &started.process_instance_id)
                .iter()
                .any(|n| n == "end_fallback"),
            "{HARNESS}: malformed arm {shape:?} routed to end_fallback"
        );
    }

    // 2. A malformed arm does not abort the scan: beside a matching
    //    well-formed arm, the well-formed one decides.
    const SCAN_KEY: &str = "TST-WF-DECISION-007-SCAN";
    let scan = seeded(
        SCAN_KEY,
        &[
            ("status == open", "go"), // bareword RHS: parse fails
            ("status == \"open\"", "other"),
        ],
        &[("go", "end_go"), ("other", "end_other")],
    );
    let routed = start_with(&scan, SCAN_KEY, open.clone()).expect("the scan reaches the valid arm");
    assert_eq!(
        decision_transition(&scan, &routed.process_instance_id).as_deref(),
        Some("other"),
        "{HARNESS}: the scan skips the fault arm and the matching valid arm decides"
    );

    // 3. FULLY-GUARDED REFUSAL — every transition is named by a malformed arm
    //    and no fact matches: the decision has no answer, the start is
    //    refused by name, and the refusal rolls back so no run is left behind.
    const GUARDED_KEY: &str = "TST-WF-DECISION-007-GUARDED";
    let guarded = seeded(
        GUARDED_KEY,
        &[("status == open", "go"), ("", "other")],
        &[("go", "end_go"), ("other", "end_other")],
    );
    let error = start_with(&guarded, GUARDED_KEY, open.clone())
        .expect_err("malformed-only fully-guarded arms must refuse");
    assert!(
        error
            .to_string()
            .contains("No valid transition from decision node"),
        "{HARNESS}: the refusal names the decision, got {error}"
    );
    let leftover: Vec<workflow::ProcessInstance> = guarded
        .store()
        .with_tx(|tx| tx.find_instances(None, None, Some(GUARDED_KEY), None, 128, 0))
        .expect("instances are listable");
    assert!(
        leftover.is_empty(),
        "{HARNESS}: the refused start committed no instance"
    );

    // 4. POSITIVE CONTROL — the same graph with the condition well-formed
    //    routes via the arm, so the otherwise/refusal above are about the
    //    malformation, not a broken graph.
    const CONTROL_KEY: &str = "TST-WF-DECISION-007-CONTROL";
    let control = seeded(
        CONTROL_KEY,
        &[("status == \"open\"", "go")],
        &[("go", "end_go"), ("fallback", "end_fallback")],
    );
    let controlled = start_with(&control, CONTROL_KEY, open.clone()).expect("the control routes");
    assert_eq!(
        decision_transition(&control, &controlled.process_instance_id).as_deref(),
        Some("go"),
        "{HARNESS}: the well-formed arm wins where every malformed shape lost"
    );
    let instance = control
        .store()
        .with_tx(|tx| tx.get_instance(&controlled.process_instance_id))
        .expect("the controlled instance is readable");
    assert_eq!(
        instance.status,
        ProcessStatus::Completed,
        "{HARNESS}: the control completes"
    );
}
