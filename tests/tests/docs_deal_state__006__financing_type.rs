//! DOCS.DEAL.STATE — financing type (TST-DOCS-DEAL-STATE-006).
//!
//! Contract: the workflow never SETS the financing type. `deal.set_financing_type` is routed to the
//! application (`db/deal-financing.ts`) but is deliberately absent from the RE_supermodel as a
//! command-node; the workflow only READS the derived `financingApplicable` fact. Pinning this seam
//! from the parsed production definition (`forge::engine::xml::parse_re_supermodel`):
//!
//!   * NO node carries `command_type == "deal.set_financing_type"` — a command-node here would be the
//!     model mutating application-owned financing state.
//!   * Every financing gate reads the FACT, not a financing-type field: the three places financing
//!     is gated (`financing_applicable`, `financing_deadline_applicable`, `closing_readiness_gate`'s
//!     lender branch) all carry a `financingApplicable == true` arm.
//!   * Cash deals (financingApplicable null/false) route around financing and the lender-clear gate:
//!     both applicable decisions carry a `skip` edge straight to `join_tracks`.
//!
//! Level: L0 Pure — the parsed production definition graph, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_deal_state__006__financing_type

use forge::engine::xml::parse_re_supermodel;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-DEAL-STATE-006).
fn docs_deal_state_006__financing_type() {
    let definition = parse_re_supermodel().expect("RE_supermodel-v1.xml must parse");
    let nodes = &definition.definition.nodes;

    // 1. The workflow is never a writer of financing type.
    let writers: Vec<&str> = nodes
        .values()
        .filter(|n| n.command_type.as_deref() == Some("deal.set_financing_type"))
        .map(|n| n.id.as_str())
        .collect();
    assert!(
        writers.is_empty(),
        "deal.set_financing_type must never appear as a workflow command-node: {writers:?}"
    );

    // 2. The three financing gates all read the derived fact.
    for (node_id, transition) in [
        ("financing_applicable", "run"),
        ("financing_deadline_applicable", "scheduled"),
    ] {
        let node = nodes
            .get(node_id)
            .unwrap_or_else(|| panic!("{node_id} must exist"));
        let arms = node.decisions.as_deref().unwrap_or_default();
        assert!(
            arms.iter().any(|a| a.condition == "financingApplicable == true" && a.transition == transition),
            "{node_id} must gate on the financingApplicable fact: {arms:?}"
        );
        assert!(
            node.transitions
                .as_deref()
                .unwrap_or_default()
                .iter()
                .any(|t| t.name == "skip" && t.to == "join_tracks"),
            "{node_id} must skip non-financed deals straight to the join"
        );
    }
    let readiness = nodes
        .get("closing_readiness_gate")
        .expect("closing_readiness_gate");
    let readiness_arms = readiness.decisions.as_deref().unwrap_or_default();
    assert!(
        readiness_arms
            .iter()
            .any(|a| a.condition == "financingApplicable == true" && a.transition == "lender"),
        "only financed deals cross the lender-clear gate: {readiness_arms:?}"
    );

    // 3. Negative control: no decision arm may branch on a financing-type literal (e.g.
    //    "financingType == ...") — the fact is the whole interface.
    for node in nodes.values() {
        for arm in node.decisions.as_deref().unwrap_or_default() {
            assert!(
                !arm.condition.to_lowercase().contains("financingtype"),
                "{} branches on financing type directly: {:?}",
                node.id,
                arm
            );
        }
    }
}
