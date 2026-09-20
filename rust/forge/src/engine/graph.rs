//! Lead PRE dependency graph. Parallelism is an optimization; correctness is
//! the invariant. Ambiguous independence favors sequential execution.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone)]
pub struct SmithWorkNode {
    pub id: String,
    pub purpose: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub depends_on: Vec<String>,
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerPlan {
    pub layers: Vec<Vec<String>>,
    pub valid: bool,
    pub errors: Vec<String>,
}

pub fn plan_smith_layers(nodes: &[SmithWorkNode], concurrency_cap: usize) -> LayerPlan {
    let cap = concurrency_cap.max(1);
    let by_id: BTreeMap<&str, &SmithWorkNode> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut errors = Vec::new();
    for n in nodes {
        for d in &n.depends_on {
            if !by_id.contains_key(d.as_str()) {
                errors.push(format!("{} depends on unknown node {d}", n.id));
            }
        }
    }

    let mut indeg: BTreeMap<&str, usize> = BTreeMap::new();
    let mut adj: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for n in nodes {
        indeg.insert(&n.id, n.depends_on.len());
        for d in &n.depends_on {
            adj.entry(d.as_str()).or_default().push(&n.id);
        }
    }
    let mut queue: VecDeque<&str> = indeg
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut order = Vec::new();
    while let Some(cur) = queue.pop_front() {
        order.push(cur);
        for next in adj.get(cur).into_iter().flatten() {
            if let Some(d) = indeg.get_mut(next) {
                *d -= 1;
                if *d == 0 {
                    queue.push_back(next);
                }
            }
        }
    }
    if order.len() != nodes.len() {
        return LayerPlan {
            layers: vec![],
            valid: false,
            errors: if errors.is_empty() {
                vec!["dependency cycle detected in Smith plan".into()]
            } else {
                errors
            },
        };
    }

    let mut layer_of: BTreeMap<&str, usize> = BTreeMap::new();
    let mut layers: Vec<Vec<String>> = Vec::new();
    for id in order {
        let n = by_id[id];
        let li = if n.depends_on.is_empty() {
            0
        } else {
            n.depends_on
                .iter()
                .map(|d| *layer_of.get(d.as_str()).unwrap_or(&0))
                .max()
                .unwrap_or(0)
                + 1
        };
        while layers.len() <= li {
            layers.push(vec![]);
        }
        layers[li].push(id.to_string());
        layer_of.insert(id, li);
    }

    let mut bounded = Vec::new();
    for layer in layers {
        for chunk in layer.chunks(cap) {
            bounded.push(chunk.to_vec());
        }
    }
    LayerPlan {
        layers: bounded,
        valid: errors.is_empty(),
        errors,
    }
}

pub fn split_eligibility(nodes: &[SmithWorkNode]) -> (bool, String) {
    if nodes.len() <= 1 {
        return (false, "splitting requires more than one sibling".into());
    }
    let ids: BTreeSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    for n in nodes {
        let internal: Vec<_> = n
            .depends_on
            .iter()
            .filter(|d| ids.contains(d.as_str()))
            .cloned()
            .collect();
        if !internal.is_empty() {
            return (
                false,
                format!(
                    "{} depends on a sibling ({}) -> sequential, not SPLIT",
                    n.id,
                    internal.join(",")
                ),
            );
        }
    }
    (true, "siblings are pairwise independent".into())
}

pub fn fake_edge_candidates(nodes: &[SmithWorkNode]) -> Vec<(String, String)> {
    let by_id: BTreeMap<&str, &SmithWorkNode> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut fake = Vec::new();
    for b in nodes {
        for dep in &b.depends_on {
            let Some(a) = by_id.get(dep.as_str()) else {
                continue;
            };
            let consumes = b.inputs.iter().any(|i| a.outputs.iter().any(|o| o == i));
            if !consumes {
                fake.push((a.id.clone(), b.id.clone()));
            }
        }
    }
    fake
}
