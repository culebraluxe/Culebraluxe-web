//! WF.DECISION — missing vs null (TST-WF-DECISION-004).
//!
//! Contract: a `decision` node routes on the bounded condition DSL (`identifier WS (==|!=) WS literal`), and the two
//! facts that look alike from the DSL are **not** the same fact:
//!
//! - a variable the variable map **holds** whose JSON value is `null`, and
//! - a variable the variable map does **not hold at all** — a *missing* name.
//!
//! Production keeps them distinct at the single point that reads a variable, the presence guard in
//! `evaluate_condition` (`middle/workflow/src/expr.rs:10-28`, guard at `:14-19`):
//!
//! ```text
//! let present = variables.as_object().map(|m| m.contains_key(name)).unwrap_or(false);
//! let lhs = variables.get(name).cloned().unwrap_or(Value::Null);
//! let equal = if !present { false } else { json_eq(&lhs, &rhs) };
//! ```
//!
//! The guard is the whole subject. A present `null` is equal to the `null` literal; a missing name is equal to
//! **nothing**, `null` included. If the guard were dropped — the natural-looking `.unwrap_or(Value::Null)` alone — a
//! missing name would inherit the null and `missing == null` would wrongly become true. This file pins both ends:
//!
//! - **L0 Pure — the evaluator.** `evaluate_condition` is driven directly with deterministic inputs. A present `null`
//!   answers `flag == null` true and differs from every other literal; a missing name answers `flag == null` false and
//!   `missing != null` true, and equals no literal. A non-object variable root (an absent map) has no present names, so
//!   every name is missing there too. The parser's syntactic seam is separate from the presence rule:
//!   `is_supported_expression` accepts `missing == null` (it is well-formed) even though it is false — a missing name
//!   is a false answer, never a refusal — while genuinely malformed text is refused with the production `EXPRESSION`
//!   error.
//! - **The decision boundary.** The real `WorkflowEngine<MemoryStore>` is driven through a `decision` node whose arms
//!   partition the pair: `flag == null` selects the null arm and `flag != null` selects the other, so a present `null`
//!   and a missing name **cannot** be routed to the same branch. A second graph whose only arm is `flag == null`, with
//!   no otherwise, accepts a present `null` and **refuses** a missing name ("No valid transition from decision node")
//!   instead of inventing the null branch. The engine runs over the pure in-memory store: no database, no network, no
//!   provider, no external I/O.
//!
//! Level L0 Pure, harness `WorkflowHarness`. Inputs are literal strings and literal `Value`s; the outputs are the
//! production parser's and engine's own. Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test wf_decision__004__missing_null -- --nocapture

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
/// The arm/end names of the two-arm partition: `flag == null` vs `flag != null`.
const NULL_ARM: &str = "is_null";
const NOT_NULL_ARM: &str = "not_null";
const NULL_END: &str = "end_null";
const NOT_NULL_END: &str = "end_not_null";

