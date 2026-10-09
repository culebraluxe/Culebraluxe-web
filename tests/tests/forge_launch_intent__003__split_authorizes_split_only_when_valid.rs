//! FORGE.LAUNCH_INTENT-003 — SPLIT authorizes split only when valid.
//!
//! CONTRACT. A `SPLIT` bench intent (migration 167 `launch_intent`) authorizes exactly the
//! `SPLIT` Lead decision — and a `SPLIT` decision is itself valid only when the split is
//! well-formed. The production boundary is threefold, and this test holds all three:
//!
//! 1. `forge::engine::role_slice::bench_intent_errors` — the cap: `SPLIT` intent + `SPLIT`
//!    decision is clean; `SPLIT` intent + any other decision errors naming the cap.
//! 2. `forge::engine::graph::split_eligibility` — structural validity: more than one
//!    sibling and no sibling depending on a sibling, else sequential, not SPLIT.
//! 3. `LeadHooks::routing_decision_missing` on `lead_pre` — a `SPLIT` decision without a
//!    positive `split_count` owes `lead_decision.splitCount`; the gate re-asks.
//!
//! Level: L1 Component — pure production functions, no I/O, no database, no vendor.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_launch_intent__003__split_authorizes_split_only_when_valid

use forge::engine::facts::ForgeGateEvidence;
use forge::engine::graph::{split_eligibility, SmithWorkNode};
use forge::engine::role_slice::bench_intent_errors;
use forge::roles::hooks::ForgeRoleHooks;
use forge::roles::lead::{LeadHooks, LEAD_DECISION_NODE};

fn sibling(id: &str, depends_on: &[&str]) -> SmithWorkNode {
    SmithWorkNode {
        id: id.to_string(),
        purpose: format!("{id} purpose"),
        inputs: vec![],
        outputs: vec![],
        depends_on: depends_on.iter().map(|d| d.to_string()).collect(),
        scope: "probe".to_string(),
    }
}

fn evidence_with(decision: &str, split_count: Option<i64>) -> ForgeGateEvidence {
    ForgeGateEvidence {
        lead_decision: Some(decision.to_string()),
        split_count,
        ..Default::default()
    }
}

#[test]
fn forge_launch_intent_003__split_authorizes_split_only_when_valid() {
    // ── 1. THE CAP: SPLIT INTENT AUTHORIZES THE SPLIT DECISION. ─────────────
    assert!(
        bench_intent_errors(Some("SPLIT"), Some("SPLIT")).is_empty(),
        "a SPLIT bench intent must authorize the SPLIT decision"
    );

    // ── 2. NEGATIVE: SPLIT INTENT REFUSES EVERY NON-SPLIT DECISION. ─────────
    for decision in ["SOLO", "SMITH", "HOLD", "ASSAY"] {
        let errors = bench_intent_errors(Some("SPLIT"), Some(decision));
        assert!(
            !errors.is_empty(),
            "SPLIT intent must refuse decision {decision}"
        );
        assert!(
            errors.iter().all(|error| error.contains("SPLIT")),
            "the refusal must name the cap, not the decision: {errors:?}"
        );
    }

    // ── 3. STRUCTURAL VALIDITY: INDEPENDENT SIBLINGS MAY SPLIT. ─────────────
    let siblings = vec![sibling("smith_a", &[]), sibling("smith_b", &[])];
    let (eligible, reason) = split_eligibility(&siblings);
    assert!(
        eligible,
        "independent siblings must be split-eligible: {reason}"
    );
    assert!(
        !reason.trim().is_empty(),
        "eligibility must carry its reason, never a bare bool"
    );

    // ── 4. NEGATIVE: ONE SIBLING IS NOT A SPLIT. ────────────────────────────
    let (eligible, reason) = split_eligibility(&[sibling("only", &[])]);
    assert!(
        !eligible,
        "a lone node must not be split-eligible: {reason}"
    );

    // ── 5. NEGATIVE: A SIBLING THAT DEPENDS ON A SIBLING GOES SEQUENTIAL. ───
    let chained = vec![sibling("smith_a", &[]), sibling("smith_b", &["smith_a"])];
    let (eligible, reason) = split_eligibility(&chained);
    assert!(
        !eligible,
        "dependent siblings must not be split-eligible: {reason}"
    );
    assert!(
        reason.contains("smith_b") && reason.contains("smith_a"),
        "the refusal must name the dependent and its sibling: {reason}"
    );

    // ── 6. ROUTING VALIDITY: SPLIT OWES ITS COUNT. ──────────────────────────
    assert_eq!(
        LeadHooks.routing_decision_missing(LEAD_DECISION_NODE, &evidence_with("SPLIT", Some(3))),
        None,
        "SPLIT with a positive split count owes nothing"
    );
    assert_eq!(
        LeadHooks.routing_decision_missing(LEAD_DECISION_NODE, &evidence_with("SPLIT", None)),
        Some("lead_decision.splitCount"),
        "SPLIT with no count must be re-asked for it"
    );
    assert_eq!(
        LeadHooks.routing_decision_missing(LEAD_DECISION_NODE, &evidence_with("SPLIT", Some(0))),
        Some("lead_decision.splitCount"),
        "SPLIT with a zero count is not a split"
    );

    // ── 7. CONTROL: NON-SPLIT DECISIONS OWE NO COUNT, UNKNOWN ONES OWE ONE. ─
    assert_eq!(
        LeadHooks.routing_decision_missing(LEAD_DECISION_NODE, &evidence_with("SOLO", None)),
        None,
        "SOLO owes no split count"
    );
    assert_eq!(
        LeadHooks.routing_decision_missing(LEAD_DECISION_NODE, &evidence_with("MAYBE", None)),
        Some("lead_decision"),
        "an unknown decision must be re-asked, or the SPLIT rail would be vacuous"
    );
}
