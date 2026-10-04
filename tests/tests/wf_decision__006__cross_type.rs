//! WF.DECISION — cross-type strictness exhaustive (TST-WF-DECISION-006).
//!
//! Contract: the condition DSL is `identifier WS (==|!=) WS literal`
//! (`middle/workflow/src/expr.rs:1-82`). Comparison is `json_eq`
//! (`expr.rs:83-91`) — type-strict, no coercion between JSON kinds:
//! - `Bool` equals only `Bool` of same value
//! - `String` equals only `String` bytes-equal
//! - `Number` equals only `Number` f64-equal (so `3 == 3.0`)
//! - `Null` equals only `Null`
//! - anything else (cross-kind) is false
//! - absent name `!present` is false for `==`, true for `!=` including vs null
//!   (`expr.rs:14-19`), so absence never equals anything.
//!
//! This file owns the **cross-type table** that 001-005 touch only partially:
//! - 001 equality proves equality exists
//! - 002 inequality proves complement (!= is ! ==)
//! - 003 boolean owns `true`/`false` literal syntax
//! - 004 literals owns string/number/null syntax and some cross false, plus `3==3.0`
//! - 005 identifiers/whitespace owns name shape
//! - **006 owns the exhaustive strictness table:**
//!   `true` vs `"true"` vs `1` vs `"1"` vs `null` vs `""` vs `0` vs absent,
//!   with every off-diagonal false, diagonal true, `!=` exact negation, empty
//!   string not null not 0 not false, number not stringified number, bool not
//!   stringified bool, absence != all.
//!
//! Production boundary:
//! - `middle/workflow/src/expr.rs:60-82` `parse_literal` literal kinds
//! - `middle/workflow/src/expr.rs:83-91` `json_eq` type-strict equality
//! - `middle/workflow/src/expr.rs:6-8` & `:10-28` gate + eval entry, absence handling `:14-19`
//! - `middle/workflow/src/engine/execute_node_leave.rs:343-373` decision routing
//!   must respect same strictness: a `Bool(true)` var never routes to `"true"` arm,
//!   `1` never to `"1"` arm, `null` never to `""` arm, absent never to any.
//!
//! Level L0 Pure, harness WorkflowHarness.
//! Run: cargo test -p test-harness --test wf_decision__006__cross_type -- --nocapture

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
        ("b_true", Value::Bool(true)),
        ("b_false", Value::Bool(false)),
        ("s_true", Value::from("true")),
        ("s_false", Value::from("false")),
        ("s_1", Value::from("1")),
        ("s_0", Value::from("0")),
        ("s_empty", Value::from("")),
        ("s_open", Value::from("open")),
        ("s_null_word", Value::from("null")),
        ("n_1", Value::from(1)),
        ("n_0", Value::from(0)),
        ("n_3", Value::from(3)),
        ("n_3f", Value::Number(3.0)),
        ("n_1p5", Value::from(1.5)),
        ("null_var", Value::Null),
    ])
}

fn condition(expression: &str, vars: &Value) -> bool {
    evaluate_condition(expression, vars)
        .unwrap_or_else(|e| panic!("{HARNESS}: {expression:?} must evaluate, got: {e}"))
}

fn refusal(expression: &str, vars: &Value) -> workflow::WorkflowError {
    evaluate_condition(expression, vars)
        .expect_err(&format!("{HARNESS}: {expression:?} must be refused"))
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
    let harness = EngineHarness::new(TestClock::at_unix_millis(1_700_000_100_100));
    harness
        .engine()
        .seed_definition(definition_for(key, arms, transitions))
        .expect("definition registers");
    harness
}

