//! The resolved execution profile for one already-authorized Forge role turn.
//!
//! WHAT THIS ANSWERS: *how* an authorized role executes — its model intent, the action surface its tools may
//! use, the context budget its request is prepared against, whether planning is granted, and the thresholds
//! that call this attempt stuck.
//!
//! WHAT THIS CANNOT ANSWER, BY CONSTRUCTION: *what* executes next. There is no node selection, no edge and no
//! routing surface in this file. The XML workflow decides which node is authorized; a node arrives here
//! already chosen, and the turn's result goes back to the deterministic engine, which applies the step its
//! definition names. A resolver that could name the next node would be a second workflow engine, and Forge
//! has exactly one.
//!
//! LAYERING (Forge owns policy; the provider owns provider configuration):
//!
//! ```text
//! XML authorizes a node
//!         ↓
//! service_binding::lane_for_node           (Forge-neutral lane identity)
//!         ↓
//! this module's resolve()                  (Forge policy: deterministic, pure, fail-closed)
//!         ↓
//! HarnessProfile
//!         ↓
//! provider adapter materializes its own configuration (opencode::materialize → V2AgentProfile)
//! ```
//!
//! The direction of that arrow is the point. A provider profile is materialized FROM Forge policy; Forge
//! policy is never read out OF a provider profile, because then the vendor would own the policy.

use crate::engine::action_profiles::ActionPolicy;
use crate::engine::context_policy::ContextPolicy;
use crate::engine::harness::{HarnessBackend, ModelSelection};
use crate::engine::planning::PlanningPolicy;
use crate::engine::role_mapping::LaneId;
use crate::engine::runaway::{RunawayPolicy, NO_PROGRESS_WINDOW_DISABLED};
use crate::engine::service_binding::lane_for_node;
use workflow::{Result, WorkflowError};

/// The profile family this module resolves. One family today. A different family (a bench profile, say) is a
/// new id at the same version, never a silent reuse of this one.
pub const HARNESS_PROFILE_ID: &str = "forge-role-profile";

/// The version of the RESOLVED shape. A profile is only reconstructable against the version that produced it,
/// so this belongs in the identity a turn records rather than in a comment. Bump it when a field changes
/// meaning, or when a table below changes a default: the same input resolving differently under the same
/// version is exactly what makes a recorded profile unreadable.
pub const HARNESS_PROFILE_VERSION: u32 = 1;

/// Enough identity to reconstruct, from a turn's own record, which configuration actually executed it.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileIdentity {
    pub profile_id: &'static str,
    pub profile_version: u32,
    /// The workflow node this turn was authorized for — the node the WORKFLOW chose. It is recorded as
    /// identity, never produced as a decision by this module.
    pub node_id: String,
    /// The Forge lane the node resolved to (`service_binding`), and therefore the role that executed.
    pub lane: LaneId,
    /// Which backend executes. It is part of the identity because one Forge policy materializes differently
    /// on each provider, and a turn must be able to say which one ran it.
    pub backend: HarnessBackend,
    /// Forge's model intent (cheap/judgment/explicit), never a vendor model name. The adapter translates it.
    pub model: ModelSelection,
    /// The execution this turn belongs to, when the lane runs under a scoped interrupt handle.
    pub execution_id: Option<String>,
    /// Where this attempt sits in the lane's bounded retry loop: 0 is the first attempt, and every later
    /// attempt is a repair of the same authorized node rather than a new unit of work.
    pub role_attempt: u32,
    /// The generation that paid for the turn: the discriminator a REPLAN changes. The same attempt under a new
    /// generation is a different execution, and the identity says so.
    pub execution_generation: Option<String>,
}

