//! WF.DECISION — numeric comparisons (TST-WF-DECISION-005: numeric comparisons).
//!
//! Contract: a `decision` node routes on the bounded condition DSL `identifier WS (==|!=) WS literal`
//! (`middle/workflow/src/expr.rs:1`). A **numeric comparison** is exactly the comparison the DSL can make on a
//! number:
//!
//! - a numeric literal is any right-hand side `s` for which `s.parse::<i64>()` or `s.parse::<f64>()` succeeds,
//!   and it is stored as `s.parse::<f64>()` (`parse_literal`, `middle/workflow/src/expr.rs:73-79`);
//! - the comparison is exact IEEE-754 `f64` equality (`json_eq`, `middle/workflow/src/expr.rs:88`).
//!
//! The production evaluator is `evaluate_condition` (`middle/workflow/src/expr.rs:10-28`), called once per arm
//! by `WorkflowEngine::evaluate_decision` (`middle/workflow/src/engine/execute_node_leave.rs:347-373`, the call
//! at `:361`); the arm whose condition is true wins, and when no arm matches the decision has no answer and is
//! refused (`:368-370`, `:531-550`), never routed to an invented branch.
//!
//! Numeric comparison has four load-bearing properties, each proved from both the pure evaluator and the real
//! engine:
//!
//! - **Value, not spelling.** `3`, `3.0`, `3.00`, `3e0` and `+3` are the same `f64` and compare equal; `-7`
//!   equals `-7.0`; `1000` equals `1e3`; `0.0` equals `-0.0`. A reader that compared the literal text (or
//!   coerced through a decimal type) would split these.
//! - **Exact `f64`, no tolerance.** Two different `f64` values are unequal (`1.5` vs `1.51`), `f64` precision is
//!   the contract (`9007199254740993` rounds to `9007199254740992.0`, so it equals that value), `NaN` differs
//!   from itself (IEEE-754 `==`), and an overflowing literal such as `1e400` is `infinity`, which equals
//!   `infinity`.
//! - **Type-strict.** A number equals only a number: `count == "3"`, `count == true` and `count == null` are all
//!   false, with no string→number coercion in either direction.
//! - **Equality only.** The DSL has no ordering and no arithmetic: `count > 3`, `count <= 3`, `count + 1 == 4`
//!   and friends are REFUSED with the production `EXPRESSION` error, and a decision arm that uses one is
//!   evaluated as false (fail-closed) rather than routing work.
//!
//! Level L0 Pure, harness `WorkflowHarness`. Inputs are literal numbers and literal `Value::Number`s; the
//! outputs are the production parser's and engine's own. Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__005__numeric_comparisons

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::value::obj;
use workflow::{
    evaluate_condition, is_supported_expression, DecisionArm, DefinitionStatus, NodeDefinition,
    ProcessDefinition, ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams,
    StartProcessResult, TransitionDefinition, Value,
};

const HARNESS: &str = "WorkflowHarness/L0 Pure";
/// The definition version every routing graph registers with the engine.
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
/// The decision node every graph routes into, the arm names and the end node each arm reaches.
const DECIDE_NODE: &str = "decide";
const GO_ARM: &str = "go";
const OTHER_ARM: &str = "other";
const WRONG_ARM: &str = "wrong";
const GO_END: &str = "end_go";
const OTHER_END: &str = "end_other";
const WRONG_END: &str = "end_wrong";

