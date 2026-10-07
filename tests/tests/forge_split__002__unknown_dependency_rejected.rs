//! FORGE.SPLIT — unknown dependency rejected (TST-FORGE-SPLIT-002).
//!
//! Contract: the Lead PRE dependency graph (`forge::engine::graph::plan_smith_layers`) is the
//! production boundary that decides which Smith siblings may run together. A node that depends
//! on a node the plan does not know is rejected: the plan is invalid and the error names the
//! unknown dependency, so a typo'd edge can never silently serialize — or silently parallelize
//! — a sibling.
//!
//! Level: L0 Pure — the production planner, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_split__002__unknown_dependency_rejected

use forge::engine::graph::{plan_smith_layers, SmithWorkNode};

/// A Smith work node with no file surface: only the dependency edges matter here.
fn node(id: &str, depends_on: &[&str]) -> SmithWorkNode {
    SmithWorkNode {
        id: id.into(),
        purpose: format!("purpose of {id}"),
        inputs: vec![],
        outputs: vec![],
        depends_on: depends_on.iter().map(|d| d.to_string()).collect(),
        scope: format!("scope of {id}"),
    }
}

#[test]
fn forge_split_002__unknown_dependency_rejected() {
    // Positive: a dependency on a node the plan does not know is rejected.
    let nodes = vec![node("a", &[]), node("x", &["ghost"])];
    let plan = plan_smith_layers(&nodes, 8);
    assert!(!plan.valid, "an unknown dependency is invalid");
    assert!(
        plan.errors
            .iter()
            .any(|e| e.contains("ghost") && e.contains("unknown")),
        "the error names the unknown node: {:?}",
        plan.errors
    );
    assert!(
        plan.errors.iter().any(|e| e.contains('x')),
        "the error names the dependent that declared it: {:?}",
        plan.errors
    );

    // Negative: the same shape with every dependency known validates cleanly, so the
    // rejection above is about the unknown edge and nothing else.
    let known = vec![node("a", &[]), node("x", &["a"])];
    let plan = plan_smith_layers(&known, 8);
    assert!(
        plan.valid,
        "known dependencies are valid: {:?}",
        plan.errors
    );
    assert!(
        plan.errors.is_empty(),
        "known dependencies produce no errors"
    );
    assert_eq!(
        plan.layers,
        vec![vec!["a".to_string()], vec!["x".to_string()]],
        "the known edge still orders the layers"
    );
}
