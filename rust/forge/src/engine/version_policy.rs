//! Port of workflow_app/definitions/version-policy.ts. Pure. No Neon.

use workflow::{json_codec::graph_to_json, ProcessGraph};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefinitionVersionPolicy {
    New,
    Replaceable,
    Immutable,
}

pub const IMMUTABLE_DEFINITION_ERROR: &str =
    "Refusing to replace a deployed process-definition version that already has instances; deploy a new version instead.";

pub fn definition_version_policy(row_exists: bool, instance_count: i32) -> DefinitionVersionPolicy {
    if !row_exists {
        DefinitionVersionPolicy::New
    } else if instance_count > 0 {
        DefinitionVersionPolicy::Immutable
    } else {
        DefinitionVersionPolicy::Replaceable
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeployDecision {
    Insert {
        reason: &'static str,
    },
    Update {
        duplicate: bool,
        reason: &'static str,
    },
    Reject {
        message: &'static str,
    },
}

pub fn graphs_equal(a: &ProcessGraph, b: &ProcessGraph) -> bool {
    graph_to_json(a) == graph_to_json(b)
}

pub fn classify_deploy(
    row_exists: bool,
    instance_count: i32,
    previous: Option<&ProcessGraph>,
    incoming: &ProcessGraph,
) -> DeployDecision {
    if !row_exists {
        return DeployDecision::Insert { reason: "new" };
    }
    if instance_count > 0 {
        return DeployDecision::Reject {
            message: IMMUTABLE_DEFINITION_ERROR,
        };
    }
    let duplicate = previous.map(|p| graphs_equal(p, incoming)).unwrap_or(false);
    DeployDecision::Update {
        duplicate,
        reason: "replaceable",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use workflow::ProcessGraph;

    #[test]
    fn executed_version_is_immutable() {
        match classify_deploy(
            true,
            1,
            None,
            &ProcessGraph {
                nodes: Default::default(),
                start_node_id: "s".into(),
                display_order: None,
            },
        ) {
            DeployDecision::Reject { .. } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn missing_row_inserts() {
        match classify_deploy(
            false,
            0,
            None,
            &ProcessGraph {
                nodes: Default::default(),
                start_node_id: "s".into(),
                display_order: None,
            },
        ) {
            DeployDecision::Insert { .. } => {}
            other => panic!("{other:?}"),
        }
    }
}
