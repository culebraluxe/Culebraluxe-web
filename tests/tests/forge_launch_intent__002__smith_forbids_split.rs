//! TST-FORGE-LAUNCH-INTENT-002: SMITH forbids SPLIT.
//! L1 Component, pure — the production boundary is `forge::engine::role_slice::bench_intent_errors` (migration 167
//! `launch_intent`).
//!
//! A SMITH bench is the operator saying "one Smith does this"; SPLIT is the Lead deciding to fan the story out anyway.
//! The refusal belongs to the CAP, not to the word: with no intent a SPLIT decision is the Lead's own call and is
//! accepted, and under a SPLIT bench the same decision is honoured. What the SMITH cap will not accept is any
//! decision but SMITH — including a narrower one, which is still the Lead overruling the operator.

use forge::engine::role_slice::bench_intent_errors;

#[test]
#[allow(non_snake_case)]
fn forge_launch_intent_002__smith_forbids_split() {
    // The case the story names.
    let errors = bench_intent_errors(Some("SMITH"), Some("SPLIT"));
    assert_eq!(
        errors.len(),
        1,
        "a SMITH bench refuses SPLIT: {errors:?}"
    );
    assert!(
        errors[0].contains("Bench intent is SMITH"),
        "the refusal names the intent: {errors:?}"
    );
    assert!(
        errors[0].contains("SPLIT"),
        "the refusal names the decision: {errors:?}"
    );

    // The ban is the cap's, not the word's: without an intent the Lead may split, and a SPLIT bench honours it.
    assert!(bench_intent_errors(None, Some("SPLIT")).is_empty());
    assert!(bench_intent_errors(Some("   "), Some("SPLIT")).is_empty());
    assert!(bench_intent_errors(Some("SPLIT"), Some("SPLIT")).is_empty());

    // A narrower decision does not slip under the cap either.
    for decision in ["SOLO", "HOLD", "ASSAY"] {
        assert!(
            !bench_intent_errors(Some("SMITH"), Some(decision)).is_empty(),
            "a SMITH bench refuses {decision} as well"
        );
    }

    // And the cap is honoured by its own decision, case-insensitively.
    assert!(bench_intent_errors(Some("SMITH"), Some("SMITH")).is_empty());
    assert!(bench_intent_errors(Some("SMITH"), Some("smith")).is_empty());

    // There must be something to refuse: the error is a sentence, not a boolean.
    let errors = bench_intent_errors(Some("SMITH"), Some("SPLIT"));
    assert!(
        errors[0].starts_with("Bench intent is SMITH: the Lead decided SPLIT"),
        "the sentence is stable for an operator reading it: {errors:?}"
    );
}
