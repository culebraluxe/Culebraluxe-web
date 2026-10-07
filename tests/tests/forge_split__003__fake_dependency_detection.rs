//! FORGE.SPLIT — fake dependency detection (TST-FORGE-SPLIT-003).
//!
//! Contract: a declared dependency edge is genuine only when the dependent actually consumes
//! something the dependency produces (`forge::engine::graph::fake_edge_candidates`). An edge
//! whose inputs touch none of the dependency's outputs is flagged as fake, so a stale or
//! copy-pasted `depends_on` cannot serialize two siblings that are really independent —
//! and a genuine producer/consumer edge is never flagged.
//!
//! Level: L0 Pure — the production graph analysis, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_split__003__fake_dependency_detection

use forge::engine::graph::{fake_edge_candidates, SmithWorkNode};

/// A Smith work node whose data surface is explicit: what it reads and what it writes.
fn node(id: &str, inputs: &[&str], outputs: &[&str], depends_on: &[&str]) -> SmithWorkNode {
    SmithWorkNode {
        id: id.into(),
        purpose: format!("purpose of {id}"),
        inputs: inputs.iter().map(|i| i.to_string()).collect(),
        outputs: outputs.iter().map(|o| o.to_string()).collect(),
        depends_on: depends_on.iter().map(|d| d.to_string()).collect(),
        scope: format!("scope of {id}"),
    }
}

#[test]
fn forge_split_003__fake_dependency_detection() {
    let nodes = vec![
        node("migrate", &[], &["schema.sql"], &[]),
        // Declares a dependency on `migrate` but reads nothing it wrote: fake.
        node("seed", &["seed-data.csv"], &["rows"], &["migrate"]),
        // Reads exactly what `migrate` wrote: a genuine producer/consumer edge.
        node("backfill", &["schema.sql"], &["report"], &["migrate"]),
        // Depends on a node the plan does not know: skipped, not flagged either way.
        node("orphan", &["x"], &["y"], &["ghost"]),
    ];
    let fake = fake_edge_candidates(&nodes);

    // Positive: the stale edge is flagged, naming both ends in dependency order.
    assert!(
        fake.contains(&("migrate".to_string(), "seed".to_string())),
        "the edge no data flows across is fake: {fake:?}"
    );
    // Negative: the genuine edge is not flagged, and unknown nodes are skipped.
    assert!(
        !fake.contains(&("migrate".to_string(), "backfill".to_string())),
        "the edge data flows across is genuine, never fake: {fake:?}"
    );
    assert!(
        !fake.iter().any(|(_, b)| b == "orphan"),
        "a dependency on an unknown node is skipped, not judged: {fake:?}"
    );
    assert_eq!(
        fake.len(),
        1,
        "exactly the fake edge is flagged and nothing else: {fake:?}"
    );
}
