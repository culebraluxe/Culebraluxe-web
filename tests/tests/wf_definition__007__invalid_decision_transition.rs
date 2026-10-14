//! WF.DEFINITION — invalid decision transition (TST-WF-DEFINITION-007).
//!
//! Contract: a decision routes by pairing each `<on>` arm's `transition` with a `<transition>` **name** declared on
//! the same node. `evaluate_decision` (`middle/workflow/src/engine/execute_node_leave.rs:346-372`) walks the arms in
//! order, returns the first arm whose condition holds **resolved to the transition of that name**, and — if no
//! condition holds — falls back to the node's OTHERWISE, the one transition no arm names (`otherwise_transition`,
//! `:542-551`). When neither an arm nor an otherwise resolves, it returns `None`, and the caller refuses
//! (`execute_node_leave.rs:51-58`):
//!
//! ```text
//! "No valid transition from decision node {id}"
//! ```
//!
//! That refusal is the contract. A decision that cannot name where to go is a graph the engine must not guess at —
//! and the reason it does not guess is recorded in production itself: `otherwise_transition` used to be the FIRST
//! transition, so a definition naming every branch answered an unknown fact by taking branch one. This file pins the
//! behaviour that replaced it.
//!
//! **An honest finding about where the check lives.** The parser validates `<transition to="…">` targets — an edge to
//! a node that does not exist is refused at parse time (`wf_definition__005__missing_target` owns that) — but it does
//! **not** validate that a decision arm names an existing transition. A decision arm pointing at nothing is
//! syntactically fine and reaches the graph. So this contract is enforced at the ENGINE boundary, not the parse
//! boundary, and this file proves both halves of that honestly:
//!
//! - the parse ACCEPTS an arm naming no transition (pinned, because a reader must not assume otherwise), and
//! - the engine REFUSES to route it, in every shape: an arm matching with no transition of that name, a decision
//!   whose every transition is guarded (no otherwise), and an arm whose condition holds on the first evaluation.
//!
//! The negative/fault case is the refusal clause: with the otherwise removed, a decision that cannot resolve
//! **fails the process rather than silently taking its first branch**.
//!
//! Level L0 Pure, harness `WorkflowHarness`. The real `WorkflowEngine<MemoryStore>` is driven through the production
//! decision boundary over the pure in-memory store — no database, no network, no live provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_definition__007__invalid_decision_transition

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::{
    DecisionArm, DefinitionStatus, NodeDefinition, ProcessDefinition, ProcessGraph, ProcessOutcome,
    ProcessStatus, StartProcessParams, StartProcessResult, TransitionDefinition, Value,
};

use forge::engine::xml::parse_process_definition_xml;

const HARNESS: &str = "WorkflowHarness/L0 Pure";
const DECIDE_NODE: &str = "decide";
const STARTED_BY: &str = "tst";
const DEFINITION_VERSION: i32 = 1;

