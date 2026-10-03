//! WF.DECISION — equality (TST-WF-DECISION-001).
//!
//! Contract: a `decision` node routes on the **bounded condition DSL**, and the only comparisons that DSL can make
//! are **equality** (`==`) and its exact complement **inequality** (`!=`). The production evaluator is
//! `evaluate_condition` (`middle/workflow/src/expr.rs:10-28`), called once per arm by
//! `WorkflowEngine::evaluate_decision` (`middle/workflow/src/engine/execute_node_leave.rs:359-373`, the call at
//! `:361`); the arm whose condition is true wins, and when no arm matches the decision falls to its unguarded
//! OTHERWISE or refuses (`middle/workflow/src/engine/execute_node_leave.rs:368-370` and `:531-550`). Equality is the
//! whole of the subject: the parser (`expr.rs:30-58`) accepts no operator other than `==`/`!=`, and the comparison
//! (`json_eq`, `expr.rs:83-91`) is **exact JSON-type equality**.
//!
//! The shape matters, and it is proved from both ends:
//!
//! - **L0 Pure — the evaluator.** `evaluate_condition` is driven directly with deterministic literal inputs. Equality
//!   holds exactly when the variable carries the same JSON type and the same value as the literal: no case folding,
//!   no trimming, no string/number coercion, no boolean/number coercion. A name the variable map does not hold is
//!   **not** `null` — absence is not equality — so `missing == null` is false and `missing != null` is true. Anything
//!   that is not an equality comparison is REFUSED (`EXPRESSION`), never silently answered.
//! - **The decision boundary.** The real `WorkflowEngine<MemoryStore>` is driven through a `decision` node whose arms
//!   are equality conditions, proving the arm an equal fact selects is the arm the process takes, that a non-equal
//!   fact takes the other arm, and that a fact equal to neither is refused rather than routed to an invented branch.
//!   The engine runs over the pure in-memory store: no database, no network, no provider, no external I/O.
//!
//! The parser's two seams are pinned to agree: `is_supported_expression` (`expr.rs:6-8`) says an expression is
//! supported exactly when `evaluate_condition` can evaluate it, so a future change to one that forgot the other
//! fails here rather than in production.
//!
//! Level L0 Pure, harness `WorkflowHarness`. Inputs are literal strings and literal `Value`s; the outputs are the
//! production parser's and engine's own. Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__001__equality

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::value::obj;
use workflow::{
    evaluate_condition, is_supported_expression, DecisionArm, DefinitionStatus, NodeDefinition,
    ProcessDefinition, ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams,
    StartProcessResult, TransitionDefinition, Value,
};

const HARNESS: &str = "WorkflowHarness/L0 Pure";
/// The definition key and version the routing half registers with the engine.
const DEFINITION_KEY: &str = "TST-WF-DECISION-001";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
/// The decision node, the two equality arms, and the end node each arm reaches.
const DECIDE_NODE: &str = "decide";
const HIGH_ARM: &str = "high";
const LOW_ARM: &str = "low";
const HIGH_END: &str = "end_high";
const LOW_END: &str = "end_low";

