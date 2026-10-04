//! WF.DECISION — boolean (TST-WF-DECISION-003).
//!
//! Contract: a `decision` node routes on the **boolean** its condition evaluates to. The condition DSL
//! (`middle/workflow/src/expr.rs`) compares a variable to a literal under exact JSON-type equality, and the evaluator
//! `evaluate_condition` (`middle/workflow/src/expr.rs:10-28`) returns that comparison as a real `Result<bool>`. The
//! decision arm selects only when the predicate is **exactly `true`**: `WorkflowEngine::evaluate_decision`
//! (`middle/workflow/src/engine/execute_node_leave.rs:347-373`) consumes the boolean at `:361`
//! (`evaluate_condition(&d.condition, variables).unwrap_or(false)`), the first arm whose boolean is true wins, and a
//! decision with no true arm and no OTHERWISE is refused rather than routed to an invented branch
//! (`middle/workflow/src/engine/execute_node_leave.rs:368-370` and `:531-550`).
//!
//! The boolean subject is proved from both ends:
//!
//! - **L0 Pure — the evaluator.** Boolean literals are exactly the two lowercase words `true` and `false`
//!   (`parse_literal`, `middle/workflow/src/expr.rs:60-81`, `:62-63`). Boolean identity is exact (`json_eq`,
//!   `:83-91`, `:86`): a boolean is equal to a boolean of the same value and to nothing else — no boolean/number
//!   coercion, no boolean/string coercion, no "truthy" reading of a non-boolean. Any other spelling of true/false is
//!   not a literal at all and is REFUSED with the production `EXPRESSION` error, never guessed into a truth value.
//! - **The decision boundary.** The real `WorkflowEngine<MemoryStore>` is driven through a `decision` node whose arms
//!   are boolean predicates: the arm whose boolean is true is the arm the process takes, the boolean false takes the
//!   other arm, a non-boolean fact takes neither and is refused, and first-match is the rule when two predicates are
//!   both true. A predicate that cannot be evaluated is treated as **false** — a decision never takes an arm on a
//!   predicate that is not exactly true, so a malformed guard cannot silently route work. The engine runs over the
//!   pure in-memory store: no database, no network, no provider, no external I/O.
//!
//! The two parser seams are pinned to agree: `is_supported_expression` (`expr.rs:6-8`) says an expression is
//! supported exactly when `evaluate_condition` can evaluate it, so a boolean spelling one accepts and the other
//! refuses fails here rather than in production.
//!
//! Level L0 Pure, harness `WorkflowHarness`. Inputs are literal booleans and literal strings; the outputs are the
//! production parser's and engine's own. Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__003__boolean

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::value::obj;
use workflow::{
    evaluate_condition, is_supported_expression, DecisionArm, DefinitionStatus, NodeDefinition,
    ProcessDefinition, ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams,
    StartProcessResult, TransitionDefinition, Value,
};

const HARNESS: &str = "WorkflowHarness/L0 Pure";
/// The definition key and version every routing graph registers with the engine.
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
/// The decision node every graph routes into, and the start->decision transition.
const DECIDE_NODE: &str = "decide";
const BEGIN: &str = "begin";

/// The deterministic variable map every boolean clause is evaluated against. Both boolean values are present, and a
/// value of every other JSON type carries the spelling a boolean coercion would reach for (`"true"` as a string, `1`
/// as a number, `null` for absence), so a comparison that coerced across types has somewhere to be caught.
fn variables() -> Value {
    obj([
        ("approved", Value::Bool(true)),
        ("rejected", Value::Bool(false)),
        ("status", Value::from("true")),
        ("count", Value::from(1)),
        ("flag", Value::Null),
    ])
}

/// Evaluate a condition the test expects to be supported; a refusal here is itself a failure of the subject.
fn condition(expression: &str, vars: &Value) -> bool {
    evaluate_condition(expression, vars)
        .unwrap_or_else(|error| panic!("{HARNESS}: {expression:?} must evaluate, got: {error}"))
}

/// Evaluate a condition the test expects to be refused and return the production error, so its code and message can
/// be asserted rather than swallowed.
fn refusal(expression: &str, vars: &Value) -> workflow::WorkflowError {
    evaluate_condition(expression, vars).expect_err(&format!(
        "{HARNESS}: {expression:?} is not a boolean literal and must be refused"
    ))
}

fn transition(name: &str, to: &str) -> TransitionDefinition {
    TransitionDefinition {
        name: name.to_string(),
        to: to.to_string(),
        condition: None,
        required: None,
    }
}

