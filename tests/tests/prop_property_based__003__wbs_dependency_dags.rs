//! PROP.PROPERTY_BASED — WBS dependency DAGs (TST-PROP-PROPERTY-BASED-003).
//!
//! Contract: a WBS dependency edge `source -> target` is refused exactly when
//! `target` can already reach `source` (self-edges always refuse), so the
//! stored graph stays a DAG. The Smith layer planner then emits every node
//! exactly once in dependency order, and answers a cyclic graph with
//! `valid: false` instead of a plan.
//!
//! Level: L0 Pure — the executable boundaries are `model::wbs` and
//! `forge::engine::graph`, no I/O.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test prop_property_based__003__wbs_dependency_dags

use forge::engine::graph::{plan_smith_layers, SmithWorkNode};
use model::wbs::{dependency_creates_cycle, WbsDependency};
use proptest::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

fn node_id() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("a".to_string()),
        Just("b".to_string()),
        Just("c".to_string()),
        Just("d".to_string()),
    ]
}

fn edge() -> impl Strategy<Value = WbsDependency> {
    (node_id(), node_id()).prop_map(|(source_id, target_id)| WbsDependency {
        project_id: "p".to_string(),
        source_id,
        target_id,
        kind: "finish_to_start".to_string(),
    })
}

/// Independent oracle for the cycle check: Floyd-Warshall transitive closure
/// over the edge set (a different algorithm from the production DFS), asking
/// whether `target` reaches `source`.
fn reaches(edges: &[WbsDependency], from: &str, to: &str) -> bool {
    if from == to {
        return true;
    }
    let mut nodes: BTreeSet<&str> = BTreeSet::new();
    for edge in edges {
        nodes.insert(edge.source_id.as_str());
        nodes.insert(edge.target_id.as_str());
    }
    nodes.insert(from);
    nodes.insert(to);
    let index: BTreeMap<&str, usize> = nodes.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    let len = nodes.len();
    let mut closed = vec![vec![false; len]; len];
    for edge in edges {
        closed[index[edge.source_id.as_str()]][index[edge.target_id.as_str()]] = true;
    }
    for k in 0..len {
        for i in 0..len {
            for j in 0..len {
                closed[i][j] = closed[i][j] || (closed[i][k] && closed[k][j]);
            }
        }
    }
    closed[index[from]][index[to]]
}

fn work_node(id: &str, depends_on: Vec<String>) -> SmithWorkNode {
    SmithWorkNode {
        id: id.to_string(),
        purpose: "property test node".to_string(),
        inputs: vec![],
        outputs: vec![],
        depends_on,
        scope: "test".to_string(),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// The cycle check agrees with transitive closure on every generated edge
    /// set; the layer planner orders every node after its dependencies and
    /// calls a cycle invalid; fixed vectors pin the documented refusals.
    #[test]
    #[allow(non_snake_case)]
    fn prop_property_based_003__wbs_dependency_dags(
        edges in prop::collection::vec(edge(), 0..8),
        source in node_id(),
        target in node_id(),
        deps in prop::collection::vec(node_id(), 0..3),
    ) {
        // Fixed positives: an unrelated edge and a forward edge are safe.
        let chain = vec![
            WbsDependency { project_id: "p".into(), source_id: "a".into(), target_id: "b".into(), kind: "finish_to_start".into() },
        ];
        prop_assert!(!dependency_creates_cycle(&chain, "b", "c"));
        prop_assert!(!dependency_creates_cycle(&[], "a", "b"));
        // Fixed negatives: self-edges and back-edges refuse.
        prop_assert!(dependency_creates_cycle(&chain, "a", "a"));
        prop_assert!(dependency_creates_cycle(&chain, "b", "a"));
        let longer = vec![
            WbsDependency { project_id: "p".into(), source_id: "a".into(), target_id: "b".into(), kind: "finish_to_start".into() },
            WbsDependency { project_id: "p".into(), source_id: "b".into(), target_id: "c".into(), kind: "finish_to_start".into() },
        ];
        prop_assert!(dependency_creates_cycle(&longer, "c", "a"));

        // Property: the production DFS agrees with transitive closure —
        // refuse exactly when `target` already reaches `source`.
        prop_assert_eq!(
            dependency_creates_cycle(&edges, &source, &target),
            reaches(&edges, &target, &source),
            "cycle check disagrees with transitive closure for {}->{}",
            source,
            target
        );

        // Property: a valid layer plan holds every node exactly once, each
        // node after its dependencies.
        let ids = ["a", "b", "c", "d"];
        let nodes: Vec<SmithWorkNode> = ids
            .iter()
            .map(|id| {
                let known: BTreeSet<&str> = ids.iter().copied().collect();
                let mut only_known: Vec<String> = deps
                    .iter()
                    .filter(|d| d.as_str() != *id && known.contains(d.as_str()))
                    .cloned()
                    .collect();
                only_known.sort();
                only_known.dedup();
                work_node(id, only_known)
            })
            .collect();
        let plan = plan_smith_layers(&nodes, 8);
        let flat: Vec<&String> = plan.layers.iter().flatten().collect();
        if plan.valid {
            prop_assert_eq!(flat.len(), nodes.len(), "a valid plan places every node once");
            let position: BTreeMap<&str, usize> = flat
                .iter()
                .enumerate()
                .map(|(i, id)| (id.as_str(), i))
                .collect();
            for node in &nodes {
                for dep in &node.depends_on {
                    prop_assert!(
                        position[dep.as_str()] < position[node.id.as_str()],
                        "{} must come after its dependency {}",
                        node.id,
                        dep,
                    );
                }
            }
        }

        // Property/fault: a two-cycle is never a valid plan.
        let cyclic = vec![
            work_node("a", vec!["b".to_string()]),
            work_node("b", vec!["a".to_string()]),
        ];
        let refused = plan_smith_layers(&cyclic, 8);
        prop_assert!(!refused.valid, "a dependency cycle must not plan valid");
        prop_assert!(refused.layers.is_empty(), "a cycle must yield no layers");
    }
}