/// The one profile an authorized role turn executes under.
///
/// Every field is an existing canonical Forge type rather than a new parallel one: the action surface is
/// `action_profiles::ActionPolicy`, the budget is `context_policy::ContextPolicy`, planning is
/// `planning::PlanningPolicy`, and the stuck thresholds are `runaway::RunawayPolicy`. This type composes
/// policy that already exists; it does not restate it.
#[derive(Debug, Clone, PartialEq)]
pub struct HarnessProfile {
    pub identity: ProfileIdentity,
    /// What this role's tools may DO with its work. It never decides what happens next.
    pub action_policy: ActionPolicy,
    /// The context budget the attempt trail that feeds this turn's retries is governed by.
    ///
    /// LIVE. The one piece of context Forge composes and re-sends is the corrective directive for the NEXT
    /// attempt, and this policy decides what survives into it
    /// (`self_heal::build_self_heal_directive_contextual`, called by `roles::lifecycle` with the SAME resolved
    /// profile that the attempt's harness was handed). No other prompt is Forge's to trim: the provider builds
    /// those, and Forge does not claim a budget it cannot apply.
    pub context_policy: ContextPolicy,
    /// How this turn is permitted to plan, if at all. `None` adds no text to a prompt.
    ///
    /// LIVE. Read where the turn's task text is constructed (`packet::TaskText`), from the same resolved profile:
    /// the profile decides WHICH scaffold a lane gets, and the packet decides what that scaffold may state.
    pub planning_policy: PlanningPolicy,
    /// The thresholds that declare this attempt stuck.
    ///
    /// LIVE. Read by the guard over the turn's own action stream (`opencode::LiveRunaway`). A seam handed no
    /// resolved profile resolves the node's own lane grant instead (`policy_for_node`), so no turn runs
    /// unguarded and no adapter authors a threshold Forge never resolved.
    pub runaway_policy: RunawayPolicy,
}

impl HarnessProfile {
    /// Whether this profile's action surface permits workspace edits.
    ///
    /// This is a fact the runtime may check, NOT a grant: the authority a vendor enforces remains the agent
    /// profile Forge shipped for this lane.
    pub fn may_edit(&self) -> bool {
        self.action_policy.surface().edit
    }
}

/// Every fact the resolver reads.
///
/// All of it is either already selected by the deterministic engine or a plain description of the turn. The
/// resolver reads no clock, no environment and no probe, which is what makes two equal inputs produce equal
/// profiles (see `the_same_input_always_resolves_to_the_same_profile`).
#[derive(Debug, Clone)]
pub struct HarnessProfileInput<'a> {
    pub node_id: &'a str,
    pub backend: HarnessBackend,
    /// Forge's model intent for this run: the row's `model_policy` (migration 179) with any attended override
    /// already applied, as Forge decided it at launch. Never a vendor model name — an adapter translates it.
    pub model: ModelSelection,
    pub execution_id: Option<&'a str>,
    /// The engine's own attempt counter (`u32`, the type its budget is measured in), so the seam never casts it.
    pub role_attempt: u32,
    pub execution_generation: Option<&'a str>,
}

/// Every policy a node's lane is granted, with no TURN of its own attached.
///
/// This is the profile minus a turn's identity: the same table, read by a seam that composes a turn's text or
/// guards its action stream but has no resolved profile to read the answers from. It exists so those seams do not
/// each re-derive a policy — and, more to the point, so none of them runs with NO policy while a comment claims a
/// transition made that acceptable. A turn under an unstated grant is a turn under a grant Forge made and then
/// forgot to apply.
///
/// The LANE is carried because it is the key the policies were resolved BY, and a policy that could not say which
/// lane it came from could not be checked against the node it was resolved for. What is deliberately absent is the
/// turn: no attempt index, no execution id, no generation.
#[derive(Debug, Clone, PartialEq)]
pub struct LanePolicy {
    pub lane: LaneId,
    pub action_policy: ActionPolicy,
    pub context_policy: ContextPolicy,
    pub planning_policy: PlanningPolicy,
    pub runaway_policy: RunawayPolicy,
}

/// Resolve the policy a node's own lane is granted, WITHOUT a turn's identity.
///
/// FAIL CLOSED, one layer down: a node the engine cannot bind to a lane, or a lane with no action policy, has no
/// policy to hand out and is refused with the same error the profile resolver reports. Returning `None` would
/// make "Forge cannot place this node" indistinguishable from "this lane grants nothing", and those are different
/// statements — the first is a broken dispatch, the second is a policy.
pub fn policy_for_node(node_id: &str) -> Result<LanePolicy> {
    let lane = lane_for_node(node_id).map_err(WorkflowError::generic)?;
    // The canonical table, not a copy of it: this call fails closed for a node with no agent profile.
    let action_policy =
        crate::engine::action_profiles::policy_for_node(node_id).map_err(WorkflowError::generic)?;
    Ok(LanePolicy {
        lane,
        action_policy,
        context_policy: ContextPolicy::default(),
        planning_policy: planning_for(lane, action_policy),
        runaway_policy: runaway_for(lane),
    })
}