/// `start -> decide -> take_left | take_right`. `arms` are the decision arms and `transitions` the edges the node
/// declares; passing only the edges the arms name leaves the decision with no otherwise, so an unmatched fact is a
/// refusal rather than an invented branch.
fn definition_for(
    key: &str,
    arms: Vec<DecisionArm>,
    transitions: Vec<TransitionDefinition>,
) -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "start".to_string(),
        NodeDefinition {
            id: "start".into(),
            node_type: "start".into(),
            transitions: Some(vec![transition("begin", DECIDE_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        DECIDE_NODE.to_string(),
        NodeDefinition {
            id: DECIDE_NODE.into(),
            node_type: "decision".into(),
            decisions: Some(arms),
            transitions: Some(transitions),
            ..Default::default()
        },
    );
    for id in ["end_left", "end_right"] {
        nodes.insert(
            id.to_string(),
            NodeDefinition {
                id: id.into(),
                node_type: "end".into(),
                name: Some(id.into()),
                outcome: Some(ProcessOutcome::Completed),
                ..Default::default()
            },
        );
    }
    ProcessDefinition {
        id: format!("{key}-def"),
        tenant_id: None,
        key: key.into(),
        version: DEFINITION_VERSION,
        name: key.into(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: "start".into(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.into(),
        to: to.into(),
        condition: None,
        required: None,
    }
}

fn arm(condition: &str, transition: &str) -> DecisionArm {
    DecisionArm {
        condition: condition.into(),
        transition: transition.into(),
    }
}

fn seeded(
    key: &str,
    arms: Vec<DecisionArm>,
    transitions: Vec<TransitionDefinition>,
) -> EngineHarness {
    let harness = EngineHarness::new(TestClock::at_unix_millis(1_700_000_900_000));
    harness
        .engine()
        .seed_definition(definition_for(key, arms, transitions))
        .expect("definition registers");
    harness
}

fn start_with(
    harness: &EngineHarness,
    key: &str,
    vars: Value,
) -> workflow::Result<StartProcessResult> {
    harness.engine().start_process(StartProcessParams {
        definition_key: key.into(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: vars,
        started_by: STARTED_BY.into(),
        tenant_id: None,
        subject: None,
    })
}

fn resting_nodes(harness: &EngineHarness, instance_id: &str) -> Vec<String> {
    harness
        .store()
        .with_tx(|tx| tx.tokens_for_instance(instance_id))
        .expect("tokens readable")
        .into_iter()
        .map(|t| t.node_id)
        .collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-DEFINITION-007); the file and the assay use it.
fn wf_definition_007__invalid_decision_transition() {
    // 1. THE VALID PAIR RESOLVES. An arm names a transition, that transition exists, the condition holds, and the
    //    engine follows that edge. Without this, a refusal clause could be satisfied by an engine that never routes
    //    at all.
    const KEY_OK: &str = "TST-WF-DEFINITION-007-OK";
    let ok = seeded(
        KEY_OK,
        vec![arm("flag == true", "left"), arm("flag == false", "right")],
        vec![
            transition("left", "end_left"),
            transition("right", "end_right"),
        ],
    );
    for (value, expected, other) in [
        (true, "end_left", "end_right"),
        (false, "end_right", "end_left"),
    ] {
        let started = start_with(
            &ok,
            KEY_OK,
            workflow::value::obj([("flag", Value::Bool(value))]),
        )
        .expect("a resolvable decision routes");
        let instance = ok
            .store()
            .with_tx(|tx| tx.get_instance(&started.process_instance_id))
            .expect("readable");
        assert_eq!(
            instance.status,
            ProcessStatus::Completed,
            "{HARNESS}: the {value} arm drove the process to completion"
        );
        let passed = resting_nodes(&ok, &instance.id);
        assert!(
            passed.iter().any(|n| n == expected),
            "{HARNESS}: flag={value} routed to {expected}, got {passed:?}"
        );
        assert!(
            !passed.iter().any(|n| n == other),
            "{HARNESS}: flag={value} did not also route to {other}, got {passed:?}"
        );
    }

    // 2. THE OTHERWISE — THE UNGUARDED TRANSITION. With every fact unmatched, the decision takes the transition no
    //    arm names. This is the clause that makes the refusal below meaningful: without an otherwise there is
    //    nothing to fall back to, and `otherwise_transition` returns None by design.
    const KEY_OTHERWISE: &str = "TST-WF-DEFINITION-007-OTHERWISE";
    let otherwise = seeded(
        KEY_OTHERWISE,
        vec![arm("flag == true", "left")],
        vec![
            transition("left", "end_left"),
            // `fallback` is named by no arm, so it is the otherwise.
            transition("fallback", "end_right"),
        ],
    );
    let started = start_with(&otherwise, KEY_OTHERWISE, workflow::value::obj([]))
        .expect("an unmatched fact falls to the otherwise");
    let instance = otherwise
        .store()
        .with_tx(|tx| tx.get_instance(&started.process_instance_id))
        .expect("readable");
    let passed = resting_nodes(&otherwise, &instance.id);
    assert!(
        passed.iter().any(|n| n == "end_right"),
        "{HARNESS}: an unmatched fact took the otherwise transition, got {passed:?}"
    );
    assert!(
        !passed.iter().any(|n| n == "end_left"),
        "{HARNESS}: the guarded transition must NOT be taken on an unmatched fact, got {passed:?}"
    );

    // 3. THE OTHERWISE IS THE UNGUARDED ONE, NOT THE FIRST. In the fixture above the GUARDED transition `left` is
    //    declared first and the otherwise `fallback` second, and step 2's unmatched fact still took `fallback`. This
    //    clause pins the declaration order so it cannot pass by accident, and states the production bug this
    //    behaviour replaced: `otherwise_transition` used to be the FIRST transition, so Forge's `qa_failure_route` —
    //    whose only unguarded transition is `hold`, by design — sent every QA failure back to Smith with no bound.
    let otherwise_definition = otherwise
        .store()
        .with_tx(|tx| tx.load_definition(KEY_OTHERWISE, Some(DEFINITION_VERSION), None))
        .expect("the seeded definition is readable");
    let declared: Vec<&str> = otherwise_definition.definition.nodes[DECIDE_NODE]
        .transitions
        .as_deref()
        .unwrap()
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(
        declared,
        vec!["left", "fallback"],
        "{HARNESS}: the guarded transition is declared FIRST and the otherwise SECOND"
    );
    assert_eq!(
        declared[0], "left",
        "{HARNESS}: the first-declared transition is the GUARDED one, so taking 'the first' would be the bug"
    );
    // And the control: a fact the guard DOES answer takes that first-declared transition, so the fixture's two
    // answers are distinguishable and step 2's `fallback` cannot be an accident of a constant.
    let guarded_fact = start_with(
        &otherwise,
        KEY_OTHERWISE,
        workflow::value::obj([("flag", Value::Bool(true))]),
    )
    .expect("the guarded fact routes");
    let instance = otherwise
        .store()
        .with_tx(|tx| tx.get_instance(&guarded_fact.process_instance_id))
        .expect("readable");
    assert!(
        resting_nodes(&otherwise, &instance.id)
            .iter()
            .any(|n| n == "end_left"),
        "{HARNESS}: the answered fact took the guarded first transition, so the two clauses differ"
    );

    // 4. THE SUBJECT, SHAPE A — AN ARM NAMES A TRANSITION THAT DOES NOT EXIST. The condition holds, the arm matches,
    //    and the resolution finds nothing, so the engine refuses instead of inventing a branch.
    const KEY_DANGLING_ARM: &str = "TST-WF-DEFINITION-007-DANGLING-ARM";
    let dangling_arm = seeded(
        KEY_DANGLING_ARM,
        vec![arm("flag == true", "no_such_transition")],
        // Only `fallback` exists, and it is the otherwise — so the arm's own branch is unresolvable while the
        // decision would otherwise still have somewhere to go.
        vec![transition("fallback", "end_right")],
    );
    let refused = start_with(
        &dangling_arm,
        KEY_DANGLING_ARM,
        workflow::value::obj([("flag", Value::Bool(true))]),
    )
    .expect_err("{HARNESS}: an arm naming no transition must be refused");
    let message = refused.to_string();
    assert!(
        message.contains("No valid transition from decision node"),
        "{HARNESS}: the refusal is the production one: {message}"
    );
    assert!(
        message.contains(DECIDE_NODE),
        "{HARNESS}: the refusal names the decision node: {message}"
    );
    // AND IT DOES NOT SILENTLY TAKE THE OTHERWISE. The arm matched, so the resolution never reaches the otherwise —
    // a matched-but-unresolvable arm is a refusal, not a fall-through. This is the half a naive implementation gets
    // wrong by trying the otherwise after a failed resolution.
    assert!(
        !message.contains("end_right"),
        "{HARNESS}: the failure is a refusal, not a silent route elsewhere: {message}"
    );
    // The control: the same graph with a fact that does NOT match the dangling arm takes the otherwise, proving the
    // graph is otherwise sound and the refusal above was the arm.
    let fell_through = start_with(&dangling_arm, KEY_DANGLING_ARM, workflow::value::obj([]))
        .expect("an unmatched fact takes the otherwise in this same graph");
    let instance = dangling_arm
        .store()
        .with_tx(|tx| tx.get_instance(&fell_through.process_instance_id))
        .expect("readable");
    assert!(
        resting_nodes(&dangling_arm, &instance.id)
            .iter()
            .any(|n| n == "end_right"),
        "{HARNESS}: the same graph routes fine when no arm matches"
    );

    // 5. THE SUBJECT, SHAPE B — EVERY TRANSITION IS GUARDED, SO THERE IS NO OTHERWISE AND NOTHING TO MATCH. A fact
    //    no arm answers is then a refusal. This is the negative/fault case in its purest form: the decision is
    //    syntactically fine and every edge it declares resolves, yet a plausible fact cannot be routed — and the
    //    engine says so rather than picking a branch.
    const KEY_NO_OTHERWISE: &str = "TST-WF-DEFINITION-007-NO-OTHERWISE";
    let no_otherwise = seeded(
        KEY_NO_OTHERWISE,
        vec![arm("flag == true", "left")],
        vec![transition("left", "end_left")],
    );
    for vars in [
        workflow::value::obj([]),
        workflow::value::obj([("flag", Value::from("something else"))]),
        workflow::value::obj([("flag", Value::from(42))]),
    ] {
        let refused = start_with(&no_otherwise, KEY_NO_OTHERWISE, vars.clone()).expect_err(&format!(
            "{HARNESS}: with every transition guarded, {vars:?} must be refused rather than routed to 'left'"
        ));
        assert!(
            refused
                .to_string()
                .contains("No valid transition from decision node"),
            "{HARNESS}: the refusal is the production one for {vars:?}: {refused}"
        );
    }
    // And the same graph DOES route a fact its arm answers, so the refusal is about the fact, not the graph.
    let answered = start_with(
        &no_otherwise,
        KEY_NO_OTHERWISE,
        workflow::value::obj([("flag", Value::Bool(true))]),
    )
    .expect("the arm's own fact is answered");
    let instance = no_otherwise
        .store()
        .with_tx(|tx| tx.get_instance(&answered.process_instance_id))
        .expect("readable");
    assert!(
        resting_nodes(&no_otherwise, &instance.id)
            .iter()
            .any(|n| n == "end_left"),
        "{HARNESS}: the arm's fact routes to end_left"
    );

    // 6. AN ARM NAMES A TRANSITION *NAME*, NOT A NODE ID — they are different namespaces. Naming a real NODE that is
    //    not a declared transition resolves to nothing, so it is refused; naming the declared transition routes.
    //    A parser or engine that conflated the two would let an author write `transition="end_left"` and get a
    //    route that happens to look right.
    const KEY_NAMESPACE: &str = "TST-WF-DEFINITION-007-NAMESPACE";
    let namespace = seeded(
        KEY_NAMESPACE,
        vec![arm("flag == true", "end_left")],
        vec![transition("left", "end_left")],
    );
    let refused = start_with(
        &namespace,
        KEY_NAMESPACE,
        workflow::value::obj([("flag", Value::Bool(true))]),
    )
    .expect_err("{HARNESS}: a NODE id is not a transition name");
    assert!(
        refused
            .to_string()
            .contains("No valid transition from decision node"),
        "{HARNESS}: naming a node where a transition is expected is refused: {refused}"
    );
    // `end_left` IS a declared node, which is what makes this the bypass it is — the target exists, but the edge
    //    named by the arm does not.
    let definition = namespace
        .store()
        .with_tx(|tx| tx.load_definition(KEY_NAMESPACE, Some(DEFINITION_VERSION), None))
        .expect("the seeded definition is readable");
    assert!(
        definition.definition.nodes.contains_key("end_left"),
        "{HARNESS}: the node named by the arm really does exist — only the transition name is wrong"
    );

    // 7. THE PARSE BOUNDARY, STATED HONESTLY: an arm naming nothing parses. The parser checks `<transition to>`
    //    targets, not decision arm names, so a broken arm reaches the graph and is caught by the engine instead.
    //    Pinned so a reader of this file does not assume the fault is caught earlier.
    let parsed = parse_process_definition_xml(&format!(
        "<process-definition key=\"TST-WF-DEFINITION-007\" version=\"1\" name=\"Dangling arm\">\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"{DECIDE_NODE}\"/>\n  </start-state>\n  <decision id=\"{DECIDE_NODE}\">\n    <on condition=\"flag == true\" transition=\"no_such_transition\"/>\n    <transition name=\"fallback\" to=\"end_left\"/>\n  </decision>\n  <end-state id=\"end_left\"/>\n</process-definition>"
    ))
    .expect("an arm naming no transition PARSES — the parser does not check decision arm names");
    let decision = &parsed.graph.nodes[DECIDE_NODE];
    assert_eq!(
        decision.decisions.as_ref().map(|d| d.len()),
        Some(1),
        "{HARNESS}: the dangling arm really is in the parsed graph"
    );
    assert_eq!(
        decision.decisions.as_ref().unwrap()[0].transition,
        "no_such_transition",
        "{HARNESS}: the parsed arm carries the unresolvable name"
    );
    assert!(
        decision.decisions.as_ref().unwrap()[0].transition
            != decision.transitions.as_ref().unwrap()[0].name,
        "{HARNESS}: the arm names something the node's transitions do not"
    );
    // The positive half, so the clause is not merely "the parser is lax": an arm naming a real transition parses too.
    let sound = parse_process_definition_xml(&format!(
        "<process-definition key=\"TST-WF-DEFINITION-007\" version=\"1\" name=\"Sound arm\">\n  <start-state id=\"start\">\n    <transition name=\"begin\" to=\"{DECIDE_NODE}\"/>\n  </start-state>\n  <decision id=\"{DECIDE_NODE}\">\n    <on condition=\"flag == true\" transition=\"left\"/>\n    <transition name=\"left\" to=\"end_left\"/>\n  </decision>\n  <end-state id=\"end_left\"/>\n</process-definition>"
    ))
    .expect("a sound arm parses");
    assert_eq!(
        sound.graph.nodes[DECIDE_NODE].decisions.as_ref().unwrap()[0].transition,
        "left",
        "{HARNESS}: the sound arm names the declared transition"
    );

    // 8. NON-VACUITY. The same resolver returns a route and a refusal on real graphs, and the difference is exactly
    //    whether an arm resolves — not the presence of a decision, and not the fact that an instance was started.
    assert!(
        start_with(
            &no_otherwise,
            KEY_NO_OTHERWISE,
            workflow::value::obj([("flag", Value::Bool(true))])
        )
        .is_ok(),
        "{HARNESS}: an answered fact routes"
    );
    assert!(
        start_with(&no_otherwise, KEY_NO_OTHERWISE, workflow::value::obj([])).is_err(),
        "{HARNESS}: an unanswered fact on the SAME graph is refused"
    );
}
