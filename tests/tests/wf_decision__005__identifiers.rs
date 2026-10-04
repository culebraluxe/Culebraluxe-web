//! WF.DECISION — identifier and whitespace taxonomy (TST-WF-DECISION-005).
//!
//! Contract: the condition DSL is `identifier WS (==|!=) WS literal`
//! (`middle/workflow/src/expr.rs:1-58`). This file owns the **identifier**
//! and **whitespace** halves that 001-004 use but do not isolate:
//!
//! - identifier starts `[A-Za-z_]` then zero or more `[A-Za-z0-9_]`; nothing else.
//!   Single-char `_` is valid (the parser allows underscore as first char and len 1).
//!   Case-exact, underscore-exact, no dot, hyphen, dollar, space.
//! - whitespace is only around the operator (and outer trim). Token forms:
//!   `status==true`, `status == true`, `status   ==   true`, `\t` counted as WS.
//!   WS inside the operator (`= =`, `! =`) is not an operator, so REFUSED.
//!   WS inside the identifier splits it and REFUSED. Outer trim is allowed
//!   because `evaluate_condition` does `expression.trim()` before `parse`.
//! - `is_supported_expression` (`expr.rs:6-8`) and `evaluate_condition`
//!   (`expr.rs:10-28`) must agree on every valid/invalid identifier/WS form.
//!
//! Production boundary:
//! - `middle/workflow/src/expr.rs:30-59` `parse` owns identifier loop and WS loops
//!   around operator (`:32-54`), operator tokenization `:43-51`, and the hand
//!   to `parse_literal` `:55-57`.
//! - `middle/workflow/src/expr.rs:6-8` gate and `:10-28` eval entry.
//! - `middle/workflow/src/engine/execute_node_leave.rs:343-373` decision routing
//!   uses same parser, so identifier shapes must route correctly in real engine.
//!
//! Level L0 Pure, harness WorkflowHarness.
//! Run: cargo test -p test-harness --test wf_decision__005__identifiers -- --nocapture

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
        ("Status", Value::from("UPPER")),
        ("STATUS", Value::from("ALLUPPER")),
        ("_flag", Value::Bool(true)),
        ("_", Value::from("underscore")),
        ("__", Value::from("double")),
        ("a", Value::from("a")),
        ("A", Value::from("A")),
        ("a1", Value::from("a1")),
        ("a1b2", Value::from("a1b2")),
        ("foo_bar", Value::from("foo")),
        ("my_var", Value::from("mine")),
        ("x_y_z", Value::from("xyz")),
        ("count", Value::from(3)),
        ("approved", Value::Bool(false)),
        ("flag", Value::Null),
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
    let harness = EngineHarness::new(TestClock::at_unix_millis(1_700_000_100_000));
    harness
        .engine()
        .seed_definition(definition_for(key, arms, transitions))
        .expect("definition registers");
    harness
}

