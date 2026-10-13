//! Provider materialization: FROM the resolved Forge policy TO the vendor configuration that implements it.
//!
//! THE DIRECTION OF THIS ARROW IS THE POINT. Forge owns policy (`engine::harness_profile`); OpenCode owns provider
//! configuration. A turn under a resolved profile does not have its policy read back OUT of the vendor's profile —
//! it takes the surface Forge granted and selects the shipped vendor profile that implements it. Reading policy out
//! of a provider profile is how the vendor ends up owning the policy, which is the boundary Forge's provider-neutral
//! rule states.
//!
//! WHAT THIS CANNOT DO. It selects a configuration; it cannot compose one, cannot widen one, and cannot name a
//! workflow node. There is no node table here, no transition, and no default: a node nothing binds, or a node whose
//! shipped profile does not implement the granted policy, is a REFUSAL. The vendor's own default agent is never the
//! fallback, because its permissions are the one thing no Forge policy authored.
//!
//! FAIL CLOSED, IN TWO STEPS, and both are load-bearing:
//! 1. the engine binds the node to an agent (`opencode_agents::v2_agent_for_node`), or the turn is refused;
//! 2. that agent's shipped profile IMPLEMENTS the granted [`ActionPolicy`], or the turn is refused.
//!
//! Step 2 is what makes the two tables one grant rather than two that happen to agree today: a lane's granted
//! authority and the configuration that executes it can no longer drift apart silently, and a drift is a refused
//! turn instead of a turn that runs under a surface Forge did not grant.

use crate::engine::action_profiles::{self, ActionPolicy};
use crate::engine::harness_profile::HarnessProfile;
use crate::engine::opencode_agents::{self, V2AgentProfile};
use workflow::{Result, WorkflowError};

/// The vendor configuration that implements one granted policy for one node.
pub fn agent_for_action_policy(
    node_id: &str,
    granted: ActionPolicy,
) -> Result<&'static V2AgentProfile> {
    let agent_id = opencode_agents::v2_agent_for_node(node_id).map_err(WorkflowError::generic)?;
    let vendor = opencode_agents::v2_profile(agent_id).ok_or_else(|| {
        WorkflowError::generic(format!(
            "node {node_id:?} binds to agent {agent_id:?}, which ships no profile; refusing to run the turn \
             under the vendor's own default configuration, whose permissions Forge did not author"
        ))
    })?;
    let implemented = action_profiles::policy_for_profile(vendor);
    if implemented != granted {
        return Err(WorkflowError::generic(format!(
            "the profile Forge ships for {node_id:?} ({agent_id}) implements {implemented:?}, not the {granted:?} \
             the resolved policy granted; refusing to run the turn under a surface Forge did not grant"
        )));
    }
    Ok(vendor)
}

/// The vendor configuration for a turn running under an already-resolved profile.
pub fn agent_for_profile(
    node_id: &str,
    profile: &HarnessProfile,
) -> Result<&'static V2AgentProfile> {
    agent_for_action_policy(node_id, profile.action_policy)
}

