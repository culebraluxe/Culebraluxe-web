//! FORGE.LAUNCH_INTENT-006 — model output cannot override operator cap.
//!
//! CONTRACT. The operator's cap (`launch_intent`, set in the Cockpit) wins over whatever the
//! model decided (`leadDecision`). The production boundary is
//! `forge::engine::role_slice::bench_intent_errors` — the function the shared lifecycle
//! (`roles::lifecycle::apply_bench_intent`) turns into a rejected deliverable: a decision
//! outside the cap travels the refusal rail instead of executing. A second operator cap,
//! `forge::engine::spend_cap::forge_spend_should_hold`, holds the same way: spend past the
//! configured figure stops the turn no matter what the model produced.
//!
//! Level: L1 Component — pure production functions, no I/O, no database, no vendor.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_launch_intent__006__model_output_cannot_override_operator_cap

use forge::engine::role_slice::{bench_intent_errors, BENCH_INTENTS};
use forge::engine::spend_cap::forge_spend_should_hold;

#[test]
fn forge_launch_intent_006__model_output_cannot_override_operator_cap() {
    // ── 1. THE CAP WINS: EVERY INTENT REFUSES EVERY FOREIGN DECISION. ───────
    for intent in BENCH_INTENTS {
        for decision in ["SOLO", "SMITH", "SPLIT", "HOLD", "ASSAY"] {
            let errors = bench_intent_errors(Some(intent), Some(decision));
            if decision == intent {
                assert!(
                    errors.is_empty(),
                    "cap {intent} must authorize its own decision"
                );
            } else {
                assert!(
                    !errors.is_empty(),
                    "model output {decision} must not override operator cap {intent}"
                );
                assert!(
                    errors
                        .iter()
                        .all(|error| error.contains(&format!("Bench intent is {intent}"))),
                    "the refusal must name the operator's cap, not the model's output: {errors:?}"
                );
            }
        }
    }

    // ── 2. NEGATIVE: THE MODEL CANNOT WIDEN A NARROW CAP. ───────────────────
    // The legacy matrix, verbatim: SPLIT under SOLO and SOLO under SMITH are both the Lead
    // substituting its judgement for the operator's, in opposite directions.
    assert!(!bench_intent_errors(Some("SOLO"), Some("SPLIT")).is_empty());
    assert!(!bench_intent_errors(Some("SMITH"), Some("SOLO")).is_empty());

    // ── 3. CONTROL: NO CAP, NO REFUSAL — ABSENCE IS NOT AN OVERRIDE. ────────
    // A decision made with no cap set is the Lead deciding, not the model overriding.
    assert!(bench_intent_errors(None, Some("SPLIT")).is_empty());
    assert!(bench_intent_errors(Some("  "), Some("SMITH")).is_empty());

    // ── 4. THE SECOND CAP: SPEND PAST THE FIGURE HOLDS THE TURN. ────────────
    assert!(
        forge_spend_should_hold(Some(1.25), Some(1.0)),
        "spend past the operator's cap must hold, whatever the model produced"
    );
    assert!(
        !forge_spend_should_hold(Some(0.5), Some(1.0)),
        "spend under the cap must not hold"
    );
    assert!(
        !forge_spend_should_hold(Some(1.0), Some(1.0)),
        "spend exactly at the cap must not hold: strictly greater is the rail"
    );

    // ── 5. NEGATIVE: AN UNMEASURED OR UNCONFIGURED CAP HOLDS NOTHING. ───────
    // A missing figure must never read as an override — or every turn without a cap
    // configured would refuse, and the spend rail would be a global stop.
    assert!(!forge_spend_should_hold(None, Some(1.0)));
    assert!(!forge_spend_should_hold(Some(99.0), None));
    assert!(!forge_spend_should_hold(None, None));
}
