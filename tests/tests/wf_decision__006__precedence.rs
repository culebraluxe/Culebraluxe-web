//! WF.DECISION — precedence (TST-WF-DECISION-006).
//!
//! Contract: a decision node answers with the FIRST arm that matches, in the
//! order the arms are declared; only when no arm matches does it fall to its
//! OTHERWISE — the transition no arm names. A fully-guarded decision whose
//! facts match no arm has no answer and the engine REFUSES instead of inventing
//! one.
//!
//! Production boundary (`middle/workflow/src/engine/execute_node_leave.rs`):
//! - `evaluate_decision` `:347-373` is the precedence itself:
//!   * `:359-367` arms are scanned in declaration order and the FIRST arm whose
//!     `evaluate_condition` is true returns its transition — a later true arm
//!     never outranks an earlier one, and arm order, not transition order or
//!     "specificity", decides the winner;
//!   * `:361` an arm whose expression does not parse is `unwrap_or(false)`
//!     (`expr.rs:10-28`): it is skipped like a false arm and does not abort the
//!     scan, so the next well-formed matching arm decides;
//!   * `:368-369` when no arm matched it takes `otherwise_transition`;
//!   * `:372` only an empty `decisions` list falls through to the first transition.
//! - `otherwise_transition` `:531-550` — otherwise is the first transition that
//!   NO arm names, not the first transition; `None` when every transition is
//!   guarded by some arm.
//! - `:51-58` a `None` from `evaluate_decision` is refused with
//!   "No valid transition from decision node {id}" inside the same transaction,
//!   so a refused start leaves no instance behind (`memory.rs:38-53` rollback).
//!
//! The highest production precedence — an explicit preferred transition
//! (`:353-358`) beating every arm — is not reachable for a decision node through
//! the public engine surface (`start_process` passes `None`; a decision token
//! is never parked for `complete_task`/`signal_token` to name one), so this file
//! exercises the reachable chain arm-order > otherwise > refusal.
//!
//! This file owns the **precedence** half of WF.DECISION that no other
//! WF.DECISION file isolates: first-match arm ordering, the otherwise fallback,
//! the fully-guarded refusal, and an unparseable arm losing to the next valid
//! one. It is L0 Pure on
//! `WorkflowHarness` (`EngineHarness` + the production `WorkflowEngine<MemoryStore>`):
//! the real engine and the real parser, no database, no provider.
//!
//! CONTROL-PLANE NOTE (for the Captain, not a test concern): this story was
//! dispatched with canonical file `wf_decision__006__precedence.rs`, but the
//! taxonomy already carries `wf_decision__006__cross_type.rs` (landed `7307d1bb`).
//! Two subjects therefore share suffix 006. The content here is precedence and is
//! landed exactly as the story demands; renumbering in the control plane is
//! separate work and is not a reason to drop either test.
//!
//! Run: cargo test -p test-harness --test wf_decision__006__precedence -- --nocapture

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

/// `start -> decide(arms, transitions) -> end_*`. Every transition target becomes
/// a completed end node, so the node the token rests on names the winning arm.
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

fn instance(harness: &EngineHarness, started: &StartProcessResult) -> workflow::ProcessInstance {
    harness
        .store()
        .with_tx(|tx| tx.get_instance(&started.process_instance_id))
        .expect("instance readable after a routed start")
}

/// The node the token came to rest on — the end node the winning arm routed to.
fn resting_node(harness: &EngineHarness, instance_id: &str) -> Vec<String> {
    harness
        .store()
        .with_tx(|tx| tx.tokens_for_instance(instance_id))
        .expect("tokens readable")
        .into_iter()
        .map(|t| t.node_id)
        .collect()
}

/// The transition the decision actually took, read from the newest `token.moved`
/// event (history is newest-first, so `max_by_key(id)` is the last move).
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

struct Route {
    status: ProcessStatus,
    nodes: Vec<String>,
    transition: Option<String>,
}

fn routed(harness: &EngineHarness, key: &str, vars: Value) -> Route {
    let started = start_with(harness, key, vars).expect("decision should route");
    let inst = instance(harness, &started);
    Route {
        status: inst.status,
        nodes: resting_node(harness, &inst.id),
        transition: decision_transition(harness, &inst.id),
    }
}

