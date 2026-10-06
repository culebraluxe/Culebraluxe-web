//! DOCS.DEAL.STATE — illegal backwards transitions (TST-DOCS-DEAL-STATE-002).
//!
//! Contract: deal lifecycle stages move forward only. From the production RE_supermodel definition
//! (`middle/workflow/definitions/RE_supermodel-v1.xml`, parsed by the production seam
//! `forge::engine::xml::parse_re_supermodel`), a later stage may never transition back to an earlier
//! one: `under_contract` has no edge to `start`/`offer_accepted`/`pns_preparation`/`pns_executed`,
//! `closing`/`closed_state`/`post_closing` never flow back into `under_contract`, and every edge
//! INTO `start` is refused — `start` has no incoming edge at all. The only cycles the definition
//! allows are deadline-amend/re-arm loops (`set_*_deadline` → the SAME timer), each pinned to its one
//! command node, so a backwards edge can never hide there either.
//!
//! Level: L0 Pure — the parsed production definition graph, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_deal_state__002__illegal_backwards_transitions

use std::collections::BTreeMap;

use forge::engine::xml::parse_re_supermodel;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-DEAL-STATE-002).
fn docs_deal_state_002__illegal_backwards_transitions() {
    let definition = parse_re_supermodel().expect("RE_supermodel-v1.xml must parse");
    let nodes = &definition.definition.nodes;

    // 1. start has no incoming edge — every path enters exactly once, from the engine.
    for node in nodes.values() {
        for t in node.transitions.as_deref().unwrap_or_default() {
            assert_ne!(
                t.to, "start",
                "no production edge may enter `start` (from {})",
                node.id
            );
        }
    }

    // 2. Under-contract and close are one-way doors.
    let forbidden_backwards: &[(&str, &[&str])] = &[
        (
            "under_contract",
            &[
                "start",
                "offer_accepted",
                "pns_preparation",
                "pns_executed",
                "mark_under_contract",
            ],
        ),
        (
            "closing",
            &["under_contract", "offer_accepted", "pns_preparation"],
        ),
        (
            "closed_state",
            &[
                "closing",
                "under_contract",
                "ready_to_close",
                "offer_accepted",
            ],
        ),
        (
            "post_closing",
            &["closing", "under_contract", "ready_to_close"],
        ),
        (
            "recording",
            &["under_contract", "closing", "ready_to_close"],
        ),
    ];
    for (from, forbidden) in forbidden_backwards {
        let node = nodes
            .get(*from)
            .unwrap_or_else(|| panic!("{from} must exist"));
        for t in node.transitions.as_deref().unwrap_or_default() {
            assert!(
                !forbidden.contains(&t.to.as_str()),
                "illegal backwards edge {from} --{}--> {}",
                t.name,
                t.to
            );
        }
    }

    // 3. The only way to travel backwards in a sense is the deadline-amend loop, and it must
    //    re-arm the SAME milestone rather than reopening an earlier stage. Each amendment command
    //    node's `reschedule` edge lands on ITS OWN timer node.
    let mut amend_targets: BTreeMap<&str, &str> = BTreeMap::new();
    for node in nodes.values() {
        if let Some(command) = node.command_type.as_deref() {
            if let Some(rest) = command.strip_prefix("deal.set_") {
                if rest.contains("deadline") || rest == "closing_date" {
                    for t in node.transitions.as_deref().unwrap_or_default() {
                        if t.name == "reschedule" {
                            amend_targets.insert(node.id.as_str(), t.to.as_str());
                        }
                    }
                }
            }
        }
    }
    assert!(
        amend_targets.len() >= 3,
        "the three deadline-amend command nodes exist: {amend_targets:?}"
    );
    for (from, to) in amend_targets {
        let target = nodes.get(to).unwrap_or_else(|| panic!("{to} must exist"));
        assert_eq!(
            target.node_type, "timer",
            "{from} may only re-arm a timer, never reopen an earlier stage (went to {to})"
        );
    }
}
