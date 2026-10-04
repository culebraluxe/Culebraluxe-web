//! WF.DECISION — inequality (TST-WF-DECISION-002).
//!
//! Contract: a `decision` node routes on the bounded condition DSL
//! (`identifier WS (==|!=) WS literal`), and the DSL's second and only other comparison is **inequality** (`!=`).
//! The production evaluator is `evaluate_condition` (`middle/workflow/src/expr.rs:10-28`), called once per arm by
//! `WorkflowEngine::evaluate_decision` (`middle/workflow/src/engine/execute_node_leave.rs:343-373`); the arm whose
//! condition is true wins, and when no arm matches the decision falls to its unguarded OTHERWISE or refuses
//! (`:368-370`, `:531-550`).
//!
//! Inequality is not "a separate feature" — it is the exact complement of equality over the same parser and the same
//! `json_eq` (`expr.rs:83-91`), and this test pins that shape from both ends:
//!
//! - **L0 Pure — the evaluator.** `evaluate_condition` is driven directly with deterministic literal inputs.
//!   `a != b` is true exactly when `a == b` is false: different value, different JSON type, and a name the variable
//!   map does not hold (absence is **not** `null`) all make inequality true, while an identical value of the same
//!   JSON type makes it false. There is no case folding, trimming or cross-type coercion. Anything that is not a
//!   bounded `==`/`!=` comparison is REFUSED (`EXPRESSION`), never silently answered.
//! - **The decision boundary.** The real `WorkflowEngine<MemoryStore>` is driven through a `decision` node whose
//!   only arm is an inequality, with no otherwise: a differing fact takes the inequality arm, and a fact equal to the
//!   literal has no arm and is refused instead of being routed to an invented branch. The engine runs over the pure
//!   in-memory store: no database, no network, no provider, no external I/O.
//!
//! The parser's two seams are pinned to agree: `is_supported_expression` (`expr.rs:6-8`) says an expression is
//! supported exactly when `evaluate_condition` can evaluate it, so a future change to one that forgot the other
//! fails here rather than in production.
//!
//! Level L0 Pure, harness `WorkflowHarness`. Inputs are literal strings and literal `Value`s; the outputs are the
//! production parser's and engine's own. Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__002__inequality

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::value::obj;
use workflow::{
    evaluate_condition, is_supported_expression, DefinitionStatus, NodeDefinition,
    ProcessDefinition, ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams,
    StartProcessResult, TransitionDefinition, Value,
};

const HARNESS: &str = "WorkflowHarness/L0 Pure";
/// The definition key and version the routing half registers with the engine.
const DEFINITION_KEY: &str = "TST-WF-DECISION-002";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
/// The decision node, its single inequality arm, and the end node that arm reaches.
const DECIDE_NODE: &str = "decide";
const GO_ARM: &str = "go";
const GO_END: &str = "end_go";

/// The deterministic variable map every inequality clause is evaluated against. Each entry is a different JSON type,
/// so a clause that coerced across types has somewhere to be caught.
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
        "{HARNESS}: {expression:?} is not a bounded equality/inequality and must be refused"
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

/// `start -> decide (decision: one inequality arm, no otherwise) -> end_go`.
///
/// There is deliberately **no** unguarded transition: when the inequality is false the decision has no answer and
/// the engine refuses, so an equal fact cannot be routed anywhere invented.
fn inequality_definition() -> ProcessDefinition {
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
            decisions: Some(vec![workflow::DecisionArm {
                condition: "status != \"blocked\"".to_string(),
                transition: GO_ARM.to_string(),
            }]),
            transitions: Some(vec![transition(GO_ARM, GO_END)]),
            ..Default::default()
        },
    );
    nodes.insert(
        GO_END.to_string(),
        NodeDefinition {
            id: GO_END.to_string(),
            node_type: "end".to_string(),
            name: Some("Go".to_string()),
            outcome: Some(ProcessOutcome::Completed),
            ..Default::default()
        },
    );
    ProcessDefinition {
        id: "tst-wf-decision-002-def".to_string(),
        tenant_id: None,
        key: DEFINITION_KEY.to_string(),
        version: DEFINITION_VERSION,
        name: "TST WF.DECISION 002".to_string(),
        description: None,
        definition: ProcessGraph {
            nodes,
            start_node_id: "start".to_string(),
            display_order: None,
        },
        status: DefinitionStatus::Active,
    }
}