#[test]
#[allow(non_snake_case)]
fn wf_decision_005__identifiers() {
    let vars = variables();

    // 1. VALID IDENTIFIER FORMS — all start [A-Za-z_] and continue [A-Za-z0-9_].
    for (expr, expected) in [
        // plain
        ("status == \"open\"", true),
        ("status == \"closed\"", false),
        // case-exact: upper vs lower are different names
        ("Status == \"UPPER\"", true),
        ("STATUS == \"ALLUPPER\"", true),
        ("Status == \"open\"", false),
        ("status == \"UPPER\"", false),
        // underscore first char
        ("_flag == true", true),
        ("_flag == false", false),
        ("_ == \"underscore\"", true),
        ("__ == \"double\"", true),
        // single char
        ("a == \"a\"", true),
        ("A == \"A\"", true),
        // underscore inside
        ("foo_bar == \"foo\"", true),
        ("my_var == \"mine\"", true),
        ("x_y_z == \"xyz\"", true),
        // digit inside, not at start
        ("a1 == \"a1\"", true),
        ("a1b2 == \"a1b2\"", true),
        // null/bool with underscore name
        ("flag == null", true),
        ("approved == false", true),
        ("count == 3", true),
        // inequality complement
        ("status != \"closed\"", true),
        ("status != \"open\"", false),
        ("_flag != false", true),
    ] {
        assert_eq!(
            condition(expr, &vars),
            expected,
            "{HARNESS}: valid identifier: {expr:?} must be {expected}"
        );
    }

    // 2. ABSENCE VS CASE — missing name is not equal, but not refused, it's false.
    for expr in [
        "missing == \"open\"",
        "Missing == \"open\"",
        "STATUs == \"open\"",
        "status_ == \"open\"",
        "statu == \"open\"",
    ] {
        assert!(
            !condition(expr, &vars),
            "{HARNESS}: absent name {expr:?} must be false"
        );
        assert!(
            condition(&expr.replacen("==", "!=", 1), &vars),
            "{HARNESS}: absent != literal is true: {expr:?}"
        );
    }

    // 3. INVALID IDENTIFIER FORMS — must be REFUSED with EXPRESSION.
    for expr in [
        "",                       // empty
        "   ",                    // only WS
        "1status == \"open\"",    // digit start
        "1 == 1",                 // digit start literal lhs
        "0 == true",              // digit only
        "1a == \"a1\"",           // digit start with letter
        "-flag == true",          // hyphen start
        "a-b == \"x\"",           // hyphen inside
        "a.b == \"x\"",           // dot inside
        "a$ == \"x\"",            // dollar inside
        "$var == \"x\"",          // dollar start
        "@ == \"x\"",             // at sign
        "sta tus == \"open\"",    // space inside identifier
        "foo bar == \"x\"",       // space inside
        "a! == \"x\"",            // bang inside
        "a? == \"x\"",            // question inside
        "\"status\" == \"open\"", // quoted lhs
        "null == null", // literal lhs, not identifier (parse expects identifier first, literal lhs is not identifier start? Actually 'n' is identifier start, so null as name would be parsed as name, but rhs null valid — this would be treated as variable named null; should still be supported? Let's assert it's supported but false, so skip refusal. We'll test separately)
    ] {
        if expr == "null == null" {
            continue;
        }
        assert!(
            !is_supported_expression(expr),
            "{HARNESS}: invalid identifier {expr:?} must be unsupported"
        );
        let err = refusal(expr, &vars);
        assert_eq!(err.code(), "EXPRESSION", "{HARNESS}: refusal code {expr:?}");
        assert!(
            err.to_string().contains("Unsupported workflow expression"),
            "{HARNESS}: refusal mentions unsupported {expr:?}"
        );
    }

    // null as identifier is syntactically valid (identifier) but semantically means variable named "null"
    // Its presence depends on variable map — we didn't define "null" var, so it should be false for == null.
    assert!(
        is_supported_expression("null == null"),
        "{HARNESS}: `null` as identifier is syntactically valid"
    );
    assert!(
        !condition("null == null", &vars),
        "{HARNESS}: variable named null absent, so null==null is false (absence != null)"
    );
    assert!(
        condition("null != null", &vars),
        "{HARNESS}: absent null var != null literal"
    );

    // 4. WHITESPACE AROUND OPERATOR — outer trim + inner WS allowed variants.
    let base = "status == \"open\"";
    assert!(condition(base, &vars), "{HARNESS}: base");
    for expr in [
        "status==\"open\"",           // no WS at all
        "status ==\"open\"",          // no WS after ==
        "status== \"open\"",          // no WS before ==
        "status == \"open\"",         // single spaces
        "status   ==   \"open\"",     // multiple spaces
        "status\t==\t\"open\"",       // tabs
        "status \t == \t \"open\"",   // mixed
        "  status == \"open\"  ",     // outer trim
        "\tstatus == \"open\"\t",     // outer tab trim
        "  status   ==   \"open\"  ", // outer + inner
        "status == \"open\"   ",      // trailing WS outer trimmed
        "   status == \"open\"",      // leading WS
        "status\t\t==\t\t\"open\"",   // double tab
    ] {
        assert!(
            is_supported_expression(expr),
            "{HARNESS}: WS variant supported: {expr:?}"
        );
        assert_eq!(
            condition(expr, &vars),
            true,
            "{HARNESS}: WS variant truth: {expr:?}"
        );
    }

    // same for !=
    for expr in [
        "status!=\"open\"",
        "status != \"open\"",
        "status   !=   \"open\"",
        "  status != \"open\"  ",
    ] {
        // These use "closed" expectation; status is open, so != "closed" is true
        // but != "open" is false. Check both.
        if expr.contains("closed") {
            continue;
        }
        // For open, != open is false
        if expr.contains("\"open\"") {
            assert_eq!(
                condition(expr, &vars),
                false,
                "{HARNESS}: WS != open is false: {expr:?}"
            );
        }
    }
    // != closed with WS variants true
    for expr in [
        "status!=\"closed\"",
        "status != \"closed\"",
        "status   !=   \"closed\"",
        "  status != \"closed\"  ",
    ] {
        assert_eq!(
            condition(expr, &vars),
            true,
            "{HARNESS}: WS != closed true: {expr:?}"
        );
    }

    // 5. WHITESPACE INSIDE OPERATOR — must be REFUSED (not an operator).
    for expr in [
        "status = = \"open\"",
        "status =  = \"open\"",
        "status ! = \"open\"",
        "status !  = \"open\"",
        "status = = true",
        "status ! = true",
        "status =  true", // single =
        "status ! true",  // ! without =
        "status ===",     // triple =
        "status !==",
    ] {
        assert!(
            !is_supported_expression(expr),
            "{HARNESS}: WS inside operator unsupported: {expr:?}"
        );
        let err = refusal(expr, &vars);
        assert_eq!(
            err.code(),
            "EXPRESSION",
            "{HARNESS}: WS op refusal code {expr:?}"
        );
    }

    // WS inside identifier splits, also refused.
    for expr in [
        "sta tus == \"open\"",
        "my var == \"mine\"",
        "a 1 == \"a1\"",
        "_ flag == true",
    ] {
        assert!(
            !is_supported_expression(expr),
            "{HARNESS}: WS inside identifier unsupported: {expr:?}"
        );
        let err = refusal(expr, &vars);
        assert_eq!(err.code(), "EXPRESSION");
    }

    // 6. PARSER AGREEMENT — every valid identifier/WS pair both is_supported && evaluate_condition Ok
    //    every invalid both !is_supported && evaluate_condition Err with EXPRESSION.
    let valid_samples = [
        "status == \"open\"",
        "_flag == true",
        "_ == \"underscore\"",
        "a1 == \"a1\"",
        "foo_bar == \"foo\"",
        "STATUS == \"ALLUPPER\"",
        "count == 3",
        "status==\"open\"",
        "status   ==   \"open\"",
        "  status == \"open\"  ",
    ];
    for expr in valid_samples {
        assert!(
            is_supported_expression(expr),
            "{HARNESS}: agreement valid supported: {expr:?}"
        );
        assert!(
            evaluate_condition(expr, &vars).is_ok(),
            "{HARNESS}: agreement valid eval: {expr:?}"
        );
    }
    let invalid_samples = [
        "1status == \"open\"",
        "a-b == \"x\"",
        "a.b == \"x\"",
        "status = = \"open\"",
        "sta tus == \"open\"",
        "",
        "   ",
    ];
    for expr in invalid_samples {
        assert!(
            !is_supported_expression(expr),
            "{HARNESS}: agreement invalid !supported: {expr:?}"
        );
        assert!(
            evaluate_condition(expr, &vars).is_err(),
            "{HARNESS}: agreement invalid err: {expr:?}"
        );
    }

    // 7. NON-VACUITY — name, value, literal each flip result; whitespace does NOT flip.
    assert!(condition("status == \"open\"", &vars), "{HARNESS}: fixture");
    let other = obj([("status", Value::from("closed"))]);
    assert!(
        !condition("status == \"open\"", &other),
        "{HARNESS}: value change flips"
    );
    assert!(
        !condition("other == \"open\"", &vars),
        "{HARNESS}: name change flips"
    );
    assert!(
        !condition("status == \"closed\"", &vars),
        "{HARNESS}: literal change flips"
    );
    // WS change does not flip
    assert_eq!(
        condition("status == \"open\"", &vars),
        condition("status==\"open\"", &vars),
        "{HARNESS}: WS no flip"
    );
    assert_eq!(
        condition("status == \"open\"", &vars),
        condition("  status   ==   \"open\"  ", &vars),
        "{HARNESS}: outer WS no flip"
    );

    // 8. DECISION BOUNDARY — identifiers with underscore / digit / case / outer WS in arm conditions.

    // underscore identifier
    const UND_KEY: &str = "TST-WF-DECISION-005-UND";
    let und_harness = seeded(
        UND_KEY,
        vec![DecisionArm {
            condition: "_flag == true".to_string(),
            transition: GO_ARM.to_string(),
        }],
        vec![transition(GO_ARM, GO_END)],
    );
    let go = start_with(&und_harness, UND_KEY, obj([("_flag", Value::Bool(true))]))
        .expect("_flag true go");
    let inst = und_harness
        .store()
        .with_tx(|tx| tx.get_instance(&go.process_instance_id))
        .unwrap();
    assert_eq!(inst.status, ProcessStatus::Completed);
    assert!(resting_node(&und_harness, &inst.id)
        .iter()
        .any(|n| n == GO_END));

    // case-exact routing
    const CASE_KEY: &str = "TST-WF-DECISION-005-CASE";
    let case_harness = seeded(
        CASE_KEY,
        vec![
            DecisionArm {
                condition: "Status == \"UPPER\"".to_string(),
                transition: GO_ARM.to_string(),
            },
            DecisionArm {
                condition: "status == \"open\"".to_string(),
                transition: OTHER_ARM.to_string(),
            },
        ],
        vec![transition(GO_ARM, GO_END), transition(OTHER_ARM, OTHER_END)],
    );
    let up = start_with(
        &case_harness,
        CASE_KEY,
        obj([("Status", Value::from("UPPER"))]),
    )
    .expect("UPPER case arm");
    let up_inst = case_harness
        .store()
        .with_tx(|tx| tx.get_instance(&up.process_instance_id))
        .unwrap();
    assert!(resting_node(&case_harness, &up_inst.id)
        .iter()
        .any(|n| n == GO_END));

    let low = start_with(
        &case_harness,
        CASE_KEY,
        obj([("status", Value::from("open"))]),
    )
    .expect("lower case arm");
    let low_inst = case_harness
        .store()
        .with_tx(|tx| tx.get_instance(&low.process_instance_id))
        .unwrap();
    assert!(resting_node(&case_harness, &low_inst.id)
        .iter()
        .any(|n| n == OTHER_END));

    // digit inside identifier arm
    const DIG_KEY: &str = "TST-WF-DECISION-005-DIG";
    let dig_harness = seeded(
        DIG_KEY,
        vec![DecisionArm {
            condition: "a1 == \"a1\"".to_string(),
            transition: GO_ARM.to_string(),
        }],
        vec![transition(GO_ARM, GO_END)],
    );
    let dig_go =
        start_with(&dig_harness, DIG_KEY, obj([("a1", Value::from("a1"))])).expect("a1 go");
    let dig_inst = dig_harness
        .store()
        .with_tx(|tx| tx.get_instance(&dig_go.process_instance_id))
        .unwrap();
    assert!(resting_node(&dig_harness, &dig_inst.id)
        .iter()
        .any(|n| n == GO_END));

    // whitespace-padded arm condition (outer and inner WS) — engine uses same parse, so must route.
    const WS_KEY: &str = "TST-WF-DECISION-005-WS";
    let ws_harness = seeded(
        WS_KEY,
        vec![DecisionArm {
            condition: "  status   ==   \"open\"  ".to_string(),
            transition: GO_ARM.to_string(),
        }],
        vec![transition(GO_ARM, GO_END)],
    );
    let ws_go = start_with(&ws_harness, WS_KEY, obj([("status", Value::from("open"))]))
        .expect("ws padded arm go");
    let ws_inst = ws_harness
        .store()
        .with_tx(|tx| tx.get_instance(&ws_go.process_instance_id))
        .unwrap();
    assert_eq!(ws_inst.status, ProcessStatus::Completed);
    assert!(resting_node(&ws_harness, &ws_inst.id)
        .iter()
        .any(|n| n == GO_END));

    let ws_refused = start_with(
        &ws_harness,
        WS_KEY,
        obj([("status", Value::from("closed"))]),
    )
    .expect_err("closed does not match ws arm");
    assert!(ws_refused
        .to_string()
        .contains("No valid transition from decision node"));

    // single "_" identifier arm
    const SINGLE_US_KEY: &str = "TST-WF-DECISION-005-SINGLE";
    let single_harness = seeded(
        SINGLE_US_KEY,
        vec![DecisionArm {
            condition: "_ == \"underscore\"".to_string(),
            transition: GO_ARM.to_string(),
        }],
        vec![transition(GO_ARM, GO_END)],
    );
    let single_go = start_with(
        &single_harness,
        SINGLE_US_KEY,
        obj([("_", Value::from("underscore"))]),
    )
    .expect("single underscore go");
    let single_inst = single_harness
        .store()
        .with_tx(|tx| tx.get_instance(&single_go.process_instance_id))
        .unwrap();
    assert!(resting_node(&single_harness, &single_inst.id)
        .iter()
        .any(|n| n == GO_END));
}
