//! FORGE.MODEL_POLICY-006 — no model changes mid-run.
//!
//! CONTRACT. The turn's model is fixed at construction from construction-time inputs and
//! cannot drift during the run. The production boundary is the construction posture itself:
//! `ModelSelection::from_parts` is referentially transparent (the same inputs select the
//! same tier every time — there is no hidden state to change), the policy table
//! (`as_model_policy`, `model_for_policy`) is a pure function of the row value, the spend
//! cap is read once at harness build (`OpenCodeHarness.spend_cap_usd`: "a cap that could
//! change in the middle of a generation is a cap nobody can reason about"), and the Maestro
//! agent resolver is a pure function of its inputs ("so the precedence is testable without
//! mutating the process environment").
//!
//! Level: L3 Composition — the composition boundary, providers faked (no vendor touched).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_model_policy__006__no_model_changes_mid_run

use forge::engine::harness::ModelSelection;
use forge::engine::maestro::resolve_maestro_agent;
use forge::engine::opencode::{as_model_policy, model_for_policy};
use forge::engine::spend_cap::parse_forge_spend_cap_usd;

#[test]
fn forge_model_policy_006__no_model_changes_mid_run() {
    // ── 1. THE SELECTION IS STABLE ACROSS RECOMPUTATION. ────────────────────
    // A run that recomputes its selection mid-run — resume, retry, second turn — gets the
    // same tier back from the same inputs. There is no call count, clock, or global in it.
    for (policy, override_) in [
        (Some("cheap"), None),
        (Some("judgment"), None),
        (None, None),
        (Some("premium"), None),
        (Some("cheap"), Some("agent-x")),
    ] {
        assert_eq!(
            ModelSelection::from_parts(policy, override_),
            ModelSelection::from_parts(policy, override_),
            "selection for ({policy:?}, {override_:?}) must not drift between calls"
        );
    }

    // ── 2. THE POLICY TABLE IS STABLE ACROSS RECOMPUTATION. ─────────────────
    for raw in [Some("cheap"), Some("judgment"), Some("premium"), None] {
        assert_eq!(as_model_policy(raw), as_model_policy(raw));
        assert_eq!(model_for_policy(raw), model_for_policy(raw));
    }

    // ── 3. THE AGENT RESOLUTION IS STABLE ACROSS RECOMPUTATION. ─────────────
    // Same inputs, same agent, every turn of the run: the resolver carries no memory.
    let first = resolve_maestro_agent(
        &ModelSelection::Cheap,
        None,
        Some("cheap-agent"),
        Some("judgment-agent"),
        Some("default-agent"),
    )
    .expect("configured cheap resolves");
    let second = resolve_maestro_agent(
        &ModelSelection::Cheap,
        None,
        Some("cheap-agent"),
        Some("judgment-agent"),
        Some("default-agent"),
    )
    .expect("configured cheap resolves again");
    assert_eq!(first, second);
    assert_eq!(first, "cheap-agent".to_string());

    // ── 4. THE CAP PARSES DETERMINISTICALLY — READ ONCE, HOLDS ALL RUN. ─────
    assert_eq!(
        parse_forge_spend_cap_usd(Some("1.5")),
        parse_forge_spend_cap_usd(Some("1.5"))
    );
    assert_eq!(parse_forge_spend_cap_usd(Some("1.5")), Some(1.5));
    assert_eq!(parse_forge_spend_cap_usd(None), None);
    assert_eq!(parse_forge_spend_cap_usd(Some("")), None);

    // ── 5. NEGATIVE: STABILITY IS NOT VACUITY — INPUTS STILL MATTER. ────────
    // A function that returned one constant would also be "stable". Distinct inputs must
    // select distinct tiers, so the stability above proves determinism, not constancy.
    assert_ne!(
        ModelSelection::from_parts(Some("cheap"), None),
        ModelSelection::from_parts(Some("judgment"), None)
    );
    assert_ne!(
        ModelSelection::from_parts(Some("cheap"), None),
        ModelSelection::from_parts(Some("cheap"), Some("agent-x"))
    );
    assert_ne!(
        resolve_maestro_agent(
            &ModelSelection::Cheap,
            None,
            Some("cheap-agent"),
            Some("judgment-agent"),
            None
        )
        .expect("cheap resolves"),
        resolve_maestro_agent(
            &ModelSelection::Judgment,
            None,
            Some("cheap-agent"),
            Some("judgment-agent"),
            None
        )
        .expect("judgment resolves"),
        "the tiers must resolve to their own agents, or mid-run stability would be one tier"
    );
}