/// Start the routing graph with `status` bound to `value`, exercising the inequality arm that fact selects.
fn start_with_status(
    harness: &EngineHarness,
    value: Value,
) -> workflow::Result<StartProcessResult> {
    harness.engine().start_process(StartProcessParams {
        definition_key: DEFINITION_KEY.to_string(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: obj([("status", value)]),
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-DECISION-002); the file and the assay use it.
fn wf_decision_002__inequality() {
    let vars = variables();

    // 1. INEQUALITY IS TRUE FOR A DIFFERENT VALUE AND FALSE FOR THE SAME ONE, for every JSON type the DSL's literal
    //    parser can produce. Each true case has a false neighbour that differs only in the value or its case, so a
    //    comparison that normalised either side, or answered constantly, would fail one of these.
    for (expression, expected) in [
        // bool
        ("approved != false", true),
        ("approved != true", false),
        ("rejected != true", true),
        ("rejected != false", false),
        // string (both quote styles)
        ("status != \"closed\"", true),
        ("status != \"open\"", false),
        ("status != 'closed'", true),
        ("status != 'open'", false),
        ("status != \"OPEN\"", true), // inequality is case-sensitive, like equality
        ("status != \"open \"", true), // no trimming
        // number
        ("count != 4", true),
        ("count != 3", false),
        ("count != 3.0", false), // 3 and 3.0 are the same number
        ("ratio != 1", true),
        ("ratio != 1.5", false),
        // null
        ("flag != true", true),
        ("flag != null", false),
        // array / object are not equal to null or to a scalar
        ("tags != null", true),
        ("meta != null", true),
    ] {
        assert_eq!(
            condition(expression, &vars),
            expected,
            "{HARNESS}: inequality on the fixture: {expression:?} must be {expected}"
        );
    }

    // 2. INEQUALITY IS THE EXACT COMPLEMENT of equality — the operator table production parses has only `==` and `!=`
    //    (`middle/workflow/src/expr.rs:43-51`). A clause pair is asserted to have opposite answers, and the absolute
    //    values are pinned too, so `!=` cannot pass by being an unrelated constant.
    for (eq_expr, ne_expr, ne_expected) in [
        ("approved == true", "approved != true", false),
        ("approved == false", "approved != false", true),
        ("status == \"open\"", "status != \"open\"", false),
        ("status == \"closed\"", "status != \"closed\"", true),
        ("count == 3", "count != 3", false),
        ("count == 4", "count != 4", true),
        ("flag == null", "flag != null", false),
        ("count == \"3\"", "count != \"3\"", true),
    ] {
        let eq = condition(eq_expr, &vars);
        let ne = condition(ne_expr, &vars);
        assert_eq!(
            ne, !eq,
            "{HARNESS}: {ne_expr:?} must be the exact complement of {eq_expr:?}"
        );
        assert_eq!(
            ne, ne_expected,
            "{HARNESS}: {ne_expr:?} on the fixture must be {ne_expected}"
        );
    }

    // 3. INEQUALITY IS TYPE-STRICT. `json_eq` (`middle/workflow/src/expr.rs:83-91`) only compares a value to a
    //    literal of the same variant, so a type mismatch is simply unequal — `count != "3"` is the canonical one,
    //    because `count` is the number 3. A coercing comparator would answer these false.
    for expression in [
        "count != \"3\"",       // number vs its quoted form
        "status != 3",          // string vs a number
        "approved != 1",        // bool vs a number
        "approved != \"true\"", // bool vs its quoted form
        "flag != \"null\"",     // null vs the word
        "tags != null",         // array vs null
        "tags != \"a\"",        // array vs a scalar
        "meta != null",         // object vs null
        "meta != \"open\"",     // object vs a scalar
    ] {
        assert!(
            condition(expression, &vars),
            "{HARNESS}: inequality must not coerce across JSON types: {expression:?}"
        );
        let flipped = expression.replacen("!=", "==", 1);
        assert!(
            !condition(&flipped, &vars),
            "{HARNESS}: the same type mismatch makes equality false: {flipped:?}"
        );
    }

    // 4. ABSENCE IS NOT NULL, so it is unequal to every literal. The evaluator requires the name to be present before
    //    it compares (`middle/workflow/src/expr.rs:14-19`); a reader that treated absence as null would answer
    //    `missing != null` false. It must be true, and true for every other literal too.
    for expression in [
        "missing != null",
        "missing != true",
        "missing != false",
        "missing != \"open\"",
        "missing != 0",
    ] {
        assert!(
            condition(expression, &vars),
            "{HARNESS}: a name the map does not hold is unequal to every literal: {expression:?}"
        );
    }
    assert!(
        !condition("missing == null", &vars),
        "{HARNESS}: absence is not null, so equality to null is false"
    );

    // 4b. The same rule when the variable root is not an object: nothing is present, so nothing is equal and every
    //     inequality is true. This is the boundary `as_object().contains_key(...)` supplies — an absent map is an
    //     empty one for comparison purposes, not a null value.
    let no_map = Value::Null;
    for expression in [
        "status != \"blocked\"",
        "approved != true",
        "approved != null",
        "level != \"high\"",
    ] {
        assert!(
            condition(expression, &no_map),
            "{HARNESS}: with no variable object every inequality holds: {expression:?}"
        );
    }
    assert!(
        !condition("approved == true", &no_map),
        "{HARNESS}: with no variable object nothing is equal"
    );

    // 5. THE PARSER BOUNDARY AGREES WITH THE EVALUATOR. `is_supported_expression` and `evaluate_condition` are two
    //    public seams over one parser; a change to one that forgot the other would be two answers to one question.
    //    Supported inequality expressions parse and evaluate; the accepted whitespace and quote styles are exercised
    //    here too.
    for expression in [
        "status != \"blocked\"",
        "approved != true",
        "count != 3",
        "flag != null",
        "  status   !=   \"blocked\"  ",
        "status != 'blocked'",
        "count != 3.0",
        "status != \"\"",
        "count != \"3\"",
    ] {
        assert!(
            is_supported_expression(expression),
            "{HARNESS}: {expression:?} names an inequality and is supported"
        );
        assert!(
            evaluate_condition(expression, &vars).is_ok(),
            "{HARNESS}: a supported expression evaluates: {expression:?}"
        );
    }

    // 6. ANYTHING THAT IS NOT A BOUNDED `==`/`!=` IS REFUSED, not silently answered — including malformed near-miss
    //    inequalities. The refusal is the production `EXPRESSION` error, never a fabricated true/false.
    for expression in [
        "a !== true",        // JS-style strict inequality
        "a <> true",         // SQL-style inequality
        "a =! true",         // reversed operator
        "a ! true",          // operator missing its `=`
        "a != true != true", // over-chained preimage
        "a != ",             // missing literal
        "a != bareword",     // unquoted literal
        "\"a\" != \"b\"",    // quoted left-hand side
        "1 != 1",            // literal left-hand side
        "a == b",            // unquoted literal on equality too
        "a && b",            // conjunction
        "a > 1",             // ordering
        "a",                 // bare name
        "!a != true",        // leading bang
        "",
        "   ",
    ] {
        assert!(
            !is_supported_expression(expression),
            "{HARNESS}: {expression:?} is not a bounded equality/inequality and is unsupported"
        );
        let error = refusal(expression, &vars);
        assert_eq!(
            error.code(),
            "EXPRESSION",
            "{HARNESS}: a non-comparison is refused with the expression code: {expression:?}"
        );
        assert!(
            error
                .to_string()
                .contains("Unsupported workflow expression"),
            "{HARNESS}: the refusal names the unsupported expression: {expression:?} — {error}"
        );
    }

    // 7. NON-VACUITY. The variable name, the variable value and the literal are each load-bearing for `!=`: changing
    //    any one flips the answer. If `condition` were a constant — or compared only the operator — one of these
    //    would not move.
    let four = obj([("count", Value::from(4))]);
    assert!(
        !condition("count != 3", &vars),
        "{HARNESS}: the fixture's value equals 3"
    );
    assert!(
        condition("count != 3", &four),
        "{HARNESS}: a different value makes inequality true"
    );
    assert!(
        condition("other != 3", &vars),
        "{HARNESS}: a name the map does not hold is unequal to 3"
    );
    assert!(
        condition("count != 3.5", &vars),
        "{HARNESS}: a different literal makes inequality true"
    );
    assert!(
        !condition("count != 4", &four),
        "{HARNESS}: the changed value makes the new inequality false"
    );

    // 8. THE DECISION BOUNDARY. The real engine routes on the same inequality. `start -> decide -> end_go`, where
    //    `decide` has one inequality arm and no otherwise: a differing fact takes the arm, and a fact equal to the
    //    literal has no arm and is refused instead of being routed somewhere invented.
    let harness = EngineHarness::new(TestClock::at_unix_millis(1_700_000_000_000));
    harness
        .engine()
        .seed_definition(inequality_definition())
        .expect("the decision definition registers with the engine");

    // 8a. `status != "blocked"` is true for "open": the inequality arm drives the process to its end node.
    let open = start_with_status(&harness, Value::from("open"))
        .expect("a differing fact takes the inequality arm");
    let open_instance = harness
        .store()
        .with_tx(|tx| tx.get_instance(&open.process_instance_id))
        .expect("the instance is readable");
    assert_eq!(
        open_instance.status,
        ProcessStatus::Completed,
        "{HARNESS}: the inequality arm drove the process to completion"
    );
    let passed = resting_node(&harness, &open_instance.id);
    assert!(
        passed.iter().any(|node| node == GO_END),
        "{HARNESS}: `status != \"blocked\"` routed to {GO_END}, got {passed:?}"
    );

    // 8b. NEGATIVE — a fact equal to the literal makes the inequality false, and with no otherwise the decision has
    //     no answer: it is REFUSED rather than routed to an invented branch. This is the case a comparator that
    //     answered `!=` constantly, or fell back to its first transition, could not pass.
    let blocked = start_with_status(&harness, Value::from("blocked"))
        .expect_err("a fact equal to the literal makes the inequality false and must be refused");
    assert!(
        blocked
            .to_string()
            .contains("No valid transition from decision node"),
        "{HARNESS}: the refusal names the decision with no inequality match: {blocked}"
    );

    // 8c. NEGATIVE — the inequality is type-strict at the boundary too. A number and null are each unequal to the
    //     string literal, so each takes the same inequality arm; nothing is coerced into "blocked".
    for not_blocked in [
        Value::from(5),
        Value::from(2.5),
        Value::Bool(true),
        Value::Null,
    ] {
        let result = start_with_status(&harness, not_blocked.clone()).unwrap_or_else(|error| {
            panic!("{HARNESS}: {not_blocked:?} is unequal to the string arm — {error}")
        });
        let instance = harness
            .store()
            .with_tx(|tx| tx.get_instance(&result.process_instance_id))
            .expect("the instance is readable");
        let passed = resting_node(&harness, &instance.id);
        assert!(
            passed.iter().any(|node| node == GO_END),
            "{HARNESS}: {not_blocked:?} is not equal to \"blocked\" and routed to {GO_END}, got {passed:?}"
        );
    }

    // 8d. NEGATIVE — absence is unequal at the boundary. A missing `status` makes `status != "blocked"` true, so the
    //     process takes the arm rather than being refused for a null-ish fact.
    let absent = start_with_status_missing_status(&harness)
        .expect("a missing name is unequal to the literal and takes the arm");
    let absent_instance = harness
        .store()
        .with_tx(|tx| tx.get_instance(&absent.process_instance_id))
        .expect("the instance is readable");
    let passed = resting_node(&harness, &absent_instance.id);
    assert!(
        passed.iter().any(|node| node == GO_END),
        "{HARNESS}: absence is unequal to \"blocked\" and routed to {GO_END}, got {passed:?}"
    );
}

/// Start the routing graph with no `status` at all, to prove absence takes the inequality arm.
fn start_with_status_missing_status(
    harness: &EngineHarness,
) -> workflow::Result<StartProcessResult> {
    harness.engine().start_process(StartProcessParams {
        definition_key: DEFINITION_KEY.to_string(),
        version: Some(DEFINITION_VERSION),
        business_key: None,
        variables: obj([]),
        started_by: STARTED_BY.to_string(),
        tenant_id: None,
        subject: None,
    })
}
