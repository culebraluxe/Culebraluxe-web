//! The lane contract: the whole of what the shared lifecycle cannot know.
//!
//! A move-only split out of `roles/lifecycle.rs`, so that file stays within the repository's file-length
//! rule and the contract has one obvious home: `roles/hooks.rs` is the contract, `roles/lifecycle.rs` is
//! the sequence that reads through it, `roles/service.rs` is the boundary that inherits the sequence.
//!
//! NOTHING HERE NAMES A LANE. A node id is passed through to the lane's own answer and never matched on,
//! and every default claims nothing — so a lane states exactly the behavior it owns, and a service that
//! overrides nothing gets the shared lifecycle and only that ([`NoRoleHooks`]).

use crate::engine::executor::ForgeRoleOutcome;
use crate::engine::facts::{marker_evidence, ForgeGateEvidence};
use crate::engine::phase::{lane_deliverable_kind, PhaseDeliverableKind, RoleEffectPorts};
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::roles::lifecycle::{ForgeRoleContext, ForgeRoleTurn};
use workflow::Result;

/// A lane's own reading of its turn — the whole of what the shared lifecycle cannot know.
///
/// Every method has a default that claims nothing, so an implementation names exactly the behavior
/// it owns and nothing else. There is intentionally no `descriptor()`/`lane()` here: role identity is
/// [`super::service::AbstractForgeService`]'s, and a hook impl must not be able to re-invent it.
pub trait ForgeRoleHooks: Send + Sync {
    /// For a lane whose turn is not a model turn at all.
    ///
    /// `Some` short-circuits the whole lifecycle — the attempt loop *and* the deliverable gate: no
    /// harness turn will be asked for, and the lane's own outcome is returned as it stands. That is the
    /// contract a model-free lane already had (the RUST_CONTRACT assay writes its own hold), and it is
    /// why the hook returns the outcome rather than mutating evidence. `None` (the default) means "this
    /// lane's work comes from a model turn", which is every lane but one.
    fn turn_without_model(
        &self,
        _ctx: &ForgeRoleContext<'_>,
        _node_id: &str,
        _task: &ActiveForgeRoleTask,
    ) -> Option<Result<ForgeRoleOutcome>> {
        None
    }

    /// Whether this node's accepted candidate SHA is the lane's own deliverable, adopted onto the
    /// evidence as each turn is read. Default: no lane claims a candidate.
    fn adopts_candidate_sha(&self, _node_id: &str) -> bool {
        false
    }

    /// The lane's reading of the reply: the evidence, as the gate will act on it.
    ///
    /// Called for every attempt, before the candidate is adopted and before the deliverable gate, so what a
    /// lane refuses here is refused on the attempt that produced it. The default is the reading every lane
    /// shares — the reply's own evidence marker taken over what the envelope already carried — and the
    /// envelope is in hand for the lanes whose deliverable does not arrive in the reply at all (see
    /// `roles::dev_ops`).
    fn collect_evidence(
        &self,
        _node_id: &str,
        base: ForgeGateEvidence,
        raw: &str,
        _ports: &RoleEffectPorts,
    ) -> std::result::Result<ForgeGateEvidence, String> {
        Ok(marker_evidence(raw, &base))
    }

    /// Which deliverable this node owes, as this lane reads it.
    ///
    /// The default is the lane table's answer ([`lane_deliverable_kind`]), which is already a per-lane
    /// answer; a lane overrides this only where one of its own nodes breaks its own rule.
    fn deliverable_kind(&self, _node_id: &str) -> PhaseDeliverableKind {
        lane_deliverable_kind(_node_id)
    }

    /// The routing decision this node owes and its reply did not carry, named the way the gate names it.
    ///
    /// Default: nothing — a lane that owns no routing node claims none. The gate turns a `Some` here into a
    /// rejected deliverable, which is what re-asks the node for the decision it withheld.
    fn routing_decision_missing(
        &self,
        _node_id: &str,
        _evidence: &ForgeGateEvidence,
    ) -> Option<&'static str> {
        None
    }

    /// The lane's reading of the turn, with the envelope in hand: the evidence collected from the
    /// reply, the writer its records go through, and the harness that produced the turn. Called once,
    /// after the attempts and before the deliverable gate, so a refusal read here is a refusal the
    /// gate acts on rather than a note.
    fn interpret_turn(
        &self,
        _ctx: &ForgeRoleContext<'_>,
        _turn: &ForgeRoleTurn<'_>,
        _evidence: &mut ForgeGateEvidence,
    ) -> Result<()> {
        Ok(())
    }
}

/// The reading of a lane that reads nothing: the shared lifecycle, and only that.
///
/// Two callers need it. A service that owns its boundary and no reading (a lane whose intelligence is
/// downstream — a publisher, a reviewer) inherits this instead of inventing an empty hook impl, and it is
/// what a node no lane claims runs under, so an unmapped node keeps the proven turn rather than failing.
pub struct NoRoleHooks;

impl ForgeRoleHooks for NoRoleHooks {}