fn seeded(key: &str, arms: &[(&str, &str)], transitions: &[(&str, &str)]) -> EngineHarness {
    let harness = EngineHarness::new(TestClock::at_unix_millis(1_700_000_200_000));
    harness
        .engine()
        .seed_definition(definition_for(key, arms, transitions))
        .expect("definition registers");
    harness
}

#[test]
#[allow(non_snake_case)]
fn wf_decision_006__precedence() {
    // ---------------------------------------------------------------------
    // 1. ARM ORDER PRECEDENCE — the first declared matching arm wins, even
    //    when a later arm is also true.
    // ---------------------------------------------------------------------
    // Arm 0 `status == "open"` and arm 1 `status != "closed"` are BOTH true for
    // status=open. The first declared arm must take it.
    const FIRST_KEY: &str = "TST-WF-DECISION-006-PRECEDENCE-FIRST";
    let first = seeded(
        FIRST_KEY,
        &[
            ("status == \"open\"", "go"),
            ("status != \"closed\"", "other"),
        ],
        &[("go", "end_go"), ("other", "end_other")],
    );
    let open = obj([("status", Value::from("open"))]);
    let both_true_first_arm = routed(&first, FIRST_KEY, open.clone());
    assert_eq!(
        both_true_first_arm.status,
        ProcessStatus::Completed,
        "{HARNESS}: decision process completes"
    );
    assert_eq!(
        both_true_first_arm.transition.as_deref(),
        Some("go"),
        "{HARNESS}: first declared matching arm wins when both match"
    );
    assert!(
        both_true_first_arm.nodes.iter().any(|n| n == "end_go"),
        "{HARNESS}: routed to end_go, got {:?}",
        both_true_first_arm.nodes
    );
    assert!(
        !both_true_first_arm.nodes.iter().any(|n| n == "end_other"),
        "{HARNESS}: the later matching arm did not win"
    );

    // The SAME two conditions, arms REORDERED. Arm order, not specificity or
    // transition order, now decides: arm 0 `status != "closed"` wins.
    const SECOND_KEY: &str = "TST-WF-DECISION-006-PRECEDENCE-SECOND";
    let second = seeded(
        SECOND_KEY,
        &[
            ("status != \"closed\"", "other"),
            ("status == \"open\"", "go"),
        ],
        &[("go", "end_go"), ("other", "end_other")],
    );
    let both_true_second_arm = routed(&second, SECOND_KEY, open.clone());
    assert_eq!(
        both_true_second_arm.transition.as_deref(),
        Some("other"),
        "{HARNESS}: reordering arms flips the winner — it is the first match"
    );
    assert!(
        both_true_second_arm.nodes.iter().any(|n| n == "end_other"),
        "{HARNESS}: reordered definition routed to end_other, got {:?}",
        both_true_second_arm.nodes
    );

    // A decision does not stop at arm 0: when the first arm is false it scans on
    // to the first true arm. `status == "open"` is false for status=closed, so
    // the later `status == "closed"` arm decides.
    const SCAN_KEY: &str = "TST-WF-DECISION-006-PRECEDENCE-SCAN";
    let scan = seeded(
        SCAN_KEY,
        &[
            ("status == \"open\"", "go"),
            ("status == \"closed\"", "other"),
        ],
        &[("go", "end_go"), ("other", "end_other")],
    );
    let closed = obj([("status", Value::from("closed"))]);
    let only_second = routed(&scan, SCAN_KEY, closed.clone());
    assert_eq!(
        only_second.transition.as_deref(),
        Some("other"),
        "{HARNESS}: a false first arm lets the scan reach the second matching arm"
    );
    assert!(
        only_second.nodes.iter().any(|n| n == "end_other"),
        "{HARNESS}: second arm alone routed to end_other"
    );

    // NEGATIVE / REFUSAL: neither arm matches and every transition is guarded,
    // so the decision has no answer and the start is refused, leaving no run.
    let refused = start_with(&first, FIRST_KEY, closed.clone())
        .expect_err("a fully-guarded decision with no match has no answer");
    assert!(
        refused
            .to_string()
            .contains("No valid transition from decision node"),
        "{HARNESS}: empty decision refuses by name, got {refused}"
    );

    // ---------------------------------------------------------------------
    // 2. OTHERWISE PRECEDENCE — no arm matched, so the transition NO arm names
    //    wins, not the first transition.
    // ---------------------------------------------------------------------
    // Shape from production: `qa_failure_route` — arms guard smith/architect and
    // `hold` is the one unguarded transition, listed LAST. A fact matching
    // neither eligibility must take `hold`, not the first transition `smith`.
    const OTHERWISE_KEY: &str = "TST-WF-DECISION-006-PRECEDENCE-OTHERWISE";
    let otherwise = seeded(
        OTHERWISE_KEY,
        &[
            ("qaRepairEligible == true", "smith"),
            ("qaReplanEligible == true", "architect"),
        ],
        &[
            ("smith", "end_smith"),
            ("architect", "end_architect"),
            ("hold", "end_hold"),
        ],
    );

    let no_eligibility = routed(&otherwise, OTHERWISE_KEY, obj([]));
    assert_eq!(
        no_eligibility.transition.as_deref(),
        Some("hold"),
        "{HARNESS}: no arm matched -> otherwise is the unguarded transition, not the first"
    );
    assert!(
        no_eligibility.nodes.iter().any(|n| n == "end_hold"),
        "{HARNESS}: otherwise routed to end_hold, got {:?}",
        no_eligibility.nodes
    );

    // A matching arm still outranks the otherwise.
    let repair = routed(
        &otherwise,
        OTHERWISE_KEY,
        obj([("qaRepairEligible", Value::Bool(true))]),
    );
    assert_eq!(
        repair.transition.as_deref(),
        Some("smith"),
        "{HARNESS}: first declared matching arm beats otherwise"
    );

    // Both arms true: first-match precedence again decides (smith over architect),
    // it does not fall to hold.
    let both = routed(
        &otherwise,
        OTHERWISE_KEY,
        obj([
            ("qaRepairEligible", Value::Bool(true)),
            ("qaReplanEligible", Value::Bool(true)),
        ]),
    );
    assert_eq!(
        both.transition.as_deref(),
        Some("smith"),
        "{HARNESS}: both eligibility arms true -> first arm wins over otherwise"
    );

    // The historical shape where the otherwise IS the first transition: the
    // unguarded `skip` remains the answer, unchanged by the rule.
    const OTHERWISE_FIRST_KEY: &str = "TST-WF-DECISION-006-PRECEDENCE-OTHERWISE-FIRST";
    let otherwise_first = seeded(
        OTHERWISE_FIRST_KEY,
        &[("inspectionRequired == true", "run")],
        &[("skip", "end_skip"), ("run", "end_run")],
    );
    let skipped = routed(&otherwise_first, OTHERWISE_FIRST_KEY, obj([]));
    assert_eq!(
        skipped.transition.as_deref(),
        Some("skip"),
        "{HARNESS}: otherwise that is also first routes as before"
    );
    let ran = routed(
        &otherwise_first,
        OTHERWISE_FIRST_KEY,
        obj([("inspectionRequired", Value::Bool(true))]),
    );
    assert_eq!(
        ran.transition.as_deref(),
        Some("run"),
        "{HARNESS}: guarded arm still beats the otherwise"
    );

    // ---------------------------------------------------------------------
    // 3. FULLY-GUARDED REFUSAL — every transition is named by an arm, no fact
    //    matches: the decision must refuse, not fall back to the first
    //    transition and not leave a half-built instance behind.
    // ---------------------------------------------------------------------
    const GUARDED_KEY: &str = "TST-WF-DECISION-006-PRECEDENCE-GUARDED";
    let guarded = seeded(
        GUARDED_KEY,
        &[
            ("leadDecision == 'SOLO'", "solo"),
            ("leadDecision == 'SMITH'", "smith"),
        ],
        &[("solo", "end_solo"), ("smith", "end_smith")],
    );

    // A declared value routes.
    let solo = routed(
        &guarded,
        GUARDED_KEY,
        obj([("leadDecision", Value::from("SOLO"))]),
    );
    assert_eq!(
        solo.transition.as_deref(),
        Some("solo"),
        "{HARNESS}: declared arm routes"
    );
    let smith = routed(
        &guarded,
        GUARDED_KEY,
        obj([("leadDecision", Value::from("SMITH"))]),
    );
    assert_eq!(
        smith.transition.as_deref(),
        Some("smith"),
        "{HARNESS}: second declared arm routes"
    );

    // An undeclared value and an absent fact both refuse.
    for undeclared in [obj([("leadDecision", Value::from("ARCHITECT"))]), obj([])] {
        let error = start_with(&guarded, GUARDED_KEY, undeclared.clone())
            .expect_err("fully-guarded no-match must refuse");
        assert!(
            error
                .to_string()
                .contains("No valid transition from decision node"),
            "{HARNESS}: {undeclared:?} refuses by name, got {error}"
        );
    }

    // The refusal rolled the whole step back: a following start with the same
    // definition still begins cleanly (no stale active instance was left).
    let after = routed(
        &guarded,
        GUARDED_KEY,
        obj([("leadDecision", Value::from("SOLO"))]),
    );
    assert_eq!(after.status, ProcessStatus::Completed);

    // ---------------------------------------------------------------------
    // 4. FAULT ARM PRECEDENCE — an arm whose condition does not parse never
    //    wins and does not abort the scan; the next well-formed matching arm
    //    decides, and the fault arm still counts as naming its transition for
    //    the otherwise rule.
    // ---------------------------------------------------------------------
    const FAULT_KEY: &str = "TST-WF-DECISION-006-PRECEDENCE-FAULT";
    let fault = seeded(
        FAULT_KEY,
        &[
            ("status == open", "go"), // bareword RHS: parse fails (expr.rs:60-81)
            ("status == \"open\"", "other"),
        ],
        &[
            ("go", "end_go"),
            ("other", "end_other"),
            ("fallback", "end_fallback"),
        ],
    );

    // status=open: arm 0 errors and is skipped, arm 1 matches -> other. The
    // unparseable arm must never be the precedence winner.
    let fault_open = routed(&fault, FAULT_KEY, open.clone());
    assert_eq!(
        fault_open.transition.as_deref(),
        Some("other"),
        "{HARNESS}: an unparseable arm loses to the next well-formed matching arm"
    );
    assert!(
        fault_open.nodes.iter().any(|n| n == "end_other"),
        "{HARNESS}: fault-skip routed to end_other, got {:?}",
        fault_open.nodes
    );

    // status=closed: arm 0 still errors, arm 1 false, and `go`/`other` are still
    // "named" by arms even though arm 0's expression is broken, so the only
    // unguarded transition is `fallback`.
    let fault_closed = routed(&fault, FAULT_KEY, obj([("status", Value::from("closed"))]));
    assert_eq!(
        fault_closed.transition.as_deref(),
        Some("fallback"),
        "{HARNESS}: a fault arm still names its transition, so otherwise is the remaining unguarded one"
    );
    assert!(
        fault_closed.nodes.iter().any(|n| n == "end_fallback"),
        "{HARNESS}: fault definition fell to end_fallback, got {:?}",
        fault_closed.nodes
    );

    // ---------------------------------------------------------------------
    // 5. NON-VACUITY — the fact value, not the shape of the graph, flips the
    //    winner between the two arms.
    // ---------------------------------------------------------------------
    assert_ne!(
        routed(&first, FIRST_KEY, obj([("status", Value::from("open"))])).transition,
        routed(&second, SECOND_KEY, obj([("status", Value::from("open"))])).transition,
        "{HARNESS}: arm order is the only difference and it changes the winner"
    );
    assert_eq!(
        routed(
            &otherwise,
            OTHERWISE_KEY,
            obj([("qaRepairEligible", Value::Bool(true))])
        )
        .transition
        .as_deref(),
        Some("smith"),
        "{HARNESS}: positive fact takes the arm"
    );
    assert_eq!(
        routed(
            &otherwise,
            OTHERWISE_KEY,
            obj([("qaRepairEligible", Value::Bool(false))])
        )
        .transition
        .as_deref(),
        Some("hold"),
        "{HARNESS}: same key, false value, falls to otherwise"
    );
}
