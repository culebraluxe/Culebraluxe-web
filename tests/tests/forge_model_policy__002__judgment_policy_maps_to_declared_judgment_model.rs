//! FORGE.MODEL_POLICY-002 — judgment policy maps to declared judgment model.
//!
//! CONTRACT. A `judgment` `model_policy` on the claimed row names the declared judgment
//! model. The production boundary is the same policy table as 001
//! (`forge::engine::opencode`: `as_model_policy`, `model_for_policy`) composed with
//! `forge::engine::harness::ModelSelection::from_parts`: only the exact word `judgment`
//! (trimmed) selects the `Judgment` tier; everything else reads as cheap.
//!
//! Level: L3 Composition — the composition boundary, providers faked (no vendor touched).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_model_policy__002__judgment_policy_maps_to_declared_judgment_model

use forge::engine::harness::ModelSelection;
use forge::engine::opencode::{
    as_model_policy, model_for_policy, MODEL_FOR_CHEAP, MODEL_FOR_JUDGMENT, OPENCODE_PINNED_MODEL,
};

#[test]
fn forge_model_policy_002__judgment_policy_maps_to_declared_judgment_model() {
    // ── 1. THE WORD JUDGMENT SELECTS JUDGMENT. ──────────────────────────────
    assert_eq!(as_model_policy(Some("judgment")), "judgment");
    assert_eq!(as_model_policy(Some("  judgment  ")), "judgment");

    // ── 2. THE POLICY NAMES THE DECLARED JUDGMENT MODEL. ────────────────────
    assert_eq!(model_for_policy(Some("judgment")), MODEL_FOR_JUDGMENT);
    assert_eq!(
        MODEL_FOR_JUDGMENT, OPENCODE_PINNED_MODEL,
        "judgment bills the same flash pin: the pro tier cannot be billed per token"
    );

    // ── 3. THE INTENT: JUDGMENT POLICY IS THE JUDGMENT TIER. ────────────────
    assert_eq!(
        ModelSelection::from_parts(Some("judgment"), None),
        ModelSelection::Judgment
    );

    // ── 4. THE TIERS ARE DISTINCT INTENTS SHARING ONE PIN. ──────────────────
    // The intents differ (routing sees them) while the billed model is the same pin — the
    // 2026-09-16 decision, quoted in the production docs. Both halves must stay true.
    assert_ne!(
        ModelSelection::from_parts(Some("judgment"), None),
        ModelSelection::from_parts(Some("cheap"), None),
        "the tiers must stay distinct intents even where the pin coincides"
    );
    assert_eq!(
        MODEL_FOR_JUDGMENT, MODEL_FOR_CHEAP,
        "…while the billed model is today the same flash pin for both"
    );

    // ── 5. NEGATIVE: THE MATCH IS EXACT — NEAR-MISSES READ AS CHEAP. ────────
    // The table trims but does not fold case: a near-miss bills least rather than throwing.
    for near_miss in ["JUDGMENT", "Judgment", "judgement", "judgment!", ""] {
        assert_eq!(
            as_model_policy(Some(near_miss)),
            "cheap",
            "{near_miss:?} must read as cheap, or judgment would be selectable by typo"
        );
        assert_eq!(
            ModelSelection::from_parts(Some(near_miss), None),
            ModelSelection::Cheap,
            "{near_miss:?} must select the cheap tier"
        );
    }

    // ── 6. NEGATIVE: JUDGMENT NEVER NAMES A VENDOR ON ITS OWN. ──────────────
    assert!(
        !matches!(
            ModelSelection::from_parts(Some("judgment"), None),
            ModelSelection::Explicit(_)
        ),
        "judgment must never name a vendor selection without an explicit override"
    );
}
