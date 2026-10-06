//! DOCS.DEAL.STATE — lender clear to close (TST-DOCS-DEAL-STATE-008).
//!
//! Contract: the canonical `deal.lender_clear_to_close` fact gates closing readiness for financed
//! deals only. From the parsed production definition (`parse_re_supermodel`):
//!
//!   * Cash deals route around the fact: `closing_readiness_gate` enters `lender_clearance_gate` ONLY
//!     on `financingApplicable == true`.
//!   * `lenderClearToClose == true` clears to `closing_confirmation_gate`.
//!   * NULL is explicit: `lenderClearToClose == null` → `lender_clearance_resolution`, whose
//!     `resolved` re-enters the SAME gate; `false` (pending) blocks readiness at
//!     `lender_clearance_pending` and never reaches the confirmation gate.
//!   * The fact is recorded by the application command `deal.set_lender_clear_to_close`, which must
//!     NOT appear as a workflow command-node (lender provider behavior is never modeled in the engine).
//!
//! Level: L0 Pure — the parsed production definition graph, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_deal_state__008__lender_clear_to_close

use forge::engine::xml::parse_re_supermodel;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-DEAL-STATE-008).
fn docs_deal_state_008__lender_clear_to_close() {
    let definition = parse_re_supermodel().expect("RE_supermodel-v1.xml must parse");
    let nodes = &definition.definition.nodes;

    let readiness = nodes
        .get("closing_readiness_gate")
        .expect("closing_readiness_gate");
    let readiness_arms = readiness.decisions.as_deref().unwrap_or_default();
    assert!(
        readiness_arms
            .iter()
            .any(|a| a.condition == "financingApplicable == true" && a.transition == "lender"),
        "only financed deals cross the lender gate: {readiness_arms:?}"
    );
    let lender = readiness
        .transitions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|t| t.name == "lender")
        .expect("lender transition");
    assert_eq!(lender.to, "lender_clearance_gate");

    let gate = nodes
        .get("lender_clearance_gate")
        .expect("lender_clearance_gate");
    let arms = gate.decisions.as_deref().unwrap_or_default();
    assert!(
        arms.iter()
            .any(|a| a.condition == "lenderClearToClose == true" && a.transition == "cleared"),
        "cleared requires the true fact: {arms:?}"
    );
    assert!(
        arms.iter()
            .any(|a| a.condition == "lenderClearToClose == null" && a.transition == "unresolved"),
        "null is explicit, never silently skipped: {arms:?}"
    );
    let unresolved = gate
        .transitions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|t| t.name == "unresolved")
        .expect("unresolved");
    assert_eq!(unresolved.to, "lender_clearance_resolution");
    let pending = gate
        .transitions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|t| t.name == "pending")
        .expect("pending");
    assert_eq!(pending.to, "lender_clearance_pending");

    let cleared = gate
        .transitions
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|t| t.name == "cleared")
        .expect("cleared");
    assert_eq!(cleared.to, "closing_confirmation_gate");

    // The resolution task re-enters the same gate; the pending task likewise — both may only loop
    // back or terminate, never bypass to ready_to_close.
    for node_id in ["lender_clearance_resolution", "lender_clearance_pending"] {
        let task = nodes.get(node_id).unwrap_or_else(|| panic!("{node_id}"));
        for t in task.transitions.as_deref().unwrap_or_default() {
            match t.name.as_str() {
                "resolved" => assert_eq!(t.to, "lender_clearance_gate"),
                "escalate" => assert_eq!(t.to, "transaction_failed"),
                other => panic!("unexpected edge {node_id} --{other}--> {}", t.to),
            }
        }
    }

    // Negative control: the engine never records the fact itself.
    let writers: Vec<&str> = nodes
        .values()
        .filter(|n| n.command_type.as_deref() == Some("deal.set_lender_clear_to_close"))
        .map(|n| n.id.as_str())
        .collect();
    assert!(
        writers.is_empty(),
        "the workflow must not own deal.set_lender_clear_to_close: {writers:?}"
    );
}
