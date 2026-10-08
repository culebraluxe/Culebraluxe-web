//! FORGE.MODEL_POLICY-005 — persisted policy wins over environment.
//!
//! CONTRACT. The model a lane bills is decided by the ROW's persisted `model_policy`
//! (migration 179), read at the claim — not by the ambient process environment. The
//! production boundary is `ModelSelection::from_parts(persisted, explicit_override)`
//! (`forge::engine::harness`): the persisted policy decides unless an explicit, attended
//! override is set and non-empty. A blank override — the usual shape of "not configured" —
//! never dislodges the persisted value, and an explicitly empty OpenCode model is refused
//! rather than silently replaced (`resolve_opencode_model`).
//!
//! Level: L3 Composition — the composition boundary, providers faked (no vendor touched;
//! only the `Some` paths are asserted, so no process environment is read).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_model_policy__005__persisted_policy_wins_over_environment

use forge::engine::harness::ModelSelection;
use forge::engine::opencode::resolve_opencode_model;

#[test]
fn forge_model_policy_005__persisted_policy_wins_over_environment() {
    // ── 1. THE PERSISTED POLICY DECIDES WITH NO EXPLICIT OVERRIDE. ──────────
    assert_eq!(
        ModelSelection::from_parts(Some("judgment"), None),
        ModelSelection::Judgment,
        "persisted judgment must win when no explicit override is set"
    );
    assert_eq!(
        ModelSelection::from_parts(Some("cheap"), None),
        ModelSelection::Cheap,
        "persisted cheap must win when no explicit override is set"
    );

    // ── 2. A BLANK OVERRIDE NEVER DISLODGES THE PERSISTED POLICY. ───────────
    // Empty and whitespace-only overrides are "not configured", not a selection: the row
    // still decides. This is the precedence that keeps an unset env var from flipping tiers.
    for blank in ["", "   ", "\t"] {
        assert_eq!(
            ModelSelection::from_parts(Some("judgment"), Some(blank)),
            ModelSelection::Judgment,
            "blank override {blank:?} must not dislodge persisted judgment"
        );
        assert_eq!(
            ModelSelection::from_parts(Some("cheap"), Some(blank)),
            ModelSelection::Cheap,
            "blank override {blank:?} must not dislodge persisted cheap"
        );
    }

    // ── 3. A NON-EMPTY ATTENDED OVERRIDE WINS — AND CARRIES ITS VENDOR. ──────
    // The one case the environment beats the row: an explicitly named selection. It never
    // passes through the policy table — an OpenCode resolver must never decide for Maestro.
    assert_eq!(
        ModelSelection::from_parts(Some("cheap"), Some("agent-x")),
        ModelSelection::Explicit("agent-x".to_string())
    );
    assert_eq!(
        ModelSelection::from_parts(Some("judgment"), Some("  agent-x  ")),
        ModelSelection::Explicit("agent-x".to_string()),
        "the override is trimmed, not taken raw"
    );

    // ── 4. NEGATIVE: AN EXPLICITLY EMPTY MODEL IS REFUSED, NOT REPLACED. ────
    // `OPENCODE_MODEL=""` is an attended typo, not an absence: resolving it errors rather
    // than silently falling back to the pin, so a misconfigured turn fails loudly.
    assert!(
        resolve_opencode_model(Some("")).is_err(),
        "an explicitly empty model must be refused"
    );
    assert!(
        resolve_opencode_model(Some("   ")).is_err(),
        "an explicitly blank model must be refused"
    );

    // ── 5. NEGATIVE: THE PRECEDENCE IS NOT VACUOUS. ─────────────────────────
    // The override genuinely changes the outcome when set, so the persisted-wins cases
    // above prove precedence rather than a function that ignores its second argument.
    assert_ne!(
        ModelSelection::from_parts(Some("judgment"), None),
        ModelSelection::from_parts(Some("judgment"), Some("agent-x")),
        "a set override must change the outcome, or the blank cases would prove nothing"
    );
}
