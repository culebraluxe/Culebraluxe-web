//! FORGE.MODEL_POLICY-001 — cheap policy maps to declared cheap model.
//!
//! CONTRACT. A `cheap` `model_policy` on the claimed row (migration 179) names the declared
//! cheap model — the pinned flash tier. The production boundary is the policy table in
//! `forge::engine::opencode` (`FORGE_MODEL_POLICIES`, `as_model_policy`, `model_for_policy`,
//! `resolve_model_for_policy`) composed with Forge's vendor-neutral intent
//! (`forge::engine::harness::ModelSelection::from_parts`): `cheap` reads as `Cheap`, and
//! `Cheap` bills the pin, never the judgment tier.
//!
//! Level: L3 Composition — the service/router/process composition boundary with external
//! providers faked at adapter boundaries (no vendor is touched; the mapping is asserted,
//! not executed).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_model_policy__001__cheap_policy_maps_to_declared_cheap_model

use forge::engine::harness::ModelSelection;
use forge::engine::opencode::{
    as_model_policy, model_for_policy, resolve_model_for_policy, FORGE_MODEL_POLICIES,
    MODEL_FOR_CHEAP, OPENCODE_PINNED_MODEL,
};

#[test]
fn forge_model_policy_001__cheap_policy_maps_to_declared_cheap_model() {
    // ── 1. THE TABLE: EXACTLY TWO POLICIES, CHEAP AMONG THEM. ───────────────
    assert_eq!(
        FORGE_MODEL_POLICIES,
        ["cheap", "judgment"],
        "the policy vocabulary is exactly two; a third would need its own story"
    );

    // ── 2. THE POLICY READS AS CHEAP — EXPLICIT, ABSENT, OR PADDED. ─────────
    assert_eq!(as_model_policy(Some("cheap")), "cheap");
    assert_eq!(as_model_policy(Some("  cheap  ")), "cheap");
    assert_eq!(
        as_model_policy(None),
        "cheap",
        "a null policy is the cheap default: it bills least"
    );

    // ── 3. THE POLICY NAMES THE DECLARED CHEAP MODEL. ───────────────────────
    assert_eq!(model_for_policy(Some("cheap")), MODEL_FOR_CHEAP);
    assert_eq!(model_for_policy(None), MODEL_FOR_CHEAP);
    assert_eq!(
        MODEL_FOR_CHEAP, OPENCODE_PINNED_MODEL,
        "cheap bills the live pin"
    );
    assert_eq!(
        OPENCODE_PINNED_MODEL, "deepseek/deepseek-flash",
        "the pin is the flash tier the price table can price"
    );

    // ── 4. THE INTENT: CHEAP POLICY IS THE CHEAP TIER. ──────────────────────
    assert_eq!(
        ModelSelection::from_parts(Some("cheap"), None),
        ModelSelection::Cheap
    );
    assert_eq!(ModelSelection::from_parts(None, None), ModelSelection::Cheap);

    // ── 5. THE LANE MODEL: RESOLUTION BILLS THE PIN. ────────────────────────
    // The cheap arm reads no environment: the pin is unconditional.
    assert_eq!(
        resolve_model_for_policy(Some("cheap"))
            .expect("cheap resolves"),
        MODEL_FOR_CHEAP.to_string()
    );

    // ── 6. NEGATIVE: CHEAP IS NEVER JUDGMENT, NEVER EXPLICIT. ───────────────
    assert_ne!(
        ModelSelection::from_parts(Some("cheap"), None),
        ModelSelection::Judgment,
        "cheap must never select the judgment tier"
    );
    assert!(
        !matches!(
            ModelSelection::from_parts(Some("cheap"), None),
            ModelSelection::Explicit(_)
        ),
        "cheap must never name a vendor selection on its own"
    );
}
