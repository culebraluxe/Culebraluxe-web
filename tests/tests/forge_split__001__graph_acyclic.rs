//! FORGE.SPLIT — graph acyclic (TST-FORGE-SPLIT-001).
//!
//! Contract: the Lead PRE dependency graph (`forge::engine::graph::plan_smith_layers`) is the
//! production boundary that decides which Smith siblings may run together. An acyclic plan
//! validates with its layers in dependency order; a cyclic plan is invalid, yields no layers,
//! and names the cycle.
//!
//! Level: L0 Pure — the production planner, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_split__001__graph_acyclic

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
fn forge_split_001__graph_acyclic() {
    // Positive: a diamond DAG validates with dependency-ordered layers.
    let dag = vec![
        node("a", &[]),
        node("b", &["a"]),
        node("c", &["a"]),
        node("d", &["b", "c"]),
    ];
    let plan = plan_smith_layers(&dag, 8);
    assert!(plan.valid, "an acyclic graph is valid: {:?}", plan.errors);
    assert!(plan.errors.is_empty(), "an acyclic graph has no errors");
    assert_eq!(
        plan.layers,
        vec![
            vec!["a".to_string()],
            vec!["b".to_string(), "c".to_string()],
            vec!["d".to_string()],
        ],
        "layers follow dependency order: independents share a layer"
    );

    // Negative: a dependency cycle is invalid, yields no layers, and names the cycle.
    let cyclic = vec![node("a", &["b"]), node("b", &["a"])];
    let plan = plan_smith_layers(&cyclic, 8);
    assert!(!plan.valid, "a cyclic graph is invalid");
    assert!(
        plan.layers.is_empty(),
        "a cyclic graph yields no layers: {:?}",
        plan.layers
    );
    assert!(
        plan.errors.iter().any(|e| e.contains("cycle")),
        "the cycle is named in the errors: {:?}",
        plan.errors
    );

    // A node that depends on itself can never run: that is a cycle too.
    let self_loop = vec![node("a", &["a"])];
    let plan = plan_smith_layers(&self_loop, 8);
    assert!(!plan.valid, "a self-dependency is invalid");
    assert!(
        plan.errors.iter().any(|e| e.contains("cycle")),
        "the self-cycle is named in the errors: {:?}",
        plan.errors
    );
}