/// Resolve the profile for one authorized role turn.
///
/// DETERMINISTIC: a pure function of the input. No environment, no clock, no probe, no I/O.
///
/// FAIL CLOSED: a node the engine cannot bind to a lane is refused, and so is a node whose lane has no action
/// policy. There is deliberately no default profile — running a node under an inherited surface would hand a
/// node authority nobody granted it, which is the same refusal `service_binding::lane_for_node` and
/// `opencode_agents::v2_agent_for_node` already make one layer down.
///
/// The POLICY comes from [`policy_for_node`] rather than from a second copy of the table: a profile is that
/// policy plus the identity of the turn it is being applied to, and an identity cannot disagree with a policy
/// when there is only one table behind both.
pub fn resolve(input: &HarnessProfileInput<'_>) -> Result<HarnessProfile> {
    let policy = policy_for_node(input.node_id)?;
    Ok(HarnessProfile {
        identity: ProfileIdentity {
            profile_id: HARNESS_PROFILE_ID,
            profile_version: HARNESS_PROFILE_VERSION,
            node_id: input.node_id.to_string(),
            lane: policy.lane,
            backend: input.backend,
            model: input.model.clone(),
            execution_id: input.execution_id.map(str::to_string),
            role_attempt: input.role_attempt,
            execution_generation: input.execution_generation.map(str::to_string),
        },
        action_policy: policy.action_policy,
        context_policy: policy.context_policy,
        planning_policy: policy.planning_policy,
        runaway_policy: policy.runaway_policy,
    })
}

/// Whether planning is granted, per lane.
///
/// Planning shapes how a role approaches work it is already authorized to do, so it is granted only where
/// there is work to shape: a surface that may not edit is given no scaffold at all.
///
/// The match is exhaustive over `LaneId` on purpose. A new lane must be given a decision here rather than
/// inheriting one, so adding a lane is a visible policy edit instead of a silent default.
fn planning_for(lane: LaneId, action_policy: ActionPolicy) -> PlanningPolicy {
    match lane {
        LaneId::Smith if action_policy.surface().edit => PlanningPolicy::ProgressContract,
        LaneId::Lead if action_policy.surface().edit => PlanningPolicy::Guided,
        LaneId::Scout
        | LaneId::Architect
        | LaneId::Lead
        | LaneId::Smith
        | LaneId::Assay
        | LaneId::Inspector
        | LaneId::DevOps => PlanningPolicy::None,
    }
}

