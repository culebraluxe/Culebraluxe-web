//! FORGE.LAUNCH_INTENT-005 — null means Lead decides.
//!
//! CONTRACT. A null (or blank) `launch_intent` on the claimed row is not a cap: the decision
//! is the Lead's. The production boundary is
//! `forge::engine::role_slice::bench_intent_errors` — no intent means no errors for any
//! decision — read at the claim together with the rest of the durable envelope
//! (`forge::engine::agent_work::AgentWorkItem`: "a null `launch_intent` leaves the decision
//! to the Lead").
//!
//! Level: L1 Component — pure production functions, no I/O, no database, no vendor.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_launch_intent__005__null_means_lead_decides

use forge::engine::role_slice::{bench_intent_errors, BENCH_INTENTS};

#[test]
fn forge_launch_intent_005__null_means_lead_decides() {
    // ── 1. NULL LEAVES EVERY DECISION TO THE LEAD. ──────────────────────────
    for decision in ["SOLO", "SMITH", "SPLIT", "HOLD", "ASSAY"] {
        assert!(
            bench_intent_errors(None, Some(decision)).is_empty(),
            "null intent must leave decision {decision} to the Lead"
        );
    }

    // ── 2. NO DECISION YET IS ALSO UNCONSTRAINED. ───────────────────────────
    assert!(
        bench_intent_errors(None, None).is_empty(),
        "null intent with no decision yet must decide nothing here"
    );

    // ── 3. BLANK IS NULL: WHITESPACE CARRIES NO CAP. ────────────────────────
    for blank in ["", "  ", "\t"] {
        for decision in ["SOLO", "SMITH", "SPLIT", "HOLD"] {
            assert!(
                bench_intent_errors(Some(blank), Some(decision)).is_empty(),
                "blank intent {blank:?} must leave decision {decision} to the Lead"
            );
        }
    }

    // ── 4. THE FOUR INTENTS THE COLUMN MAY CARRY. ───────────────────────────
    assert_eq!(
        BENCH_INTENTS,
        ["SOLO", "SMITH", "SPLIT", "HOLD"],
        "the bench vocabulary is exactly four intents; a fifth would need its own story"
    );

    // ── 5. NEGATIVE: A SET INTENT DOES CONSTRAIN, SO NULL IS LOAD-BEARING. ──
    // If every intent passed every decision, the null case would prove nothing — the check
    // would be vacuous. A set cap refuses a foreign decision, which is what makes the null
    // case's silence the Lead's freedom rather than a missing check.
    assert!(
        !bench_intent_errors(Some("HOLD"), Some("SOLO")).is_empty(),
        "a set HOLD cap must refuse SOLO, or the null case would be vacuous"
    );
    assert!(
        !bench_intent_errors(Some("SOLO"), Some("SPLIT")).is_empty(),
        "a set SOLO cap must refuse SPLIT, or the null case would be vacuous"
    );
    assert!(
        bench_intent_errors(Some("SMITH"), Some("SMITH")).is_empty(),
        "a set cap still authorizes its own decision"
    );
}