/// The deterministic variable map every numeric clause is evaluated against. Each entry is a number or a
/// non-number that a coercing comparator might treat as a number (`status` = the string `"3"`, `approved` =
/// the boolean `true`, `flag` = `null`), so a comparison that coerced across JSON types has somewhere to be
/// caught. `big` is exactly 2^53, the first integer an `f64` cannot step past one at a time; `nan` and `inf`
/// are the two IEEE-754 specials the literal parser can also name.
fn variables() -> Value {
    obj([
        ("count", Value::from(3)),               // integer spelling of 3
        ("count_f", Value::Number(3.0)),         // float spelling of 3
        ("ratio", Value::from(1.5)),
        ("half", Value::Number(0.5)),
        ("five", Value::Number(5.0)),
        ("neg", Value::from(-7)),
        ("zero", Value::Number(0.0)),
        ("neg_zero", Value::Number(-0.0)),
        ("thousand", Value::from(1000)),
        ("tiny", Value::Number(0.001)),
        ("point_one", Value::Number(0.1)),
        ("big", Value::Number(9_007_199_254_740_992.0)),
        ("nan", Value::Number(f64::NAN)),
        ("inf", Value::Number(f64::INFINITY)),
        ("status", Value::from("3")),            // the string spelling of a number
        ("approved", Value::Bool(true)),
        ("flag", Value::Null),
    ])
}

/// Evaluate a condition the test expects to be supported; a refusal here is itself a failure of the subject.
fn condition(expression: &str, vars: &Value) -> bool {
    evaluate_condition(expression, vars)
        .unwrap_or_else(|error| panic!("{HARNESS}: {expression:?} must evaluate, got: {error}"))
}

