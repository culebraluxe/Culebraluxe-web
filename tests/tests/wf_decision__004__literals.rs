//! WF.DECISION — string / number / null literals (TST-WF-DECISION-004).
//!
//! Contract: a `decision` node routes on the **literal** its condition mentions. The DSL is
//! `identifier WS (==|!=) WS literal` (`middle/workflow/src/expr.rs:1-58`). The literal is the only
//! thing on the right-hand side, and its vocabulary is exactly:
//! - `"true"` / `"false"` → `Bool` (already pinned by TST-WF-DECISION-003, not re-owned here)
//! - `"null"` → `Null`
//! - `"..."` or `'...'` → `String` (content verbatim, case-exact, no trimming, empty allowed, newline refused)
//! - `<integer>` / `<float>` → `Number` (f64, so `3` and `3.0` are the same number)
//!
//! The production evaluator is `evaluate_condition` (`middle/workflow/src/expr.rs:10-28`), called once
//! per arm by `WorkflowEngine::evaluate_decision` (`middle/workflow/src/engine/execute_node_leave.rs:343-373`).
//! The arm whose condition is true wins, and when no arm matches the decision has no answer and is refused
//! (`:368-370`, `:531-550`). `json_eq` (`expr.rs:83-91`) is exact JSON-type equality: a string is equal to a
//! string of the same bytes and to nothing else.
//!
//! This file owns the **string / number / null** half of the literal taxonomy:
//!
//! - **L0 Pure — the evaluator.** `evaluate_condition` is driven directly. String literals are case-sensitive,
//!   whitespace-exact, and empty-string-capable; both quote styles are accepted and their content is the literal
//!   without the quotes. A string that contains a newline is not a literal at all and is REFUSED. `null` is exactly
//!   the four lowercase letters `null`; any other spelling is not a literal. Numbers are `f64`, so `3 == 3.0`.
//!   Anything that is not a bounded `name == literal` / `name != literal` is REFUSED with the production `EXPRESSION`
//!   error, never guessed.
//! - **The decision boundary.** The real `WorkflowEngine<MemoryStore>` is driven through decision nodes whose arms
//!   are string, number and null equalities, proving the arm a fact equal to that literal selects is the arm the
//!   process takes, that a fact of the same type with a different value takes the other arm or is refused, and that
//!   a fact of a different JSON type is never coerced into an arm. Engine runs over pure in-memory store.
//!
//! The two parser seams are pinned to agree: `is_supported_expression` (`expr.rs:6-8`) and `evaluate_condition` say
//! the same thing about a literal, so a future change that accepts a new spelling in one and forgets the other fails
//! here.
//!
//! Level L0 Pure, harness `WorkflowHarness`. Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__004__literals -- --nocapture

use std::collections::BTreeMap;

use test_harness::{EngineHarness, TestClock};
use workflow::value::obj;
use workflow::{
    evaluate_condition, is_supported_expression, DecisionArm, DefinitionStatus, NodeDefinition,
    ProcessDefinition, ProcessGraph, ProcessOutcome, ProcessStatus, StartProcessParams,
    StartProcessResult, TransitionDefinition, Value,
};

const HARNESS: &str = "WorkflowHarness/L0 Pure";
const DEFINITION_VERSION: i32 = 1;
const STARTED_BY: &str = "tst";
const DECIDE_NODE: &str = "decide";
const GO_ARM: &str = "go";
const OTHER_ARM: &str = "other";
const GO_END: &str = "end_go";
const OTHER_END: &str = "end_other";

fn variables() -> Value {
    obj([
        ("status", Value::from("open")),
        ("empty", Value::from("")),
        ("count", Value::from(3)),
        ("ratio", Value::from(1.5)),
        ("flag", Value::Null),
        ("name", Value::from("Alice")),
        ("approved", Value::Bool(true)),
    ])
}

