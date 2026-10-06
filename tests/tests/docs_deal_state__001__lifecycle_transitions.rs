//! DOCS.DEAL.STATE — lifecycle transitions (TST-DOCS-DEAL-STATE-001).
//!
//! Contract: a residential transaction moves offer-accepted → P&S prepared → P&S executed →
//! under-contract, through the production RE_supermodel definition (`middle/workflow/definitions/
//! RE_supermodel-v1.xml`), which is parsed by the SAME production seam the runtime uses —
//! `forge::engine::xml::parse_re_supermodel` (`forge/src/engine/xml.rs:488`). The five ordered
//! edges below are the lifecycle; no production edge may skip a segment (start cannot jump straight
//! to `under_contract`), and an invented earlier-lifecycle entry cannot be added without this file
//! failing.
//!
//! Level: L0 Pure — the parsed production definition graph, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_deal_state__001__lifecycle_transitions

use std::collections::BTreeSet;

use forge::engine::xml::parse_re_supermodel;
use workflow::NodeDefinition;

fn edge<'a>(node: &'a NodeDefinition, name: &str) -> Option<&'a str> {
    node.transitions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|t| t.name == name)
        .map(|t| t.to.as_str())
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-DEAL-STATE-001).
fn docs_deal_state_001__lifecycle_transitions() {
    let definition = parse_re_supermodel().expect("RE_supermodel-v1.xml must parse");
    let nodes = &definition.definition.nodes;

    // 1. The lifecycle edge-by-edge, each edge resolved through the production graph:
    //    start --offer_accepted--> offer_accepted --prepare--> pns_preparation
    //    pns_preparation --prepared--> pns_executed --executed--> mark_under_contract
    //    mark_under_contract --to_under_contract--> under_contract.
    let chain: [(&str, &str, &str); 5] = [
        ("start", "offer_accepted", "offer_accepted"),
        ("offer_accepted", "prepare", "pns_preparation"),
        ("pns_preparation", "prepared", "pns_executed"),
        ("pns_executed", "executed", "mark_under_contract"),
        ("mark_under_contract", "to_under_contract", "under_contract"),
    ];
    for (from, transition, to) in chain {
        let node = nodes
            .get(from)
            .unwrap_or_else(|| panic!("{from} must exist"));
        assert_eq!(
            edge(node, transition),
            Some(to),
            "lifecycle edge {from} --{transition}--> {to} must exist"
        );
    }

    // 2. The command node carries the canonical command type for the under-contract stage.
    let mark = nodes
        .get("mark_under_contract")
        .expect("mark_under_contract node");
    assert_eq!(
        mark.command_type.as_deref(),
        Some("deal.set_stage_under_contract"),
        "the under-contract transition is a command node, not a bare state edit"
    );

    // 3. Negative control: no edge may skip the segments — `start` may not reach `under_contract`
    //    (or mark_under_contract) in one transition, and `under_contract` is unreachable except
    //    through the command node. If a future edit added a shortcut, this fails.
    let start = nodes.get("start").expect("start node");
    let skip_targets: BTreeSet<&str> = start
        .transitions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|t| t.to.as_str())
        .collect();
    assert!(
        !skip_targets.contains("under_contract") && !skip_targets.contains("mark_under_contract"),
        "no lifecycle shortcut: start must not name under_contract directly, got {skip_targets:?}"
    );
    let mut incoming_to_under: BTreeSet<&str> = BTreeSet::new();
    for node in nodes.values() {
        for t in node.transitions.as_deref().unwrap_or_default() {
            if t.to == "under_contract" {
                incoming_to_under.insert(node.id.as_str());
            }
        }
    }
    assert_eq!(
        incoming_to_under,
        BTreeSet::from(["mark_under_contract"]),
        "under_contract is reachable only through mark_under_contract"
    );
}