/// The stuck thresholds for a lane. Configuration, per lane, resolved here.
///
/// Exhaustive over `LaneId` on purpose, like `planning_for` above: a new lane is given a decision here rather than
/// inheriting one.
///
/// The REPETITION threshold is the module's own default in every lane, because an identical action repeated three
/// times in a row is the same observation wherever it happens. The NO-PROGRESS window is what differs, and it
/// differs for a measured reason: this detector sees progress partly as `files_changed`, and a lane that may not
/// edit changes no files by construction — so a window tuned for an implementing lane would eventually stop a
/// long, legitimate read. Those lanes keep repetition and oscillation, which need no file change to be seen.
///
/// This is the decision the resolver makes, not a claim about any model: the thresholds are thresholds, and the
/// thing that consults them is the turn's own action stream (`engine::opencode`).
fn runaway_for(lane: LaneId) -> RunawayPolicy {
    match lane {
        LaneId::Smith | LaneId::Lead => RunawayPolicy::default(),
        LaneId::Scout | LaneId::Architect | LaneId::Assay | LaneId::Inspector | LaneId::DevOps => {
            RunawayPolicy {
                no_progress_window: NO_PROGRESS_WINDOW_DISABLED,
                ..RunawayPolicy::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every node a turn can actually be run for: bound in the shipped table, not a human gate, and bound to a
    /// lane — the same precondition `roles::lifecycle` checks before it asks any adapter for a turn.
    ///
    /// The nodes come from the table the binary ships, not from names typed here, so the test cannot drift into
    /// asserting a vocabulary Forge does not have.
    fn runnable_nodes() -> Vec<&'static str> {
        let bindings = crate::engine::service_binding::forge_service_bindings()
            .unwrap_or_else(|error| panic!("this build ships a service binding table: {error}"));
        bindings
            .keys()
            .filter(|node| !crate::engine::service_binding::is_human_gate(node.as_str()))
            .filter(|node| crate::engine::service_binding::lane_for_node(node.as_str()).is_ok())
            .filter(|node| crate::engine::action_profiles::policy_for_node(node.as_str()).is_ok())
            .map(String::as_str)
            .collect()
    }

    /// One node in a lane, taken from the shipped table rather than invented here.
    fn node_for_lane(lane: LaneId) -> &'static str {
        let bindings = crate::engine::service_binding::forge_service_bindings()
            .unwrap_or_else(|error| panic!("this build ships a service binding table: {error}"));
        for node in bindings.keys() {
            if crate::engine::service_binding::is_human_gate(node.as_str()) {
                continue;
            }
            if crate::engine::service_binding::lane_for_node(node.as_str()).ok() != Some(lane) {
                continue;
            }
            if crate::engine::action_profiles::policy_for_node(node.as_str()).is_ok() {
                return node.as_str();
            }
        }
        panic!("the shipped binding table has no runnable node in the requested lane");
    }

    fn input<'a>(node: &'a str, backend: HarnessBackend) -> HarnessProfileInput<'a> {
        HarnessProfileInput {
            node_id: node,
            backend,
            model: ModelSelection::from_parts(None, None),
            execution_id: Some("exec-1"),
            role_attempt: 0,
            execution_generation: Some("gen-1"),
        }
    }

    #[test]
    fn the_same_input_always_resolves_to_the_same_profile() {
        let node = node_for_lane(LaneId::Smith);
        let first = resolve(&input(node, HarnessBackend::OpenCode)).expect("a Smith node resolves");
        let second =
            resolve(&input(node, HarnessBackend::OpenCode)).expect("a Smith node resolves");
        assert_eq!(
            first, second,
            "resolution must not depend on anything but its input"
        );
    }

    #[test]
    fn every_node_a_turn_can_run_resolves_a_profile() {
        let nodes = runnable_nodes();
        assert!(
            !nodes.is_empty(),
            "the binding table must have runnable nodes"
        );
        for node in nodes {
            let profile = resolve(&input(node, HarnessBackend::OpenCode))
                .unwrap_or_else(|error| panic!("{node}: {error}"));
            assert_eq!(profile.identity.node_id, node);
        }
    }

    #[test]
    fn an_implementing_role_and_a_read_only_role_do_not_resolve_alike() {
        let smith = resolve(&input(
            node_for_lane(LaneId::Smith),
            HarnessBackend::OpenCode,
        ))
        .expect("a Smith node resolves");
        let scout = resolve(&input(
            node_for_lane(LaneId::Scout),
            HarnessBackend::OpenCode,
        ))
        .expect("a Scout node resolves");

        assert!(smith.may_edit(), "Smith implements");
        assert!(!scout.may_edit(), "Scout reads");
        assert_eq!(smith.planning_policy, PlanningPolicy::ProgressContract);
        assert_eq!(scout.planning_policy, PlanningPolicy::None);
    }

    #[test]
    fn an_unbindable_node_is_refused_rather_than_run_with_an_inherited_surface() {
        let error = resolve(&input(
            "no-such-node-in-any-definition",
            HarnessBackend::OpenCode,
        ))
        .expect_err("a node nothing binds must not resolve to a permissive profile");
        let detail = error.to_string();
        assert!(
            detail.contains("no-such-node-in-any-definition"),
            "the refusal must name the node: {detail}"
        );
    }

    #[test]
    fn the_resolved_action_policy_is_the_canonical_table_and_not_a_second_one() {
        for node in runnable_nodes() {
            let resolved = resolve(&input(node, HarnessBackend::OpenCode))
                .unwrap_or_else(|error| panic!("{node}: {error}"));
            let canonical = crate::engine::action_profiles::policy_for_node(node)
                .unwrap_or_else(|error| panic!("{node}: {error}"));
            assert_eq!(resolved.action_policy, canonical, "{node}");
        }
    }

    #[test]
    fn planning_is_never_granted_to_a_surface_that_may_not_edit() {
        for lane in LaneId::ALL {
            for action_policy in [
                ActionPolicy::StructuredReadOnly,
                ActionPolicy::StructuredOnly,
                ActionPolicy::VerificationOnly,
                ActionPolicy::CompositeShell,
                ActionPolicy::ReleaseOperations,
            ] {
                if !action_policy.surface().edit {
                    assert_eq!(
                        planning_for(lane, action_policy),
                        PlanningPolicy::None,
                        "{action_policy:?} may not edit, so it gets no scaffold"
                    );
                }
            }
        }
    }

    #[test]
    fn the_resolver_is_provider_neutral_and_touches_no_provider() {
        let node = node_for_lane(LaneId::Smith);
        let mut opencode = resolve(&input(node, HarnessBackend::OpenCode)).expect("resolves");
        let maestro = resolve(&input(node, HarnessBackend::Maestro)).expect("resolves");

        assert_eq!(
            opencode.identity.backend,
            HarnessBackend::OpenCode,
            "the backend is recorded, not acted on"
        );
        opencode.identity.backend = maestro.identity.backend;
        assert_eq!(
            opencode, maestro,
            "the policy is the same on every provider: only the identity says which one runs it"
        );
    }

    #[test]
    fn the_identity_carries_what_a_turn_record_needs_to_reconstruct_the_configuration() {
        let node = node_for_lane(LaneId::Smith);
        let profile = resolve(&HarnessProfileInput {
            node_id: node,
            backend: HarnessBackend::OpenCode,
            model: ModelSelection::Judgment,
            execution_id: Some("exec-9"),
            role_attempt: 2,
            execution_generation: Some("gen-7"),
        })
        .expect("resolves");

        assert_eq!(profile.identity.profile_id, HARNESS_PROFILE_ID);
        assert_eq!(profile.identity.profile_version, HARNESS_PROFILE_VERSION);
        assert_eq!(profile.identity.node_id, node);
        assert_eq!(profile.identity.lane, LaneId::Smith);
        assert_eq!(profile.identity.model, ModelSelection::Judgment);
        assert_eq!(profile.identity.execution_id.as_deref(), Some("exec-9"));
        assert_eq!(profile.identity.role_attempt, 2, "a repair attempt says so");
        assert_eq!(
            profile.identity.execution_generation.as_deref(),
            Some("gen-7")
        );
    }

    /// The authority boundary, as a regression test rather than a promise.
    ///
    /// The forbidden tokens are assembled with `concat!` so this test's own source cannot contain them: a guard
    /// that matched itself would pass while the rule it guards was broken.
    #[test]
    fn this_module_cannot_choose_a_workflow_node_and_does_no_io() {
        let source = include_str!("harness_profile.rs");
        for forbidden in [
            concat!("Workflow", "Engine"),
            concat!("next", "_node"),
            concat!("set", "_next"),
            concat!("transition", "_to"),
            concat!(".", "transition("),
            concat!("Transition", "::"),
        ] {
            assert!(
                !source.contains(forbidden),
                "the profile layer must not be able to choose a node: {forbidden}"
            );
        }
        for forbidden in [
            concat!("std::", "env"),
            concat!("std::", "process"),
            concat!("std::", "fs"),
            concat!("System", "Time"),
            concat!("::", "now()"),
        ] {
            assert!(
                !source.contains(forbidden),
                "resolution must stay a pure function of its input: {forbidden}"
            );
        }
    }

    #[test]
    fn a_read_only_lane_is_not_given_the_stuck_window_that_fits_an_implementing_lane() {
        let smith = resolve(&input(
            node_for_lane(LaneId::Smith),
            HarnessBackend::OpenCode,
        ))
        .expect("a Smith node resolves");
        let scout = resolve(&input(
            node_for_lane(LaneId::Scout),
            HarnessBackend::OpenCode,
        ))
        .expect("a Scout node resolves");

        assert_eq!(
            scout.runaway_policy.no_progress_window, NO_PROGRESS_WINDOW_DISABLED,
            "a lane that changes no files cannot be called stuck for not changing files"
        );
        assert_ne!(
            smith.runaway_policy.no_progress_window, NO_PROGRESS_WINDOW_DISABLED,
            "an implementing lane must show its work: a long stretch of no change is a runaway there"
        );
        assert_eq!(
            scout.runaway_policy.max_repeats, smith.runaway_policy.max_repeats,
            "repeating one action three times is the same observation in every lane"
        );
    }

    /// The policy fields a turn's request and stop are decided by, held equal across attempts.
    ///
    /// This is why the corrective directive for the NEXT attempt may be composed under the CURRENT attempt's
    /// profile: the identity moves with the attempt, the policy does not. If this ever fails, that composition is
    /// deciding a turn with another turn's policy.
    #[test]
    fn the_policy_fields_do_not_vary_with_the_attempt() {
        let node = node_for_lane(LaneId::Smith);
        let first = resolve(&input(node, HarnessBackend::OpenCode)).expect("a Smith node resolves");
        let mut later_input = input(node, HarnessBackend::OpenCode);
        later_input.role_attempt = 3;
        let later = resolve(&later_input).expect("a Smith node resolves");

        assert_ne!(
            first.identity.role_attempt, later.identity.role_attempt,
            "the attempt is part of the identity"
        );
        assert_eq!(first.action_policy, later.action_policy);
        assert_eq!(first.context_policy, later.context_policy);
        assert_eq!(first.planning_policy, later.planning_policy);
        assert_eq!(first.runaway_policy, later.runaway_policy);
    }
}