/// Evaluate a condition the test expects to be refused and return the production error, so its code and
/// message can be asserted rather than swallowed.
fn refusal(expression: &str, vars: &Value) -> workflow::WorkflowError {
    evaluate_condition(expression, vars).expect_err(&format!(
        "{HARNESS}: {expression:?} is not a bounded numeric comparison and must be refused"
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

/// `start -> decide (decision: the given arms) -> the given end nodes`. No otherwise is ever added: a decision
/// either matches a numeric arm by being exactly true or refuses.
fn definition_for(
    key: &str,
    arms: Vec<DecisionArm>,
    transitions: Vec<TransitionDefinition>,
) -> ProcessDefinition {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        "start".to_string(),
        NodeDefinition {
            id: "start".to_string(),
            node_type: "start".to_string(),
            transitions: Some(vec![transition("begin", DECIDE_NODE)]),
            ..Default::default()
        },
    );
    nodes.insert(
        DECIDE_NODE.to_string(),
        NodeDefinition {
            id: DECIDE_NODE.to_string(),
            node_type: "decision".to_string(),
            decisions: Some(arms),
            transitions: Some(transitions),
            ..Default::default()
        },
    );
    for id in [GO_END, OTHER_END, WRONG_END] {
        nodes.entry(id.to_string()).or_insert_with(|| NodeDefinition {
            id: id.to_string(),
            node_type: "end".to_string(),
            name: Some(id.to_string()),
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

/// Start a routing graph with the supplied variables.
fn start_with(harness: &EngineHarness, key: &str, vars: Value) -> workflow::Result<StartProcessResult> {
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

/// Register a routing graph and return the harness that owns it.
fn seeded(
    clock: i64,
    key: &str,
    arms: Vec<DecisionArm>,
    transitions: Vec<TransitionDefinition>,
) -> EngineHarness {
    let harness = EngineHarness::new(TestClock::at_unix_millis(clock));
    harness
        .engine()
        .seed_definition(definition_for(key, arms, transitions))
        .expect("the numeric decision definition registers with the engine");
    harness
}

/// Start a graph and return the terminal node the numeric arm chose, asserting the process completed.
fn route(harness: &EngineHarness, key: &str, vars: Value) -> Vec<String> {
    let started = start_with(harness, key, vars)
        .expect("a numeric arm must select a route for a matching numeric fact");
    let instance = harness
        .store()
        .with_tx(|tx| tx.get_instance(&started.process_instance_id))
        .expect("the instance is readable");
    assert_eq!(
        instance.status,
        ProcessStatus::Completed,
        "{HARNESS}: the selected numeric arm drove the process to completion"
    );
    resting_node(harness, &instance.id)
}

/// Start a graph with a fact no numeric arm matches and assert the decision REFUSED rather than routing it to
/// an invented branch.
fn refuse(harness: &EngineHarness, key: &str, vars: Value) -> workflow::WorkflowError {
    let error = start_with(harness, key, vars).expect_err(
        "an unmatched numeric fact must be refused, not routed to an invented branch",
    );
    assert!(
        error
            .to_string()
            .contains("No valid transition from decision node"),
        "{HARNESS}: the refusal names the decision with no numeric match: {error}"
    );
    error
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name; the file and the assay use it.
fn wf_decision_005__numeric_comparisons() {
    let vars = variables();

    // 1. VALUE, NOT SPELLING. Integer, decimal, exponent and leading-plus spellings of the same `f64` compare
    //    equal; a different value does not. Every true case has a false neighbour that differs only in value, so
    //    a comparator that read the literal text, or rounded through a fixed decimal type, would fail a row.
    for (expression, expected) in [
        ("count == 3", true),
        ("count == 3.0", true),
        ("count == 3.00", true),
        ("count == 3.000", true),
        ("count == 3e0", true),
        ("count == +3", true),
        ("count_f == 3", true),
        ("count_f == 3.0", true),
        ("ratio == 1.5", true),
        ("ratio == 1.50", true),
        ("ratio == 1.5e0", true),
        ("half == 0.5", true),
        ("half == .5", true),
        ("five == 5.", true),
        ("zero == 0", true),
        ("zero == 0.0", true),
        ("zero == -0.0", true),
        ("neg_zero == 0", true),
        ("neg == -7", true),
        ("neg == -7.0", true),
        ("neg == -7e0", true),
        ("thousand == 1e3", true),
        ("thousand == 1E3", true),
        ("thousand == 1000.0", true),
        ("tiny == 1e-3", true),
        ("tiny == 0.001", true),
        ("count == 4", false),
        ("count == 2.9999", false),
        ("count_f == 3.5", false),
        ("ratio == 1.4", false),
        ("ratio == 1.51", false),
        ("neg == 7", false),
        ("neg == -6.999", false),
        ("zero == 1", false),
        ("thousand == 1e2", false),
        ("tiny == 0.01", false),
    ] {
        assert_eq!(
            condition(expression, &vars),
            expected,
            "{HARNESS}: numeric equality is by value: {expression:?} must be {expected}"
        );
    }

    // 1b. Inequality is the exact complement of equality over the same numeric value, so `!=` cannot pass by
    //     being an unrelated constant.
    for (eq_expr, ne_expr, ne_expected) in [
        ("count == 3", "count != 3", false),
        ("count == 3.0", "count != 3.0", false),
        ("count == 4", "count != 4", true),
        ("neg == -7", "neg != -7", false),
        ("neg == 7", "neg != 7", true),
        ("ratio == 1.5", "ratio != 1.5", false),
        ("zero == -0.0", "zero != -0.0", false),
    ] {
        assert_eq!(
            condition(ne_expr, &vars),
            !condition(eq_expr, &vars),
            "{HARNESS}: {ne_expr:?} must be the exact complement of {eq_expr:?}"
        );
        assert_eq!(
            condition(ne_expr, &vars),
            ne_expected,
            "{HARNESS}: {ne_expr:?} on the fixture must be {ne_expected}"
        );
    }

    // 2. THE NUMERIC LITERAL VOCABULARY. Integer, decimal, leading/trailing-dot, signed, exponent and special
    //    spellings are supported (`parse_literal`, `middle/workflow/src/expr.rs:73-79`); each is asserted to be
    //    both supported and evaluable, so the gate and the evaluator cannot disagree about what a number is.
    for (expression, expected) in [
        ("count == 3", true),
        ("count == 3.", true),
        ("count == +3", true),
        ("half == .5", true),
        ("five == 5.", true),
        ("neg == -7", true),
        ("neg == -7.0", true),
        ("thousand == 1e3", true),
        ("thousand == 1E3", true),
        ("thousand == 1.0e3", true),
        ("tiny == 1e-3", true),
        ("inf == inf", true),
        ("nan == nan", false), // a special value is still parsed as a number
    ] {
        assert!(
            is_supported_expression(expression),
            "{HARNESS}: {expression:?} names a numeric literal and is supported"
        );
        assert_eq!(
            condition(expression, &vars),
            expected,
            "{HARNESS}: supported numeric literal: {expression:?} must be {expected}"
        );
    }

    // 2b. NON-NUMBERS ARE REFUSED, never guessed. These are near-misses that a permissive number parser (one
    //     that stripped separators, accepted a radix prefix, or read a word) would accept; the production parser
    //     refuses every one with the `EXPRESSION` error.
    for expression in [
        "count == 1_000",  // digit separator
        "count == 0x3",    // radix prefix
        "count == 1,000",  // thousands comma
        "count == 1.2.3",  // two decimal points
        "count == 3 0",    // space inside the number
        "count == 3e",     // exponent with no digits
        "count == 3+",     // trailing operator
        "count == +",      // sign with no digits
        "count == -",      // sign with no digits
        "count == .",      // dot with no digits
        "count == 1f",     // typed suffix
        "count == 3px",    // unit suffix
    ] {
        assert!(
            !is_supported_expression(expression),
            "{HARNESS}: {expression:?} is not a number and is unsupported"
        );
        let error = refusal(expression, &vars);
        assert_eq!(
            error.code(),
            "EXPRESSION",
            "{HARNESS}: a non-number is refused with the expression code: {expression:?}"
        );
        assert!(
            error
                .to_string()
                .contains("Unsupported workflow expression"),
            "{HARNESS}: the refusal names the unsupported expression: {expression:?} — {error}"
        );
    }

    // 3. EQUALITY ONLY — ORDERING AND ARITHMETIC ARE REFUSED. The DSL compares; it does not order, add or
    //    compare ranges. Every one of these reads as a numeric comparison and none is a bounded `==`/`!=`, so
    //    each is refused with the production `EXPRESSION` error rather than answered. This is the negative core:
    //    a decision can never express `count > 3` as a route.
    for expression in [
        "count > 3",
        "count < 3",
        "count >= 3",
        "count <= 3",
        "count => 3",
        "count =< 3",
        "count <> 3",
        "count = 3",
        "count === 3",
        "count !== 3",
        "count == 1 + 2",
        "count + 1 == 4",
        "count - 1 == 2",
        "count * 1 == 3",
        "count / 1 == 3",
        "count % 2 == 1",
        "3 == count",
        "-3 == count",
        "-count == -3",
        "count between 1 and 5",
        "count == 3.0.0",
        "count == 1_000",
        "count == 0x3",
        "count == 3 0",
    ] {
        assert!(
            !is_supported_expression(expression),
            "{HARNESS}: {expression:?} is not a bounded numeric equality and is unsupported"
        );
        let error = refusal(expression, &vars);
        assert_eq!(
            error.code(),
            "EXPRESSION",
            "{HARNESS}: an ordering/arithmetic comparison is refused: {expression:?}"
        );
        assert!(
            error
                .to_string()
                .contains("Unsupported workflow expression"),
            "{HARNESS}: the refusal names the unsupported expression: {expression:?} — {error}"
        );
    }

    // 4. TYPE-STRICT. A number equals only a number. The string `"3"`, the boolean `true` and `null` are not
    //    the number `3`; and a string variable is not the number literal. Each is checked with its exact
    //    complement, so `!=` cannot pass by being an unrelated constant.
    for expression in [
        "count == \"3\"",   // number vs its quoted form
        "count == \"3.0\"", // number vs its quoted decimal form
        "count == \" 3\"",  // no trimming into a number
        "count == \"3 \"",  // no trimming into a number
        "count == true",    // number vs boolean
        "count == null",    // number vs null
        "count == \"3px\"", // number vs a string that merely starts numeric
        "status == 3",      // string variable vs number literal
        "status == 3.0",
        "approved == 1",    // boolean vs the number a truthy reading would use
        "approved == 0",
        "flag == 0",        // null vs zero
    ] {
        assert!(
            is_supported_expression(expression),
            "{HARNESS}: {expression:?} is a well-formed comparison and is supported"
        );
        assert!(
            !condition(expression, &vars),
            "{HARNESS}: a number must not be coerced across JSON types: {expression:?}"
        );
        let flipped = expression.replacen("==", "!=", 1);
        assert!(
            condition(&flipped, &vars),
            "{HARNESS}: the type mismatch makes {flipped:?} true"
        );
    }

    // 5. EXACT f64, NO TOLERANCE — AND THE IEEE-754 SPECIALS. The comparison is the `f64` `==` in `json_eq`
    //    (`middle/workflow/src/expr.rs:88`), so floating-point facts are the contract: a value past 2^53 rounds
    //    to a representable neighbour, two different `f64` values are unequal, `NaN` differs from itself, and a
    //    literal that overflows is `infinity`.
    for (expression, expected) in [
        // 2^53 is the first integer with no `f64` neighbour between it and 2^53 + 1; the literal 2^53 + 1
        // parses to exactly 2^53, so it equals `big`, while 2^53 + 2 is representable and does not.
        ("big == 9007199254740992", true),
        ("big == 9007199254740993", true),
        ("big == 9007199254740993.0", true),
        ("big == 9007199254740994", false),
        ("big != 9007199254740994", true),
        // two spellings of the same f64 round to the same value; a different f64 does not.
        ("point_one == 0.1", true),
        ("point_one == 0.10000000000000001", true),
        ("point_one == 0.1000000000000001", false),
        ("point_one != 0.1000000000000001", true),
        // NaN is never equal to anything, itself included (IEEE-754 `==`).
        ("nan == nan", false),
        ("nan != nan", true),
        ("nan == 1", false),
        ("nan != 1", true),
        // Infinity is equal to itself and to any literal that overflows to it; it is not a finite number.
        ("inf == inf", true),
        ("inf != inf", false),
        ("inf == -inf", false),
        ("inf == 1e400", true),
        ("inf != 1", true),
        ("inf == 1", false),
    ] {
        assert_eq!(
            condition(expression, &vars),
            expected,
            "{HARNESS}: f64 numeric semantics: {expression:?} must be {expected}"
        );
    }

    // 6. THE PARSER SEAMS AGREE. `is_supported_expression` (`middle/workflow/src/expr.rs:6-8`) and
    //    `evaluate_condition` (`:10-28`) are two public seams over one parser: supported exactly when the other
    //    can evaluate. A numeric spelling accepted by one and refused by the other would fail here.
    for expression in [
        "count == 3",
        "count == +3",
        "half == .5",
        "neg == -7.0",
        "thousand == 1e3",
        "tiny == 1e-3",
        "inf == inf",
        "nan == nan",
        "count == \"3\"",
        "status == 3",
        "count > 3",
        "count + 1 == 4",
        "count == 1_000",
        "3 == count",
    ] {
        assert_eq!(
            is_supported_expression(expression),
            evaluate_condition(expression, &vars).is_ok(),
            "{HARNESS}: the two parser seams must agree on {expression:?}"
        );
    }

    // 7. THE DECISION BOUNDARY — the real engine routes on the same numeric comparison.
    //
    // 7a. Two numeric arms and no otherwise: the arm the fact equals is the arm the process takes, an integer
    //     and the float spelling of the same number take the same arm, and a fact no arm equals is refused.
    const NUM_KEY: &str = "TST-WF-DECISION-005-NUMERIC";
    let num_harness = seeded(
        1_700_000_500_000,
        NUM_KEY,
        vec![
            DecisionArm {
                condition: "count == 3".to_string(),
                transition: GO_ARM.to_string(),
            },
            DecisionArm {
                condition: "count == 4".to_string(),
                transition: OTHER_ARM.to_string(),
            },
        ],
        vec![transition(GO_ARM, GO_END), transition(OTHER_ARM, OTHER_END)],
    );

    let passed = route(&num_harness, NUM_KEY, obj([("count", Value::from(3))]));
    assert!(
        passed.iter().any(|node| node == GO_END),
        "{HARNESS}: `count == 3` routed 3 to {GO_END}, got {passed:?}"
    );
    assert!(
        !passed.iter().any(|node| node == OTHER_END),
        "{HARNESS}: the `count == 4` arm was not taken for 3, got {passed:?}"
    );

    let passed = route(&num_harness, NUM_KEY, obj([("count", Value::Number(3.0))]));
    assert!(
        passed.iter().any(|node| node == GO_END),
        "{HARNESS}: the float spelling 3.0 is the same number as 3 and takes {GO_END}, got {passed:?}"
    );

    let passed = route(&num_harness, NUM_KEY, obj([("count", Value::from(4))]));
    assert!(
        passed.iter().any(|node| node == OTHER_END),
        "{HARNESS}: `count == 4` routed 4 to {OTHER_END}, got {passed:?}"
    );

    // A different number, the string spelling of 3, a boolean and null are each unequal to both arms: with
    // every transition guarded by an arm, the decision has no answer and refuses.
    refuse(&num_harness, NUM_KEY, obj([("count", Value::from(9))]));
    refuse(&num_harness, NUM_KEY, obj([("count", Value::from("3"))]));
    refuse(&num_harness, NUM_KEY, obj([("count", Value::Bool(true))]));
    refuse(&num_harness, NUM_KEY, obj([("count", Value::Null)]));
    refuse(&num_harness, NUM_KEY, obj([])); // absent is not zero

    // 7b. A negative numeric arm. `-7` and `-7.0` are the same number and take the arm; `7` is refused.
    const NEG_KEY: &str = "TST-WF-DECISION-005-NEGATIVE";
    let neg_harness = seeded(
        1_700_000_501_000,
        NEG_KEY,
        vec![DecisionArm {
            condition: "balance == -7".to_string(),
            transition: GO_ARM.to_string(),
        }],
        vec![transition(GO_ARM, GO_END)],
    );
    let passed = route(&neg_harness, NEG_KEY, obj([("balance", Value::from(-7))]));
    assert!(
        passed.iter().any(|node| node == GO_END),
        "{HARNESS}: -7 takes the negative arm, got {passed:?}"
    );
    let passed = route(&neg_harness, NEG_KEY, obj([("balance", Value::Number(-7.0))]));
    assert!(
        passed.iter().any(|node| node == GO_END),
        "{HARNESS}: -7.0 is the same number as -7, got {passed:?}"
    );
    refuse(&neg_harness, NEG_KEY, obj([("balance", Value::from(7))]));

    // 7c. An exponent literal routes at the boundary too, and `f64` equality is value-based there as well.
    const EXP_KEY: &str = "TST-WF-DECISION-005-EXPONENT";
    let exp_harness = seeded(
        1_700_000_502_000,
        EXP_KEY,
        vec![DecisionArm {
            condition: "total == 1e3".to_string(),
            transition: GO_ARM.to_string(),
        }],
        vec![transition(GO_ARM, GO_END)],
    );
    let passed = route(&exp_harness, EXP_KEY, obj([("total", Value::from(1000))]));
    assert!(
        passed.iter().any(|node| node == GO_END),
        "{HARNESS}: 1000 takes the `total == 1e3` arm, got {passed:?}"
    );
    let passed = route(&exp_harness, EXP_KEY, obj([("total", Value::Number(1000.0))]));
    assert!(
        passed.iter().any(|node| node == GO_END),
        "{HARNESS}: 1000.0 is the same number as 1e3, got {passed:?}"
    );
    refuse(&exp_harness, EXP_KEY, obj([("total", Value::from(999))]));

    // 7d. FAIL-CLOSED — an ordering arm is unevaluable and is treated as FALSE, never true. The first arm is
    //     `count > 3` (outside the DSL); the second is a genuine numeric equality. A matching fact must route
    //     through the valid arm, proving the malformed guard did not silently win; a fact the ordering arm would
    //     have matched, but the equality arm does not, is refused rather than routed.
    const ORDER_KEY: &str = "TST-WF-DECISION-005-ORDERING";
    let order_harness = seeded(
        1_700_000_503_000,
        ORDER_KEY,
        vec![
            DecisionArm {
                condition: "count > 3".to_string(),
                transition: WRONG_ARM.to_string(),
            },
            DecisionArm {
                condition: "count == 3".to_string(),
                transition: GO_ARM.to_string(),
            },
        ],
        vec![transition(WRONG_ARM, WRONG_END), transition(GO_ARM, GO_END)],
    );
    assert!(
        !is_supported_expression("count > 3"),
        "{HARNESS}: `>` is outside the DSL and is unsupported"
    );
    assert!(
        evaluate_condition("count > 3", &obj([("count", Value::from(99))])).is_err(),
        "{HARNESS}: an ordering predicate is an error, and the decision treats that error as false"
    );
    let passed = route(&order_harness, ORDER_KEY, obj([("count", Value::from(3))]));
    assert!(
        passed.iter().any(|node| node == GO_END),
        "{HARNESS}: the true numeric equality must win through the valid arm, got {passed:?}"
    );
    assert!(
        !passed.iter().any(|node| node == WRONG_END),
        "{HARNESS}: an unevaluable ordering predicate must never be taken as true, got {passed:?}"
    );
    refuse(&order_harness, ORDER_KEY, obj([("count", Value::from(99))]));

    // 7e. A decision whose ONLY arm is an ordering has no true predicate at all and is refused for every fact,
    //     so an ordering numeric comparison can never become a route.
    const ONLY_ORDER_KEY: &str = "TST-WF-DECISION-005-ONLYORDER";
    let only_order = seeded(
        1_700_000_504_000,
        ONLY_ORDER_KEY,
        vec![DecisionArm {
            condition: "count > 3".to_string(),
            transition: WRONG_ARM.to_string(),
        }],
        vec![transition(WRONG_ARM, WRONG_END)],
    );
    refuse(&only_order, ONLY_ORDER_KEY, obj([("count", Value::from(99))]));
    refuse(&only_order, ONLY_ORDER_KEY, obj([("count", Value::from(3))]));

    // 8. NON-VACUITY. The variable name, the variable value and the literal are each load-bearing: changing any
    //    one flips the numeric comparison. If `condition` were a constant, one of these would not move. The last
    //    two pins prove the equivalence across spellings is value-based, not a second comparison path.
    let count3 = obj([("count", Value::from(3))]);
    let count4 = obj([("count", Value::from(4))]);
    assert!(
        condition("count == 3", &count3),
        "{HARNESS}: the fixture is equal to 3"
    );
    assert!(
        !condition("count == 3", &count4),
        "{HARNESS}: a different value is not equal"
    );
    assert!(
        !condition("other == 3", &count3),
        "{HARNESS}: a different name is not equal"
    );
    assert!(
        !condition("count == 4", &count3),
        "{HARNESS}: a different literal is not equal"
    );
    assert!(
        condition("count != 3", &count4),
        "{HARNESS}: the changed value makes inequality true"
    );
    assert_eq!(
        condition("count == 3", &count3),
        condition("count == 3.0", &count3),
        "{HARNESS}: integer and decimal spellings are one comparison"
    );
    assert_eq!(
        condition("count == 3.0", &count3),
        condition("count == 3.00", &count3),
        "{HARNESS}: decimal spellings are one comparison"
    );
}