/// The deterministic variable map. `flag` is **present and null**; `status`, `count`, `approved` and `empty` are
/// present with other JSON types; `missing` is absent by construction — it appears nowhere below.
fn variables() -> Value {
    obj([
        ("flag", Value::Null),
        ("status", Value::from("open")),
        ("count", Value::from(3)),
        ("approved", Value::Bool(true)),
        ("empty", Value::from("")),
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

/// `start -> decide (decision: the given arms) -> end_null | end_not_null`. Pass only the transitions the arms name;
/// with no unguarded transition the decision has no otherwise and refuses an unmatched fact.
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
    for id in [NULL_END, NOT_NULL_END] {
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

fn seeded(
    key: &str,
    arms: Vec<DecisionArm>,
    transitions: Vec<TransitionDefinition>,
) -> EngineHarness {
    let harness = EngineHarness::new(TestClock::at_unix_millis(1_700_000_200_000));
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

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-WF-DECISION-004); the file and the assay use it.
fn wf_decision_004__missing_null() {
    let vars = variables();

    // 0. THE DISTINCTION AT THE REPRESENTATION. `flag` is a present key whose value is `Value::Null`; `missing` has no
    //    entry at all. This is the raw fact `evaluate_condition` reads, and the two look different before any
    //    comparison happens.
    assert_eq!(
        vars.get("flag"),
        Some(&Value::Null),
        "{HARNESS}: `flag` is present and null"
    );
    assert_eq!(
        vars.get("missing"),
        None,
        "{HARNESS}: `missing` has no entry — absence, not null"
    );

    // 1. A PRESENT NULL IS NULL. It equals the `null` literal and nothing else; every other spelling or type is a
    //    different value, so a comparison that coerced would flip one of these.
    for (expression, expected) in [
        ("flag == null", true),
        ("flag != null", false),
        ("flag == \"null\"", false), // the four-letter word is a string, not null
        ("flag != \"null\"", true),
        ("flag == 0", false),
        ("flag == 0.0", false),
        ("flag == false", false),
        ("flag == \"\"", false),
        ("flag == true", false),
    ] {
        assert_eq!(
            condition(expression, &vars),
            expected,
            "{HARNESS}: present null: {expression:?} must be {expected}"
        );
    }

    // 2. A MISSING NAME IS NOT NULL — the subject of this file. It is equal to nothing, including `null`, so
    //    `missing == null` is false and `missing != null` is true. Each `==` clause is paired with its exact `!=`
    //    complement, so a constant answer to either operator cannot pass the table.
    for expression in [
        "missing == null", // the canonical pair this file exists for
        "missing == true",
        "missing == false",
        "missing == 0",
        "missing == 1.5",
        "missing == \"\"",
        "missing == \"null\"",
        "missing == \"open\"",
        "missing == 3",
    ] {
        assert!(
            !condition(expression, &vars),
            "{HARNESS}: a missing name equals nothing: {expression:?}"
        );
        let flipped = expression.replacen("==", "!=", 1);
        assert!(
            condition(&flipped, &vars),
            "{HARNESS}: a missing name differs from every literal: {flipped:?}"
        );
    }
    assert!(
        condition("missing != null", &vars),
        "{HARNESS}: absence is not null, so it differs from null"
    );

    // 3. PRESENCE IS PER NAME, NOT "THE MAP CONTAINS A NULL". A null under one name does not make another name
    //    present, and `flag` is not equal to that other name's null. If the guard were dropped and a missing name
    //    inherited `Value::Null`, `flag == null` over `other_null` would be wrongly true.
    let present_null = obj([("flag", Value::Null)]);
    let absent = obj([]);
    let other_null = obj([("other", Value::Null)]);
    assert!(
        condition("flag == null", &present_null),
        "{HARNESS}: the present null equals null"
    );
    assert!(
        !condition("flag == null", &absent),
        "{HARNESS}: with no key at all `flag` is missing, not null"
    );
    assert!(
        !condition("flag == null", &other_null),
        "{HARNESS}: a null under another name does not make `flag` present"
    );
    assert!(
        condition("other == null", &other_null),
        "{HARNESS}: the other name's own null is present"
    );
    assert!(
        !condition("other == null", &present_null),
        "{HARNESS}: `other` is missing even though `flag` is null"
    );

    // 4. A NON-OBJECT ROOT HAS NO PRESENT NAMES. `as_object()` is `None` for null, bool, number, string and array
    //    roots — and the empty object holds no names either — so every name is missing there and the missing/null
    //    rule applies unchanged.
    for root in [
        Value::Null,
        Value::Bool(true),
        Value::from(3),
        Value::from("open"),
        Value::from(vec!["a".to_string()]),
        Value::object(),
    ] {
        assert!(
            !condition("flag == null", &root),
            "{HARNESS}: no present `flag` in {root:?}, so it is not null"
        );
        assert!(
            condition("flag != null", &root),
            "{HARNESS}: no present `flag` in {root:?}, so it differs from null"
        );
        assert!(
            !condition("missing == null", &root),
            "{HARNESS}: a missing name is not null in {root:?}"
        );
    }

    // 5. THE PARSER SEAM IS SYNTACTIC, NOT SEMANTIC. `is_supported_expression` and `evaluate_condition` agree that
    //    `missing == null` is well-formed — it is a false answer, not a refusal. A missing name is never an error.
    for expression in [
        "flag == null",
        "flag != null",
        "missing == null",
        "missing != null",
    ] {
        assert!(
            is_supported_expression(expression),
            "{HARNESS}: {expression:?} is a bounded comparison and is supported"
        );
        assert!(
            evaluate_condition(expression, &vars).is_ok(),
            "{HARNESS}: {expression:?} evaluates (false or true), it is not refused"
        );
    }

    // 6. THE REFUSAL PATH IS SEPARATE FROM THE MISSING PATH. Genuinely malformed text around a missing/null subject
    //    is REFUSED with the production `EXPRESSION` error, never silently answered false — so "missing" is not
    //    confused with "unsupported", and a fabricated answer would be caught here.
    for expression in [
        "missing = null",          // single `=`
        "missing === null",        // triple `=`
        "missing == null == true", // over-chained
        "missing == nope",         // bareword literal
        "missing == ",             // missing literal
        "missing",                 // bare name, no comparison
        "",                        // empty
        "   ",                     // whitespace only
    ] {
        assert!(
            !is_supported_expression(expression),
            "{HARNESS}: {expression:?} is not a bounded comparison and is unsupported"
        );
        let error = refusal(expression, &vars);
        assert_eq!(
            error.code(),
            "EXPRESSION",
            "{HARNESS}: malformed text is refused with the expression code: {expression:?}"
        );
        assert!(
            error
                .to_string()
                .contains("Unsupported workflow expression"),
            "{HARNESS}: the refusal names the unsupported expression: {expression:?} — {error}"
        );
    }

    // 7. NON-VACUITY. The name and the value are each load-bearing: changing the name flips equality, and a present
    //    `Value::Null` is not the same as a missing name. If `condition` answered constantly, or compared only the
    //    literal, one of these would not move.
    assert!(
        condition("flag == null", &vars),
        "{HARNESS}: the fixture's present null equals null"
    );
    assert!(
        !condition("missing == null", &vars),
        "{HARNESS}: the fixture's absent name does not equal null"
    );
    let present_flag = obj([("flag", Value::from("open"))]);
    assert!(
        !condition("flag == null", &present_flag),
        "{HARNESS}: a present non-null is not null"
    );
    assert!(
        condition("flag != null", &present_flag),
        "{HARNESS}: a present non-null differs from null"
    );

    // 8. THE DECISION BOUNDARY — THE PARTITION. `start -> decide -> end_null | end_not_null`, where the two arms are
    //    exactly `flag == null` and `flag != null` and every transition is named by an arm (no otherwise). A present
    //    null takes the null arm; a missing name takes the **other** arm. If the engine conflated the two, a missing
    //    `flag` would land on `end_null` and this asserts otherwise.
    const PARTITION_KEY: &str = "TST-WF-DECISION-004-MISSING-NULL-PARTITION";
    let partition = seeded(
        PARTITION_KEY,
        vec![
            DecisionArm {
                condition: "flag == null".to_string(),
                transition: NULL_ARM.to_string(),
            },
            DecisionArm {
                condition: "flag != null".to_string(),
                transition: NOT_NULL_ARM.to_string(),
            },
        ],
        vec![
            transition(NULL_ARM, NULL_END),
            transition(NOT_NULL_ARM, NOT_NULL_END),
        ],
    );

    // 8a. A present null takes the null arm.
    let present = start_with(&partition, PARTITION_KEY, obj([("flag", Value::Null)]))
        .expect("a present null selects the null arm");
    let present_inst = partition
        .store()
        .with_tx(|tx| tx.get_instance(&present.process_instance_id))
        .expect("the instance is readable");
    assert_eq!(
        present_inst.status,
        ProcessStatus::Completed,
        "{HARNESS}: the null arm drove the process to completion"
    );
    let passed = resting_node(&partition, &present_inst.id);
    assert!(
        passed.iter().any(|node| node == NULL_END),
        "{HARNESS}: a present null routed to {NULL_END}, got {passed:?}"
    );
    assert!(
        !passed.iter().any(|node| node == NOT_NULL_END),
        "{HARNESS}: a present null did not take the not-null arm, got {passed:?}"
    );

    // 8b. THE SUBJECT — a missing name takes the not-null arm, not the null arm. This is where conflating absence
    //     with null would land on `end_null` and fail.
    let missing = start_with(&partition, PARTITION_KEY, obj([]))
        .expect("a missing name is not null, so it takes the not-null arm");
    let missing_inst = partition
        .store()
        .with_tx(|tx| tx.get_instance(&missing.process_instance_id))
        .expect("the second instance is readable");
    let passed = resting_node(&partition, &missing_inst.id);
    assert!(
        passed.iter().any(|node| node == NOT_NULL_END),
        "{HARNESS}: a missing name routed to {NOT_NULL_END}, got {passed:?}"
    );
    assert!(
        !passed.iter().any(|node| node == NULL_END),
        "{HARNESS}: a missing name must NOT take the null arm, got {passed:?}"
    );

    // 8c. A present non-null also takes the not-null arm, and a null under a different name leaves `flag` missing —
    //     so it too takes the not-null arm. The routing keys on the name `flag`, not on "the map holds a null".
    for (label, value) in [
        ("string \"null\"", Value::from("null")),
        ("zero", Value::from(0)),
        ("false", Value::Bool(false)),
        ("empty string", Value::from("")),
    ] {
        let started = start_with(&partition, PARTITION_KEY, obj([("flag", value.clone())]))
            .expect("a present non-null takes the not-null arm");
        let inst = partition
            .store()
            .with_tx(|tx| tx.get_instance(&started.process_instance_id))
            .expect("readable");
        let passed = resting_node(&partition, &inst.id);
        assert!(
            passed.iter().any(|node| node == NOT_NULL_END),
            "{HARNESS}: present {label} ({value:?}) routed to {NOT_NULL_END}, got {passed:?}"
        );
    }
    let other_null = start_with(&partition, PARTITION_KEY, obj([("other", Value::Null)]))
        .expect("a null under another name leaves `flag` missing, so it takes the not-null arm");
    let other_inst = partition
        .store()
        .with_tx(|tx| tx.get_instance(&other_null.process_instance_id))
        .expect("readable");
    let passed = resting_node(&partition, &other_inst.id);
    assert!(
        passed.iter().any(|node| node == NOT_NULL_END),
        "{HARNESS}: a null under `other` leaves `flag` missing, routed to {NOT_NULL_END}, got {passed:?}"
    );

    // 9. THE DECISION BOUNDARY — THE NULL-ONLY ARM REFUSES A MISSING NAME. A second graph with only `flag == null`,
    //    no otherwise. A present null reaches the end; a missing name matches no arm, so the decision has no answer
    //    and the engine REFUSES rather than inventing the null branch. This is the negative/fault case: with the
    //    presence guard removed, a missing name would take the null arm and complete here instead of refusing.
    const NULL_ONLY_KEY: &str = "TST-WF-DECISION-004-MISSING-NULL-NULL-ONLY";
    let null_only = seeded(
        NULL_ONLY_KEY,
        vec![DecisionArm {
            condition: "flag == null".to_string(),
            transition: NULL_ARM.to_string(),
        }],
        vec![transition(NULL_ARM, NULL_END)],
    );

    let accepted = start_with(&null_only, NULL_ONLY_KEY, obj([("flag", Value::Null)]))
        .expect("a present null satisfies the null-only arm");
    let accepted_inst = null_only
        .store()
        .with_tx(|tx| tx.get_instance(&accepted.process_instance_id))
        .expect("readable");
    assert_eq!(accepted_inst.status, ProcessStatus::Completed);
    let passed = resting_node(&null_only, &accepted_inst.id);
    assert!(
        passed.iter().any(|node| node == NULL_END),
        "{HARNESS}: a present null reached {NULL_END}, got {passed:?}"
    );

    let refused = start_with(&null_only, NULL_ONLY_KEY, obj([])).expect_err(
        "a missing name must be refused by a null-only decision, not take the null arm",
    );
    assert!(
        refused
            .to_string()
            .contains("No valid transition from decision node"),
        "{HARNESS}: a missing name has no null branch and is refused: {refused}"
    );

    // 9b. The same refusal for a null under a different name (`flag` is missing, so the null arm does not match) and
    //     for every present non-null value.
    for vars in [
        obj([("other", Value::Null)]),
        obj([("flag", Value::from("null"))]),
        obj([("flag", Value::from(0))]),
        obj([("flag", Value::Bool(false))]),
        obj([("flag", Value::from(""))]),
    ] {
        start_with(&null_only, NULL_ONLY_KEY, vars).expect_err(&format!(
            "{HARNESS}: a value that is not a present `flag: null` must be refused by the null-only arm"
        ));
    }
}