/// The vendor configuration for a seam handed no resolved profile.
///
/// Still a grant, never a default: the node's OWN lane policy is resolved first (`harness_profile::policy_for_node`,
/// the same table the profile resolver reads), so an unprofiled seam materializes from the lane it belongs to rather
/// than from nothing. That is the difference between a transition state and an exemption.
pub fn agent_for_lane(node_id: &str) -> Result<&'static V2AgentProfile> {
    let policy = crate::engine::harness_profile::policy_for_node(node_id)?;
    agent_for_action_policy(node_id, policy.action_policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::harness::{HarnessBackend, ModelSelection};
    use crate::engine::harness_profile::{resolve, HarnessProfileInput};
    use crate::engine::role_mapping::LaneId;

    /// Every node a turn can actually be run for: bound in the shipped table, not a human gate, and bound to a lane
    /// with an action policy — the same precondition `roles::lifecycle` checks before it asks any adapter for a turn.
    fn runnable_nodes() -> Vec<String> {
        let bindings = crate::engine::service_binding::forge_service_bindings()
            .unwrap_or_else(|error| panic!("this build ships a service binding table: {error}"));
        bindings
            .keys()
            .filter(|node| !crate::engine::service_binding::is_human_gate(node.as_str()))
            .filter(|node| crate::engine::service_binding::lane_for_node(node.as_str()).is_ok())
            .filter(|node| crate::engine::action_profiles::policy_for_node(node.as_str()).is_ok())
            .cloned()
            .collect()
    }

    fn node_for_lane(lane: LaneId) -> String {
        runnable_nodes()
            .into_iter()
            .find(|node| crate::engine::service_binding::lane_for_node(node).ok() == Some(lane))
            .unwrap_or_else(|| panic!("the shipped table binds a node to {lane:?}"))
    }

    fn profile(node: &str) -> HarnessProfile {
        resolve(&HarnessProfileInput {
            node_id: node,
            backend: HarnessBackend::OpenCode,
            model: ModelSelection::from_parts(None, None),
            execution_id: None,
            role_attempt: 0,
            execution_generation: None,
        })
        .unwrap_or_else(|error| panic!("{node}: {error}"))
    }

    /// The live assertion: every node the engine dispatches materializes, and the configuration it materializes
    /// under a resolved profile is the same one the lane's own grant selects. One policy, one configuration — the
    /// two entry points cannot disagree, because the equality is checked rather than assumed.
    #[test]
    fn every_node_the_engine_runs_materializes_the_agent_its_grant_selects() {
        let nodes = runnable_nodes();
        assert!(!nodes.is_empty(), "the shipped table binds runnable nodes");
        for node in &nodes {
            let lane = agent_for_lane(node).unwrap_or_else(|error| panic!("{node}: {error}"));
            let from_profile = agent_for_profile(node, &profile(node))
                .unwrap_or_else(|error| panic!("{node}: {error}"));
            assert_eq!(
                lane.id, from_profile.id,
                "{node}: the resolved policy and the lane's own grant must select one configuration"
            );
            assert_eq!(
                lane.id,
                opencode_agents::v2_agent_for_node(node).expect("the node is bound"),
                "{node}: materialization SELECTS the shipped configuration rather than composing one"
            );
        }
    }

    /// A grant the shipped configuration does not implement is refused, naming both sides. The refusal is the whole
    /// point: a downgrade to whatever the agent happens to implement would be Forge running a turn under a surface
    /// nobody granted, and a silent acceptance would make step 2 of the check decorative.
    #[test]
    fn a_grant_the_shipped_agent_does_not_implement_is_refused_rather_than_downgraded() {
        let node = node_for_lane(LaneId::Smith);
        let agent = opencode_agents::v2_agent_for_node(&node).expect("a Smith node is bound");
        let implemented = action_profiles::policy_for_profile(
            opencode_agents::v2_profile(agent).expect("it ships"),
        );
        assert_ne!(
            implemented,
            ActionPolicy::StructuredReadOnly,
            "this test needs a grant the shipped configuration does not implement"
        );

        let error = agent_for_action_policy(&node, ActionPolicy::StructuredReadOnly)
            .err()
            .expect("a configuration that does not implement the grant must be refused");
        let detail = error.to_string();
        assert!(
            detail.contains(&node),
            "the refusal names the node: {detail}"
        );
        assert!(
            detail.contains(&format!("{:?}", ActionPolicy::StructuredReadOnly)),
            "and the grant that was asked for: {detail}"
        );
        assert!(
            detail.contains(&format!("{implemented:?}")),
            "and what the shipped configuration actually implements: {detail}"
        );
    }

    /// A node nothing binds has no configuration to materialize, and the vendor's default agent is not an answer:
    /// it is the one configuration whose permissions Forge did not author.
    #[test]
    fn a_node_no_agent_is_bound_to_is_refused_before_any_configuration_is_selected() {
        let error = agent_for_lane("no-such-node-in-any-definition")
            .err()
            .expect("a node nothing binds must not materialize");
        assert!(
            error.to_string().contains("no-such-node-in-any-definition"),
            "the refusal must name the node: {error}"
        );
    }

    /// The authority boundary, as a regression test. The forbidden tokens are assembled with `concat!` so this
    /// test's own source cannot contain them: a guard that matched itself would pass while the rule it guards broke.
    #[test]
    fn this_module_selects_a_configuration_and_cannot_choose_a_workflow_node() {
        let source = include_str!("materialize.rs");
        for forbidden in [
            concat!("Workflow", "Engine"),
            concat!("next", "_node"),
            concat!("transition", "_to"),
            concat!("set", "_next"),
        ] {
            assert!(
                !source.contains(forbidden),
                "the adapter materializes policy; it must not route work: {forbidden}"
            );
        }
    }
}
