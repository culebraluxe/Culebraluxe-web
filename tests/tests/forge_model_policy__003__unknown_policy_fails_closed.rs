//! FORGE.MODEL_POLICY-003 — unknown policy fails closed.
//!
//! CONTRACT. An unrecognised `model_policy` never throws, never selects judgment, and never
//! names a vendor: it reads as `cheap` (bills least). And a turn with no configured agent
//! never guesses one: `forge::engine::maestro::resolve_maestro_agent` fails closed before a
//! claim is opened and before a token is spent. The production boundary is
//! `forge::engine::opencode::as_model_policy` + `ModelSelection::from_parts` (fail to the
//! cheapest) composed with the Maestro resolver (fail to an error, never a guess).
//!
//! Level: L3 Composition — the composition boundary, providers faked (no vendor touched).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_model_policy__003__unknown_policy_fails_closed

use forge::engine::harness::ModelSelection;
use forge::engine::maestro::resolve_maestro_agent;
use forge::engine::opencode::{as_model_policy, model_for_policy, MODEL_FOR_CHEAP};

#[test]
fn forge_model_policy_003__unknown_policy_fails_closed() {
    // ── 1. UNKNOWN READS AS CHEAP, NEVER THROWS. ────────────────────────────
    for unknown in ["premium", "nonsense", "cheap2", "", "null", "  "] {
        assert_eq!(
            as_model_policy(Some(unknown)),
            "cheap",
            "{unknown:?} must read as cheap"
        );
        assert_eq!(
            model_for_policy(Some(unknown)),
            MODEL_FOR_CHEAP,
            "{unknown:?} must bill the cheap model"
        );
        assert_eq!(
            ModelSelection::from_parts(Some(unknown), None),
            ModelSelection::Cheap,
            "{unknown:?} must select the cheap tier"
        );
    }
    assert_eq!(as_model_policy(None), "cheap");

    // ── 2. NEGATIVE: UNKNOWN NEVER SELECTS JUDGMENT OR A VENDOR. ────────────
    assert_ne!(
        ModelSelection::from_parts(Some("premium"), None),
        ModelSelection::Judgment,
        "an unknown policy must never reach the judgment tier"
    );
    assert!(
        !matches!(
            ModelSelection::from_parts(Some("premium"), None),
            ModelSelection::Explicit(_)
        ),
        "an unknown policy must never name a vendor selection"
    );

    // ── 3. A BLANK OVERRIDE IS NOT AN OVERRIDE. ─────────────────────────────
    // An empty explicit selection falls through to the policy instead of becoming an
    // explicit empty vendor name — a blank env var cannot silently become the turn's agent.
    assert_eq!(
        ModelSelection::from_parts(Some("judgment"), Some("")),
        ModelSelection::Judgment
    );
    assert_eq!(
        ModelSelection::from_parts(Some("judgment"), Some("   ")),
        ModelSelection::Judgment
    );

    // ── 4. NO CONFIGURED AGENT FAILS CLOSED — NEVER A GUESS. ────────────────
    // With no explicit agent, no tier agent and no default, the Maestro side refuses before
    // a claim opens and before a token spends. There is deliberately no silent fallback to
    // an OpenCode model and no arbitrary pick.
    let refused = resolve_maestro_agent(&ModelSelection::Cheap, None, None, None, None);
    assert!(
        refused.is_err(),
        "an unconfigured Maestro turn must fail closed, not guess a vendor"
    );
    let refused = resolve_maestro_agent(&ModelSelection::Judgment, None, None, None, None);
    assert!(
        refused.is_err(),
        "an unconfigured judgment turn must fail closed too"
    );

    // ── 5. CONTROL: AN EXPLICIT SELECTION STILL CARRIES ITS AGENT. ──────────
    // Fail-closed is about the absence of configuration, not about refusing a named one.
    assert_eq!(
        resolve_maestro_agent(
            &ModelSelection::Explicit("agent-x".to_string()),
            None,
            None,
            None,
            None
        )
        .expect("explicit selection resolves"),
        "agent-x".to_string()
    );
}
