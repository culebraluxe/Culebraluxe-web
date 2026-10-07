//! PROP.PROPERTY_BASED — expression evaluator (TST-PROP-PROPERTY-BASED-001).
//!
//! Contract: the workflow condition DSL (`identifier WS (==|!=) WS literal`)
//! decides branch conditions. A supported expression evaluates against the
//! variable object; a missing variable is never equal (`==` is false,
//! `!=` is true); cross-type equality is false; anything outside the DSL is
//! an `Err`, never a silent default.
//!
//! Level: L0 Pure — the executable boundary is `workflow::expr`, no I/O.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test prop_property_based__001__expression_evaluator

use proptest::prelude::*;
use workflow::{evaluate_condition, is_supported_expression, value::obj, Value};

/// Identifiers the DSL accepts as a leading name: ASCII alpha/`_`, then alnum/`_`.
fn ident() -> impl Strategy<Value = String> {
    prop::string::string_regex("[A-Za-z_][A-Za-z0-9_]{0,12}").unwrap()
}

/// A literal the DSL parses, rendered exactly as the DSL expects it.
fn literal() -> impl Strategy<Value = (String, Value)> {
    prop_oneof![
        Just(("true".to_string(), Value::Bool(true))),
        Just(("false".to_string(), Value::Bool(false))),
        Just(("null".to_string(), Value::Null)),
        "[A-Za-z0-9 .,!?-]{0,16}".prop_map(|s| (format!("\"{s}\""), Value::String(s))),
        (-9999i64..9999).prop_map(|n| (n.to_string(), Value::Number(n as f64))),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Every supported `name == literal` / `name != literal` round-trips
    /// through the evaluator: `!=` is the exact negation of `==`, support
    /// implies evaluation never errors, and the fixed positive/negative
    /// vectors pin the documented forms.
    #[test]
    #[allow(non_snake_case)]
    fn prop_property_based_001__expression_evaluator(
        name in ident(),
        (text, value) in literal(),
        op in prop_oneof![Just("=="), Just("!=")],
    ) {
        // Fixed positives: the documented forms evaluate (re-checked per case;
        // pure and cheap, and they pin the DSL against strategy drift).
        let vars = obj([
            ("approved", Value::Bool(true)),
            ("status", Value::from("open")),
            ("count", Value::from(3)),
            ("flag", Value::Null),
        ]);
        prop_assert!(evaluate_condition("approved == true", &vars).unwrap());
        prop_assert!(!evaluate_condition("approved == false", &vars).unwrap());
        prop_assert!(evaluate_condition("status != \"draft\"", &vars).unwrap());
        prop_assert!(evaluate_condition("count == 3", &vars).unwrap());
        prop_assert!(evaluate_condition("flag == null", &vars).unwrap());
        // Fixed positives: a missing variable is never equal.
        prop_assert!(!evaluate_condition("missing == null", &vars).unwrap());
        prop_assert!(evaluate_condition("missing != null", &vars).unwrap());
        // Fixed negative: cross-type equality is false, never a coercion.
        let mixed = obj([("count", Value::from("3"))]);
        prop_assert!(!evaluate_condition("count == 3", &mixed).unwrap());
        // Fixed faults: outside the DSL is unsupported and an error, never a default.
        prop_assert!(!is_supported_expression("a && b"));
        prop_assert!(!is_supported_expression("a === true"));
        prop_assert!(!is_supported_expression("a > 1"));
        prop_assert!(evaluate_condition("foo === true", &vars).is_err());
        prop_assert!(evaluate_condition("1 == 1", &vars).is_err());
        prop_assert!(evaluate_condition("", &vars).is_err());

        // Property: every generated DSL expression is supported and evaluates,
        // and `!=` is the exact negation of `==` over the same variables.
        let expression = format!("{name} {op} {text}");
        prop_assert!(
            is_supported_expression(&expression),
            "a generated DSL expression must be supported: {expression}"
        );
        let case_vars = obj([(name.as_str(), value.clone())]);
        let seen = evaluate_condition(&expression, &case_vars)
            .unwrap_or_else(|_| panic!("a supported expression must evaluate: {expression}"));
        let eq = evaluate_condition(&format!("{name} == {text}"), &case_vars).expect("supported");
        let ne = evaluate_condition(&format!("{name} != {text}"), &case_vars).expect("supported");
        prop_assert_eq!(ne, !eq, "!= must be the exact negation of == for {}", name);
        prop_assert_eq!(seen, if op == "==" { eq } else { ne });
    }
}