/// `start -> decide -> end*`. Each arm is `(condition, transition name, destination end node)`; every destination
/// becomes an `end` node, and the destination of an arm is what proves which boolean won. No otherwise transition is
/// ever added: a boolean decision either matches an arm by being exactly true or refuses.
fn boolean_definition(key: &str, arms: &[(&str, &str, &str)]) -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "start".to_string(),
        NodeDefinition {
            id: "start".to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition(BEGIN, DECIDE_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        DECIDE_NODE.to_string(),
        NodeDefinition {
            id: DECIDE_NODE.to_string(),
            node_type: "decision".to_string(),
            decisions: Some(
                arms.iter()
                    .map(|(condition, name, _)| DecisionArm {
                        condition: (*condition).to_string(),
                        transition: (*name).to_string(),
                    })
                    .collect(),
            ),
            transitions: Some(
                arms.iter()
                    .map(|(_, name, to)| transition(name, to))
                    .collect(),
            ),
            ..Default::default()
        },
    );
    for (_, _, to) in arms {
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

/// Start a routing graph with `flag` bound to `value`, exercising the boolean predicate that fact selects.
fn start_with_flag(
    harness: &EngineHarness,
    key: &str,
    value: Value,
) -> workflow::Result<StartProcessResult> {
    harness.engine().start_process(StartProcessParams {
        definition_key: key.to_string(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: obj([("flag", value)]),
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    })
}

/// Start a routing graph with no `flag` at all: the predicate has nothing to compare, so it must not be true.
fn start_without_flag(harness: &EngineHarness, key: &str) -> workflow::Result<StartProcessResult> {
    harness.engine().start_process(StartProcessParams {
        definition_key: key.to_string(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: obj([]),
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    })
}

/// The nodes the process's token came to rest at, read through the production store.
fn resting_node(harness: &EngineHarness, instance_id: &str) -> Vec<String> {
    harness
        .store()
        .with_tx(|tx| tx.tokens_for_instance(instance_id))
        .expect("the instance tokens are readable")
        .into_iter()
        .map(|token| token.node_id)
        .collect()
}

/// Register a boolean routing graph and return the harness that owns it.
fn seeded(clock: i64, key: &str, arms: &[(&str, &str, &str)]) -> EngineHarness {
    let harness = EngineHarness::new(TestClock::at_unix_millis(clock));
    harness
        .engine()
        .seed_definition(boolean_definition(key, arms))
        .expect("the boolean decision definition registers with the engine");
    harness
}

/// Start a graph seeded by [`seeded`] and return the terminal node the boolean chose.
fn route(harness: &EngineHarness, key: &str, value: Value) -> Vec<String> {
    let started = start_with_flag(harness, key, value)
        .expect("a boolean arm must select a route for a boolean fact");
    let instance = harness
        .store()
        .with_tx(|tx| tx.get_instance(&started.process_instance_id))
        .expect("the instance is readable");
    assert_eq!(
        instance.status,
        ProcessStatus::Completed,
        "{HARNESS}: the selected boolean arm drove the process to completion"
    );
    resting_node(harness, &instance.id)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-DECISION-003); the file and the assay use it.
fn wf_decision_003__boolean() {
    let vars = variables();

    // 1. THE PREDICATE IS A BOOLEAN, AND BOOLEAN IDENTITY IS EXACT. A boolean variable equals the boolean literal of
    //    the same value and is not equal to the other one; `!=` is its exact complement. The absolute values are
    //    pinned so an inequality that was a constant would still fail.
    for (expression, expected) in [
        ("approved == true", true),
        ("approved == false", false),
        ("rejected == true", false),
        ("rejected == false", true),
        ("approved != true", false),
        ("approved != false", true),
        ("rejected != true", true),
        ("rejected != false", false),
    ] {
        assert_eq!(
            condition(expression, &vars),
            expected,
            "{HARNESS}: boolean identity on the fixture: {expression:?} must be {expected}"
        );
    }

    // 2. THE BOOLEAN LITERALS ARE EXACTLY `true` AND `false`. Every other spelling is not a literal at all: the parse
    //    fails (`parse_literal`, `expr.rs:60-81`) and the evaluator refuses with the production `EXPRESSION` error
    //    rather than guessing a truth value. `is_supported_expression` and `evaluate_condition` are asserted to agree
    //    on every one of them.
    for expression in [
        "approved == TRUE",
        "approved == True",
        "approved == FALSE",
        "approved == False",
        "approved == yes",
        "approved == no",
        "approved == on",
        "approved == off",
        "approved == tru",
        "approved == fals",
    ] {
        assert!(
            !is_supported_expression(expression),
            "{HARNESS}: {expression:?} is not a boolean literal and is unsupported"
        );
        let error = refusal(expression, &vars);
        assert_eq!(
            error.code(),
            "EXPRESSION",
            "{HARNESS}: a non-boolean spelling is refused with the expression code: {expression:?}"
        );
        assert!(
            error
                .to_string()
                .contains("Unsupported workflow expression"),
            "{HARNESS}: the refusal names the unsupported expression: {expression:?} — {error}"
        );
    }

    // 3. LITERALS THAT LOOK BOOLEAN BUT ARE A DIFFERENT JSON TYPE PARSE, YET ARE NOT EQUAL TO A BOOLEAN. `"true"` is
    //    a string, `1`/`0` are numbers and `null` is null; each is supported and each comparison is false. This is
    //    the boolean half of exact JSON-type equality (`json_eq`, `expr.rs:83-91`).
    for expression in [
        "approved == \"true\"",
        "rejected == \"false\"",
        "approved == 1",
        "rejected == 0",
        "approved == null",
        "rejected == null",
    ] {
        assert!(
            is_supported_expression(expression),
            "{HARNESS}: {expression:?} is a well-formed comparison and is supported"
        );
        assert!(
            !condition(expression, &vars),
            "{HARNESS}: a non-boolean literal is not equal to a boolean: {expression:?}"
        );
    }

    // 4. NO CROSS-TYPE COERCION IN EITHER DIRECTION. A boolean variable is not equal to a string, a number or null;
    //    and a string or number variable is not equal to a boolean. Each clause is checked with its exact
    //    complement, so `!=` cannot pass by being an unrelated constant.
    for expression in [
        "approved == \"true\"", // bool vs its quoted form
        "approved == 1",        // bool vs the number a truthy reading would use
        "approved == 0",
        "approved == null", // bool vs null
        "rejected == \"false\"",
        "rejected == 1",
        "rejected == 0",
        "rejected == null",
        "status == true", // the string "true" is not the boolean true
        "status == false",
        "count == true", // the number 1 is not the boolean true
        "count == false",
        "flag == true",  // null is not true
        "flag == false", // null is not false
    ] {
        assert!(
            !condition(expression, &vars),
            "{HARNESS}: a boolean must not coerce across JSON types: {expression:?}"
        );
        let flipped = expression.replacen("==", "!=", 1);
        assert!(
            condition(&flipped, &vars),
            "{HARNESS}: the type mismatch makes {flipped:?} true"
        );
    }

    // 5. THE DECISION BOUNDARY CONSUMES THE BOOLEAN. The real engine routes on the same predicate.
    //    `start -> decide -> end_true|end_false`, with `decide` carrying two boolean arms and no otherwise.
    const BOOL_KEY: &str = "TST-WF-DECISION-003-BOOL";
    let pairs = [
        ("flag == true", "yes", "end_true"),
        ("flag == false", "no", "end_false"),
    ];
    let harness = seeded(1_700_000_000_000, BOOL_KEY, &pairs);

    // 5a. true takes the true arm and not the false one.
    let passed = route(&harness, BOOL_KEY, Value::Bool(true));
    assert!(
        passed.iter().any(|node| node == "end_true"),
        "{HARNESS}: `flag == true` must route a true flag to end_true, got {passed:?}"
    );
    assert!(
        !passed.iter().any(|node| node == "end_false"),
        "{HARNESS}: `flag == false` must not take a true flag, got {passed:?}"
    );

    // 5b. false takes the other arm — on a fresh instance, so the two starts cannot share a token.
    let passed = route(&harness, BOOL_KEY, Value::Bool(false));
    assert!(
        passed.iter().any(|node| node == "end_false"),
        "{HARNESS}: `flag == false` must route a false flag to end_false, got {passed:?}"
    );
    assert!(
        !passed.iter().any(|node| node == "end_true"),
        "{HARNESS}: `flag == true` must not take a false flag, got {passed:?}"
    );

    // 5c. NEGATIVE — a fact that is not a boolean makes neither boolean predicate true, so the decision has no legal
    //     branch and is REFUSED rather than coerced into an arm. A reader that treated `"true"` or `1` as true would
    //     complete here instead of failing.
    for not_a_boolean in [
        Value::from("true"),
        Value::from("false"),
        Value::from(1),
        Value::from(0),
        Value::Null,
    ] {
        let refused = start_with_flag(&harness, BOOL_KEY, not_a_boolean.clone()).expect_err(
            &format!("{HARNESS}: {not_a_boolean:?} is not a boolean and must be refused"),
        );
        assert!(
            refused
                .to_string()
                .contains("No valid transition from decision node"),
            "{HARNESS}: the refusal names the decision with no true arm: {refused}"
        );
    }

    // 5d. NEGATIVE — absence is not a boolean either, so an unbound flag is refused.
    start_without_flag(&harness, BOOL_KEY)
        .expect_err("a decision with no bound boolean has no true arm and must be refused");

    // 6. FAIL-CLOSED: a predicate that cannot be evaluated is FALSE, never true. The first arm is an unevaluable
    //    expression (`&&` is outside the bounded DSL); the second is a genuine boolean. A true fact must route
    //    through the *valid* arm — proving the malformed guard did not silently win.
    const FALLBACK_KEY: &str = "TST-WF-DECISION-003-FAILCLOSED";
    let fallback = seeded(
        1_700_000_001_000,
        FALLBACK_KEY,
        &[
            ("flag && true", "wrong", "end_wrong"),
            ("flag == true", "right", "end_right"),
        ],
    );
    assert!(
        !is_supported_expression("flag && true"),
        "{HARNESS}: `&&` is not a boolean comparison and is unsupported"
    );
    assert!(
        evaluate_condition("flag && true", &obj([("flag", Value::Bool(true))])).is_err(),
        "{HARNESS}: an unevaluable predicate is an error, and the decision treats that error as false"
    );
    let passed = route(&fallback, FALLBACK_KEY, Value::Bool(true));
    assert!(
        passed.iter().any(|node| node == "end_right"),
        "{HARNESS}: the true boolean must win through the valid arm, got {passed:?}"
    );
    assert!(
        !passed.iter().any(|node| node == "end_wrong"),
        "{HARNESS}: an unevaluable predicate must never be taken as true, got {passed:?}"
    );
    //    The same graph on a false fact: neither the unevaluable arm nor the equal arm is true, so it is refused.
    start_with_flag(&fallback, FALLBACK_KEY, Value::Bool(false))
        .expect_err("a false fact matches no true arm and must be refused");

    // 6b. A decision whose ONLY arm is unevaluable has no true predicate at all and is refused, not routed.
    const ONLY_BAD_KEY: &str = "TST-WF-DECISION-003-ONLYBAD";
    let only_bad = seeded(
        1_700_000_002_000,
        ONLY_BAD_KEY,
        &[("flag && true", "wrong", "end_wrong")],
    );
    start_with_flag(&only_bad, ONLY_BAD_KEY, Value::Bool(true))
        .expect_err("an unevaluable guard must not route true work to an invented branch");

    // 7. FIRST-MATCH: when two predicates are both true, the first listed wins — the boolean result, not the order of
    //    transitions, decides, and the engine stops at the first true arm.
    const ORDER_KEY: &str = "TST-WF-DECISION-003-ORDER";
    let both_true = seeded(
        1_700_000_003_000,
        ORDER_KEY,
        &[
            ("flag == true", "first", "end_first"),
            ("flag != false", "second", "end_second"),
        ],
    );
    let passed = route(&both_true, ORDER_KEY, Value::Bool(true));
    assert!(
        passed.iter().any(|node| node == "end_first"),
        "{HARNESS}: the first true predicate wins, got {passed:?}"
    );
    assert!(
        !passed.iter().any(|node| node == "end_second"),
        "{HARNESS}: a later true predicate must not be reached, got {passed:?}"
    );

    //    And the order does not override the boolean: when the first predicate is false the second true arm is taken.
    const ORDER2_KEY: &str = "TST-WF-DECISION-003-ORDER2";
    let second_true = seeded(
        1_700_000_004_000,
        ORDER2_KEY,
        &[
            ("flag == true", "first", "end_first"),
            ("flag != true", "second", "end_second"),
        ],
    );
    let passed = route(&second_true, ORDER2_KEY, Value::Bool(false));
    assert!(
        passed.iter().any(|node| node == "end_second"),
        "{HARNESS}: a false first predicate falls through to the next true arm, got {passed:?}"
    );
    assert!(
        !passed.iter().any(|node| node == "end_first"),
        "{HARNESS}: the false first predicate must not be taken, got {passed:?}"
    );

    // 8. NON-VACUITY. The variable name, the variable value and the literal are each load-bearing: changing any one
    //    flips the boolean. If `condition` were a constant, one of these would not move.
    let true_map = obj([("flag", Value::Bool(true))]);
    let false_map = obj([("flag", Value::Bool(false))]);
    assert!(
        condition("flag == true", &true_map),
        "{HARNESS}: a true boolean equals true"
    );
    assert!(
        !condition("flag == true", &false_map),
        "{HARNESS}: a false boolean does not equal true"
    );
    assert!(
        !condition("other == true", &true_map),
        "{HARNESS}: a different name is not equal"
    );
    assert!(
        condition("flag != true", &false_map),
        "{HARNESS}: a false boolean differs from true"
    );
    assert!(
        !condition("flag == false", &true_map),
        "{HARNESS}: the literal is load-bearing"
    );
}