#[test]
#[allow(non_snake_case)]
fn wf_decision_006__cross_type() {
    let vars = variables();

    // 1. DIAGONAL TRUE — exact type+value match only.
    for (expr, expected) in [
        ("b_true == true", true),
        ("b_false == false", true),
        ("s_true == \"true\"", true),
        ("s_false == \"false\"", true),
        ("s_1 == \"1\"", true),
        ("s_0 == \"0\"", true),
        ("s_empty == \"\"", true),
        ("s_open == \"open\"", true),
        ("s_null_word == \"null\"", true),
        ("n_1 == 1", true),
        ("n_0 == 0", true),
        ("n_3 == 3", true),
        ("n_3 == 3.0", true),
        ("n_3f == 3", true),
        ("n_3f == 3.0", true),
        ("n_1p5 == 1.5", true),
        ("null_var == null", true),
        // single-quoted literal same string
        ("s_open == 'open'", true),
        ("s_empty == ''", true),
    ] {
        assert_eq!(
            condition(expr, &vars),
            expected,
            "{HARNESS}: diagonal true: {expr:?}"
        );
        // != negation
        let ne = expr.replacen("==", "!=", 1);
        assert_eq!(
            condition(&ne, &vars),
            !expected,
            "{HARNESS}: diagonal != negation: {ne:?}"
        );
        assert!(
            is_supported_expression(expr),
            "{HARNESS}: supported: {expr:?}"
        );
    }

    // 2. OFF-DIAGONAL CROSS-TYPE — bool vs stringified bool vs number vs stringified number vs null vs empty vs 0 vs false.
    // Every cross should be false for == and true for !=.
    let cross_false: &[(&str, &str)] = &[
        // bool true var vs non-bool literals
        ("b_true == \"true\"", "b_true true vs string true"),
        ("b_true == 'true'", "single quote string"),
        ("b_true == 1", "bool vs number 1"),
        ("b_true == 0", "bool vs 0"),
        ("b_true == \"1\"", "bool vs string 1"),
        ("b_true == null", "bool vs null"),
        ("b_true == \"\"", "bool vs empty"),
        ("b_true == \"null\"", "bool vs string null"),
        ("b_true == false", "true != false"),
        // bool false vs
        ("b_false == \"false\"", "false vs string false"),
        ("b_false == 0", "false vs 0"),
        ("b_false == 1", "false vs 1"),
        ("b_false == \"0\"", "false vs string 0"),
        ("b_false == null", "false vs null"),
        ("b_false == \"\"", "false vs empty"),
        // string true vs bool true, number etc
        ("s_true == true", "string true vs bool true"),
        ("s_true == false", "string true vs bool false"),
        ("s_true == 1", "string true vs number"),
        ("s_true == null", "string true vs null"),
        ("s_true == \"\"", "string true vs empty"),
        ("s_true == 0", "string true vs 0"),
        // s_1 string "1" vs number 1, bool, null, empty
        ("s_1 == 1", "string 1 vs number 1"),
        ("s_1 == true", "string 1 vs bool"),
        ("s_1 == null", "string 1 vs null"),
        ("s_1 == \"\"", "string 1 vs empty"),
        ("s_1 == 0", "string 1 vs 0"),
        // empty string vs null vs 0 vs false vs "0"
        ("s_empty == null", "empty vs null"),
        ("s_empty == 0", "empty vs 0"),
        ("s_empty == false", "empty vs false"),
        ("s_empty == \"0\"", "empty vs string 0"),
        ("s_empty == \"null\"", "empty vs string null"),
        // null_var vs everything non-null
        ("null_var == \"\"", "null vs empty"),
        ("null_var == 0", "null vs 0"),
        ("null_var == \"0\"", "null vs string 0"),
        ("null_var == false", "null vs false"),
        ("null_var == true", "null vs true"),
        ("null_var == \"null\"", "null vs string null"),
        ("null_var == \"true\"", "null vs string true"),
        ("null_var == 1", "null vs 1"),
        ("null_var == \"1\"", "null vs string 1"),
        // number vs stringified number, bool, null, empty
        ("n_1 == \"1\"", "number 1 vs string 1"),
        ("n_1 == true", "number 1 vs bool true"),
        ("n_1 == \"true\"", "number 1 vs string true"),
        ("n_1 == null", "number 1 vs null"),
        ("n_1 == \"\"", "number 1 vs empty"),
        ("n_0 == false", "number 0 vs false"),
        ("n_0 == \"0\"", "number 0 vs string 0"),
        ("n_0 == null", "number 0 vs null"),
        ("n_0 == \"\"", "number 0 vs empty"),
        ("n_3 == \"3\"", "number 3 vs string 3"),
        ("n_3 == true", "number 3 vs bool"),
        ("n_1p5 == \"1.5\"", "number 1.5 vs string 1.5"),
        // s_null_word "null" vs null actual
        ("s_null_word == null", "string null vs null"),
        ("s_null_word == \"\"", "string null vs empty"),
        ("s_null_word == 0", "string null vs 0"),
    ];

    for (expr, why) in cross_false {
        assert!(
            !condition(expr, &vars),
            "{HARNESS}: cross-type must be false: {expr:?} ({why})"
        );
        let ne = expr.replacen("==", "!=", 1);
        assert!(
            condition(&ne, &vars),
            "{HARNESS}: cross-type makes != true: {ne:?} ({why})"
        );
        assert!(
            is_supported_expression(expr),
            "{HARNESS}: cross-type still supported (syntax ok): {expr:?}"
        );
        assert!(
            evaluate_condition(expr, &vars).is_ok(),
            "{HARNESS}: cross-type evaluates ok: {expr:?}"
        );
    }

    // 3. ABSENCE STRICT — absent never equals any literal, even null or empty.
    for expr in [
        "missing == true",
        "missing == false",
        "missing == \"true\"",
        "missing == \"\"",
        "missing == ''",
        "missing == \"null\"",
        "missing == null",
        "missing == 0",
        "missing == 1",
        "missing == 3",
        "missing == \"open\"",
        "missing == \"1\"",
    ] {
        assert!(
            !condition(expr, &vars),
            "{HARNESS}: absent == anything false: {expr:?}"
        );
        let ne = expr.replacen("==", "!=", 1);
        assert!(
            condition(&ne, &vars),
            "{HARNESS}: absent != anything true: {ne:?}"
        );
    }

    // 4. EMPTY STRING EDGES — empty is its own type/value, not null, not 0, not false, not "0", not "null".
    // Already part cross, but explicit oriented list.
    for (expr, expected) in [
        ("s_empty == \"\"", true),
        ("s_empty == ''", true),
        ("s_empty == \" \"", false),
        ("s_empty == \"0\"", false),
        ("s_empty == \"false\"", false),
        ("s_empty == \"null\"", false),
        ("s_empty == null", false),
        ("s_empty == 0", false),
        ("s_empty == false", false),
    ] {
        assert_eq!(
            condition(expr, &vars),
            expected,
            "{HARNESS}: empty edge: {expr:?}"
        );
    }

    // 5. NUMBER COERCION EDGES — 3 == 3.0 true, but 3 != "3", 3 != true, 3.0 != "3.0", etc.
    for (expr, expected) in [
        ("n_3 == 3", true),
        ("n_3 == 3.0", true),
        ("n_3f == 3", true),
        ("n_3f == 3.0", true),
        ("n_3 == 3.00", true),
        ("n_1p5 == 1.5", true),
        ("n_1p5 == 1.50", true),
        ("n_3 == \"3\"", false),
        ("n_3 == \"3.0\"", false),
        ("n_3 == true", false),
        ("n_1 == \"true\"", false),
        ("n_0 == \"\"", false),
        ("n_0 == null", false),
        ("n_1 == true", false),
    ] {
        assert_eq!(
            condition(expr, &vars),
            expected,
            "{HARNESS}: number coercion: {expr:?}"
        );
    }

    // 6. BOOL vs STRINGIFIED — true is not "true", false not "false", and vice versa.
    for (expr, expected) in [
        ("b_true == true", true),
        ("b_true == \"true\"", false),
        ("b_true == \"True\"", false),
        ("b_true == 1", false),
        ("b_true == \"1\"", false),
        ("b_false == false", true),
        ("b_false == \"false\"", false),
        ("b_false == 0", false),
        ("b_false == \"0\"", false),
        ("s_true == \"true\"", true),
        ("s_true == true", false),
        ("s_false == \"false\"", true),
        ("s_false == false", false),
    ] {
        assert_eq!(
            condition(expr, &vars),
            expected,
            "{HARNESS}: bool edge: {expr:?}"
        );
    }

    // 7. NULL vs ALL — null equals only null, nothing else, including string "null", empty, 0, false.
    for (expr, expected) in [
        ("null_var == null", true),
        ("null_var == \"null\"", false),
        ("null_var == \"\"", false),
        ("null_var == ''", false),
        ("null_var == 0", false),
        ("null_var == false", false),
        ("null_var == \"false\"", false),
        ("null_var == \"0\"", false),
        ("null_var == 1", false),
        ("s_null_word == \"null\"", true),
        ("s_null_word == null", false),
    ] {
        assert_eq!(
            condition(expr, &vars),
            expected,
            "{HARNESS}: null edge: {expr:?}"
        );
    }

    // 8. PARSER AGREEMENT — every cross still supported syntactically, evaluates Ok (false), not Refused.
    // Invalid forms would be bareword literal etc, tested in 004. Here just prove agreement.
    let valid_cross = [
        "b_true == \"true\"",
        "n_1 == \"1\"",
        "s_empty == null",
        "null_var == \"\"",
    ];
    for expr in valid_cross {
        assert!(
            is_supported_expression(expr),
            "{HARNESS}: cross supported {expr:?}"
        );
        assert!(
            evaluate_condition(expr, &vars).is_ok(),
            "{HARNESS}: cross eval ok {expr:?}"
        );
    }

    // 9. REFUSAL REMAINS — ensure bareword rhs still refused (not confused with absent).
    for expr in ["status == open", "status == NULL", "status == True"] {
        assert!(
            !is_supported_expression(expr),
            "{HARNESS}: bareword rhs refused: {expr:?}"
        );
        let err = refusal(expr, &vars);
        assert_eq!(err.code(), "EXPRESSION");
    }

    // 10. NON-VACUITY — name, value, literal each flip; type change flips.
    assert!(condition("b_true == true", &vars), "{HARNESS}: fixture");
    assert!(
        !condition("b_true == false", &vars),
        "{HARNESS}: literal flip"
    );
    assert!(
        !condition("b_false == true", &vars),
        "{HARNESS}: name flip: b_true vs b_false map"
    );
    let other = obj([("b_true", Value::Bool(false))]);
    assert!(
        !condition("b_true == true", &other),
        "{HARNESS}: value flip"
    );
    // type flip
    assert!(
        !condition("b_true == \"true\"", &vars),
        "{HARNESS}: type flip bool->string"
    );
    assert!(
        !condition("n_1 == \"1\"", &vars),
        "{HARNESS}: type flip number->string"
    );

    // 11. DECISION BOUNDARY — cross-type arms never match out-of-kind fact.
    // a decision has go = "b_true == true", other = "s_true == \"true\"" — Bool(true) takes go, String("true") takes other, cross not.
    const CROSS_KEY: &str = "TST-WF-DECISION-006-CROSS";
    let cross_harness = seeded(
        CROSS_KEY,
        vec![
            DecisionArm {
                condition: "flag == true".to_string(),
                transition: GO_ARM.to_string(),
            },
            DecisionArm {
                condition: "flag == \"true\"".to_string(),
                transition: OTHER_ARM.to_string(),
            },
        ],
        vec![transition(GO_ARM, GO_END), transition(OTHER_ARM, OTHER_END)],
    );

    // bool true -> go
    let go = start_with(
        &cross_harness,
        CROSS_KEY,
        obj([("flag", Value::Bool(true))]),
    )
    .expect("bool true go");
    let go_inst = cross_harness
        .store()
        .with_tx(|tx| tx.get_instance(&go.process_instance_id))
        .expect("readable");
    assert_eq!(go_inst.status, ProcessStatus::Completed);
    assert!(resting_node(&cross_harness, &go_inst.id)
        .iter()
        .any(|n| n == GO_END));

    // string true -> other
    let other = start_with(
        &cross_harness,
        CROSS_KEY,
        obj([("flag", Value::from("true"))]),
    )
    .expect("string true other");
    let other_inst = cross_harness
        .store()
        .with_tx(|tx| tx.get_instance(&other.process_instance_id))
        .expect("readable");
    assert!(resting_node(&cross_harness, &other_inst.id)
        .iter()
        .any(|n| n == OTHER_END));

    // number 1 -> neither (refused)
    let not_match = start_with(&cross_harness, CROSS_KEY, obj([("flag", Value::from(1))]))
        .expect_err("number 1 neither bool nor string true");
    assert!(not_match
        .to_string()
        .contains("No valid transition from decision node"));

    // null -> neither
    let null_no = start_with(&cross_harness, CROSS_KEY, obj([("flag", Value::Null)]))
        .expect_err("null neither");
    assert!(null_no
        .to_string()
        .contains("No valid transition from decision node"));

    // absent -> neither (since absence false for both ==)
    let absent_no =
        start_with(&cross_harness, CROSS_KEY, obj([])).expect_err("absent neither bool nor string");
    assert!(absent_no
        .to_string()
        .contains("No valid transition from decision node"));

    // 12. DECISION BOUNDARY — empty vs null vs 0 vs false distinct arms.
    const EMPTY_NULL_KEY: &str = "TST-WF-DECISION-006-EMPTYNULL";
    let en_harness = seeded(
        EMPTY_NULL_KEY,
        vec![
            DecisionArm {
                condition: "v == \"\"".to_string(),
                transition: GO_ARM.to_string(),
            },
            DecisionArm {
                condition: "v == null".to_string(),
                transition: OTHER_ARM.to_string(),
            },
        ],
        vec![transition(GO_ARM, GO_END), transition(OTHER_ARM, OTHER_END)],
    );

    let empty_go =
        start_with(&en_harness, EMPTY_NULL_KEY, obj([("v", Value::from(""))])).expect("empty go");
    let ei = en_harness
        .store()
        .with_tx(|tx| tx.get_instance(&empty_go.process_instance_id))
        .unwrap();
    assert!(resting_node(&en_harness, &ei.id)
        .iter()
        .any(|n| n == GO_END));

    let null_other =
        start_with(&en_harness, EMPTY_NULL_KEY, obj([("v", Value::Null)])).expect("null other");
    let ni = en_harness
        .store()
        .with_tx(|tx| tx.get_instance(&null_other.process_instance_id))
        .unwrap();
    assert!(resting_node(&en_harness, &ni.id)
        .iter()
        .any(|n| n == OTHER_END));

    // 0 should not match empty nor null
    let zero_no = start_with(&en_harness, EMPTY_NULL_KEY, obj([("v", Value::from(0))]))
        .expect_err("0 neither empty nor null");
    assert!(zero_no
        .to_string()
        .contains("No valid transition from decision node"));

    // 13. DECISION BOUNDARY — number vs stringified number.
    const NUM_STR_KEY: &str = "TST-WF-DECISION-006-NUMSTR";
    let ns_harness = seeded(
        NUM_STR_KEY,
        vec![
            DecisionArm {
                condition: "c == 1".to_string(),
                transition: GO_ARM.to_string(),
            },
            DecisionArm {
                condition: "c == \"1\"".to_string(),
                transition: OTHER_ARM.to_string(),
            },
        ],
        vec![transition(GO_ARM, GO_END), transition(OTHER_ARM, OTHER_END)],
    );

    let num_go =
        start_with(&ns_harness, NUM_STR_KEY, obj([("c", Value::from(1))])).expect("number 1 go");
    let ni = ns_harness
        .store()
        .with_tx(|tx| tx.get_instance(&num_go.process_instance_id))
        .unwrap();
    assert!(resting_node(&ns_harness, &ni.id)
        .iter()
        .any(|n| n == GO_END));

    let str_other = start_with(&ns_harness, NUM_STR_KEY, obj([("c", Value::from("1"))]))
        .expect("string 1 other");
    let si = ns_harness
        .store()
        .with_tx(|tx| tx.get_instance(&str_other.process_instance_id))
        .unwrap();
    assert!(resting_node(&ns_harness, &si.id)
        .iter()
        .any(|n| n == OTHER_END));

    // bool true should not match either numeric/string 1
    let bool_no = start_with(&ns_harness, NUM_STR_KEY, obj([("c", Value::Bool(true))]))
        .expect_err("bool true neither 1 nor \"1\"");
    assert!(bool_no
        .to_string()
        .contains("No valid transition from decision node"));
}
