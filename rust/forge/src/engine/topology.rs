use std::collections::{BTreeMap, BTreeSet};
use workflow::{NodeDefinition, ProcessGraph};

pub const FORGE_SDLC_KEY: &str = "FORGE_SDLC";
pub const FORGE_SDLC_VERSION: i32 = 6;
pub const POSITIONS: &[&str] = &["scout", "architect", "lead", "smith", "qa", "dev_ops"];

#[derive(Debug, Clone)]
pub struct ForgeSdlcTopology {
    pub key: String,
    pub version: i32,
    pub node_ids: BTreeSet<String>,
    pub tasks: BTreeMap<String, ForgeTaskShape>,
    pub end_ids: BTreeSet<String>,
    pub dynamic_fork_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ForgeTaskShape {
    pub id: String,
    pub label: Option<String>,
    pub responsibility: Option<String>,
}

pub fn topology_from_graph(key: &str, version: i32, graph: &ProcessGraph) -> ForgeSdlcTopology {
    let mut tasks = BTreeMap::new();
    let mut end_ids = BTreeSet::new();
    let mut dynamic_fork_id = None;
    for (id, node) in &graph.nodes {
        match node.node_type.as_str() {
            "task" | "command" => {
                tasks.insert(id.clone(), ForgeTaskShape {
                    id: id.clone(),
                    label: node.name.clone(),
                    responsibility: node.responsibility.clone().or_else(|| {
                        node.candidate_groups.as_ref().and_then(|g| g.first().cloned())
                    }),
                });
            }
            "end" => { end_ids.insert(id.clone()); }
            "dynamic-fork" => dynamic_fork_id = Some(id.clone()),
            _ => {}
        }
    }
    ForgeSdlcTopology {
        key: key.to_string(), version,
        node_ids: graph.nodes.keys().cloned().collect(),
        tasks, end_ids, dynamic_fork_id,
    }
}

pub fn structural_problems(t: &ForgeSdlcTopology) -> Vec<String> {
    let mut problems = Vec::new();
    if t.key != FORGE_SDLC_KEY {
        problems.push(format!("FORGE_SDLC key is '{}', expected '{FORGE_SDLC_KEY}'", t.key));
    }
    if t.version != FORGE_SDLC_VERSION {
        problems.push(format!("FORGE_SDLC version is '{}', expected {FORGE_SDLC_VERSION}", t.version));
    }
    for id in ["start", "classify_work", "execution_shape", "qa_result", "hold"] {
        if !t.node_ids.contains(id) { problems.push(format!("required superset node '{id}' missing")); }
    }
    for end in ["complete", "cancelled", "failed", "archive_research"] {
        if !t.end_ids.contains(end) { problems.push(format!("required terminus '{end}' missing")); }
    }
    match &t.dynamic_fork_id {
        Some(id) if t.tasks.contains_key(id) => problems.push(format!("dynamic-fork '{id}' must not be a task/command node")),
        None => problems.push("FORGE_SDLC must declare a dynamic SPLIT fork (split_dispatch)".into()),
        _ => {}
    }
    for task in t.tasks.values() {
        let r = task.responsibility.as_deref().unwrap_or("");
        if !POSITIONS.contains(&r) {
            problems.push(format!("node '{}' has responsibility '{}', which is not a Forge position", task.id, if r.is_empty() { "(none)" } else { r }));
        }
    }
    problems
}

pub fn ensure_topology(t: &ForgeSdlcTopology) -> Result<(), String> {
    let problems = structural_problems(t);
    if problems.is_empty() { Ok(()) } else {
        Err(format!("FORGE_SDLC topology invariant violated:\n{}", problems.iter().map(|p| format!("  - {p}")).collect::<Vec<_>>().join("\n")))
    }
}

pub fn with_position(mut node: NodeDefinition, position: &str) -> NodeDefinition {
    node.description = Some(position.to_string());
    let mut groups = node.candidate_groups.unwrap_or_default();
    if !groups.iter().any(|g| g == position) { groups.insert(0, position.to_string()); }
    node.candidate_groups = Some(groups);
    node
}