fn condition(expression: &str, vars: &Value) -> bool {
    evaluate_condition(expression, vars)
        .unwrap_or_else(|e| panic!("{HARNESS}: {expression:?} must evaluate, got: {e}"))
}

fn refusal(expression: &str, vars: &Value) -> workflow::WorkflowError {
    evaluate_condition(expression, vars).expect_err(&format!(
        "{HARNESS}: {expression:?} is not a literal and must be refused"
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
    for id in [GO_END, OTHER_END] {
        nodes
            .entry(id.to_string())
            .or_insert_with(|| NodeDefinition {
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

fn resting_node(harness: &EngineHarness, instance_id: &str) -> Vec<String> {
    harness
        .store()
        .with_tx(|tx| tx.tokens_for_instance(instance_id))
        .expect("tokens readable")
        .into_iter()
        .map(|t| t.node_id)
        .collect()
}

fn seeded(
    key: &str,
    arms: Vec<DecisionArm>,
    transitions: Vec<TransitionDefinition>,
) -> EngineHarness {
    let harness = EngineHarness::new(TestClock::at_unix_millis(1_700_000_100_000));
    harness
        .engine()
        .seed_definition(definition_for(key, arms, transitions))
        .expect("definition registers");
    harness
}

#[test]
#[allow(non_snake_case)]
fn wf_decision_004__literals() {
    let vars = variables();

    // 1. STRING LITERALS — exact bytes, both quote styles, empty string, case-exact, no trimming.
    for (expr, expected) in [
        ("status == \"open\"", true),
        ("status == 'open'", true),
        ("status == \"OPEN\"", false),
        ("status == \"open \"", false),
        ("status == \" open\"", false),
        ("status == \"\"", false),
        ("empty == \"\"", true),
        ("empty == ''", true),
        ("empty == \"a\"", false),
        ("name == \"Alice\"", true),
        ("name == 'Alice'", true),
        ("name == \"alice\"", false),
        ("status == \"closed\"", false),
    ] {
        assert_eq!(
            condition(expr, &vars),
            expected,
            "{HARNESS}: string literal: {expr:?} must be {expected}"
        );
    }
    // complement
    assert!(
        condition("status != \"closed\"", &vars),
        "{HARNESS}: different string is not equal"
    );
    assert!(
        !condition("status != \"open\"", &vars),
        "{HARNESS}: same string is equal"
    );
    assert!(
        condition("empty != \"a\"", &vars),
        "{HARNESS}: empty differs from a"
    );
    assert!(
        !condition("empty != \"\"", &vars),
        "{HARNESS}: empty equals empty"
    );

    // 2. NUMBER LITERALS — integer and float are the same f64 under `Value::Number`; 3 == 3.0.
    for (expr, expected) in [
        ("count == 3", true),
        ("count == 3.0", true),
        ("count == 4", false),
        ("count == 3.5", false),
        ("ratio == 1.5", true),
        ("ratio == 1", false),
        ("ratio == 1.50", true),
        ("count != 4", true),
        ("count != 3", false),
        ("count != 3.0", false),
    ] {
        assert_eq!(
            condition(expr, &vars),
            expected,
            "{HARNESS}: number literal: {expr:?} must be {expected}"
        );
    }

    // 3. NULL LITERAL — exactly `null`, lowercase. Every other spelling is not a literal at all.
    for (expr, expected) in [
        ("flag == null", true),
        ("flag != null", false),
        ("status == null", false),
        ("status != null", true),
        ("count == null", false),
        ("approved == null", false),
    ] {
        assert_eq!(
            condition(expr, &vars),
            expected,
            "{HARNESS}: null literal: {expr:?} must be {expected}"
        );
    }

    // 4. TYPE-STRICTNESS ACROSS LITERAL KINDS — string is not number, number is not null, null is not bool.
    for expr in [
        "count == \"3\"",
        "status == 3",
        "status == 1.5",
        "flag == \"null\"",
        "status == \"null\"",
        "approved == \"true\"",
        "count == \"3.0\"",
        "empty == 0",
        "flag == 0",
        "flag == \"\"",
    ] {
        assert!(
            !condition(expr, &vars),
            "{HARNESS}: cross-type must be false: {expr:?}"
        );
        let flipped = expr.replacen("==", "!=", 1);
        assert!(
            condition(&flipped, &vars),
            "{HARNESS}: cross-type makes != true: {flipped:?}"
        );
    }

    // 5. ABSENCE IS NOT A LITERAL AND NOT EQUAL TO ANY LITERAL, including null and empty string.
    for expr in [
        "missing == null",
        "missing == \"\"",
        "missing == \"open\"",
        "missing == 0",
        "missing == 3",
    ] {
        assert!(
            !condition(expr, &vars),
            "{HARNESS}: absence equals nothing: {expr:?}"
        );
    }
    for expr in [
        "missing != null",
        "missing != \"\"",
        "missing != \"open\"",
        "missing != 0",
    ] {
        assert!(
            condition(expr, &vars),
            "{HARNESS}: absence differs from every literal: {expr:?}"
        );
    }

    // 6. PARSER BOUNDARY AGREES — is_supported_expression exactly matches evaluate_condition for these forms.
    for expr in [
        "status == \"open\"",
        "status == 'open'",
        "empty == \"\"",
        "empty == ''",
        "count == 3",
        "count == 3.0",
        "ratio == 1.5",
        "flag == null",
        "status != \"blocked\"",
        "count != 3",
        "flag != null",
        "  status   ==   \"open\"  ",
    ] {
        assert!(
            is_supported_expression(expr),
            "{HARNESS}: supported literal: {expr:?}"
        );
        assert!(
            evaluate_condition(expr, &vars).is_ok(),
            "{HARNESS}: supported literal evaluates: {expr:?}"
        );
    }

    // 7. REFUSAL TABLE — anything that is not identifier op literal is REFUSED with EXPRESSION.
    //    This includes: bareword literal, mismatched quotes, newline inside string, empty rhs, bareword rhs,
    //    quoted lhs, literal lhs, NULL / Null spellings, missing operator, over-chained.
    let newline_expr = "status == \"a\nb\"";
    assert!(
        !is_supported_expression(newline_expr),
        "{HARNESS}: newline inside string must be unsupported"
    );
    let err = refusal(newline_expr, &vars);
    assert_eq!(err.code(), "EXPRESSION", "{HARNESS}: newline refusal code");

    for expr in [
        "\"open\" == \"open\"", // quoted lhs
        "1 == 1",               // literal lhs
        "status == NULL",       // uppercase null
        "status == Null",       // capitalized null
        "status == open",       // bareword rhs
        "status == ",           // missing literal
        "status == \"open",     // missing closing quote
        "status == 'open",      // missing closing single quote
        "status == \"open'",    // mismatched quotes
        "status = \"open\"",    // single =
        "status === \"open\"",  // triple =
        "a == b == c",          // over-chained
        "a",
        "",
        "   ",
    ] {
        assert!(
            !is_supported_expression(expr),
            "{HARNESS}: {expr:?} is not a literal comparison and is unsupported"
        );
        let error = refusal(expr, &vars);
        assert_eq!(
            error.code(),
            "EXPRESSION",
            "{HARNESS}: refusal code for {expr:?}"
        );
        assert!(
            error
                .to_string()
                .contains("Unsupported workflow expression"),
            "{HARNESS}: refusal names unsupported: {expr:?} {error}"
        );
    }

    // 8. NON-VACUITY — variable name, variable value and literal each flip the answer.
    let other_status = obj([("status", Value::from("closed"))]);
    assert!(
        condition("status == \"open\"", &vars),
        "{HARNESS}: fixture open"
    );
    assert!(
        !condition("status == \"open\"", &other_status),
        "{HARNESS}: different value"
    );
    assert!(
        !condition("other == \"open\"", &vars),
        "{HARNESS}: different name"
    );
    assert!(
        !condition("status == \"open \"", &vars),
        "{HARNESS}: different literal"
    );
    let four = obj([("count", Value::from(4))]);
    assert!(condition("count == 3", &vars), "{HARNESS}: count is 3");
    assert!(!condition("count == 3", &four), "{HARNESS}: count 4 not 3");
    let null_map = obj([("flag", Value::Null)]);
    let not_null = obj([("flag", Value::from("x"))]);
    assert!(
        condition("flag == null", &null_map),
        "{HARNESS}: null equals null"
    );
    assert!(
        !condition("flag == null", &not_null),
        "{HARNESS}: string not null"
    );

    // 9. DECISION BOUNDARY — STRING. start -> decide (two string arms) -> end_go | end_other, no otherwise.
    //    A fact equal to one literal takes that arm; equal to neither is refused.
    const STR_KEY: &str = "TST-WF-DECISION-004-STR";
    let str_harness = seeded(
        STR_KEY,
        vec![
            DecisionArm {
                condition: "status == \"high\"".to_string(),
                transition: GO_ARM.to_string(),
            },
            DecisionArm {
                condition: "status == \"low\"".to_string(),
                transition: OTHER_ARM.to_string(),
            },
        ],
        vec![transition(GO_ARM, GO_END), transition(OTHER_ARM, OTHER_END)],
    );

    let high = start_with(
        &str_harness,
        STR_KEY,
        obj([("status", Value::from("high"))]),
    )
    .expect("high selects go");
    let high_inst = str_harness
        .store()
        .with_tx(|tx| tx.get_instance(&high.process_instance_id))
        .expect("readable");
    assert_eq!(high_inst.status, ProcessStatus::Completed);
    let passed = resting_node(&str_harness, &high_inst.id);
    assert!(
        passed.iter().any(|n| n == GO_END),
        "{HARNESS}: high routed to {GO_END}, got {passed:?}"
    );
    assert!(!passed.iter().any(|n| n == OTHER_END));

    let low = start_with(&str_harness, STR_KEY, obj([("status", Value::from("low"))]))
        .expect("low selects other");
    let low_inst = str_harness
        .store()
        .with_tx(|tx| tx.get_instance(&low.process_instance_id))
        .expect("readable");
    let passed = resting_node(&str_harness, &low_inst.id);
    assert!(
        passed.iter().any(|n| n == OTHER_END),
        "{HARNESS}: low routed"
    );

    let unmatched = start_with(
        &str_harness,
        STR_KEY,
        obj([("status", Value::from("medium"))]),
    )
    .expect_err("unmatched string must be refused");
    assert!(unmatched
        .to_string()
        .contains("No valid transition from decision node"));

    // type-strict at boundary: number not coerced into string arm.
    for not_str in [Value::from(5), Value::Bool(true), Value::Null] {
        start_with(&str_harness, STR_KEY, obj([("status", not_str.clone())])).expect_err(&format!(
            "{HARNESS}: {not_str:?} not equal to any string arm"
        ));
    }

    // 10. DECISION BOUNDARY — NUMBER. start -> decide (one numeric arm).
    const NUM_KEY: &str = "TST-WF-DECISION-004-NUM";
    let num_harness = seeded(
        NUM_KEY,
        vec![DecisionArm {
            condition: "count == 3".to_string(),
            transition: GO_ARM.to_string(),
        }],
        vec![transition(GO_ARM, GO_END)],
    );

    let ok =
        start_with(&num_harness, NUM_KEY, obj([("count", Value::from(3))])).expect("3 takes go");
    let inst = num_harness
        .store()
        .with_tx(|tx| tx.get_instance(&ok.process_instance_id))
        .expect("readable");
    assert_eq!(inst.status, ProcessStatus::Completed);
    let passed = resting_node(&num_harness, &inst.id);
    assert!(passed.iter().any(|n| n == GO_END));

    // 3.0 is same number as 3 under f64, so it also takes the arm — this is the number-equality seam.
    let ok_f = start_with(&num_harness, NUM_KEY, obj([("count", Value::Number(3.0))]))
        .expect("3.0 takes go (same f64)");
    let inst_f = num_harness
        .store()
        .with_tx(|tx| tx.get_instance(&ok_f.process_instance_id))
        .expect("readable");
    let passed_f = resting_node(&num_harness, &inst_f.id);
    assert!(
        passed_f.iter().any(|n| n == GO_END),
        "{HARNESS}: 3.0 same as 3"
    );

    let bad = start_with(&num_harness, NUM_KEY, obj([("count", Value::from(4))]))
        .expect_err("4 does not equal 3");
    assert!(bad
        .to_string()
        .contains("No valid transition from decision node"));

    // string "3" not coerced into number arm.
    start_with(&num_harness, NUM_KEY, obj([("count", Value::from("3"))]))
        .expect_err("string 3 not number 3");

    // 11. DECISION BOUNDARY — NULL. start -> decide (null arm).
    const NULL_KEY: &str = "TST-WF-DECISION-004-NULL";
    let null_harness = seeded(
        NULL_KEY,
        vec![DecisionArm {
            condition: "flag == null".to_string(),
            transition: GO_ARM.to_string(),
        }],
        vec![transition(GO_ARM, GO_END)],
    );

    let ok_null =
        start_with(&null_harness, NULL_KEY, obj([("flag", Value::Null)])).expect("null takes go");
    let inst_null = null_harness
        .store()
        .with_tx(|tx| tx.get_instance(&ok_null.process_instance_id))
        .expect("readable");
    assert_eq!(inst_null.status, ProcessStatus::Completed);
    let passed_null = resting_node(&null_harness, &inst_null.id);
    assert!(passed_null.iter().any(|n| n == GO_END));

    // null is null, but "null" string, 0, false, "" are not.
    for not_null in [
        Value::from("null"),
        Value::from(0),
        Value::Bool(false),
        Value::from(""),
    ] {
        let refused = start_with(&null_harness, NULL_KEY, obj([("flag", not_null.clone())]))
            .expect_err(&format!("{HARNESS}: {not_null:?} not null"));
        assert!(refused
            .to_string()
            .contains("No valid transition from decision node"));
    }

    // absence is not null at boundary either — missing flag refused.
    let missing = start_with(&null_harness, NULL_KEY, obj([]))
        .expect_err("missing flag is not null at boundary");
    assert!(missing
        .to_string()
        .contains("No valid transition from decision node"));

    // 12. DECISION BOUNDARY — EMPTY STRING literal own arm.
    const EMPTY_KEY: &str = "TST-WF-DECISION-004-EMPTY";
    let empty_harness = seeded(
        EMPTY_KEY,
        vec![DecisionArm {
            condition: "name == \"\"".to_string(),
            transition: GO_ARM.to_string(),
        }],
        vec![transition(GO_ARM, GO_END)],
    );
    let empty_ok = start_with(&empty_harness, EMPTY_KEY, obj([("name", Value::from(""))]))
        .expect("empty string takes empty arm");
    let empty_inst = empty_harness
        .store()
        .with_tx(|tx| tx.get_instance(&empty_ok.process_instance_id))
        .expect("readable");
    let passed_empty = resting_node(&empty_harness, &empty_inst.id);
    assert!(
        passed_empty.iter().any(|n| n == GO_END),
        "{HARNESS}: empty literal arm"
    );

    let nonempty = start_with(&empty_harness, EMPTY_KEY, obj([("name", Value::from("x"))]))
        .expect_err("non-empty does not equal empty literal");
    assert!(nonempty
        .to_string()
        .contains("No valid transition from decision node"));
}
