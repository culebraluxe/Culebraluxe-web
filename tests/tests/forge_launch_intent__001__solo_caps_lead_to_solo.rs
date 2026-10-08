//! TST-FORGE-LAUNCH-INTENT-001: SOLO caps Lead to solo.
//! L1 Component, pure — the production boundary is `forge::engine::role_slice::bench_intent_errors`, the port of
//! `benchIntentErrors` that gates a Lead decision against the bench intent (migration 167 `launch_intent`).
//!
//! A SOLO bench is a CAP the operator set: the only decision that honours it is SOLO itself. A broader decision
//! (SPLIT, SMITH) and a narrower one (HOLD) are both the Lead substituting its own judgement, so both are refused
//! and the refusal names the intent and the decision. No intent at all is not a cap, and a lane with no decision yet
//! is left to the missing-decision rail rather than refused twice.

use forge::engine::role_slice::{bench_intent_errors, BENCH_INTENTS};

#[test]
#[allow(non_snake_case)]
fn forge_launch_intent_001__solo_caps_lead_to_solo() {
    // The cap honours its own decision, whatever case the Lead wrote it in.
    assert!(bench_intent_errors(Some("SOLO"), Some("SOLO")).is_empty());
    assert!(bench_intent_errors(Some("SOLO"), Some("solo")).is_empty());

    // Anything else is refused, and the refusal names the intent and the decision.
    for decision in ["SMITH", "SPLIT", "HOLD", "ASSAY"] {
        let errors = bench_intent_errors(Some("SOLO"), Some(decision));
        assert_eq!(
            errors.len(),
            1,
            "a SOLO bench must refuse {decision}: {errors:?}"
        );
        assert!(
            errors[0].contains("Bench intent is SOLO"),
            "the refusal names the intent: {errors:?}"
        );
        assert!(
            errors[0].contains(decision),
            "the refusal names the decision: {errors:?}"
        );
    }

    // No decision yet is not a refusal — another rail owns that case.
    assert!(bench_intent_errors(Some("SOLO"), None).is_empty());
    assert!(bench_intent_errors(Some("SOLO"), Some("")).is_empty());
    // And a dispatch with no intent carries no cap at all.
    assert!(bench_intent_errors(None, Some("SPLIT")).is_empty());

    // The whole lattice, so the rule cannot be a special case for SOLO and SPLIT alone: the clean pair is equality.
    for intent in BENCH_INTENTS {
        for decision in BENCH_INTENTS {
            let errors = bench_intent_errors(Some(intent), Some(decision));
            assert_eq!(
                errors.is_empty(),
                intent == decision,
                "intent {intent} / decision {decision}: {errors:?}"
            );
        }
    }
}