/// The deterministic variable map every equality clause is evaluated against. Each entry is a different JSON type, so
/// a clause that coerces across types has somewhere to be caught.
fn variables() -> Value {
    obj([
        ("approved", Value::Bool(true)),
        ("rejected", Value::Bool(false)),
        ("status", Value::from("open")),
        ("count", Value::from(3)),
        ("ratio", Value::from(1.5)),
        ("flag", Value::Null),
        ("tags", Value::from(vec!["a".to_string()])),
        ("meta", Value::object()),
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
        "{HARNESS}: {expression:?} is not an equality comparison and must be refused"
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

/// `start -> decide (decision: two equality arms) -> end_high | end_low`. Every transition is named by an arm, so the
/// decision has **no** otherwise: a fact equal to neither literal has no legal branch and is refused.
fn decision_definition() -> ProcessDefinition {
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
            decisions: Some(vec![
                DecisionArm {
                    condition: "level == \"high\"".to_string(),
                    transition: HIGH_ARM.to_string(),
                },
                DecisionArm {
                    condition: "level == \"low\"".to_string(),
                    transition: LOW_ARM.to_string(),
                },
            ]),
            transitions: Some(vec![
                transition(HIGH_ARM, HIGH_END),
                transition(LOW_ARM, LOW_END),
            ]),
            ..Default::default()
        },
    );
    for (id, name) in [(HIGH_END, "High"), (LOW_END, "Low")] {
        nodes.insert(
            id.to_string(),
            NodeDefinition {
                id: id.to_string(),
                node_type: "end".to_string(),
                name: Some(name.to_string()),
                outcome: Some(ProcessOutcome::Completed),
                ..Default::default()
            },
        );
    }
    ProcessDefinition {
        id: "tst-wf-decision-001-def".to_string(),
        tenant_id: None,
        key: DEFINITION_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: "TST WF.DECISION 001".to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: "start".to_string(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

/// Start the routing graph with `level` bound to `value`, exercising the equality arm that fact selects.
fn start_with_level(harness: &EngineHarness, value: Value) -> workflow::Result<StartProcessResult> {
    harness.engine().start_process(StartProcessParams {
        definition_key: DEFINITION_KEY.to_string(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: obj([("level", value)]),
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    })
}

/// The node the process's token came to rest at, read through the production store.
fn resting_node(harness: &EngineHarness, instance_id: &str) -> Vec<String> {
    harness
        .store()
        .with_tx(|tx| tx.tokens_for_instance(instance_id))
        .expect("the instance tokens are readable")
        .into_iter()
        .map(|token| token.node_id)
        .collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-DECISION-001); the file and the assay use it.
fn wf_decision_001__equality() {
    let vars = variables();

    // 1. EQUALITY IS TRUE EXACTLY WHEN THE SAME JSON TYPE CARRIES THE SAME VALUE. Bool, string (both quote styles),
    //    number and null are each covered, and every clause has a false neighbour that differs only in the value or
    //    its case, so a comparison that normalised either side would flip one of these.
    for (expression, expected) in [
        ("approved == true", true),
        ("rejected == false", true),
        ("status == \"open\"", true),
        ("status == 'open'", true),
        ("count == 3", true),
        ("count == 3.0", true),
        ("ratio == 1.5", true),
        ("flag == null", true),
        ("approved == false", false),
        ("rejected == true", false),
        ("status == \"OPEN\"", false),
        ("status == \"open \"", false),
        ("status == \"draft\"", false),
        ("count == 4", false),
        ("ratio == 1", false),
        ("flag == true", false),
    ] {
        assert_eq!(
            condition(expression, &vars),
            expected,
            "{HARNESS}: equality on the fixture: {expression:?} must be {expected}"
        );
    }

    // 2. INEQUALITY IS THE EXACT COMPLEMENT of equality — the operator table production parses has only `==` and `!=`
    //    (`middle/workflow/src/expr.rs:43-51`). A clause pair is asserted to be opposite and the absolute values are
    //    pinned too, so `!=` cannot pass by being an unrelated constant.
    for (eq_expr, ne_expr) in [
        ("approved == true", "approved != true"),
        ("approved == false", "approved != false"),
        ("status == \"open\"", "status != \"open\""),
        ("status == \"draft\"", "status != \"draft\""),
        ("count == 3", "count != 3"),
        ("flag == null", "flag != null"),
        ("count == \"3\"", "count != \"3\""),
    ] {
        assert_eq!(
            condition(ne_expr, &vars),
            !condition(eq_expr, &vars),
            "{HARNESS}: {ne_expr:?} must be the exact complement of {eq_expr:?}"
        );
    }
    assert!(
        condition("status != \"draft\"", &vars),
        "{HARNESS}: a different string is not equal"
    );
    assert!(
        !condition("status != \"open\"", &vars),
        "{HARNESS}: the same string is equal"
    );
    assert!(
        !condition("flag != null", &vars),
        "{HARNESS}: null equals null"
    );

    // 3. EQUALITY IS TYPE-STRICT. `json_eq` (`middle/workflow/src/expr.rs:83-91`) only compares a value to a literal
    //    of the same variant; there is no string/number/boolean coercion. Each of these reads as a match to a
    //    coercing comparator and must be false here — `count == "3"` is the canonical one, because `count` is the
    //    number 3.
    for expression in [
        "count == \"3\"",       // number vs its quoted form
        "status == 3",          // string vs a number
        "approved == 1",        // bool vs a number
        "approved == \"true\"", // bool vs its quoted form
        "flag == \"null\"",     // null vs the word
        "tags == null",         // array vs null
        "tags == \"a\"",        // array vs a scalar
        "meta == null",         // object vs null
        "meta == \"open\"",     // object vs a scalar
    ] {
        assert!(
            !condition(expression, &vars),
            "{HARNESS}: equality must not coerce across JSON types: {expression:?}"
        );
        let flipped = expression.replacen("==", "!=", 1);
        assert!(
            condition(&flipped, &vars),
            "{HARNESS}: the type mismatch makes {flipped:?} true"
        );
    }

    // 4. ABSENCE IS NOT NULL. The evaluator requires the name to be present before it compares
    //    (`middle/workflow/src/expr.rs:14-19`): a missing name has no value at all, so it is equal to nothing, null
    //    included. A reader that treated absence as null would answer `missing == null` true and `missing != null`
    //    false; both are asserted to be the other way.
    for expression in [
        "missing == null",
        "missing == true",
        "missing == \"open\"",
        "missing == 0",
    ] {
        assert!(
            !condition(expression, &vars),
            "{HARNESS}: a name the map does not hold is equal to nothing: {expression:?}"
        );
    }
    assert!(
        condition("missing != null", &vars),
        "{HARNESS}: absence is not null, so it differs from null"
    );
    assert!(
        condition("missing != true", &vars),
        "{HARNESS}: absence differs from every literal"
    );

    // 4b. The same rule when the variable root is not an object: nothing is present, so every equality is false and
    //     every inequality is true. This is the boundary `as_object().contains_key(...)` supplies — an absent map is
    //     an empty one for comparison purposes, not a null value.
    let no_map = Value::Null;
    for expression in ["approved == true", "approved == null", "level == \"high\""] {
        assert!(
            !condition(expression, &no_map),
            "{HARNESS}: with no variable object there is nothing to equal: {expression:?}"
        );
    }
    assert!(
        condition("approved != true", &no_map),
        "{HARNESS}: with no variable object every inequality holds"
    );
    assert!(
        condition("approved != null", &no_map),
        "{HARNESS}: an absent name is not null even without a map"
    );

    // 5. THE PARSER BOUNDARY AGREES WITH THE EVALUATOR. `is_supported_expression` and `evaluate_condition` are two
    //    public seams over one parser; a change to one that forgot the other would be two answers to one question.
    //    Supported expressions parse and evaluate; the accepted whitespace and quote styles are exercised here too.
    for expression in [
        "approved == true",
        "status != \"draft\"",
        "count == 3",
        "flag == null",
        "  approved   ==   true  ",
        "status == 'open'",
        "count == 3.0",
        "status == \"\"",
        "count == \"3\"",
    ] {
        assert!(
            is_supported_expression(expression),
            "{HARNESS}: {expression:?} names an equality and is supported"
        );
        assert!(
            evaluate_condition(expression, &vars).is_ok(),
            "{HARNESS}: a supported expression evaluates: {expression:?}"
        );
    }

    // 6. ANYTHING THAT IS NOT AN EQUALITY IS REFUSED, not silently answered. The DSL is bounded
    //    (`middle/workflow/src/expr.rs:1`): `&&`, ordering, a bare name, a missing operator, a missing literal, a
    //    quoted left-hand side, an unquoted literal, an over-chained preimage and an empty expression all fail the
    //    parse, and the refusal is the production `EXPRESSION` error, never a fabricated true/false.
    for expression in [
        "a && b",
        "a || b",
        "a === true",
        "a !== true",
        "a > 1",
        "a",
        "a == ",
        "1 == 1",
        "\"a\" == \"b\"",
        "a == bareword",
        "a == b",
        "a==true==true",
        "a = true",
        "a <> b",
        "",
        "   ",
    ] {
        assert!(
            !is_supported_expression(expression),
            "{HARNESS}: {expression:?} is not a bounded equality and is unsupported"
        );
        let error = refusal(expression, &vars);
        assert_eq!(
            error.code(),
            "EXPRESSION",
            "{HARNESS}: a non-equality is refused with the expression code: {expression:?}"
        );
        assert!(
            error
                .to_string()
                .contains("Unsupported workflow expression"),
            "{HARNESS}: the refusal names the unsupported expression: {expression:?} — {error}"
        );
    }

    // 7. NON-VACUITY. The variable name, the variable value and the literal are each load-bearing: changing any one
    //    flips equality. If `condition` were a constant — or compared only the operator — one of these would not move.
    let four = obj([("count", Value::from(4))]);
    assert!(
        condition("count == 3", &vars),
        "{HARNESS}: the fixture is equal to 3"
    );
    assert!(
        !condition("count == 3", &four),
        "{HARNESS}: a different value is not equal"
    );
    assert!(
        !condition("other == 3", &vars),
        "{HARNESS}: a different name is not equal"
    );
    assert!(
        !condition("count == 3.5", &vars),
        "{HARNESS}: a different literal is not equal"
    );
    assert!(
        condition("count != 3", &four),
        "{HARNESS}: the changed value makes inequality true"
    );

    // 8. THE DECISION BOUNDARY. The real engine routes on the same equality. `start -> decide -> end_high|end_low`,
    //    where `decide` has two equality arms and no otherwise: the arm the fact equals is the arm the process takes,
    //    and a fact equal to neither is refused instead of being routed somewhere invented.
    let harness = EngineHarness::new(TestClock::at_unix_millis(1_700_000_000_000));
    harness
        .engine()
        .seed_definition(decision_definition())
        .expect("the decision definition registers with the engine");

    // 8a. `level == "high"` selects the high arm.
    let high = start_with_level(&harness, Value::from("high"))
        .expect("the equal fact selects an equality arm");
    let high_instance = harness
        .store()
        .with_tx(|tx| tx.get_instance(&high.process_instance_id))
        .expect("the instance is readable");
    assert_eq!(
        high_instance.status,
        ProcessStatus::Completed,
        "{HARNESS}: the selected equality arm drove the process to completion"
    );
    let passed = resting_node(&harness, &high_instance.id);
    assert!(
        passed.iter().any(|node| node == HIGH_END),
        "{HARNESS}: `level == \"high\"` routed to {HIGH_END}, got {passed:?}"
    );
    assert!(
        !passed.iter().any(|node| node == LOW_END),
        "{HARNESS}: the low equality arm was not taken for a high fact, got {passed:?}"
    );

    // 8b. `level == "low"` selects the other arm on a fresh instance: the equality is on the fact, not on a fixed
    //     branch. (Each start mints its own instance, so the two processes cannot share a token.)
    let low = start_with_level(&harness, Value::from("low"))
        .expect("the other equal fact selects the other arm");
    let low_instance = harness
        .store()
        .with_tx(|tx| tx.get_instance(&low.process_instance_id))
        .expect("the second instance is readable");
    assert_ne!(
        low_instance.id, high_instance.id,
        "{HARNESS}: the two starts are two distinct instances"
    );
    let passed = resting_node(&harness, &low_instance.id);
    assert!(
        passed.iter().any(|node| node == LOW_END),
        "{HARNESS}: `level == \"low\"` routed to {LOW_END}, got {passed:?}"
    );
    assert!(
        !passed.iter().any(|node| node == HIGH_END),
        "{HARNESS}: the high equality arm was not taken for a low fact, got {passed:?}"
    );

    // 8c. NEGATIVE — a fact equal to neither literal is REFUSED. Every transition is guarded by an equality arm, so
    //     a decision that invented a branch (or fell to its first transition) for an unmatched fact would complete
    //     here instead of failing; that is exactly the regression the otherwise policy forbids
    //     (`middle/workflow/src/engine/execute_node_leave.rs:531-550`).
    let unmatched = start_with_level(&harness, Value::from("medium"))
        .expect_err("a fact equal to no arm must be refused, not routed to an invented branch");
    assert!(
        unmatched
            .to_string()
            .contains("No valid transition from decision node"),
        "{HARNESS}: the refusal names the decision with no equality match: {unmatched}"
    );

    // 8d. NEGATIVE — the routing equality is type-strict too. A number, a bool and null are each equal to neither
    //     string literal, so each is refused rather than coerced into an arm.
    for not_a_string in [Value::from(5), Value::Bool(true), Value::Null] {
        start_with_level(&harness, not_a_string.clone()).expect_err(&format!(
            "{HARNESS}: {not_a_string:?} is not equal to either string arm and must be refused"
        ));
    }
}
