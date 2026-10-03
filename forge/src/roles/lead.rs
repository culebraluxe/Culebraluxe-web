//! Lead lane.
//!
//! Lead owns three phases of the same turn — the pre-implementation decision, the implement (SOLO), and the
//! post decision — plus the failure classifier that names what went wrong. Only the implement phase is
//! intelligence this service has to supply, and it is not Lead's own: when the Lead decides SOLO it performs
//! Smith's act of delivering code — the candidate becomes the run's candidate and the patch is captured — so
//! the reading is inherited from [`crate::roles::smith`] rather than re-implemented here.
//!
//! THE OTHER TWO READINGS ARE LEAD'S, and they live here rather than in the engine (`2026-10-02`, the seam
//! closure): a decision is taken once, in the PRE phase, so a reply that restates one during any other Lead
//! turn is stripped rather than believed; and the classifier's reply is read as a class, with a failed release
//! stage promoted to the class the gate routes on.

use crate::engine::executor::ForgeRoleRunner;
use crate::engine::facts::{marker_evidence, ForgeGateEvidence};
use crate::engine::phase::{PhaseDeliverableKind, RoleEffectPorts, FAILURE_CLASSES};
use crate::engine::role_mapping::LaneId;
use crate::engine::service_binding::service_for_node;
use crate::roles::hooks::ForgeRoleHooks;
use crate::roles::lifecycle::{ForgeRoleContext, ForgeRoleTurn};
use crate::roles::service::{AbstractForgeService, ForgeServiceDescriptor};
use workflow::Result;

pub use crate::engine::graph::{plan_smith_layers, split_eligibility};

pub const LEAD_SERVICE_ID: &str = "forge.lead";

/// The decisions the gate can route.
///
/// The vocabulary is the gate's (`engine::phase::FAILURE_CLASSES` is its sibling); WHICH node must carry one is
/// Lead's, which is why the rule that reads this list stands below rather than in the engine.
const LEAD_DECISIONS: &[&str] = &["SOLO", "SMITH", "SPLIT", "HOLD", "ASSAY"];

/// The Lead node asked for the class of a failure rather than for a decision.
pub const FAILURE_CLASSIFIER_NODE: &str = "failure_classifier";

/// The Lead node that owes the run's decision: the PRE phase.
///
/// The Lead's phases are the one piece of node metadata the workflow definition does not carry (its service
/// binding says `forge.lead` for all four Lead nodes), so they are stated here, by the lane they describe, rather
/// than in a node table every lane would have to read. The other two are [`FAILURE_CLASSIFIER_NODE`] and the solo
/// implement turn ([`implements`]).
pub const LEAD_DECISION_NODE: &str = "lead_pre";

/// Whether this Lead node writes code: the SOLO implement turn, which performs Smith's act of delivering code and
/// is therefore named by Smith's own rule rather than by a second list. The OpenCode profile reads this to give
/// that one Lead turn implement authority and every other Lead turn none.
pub fn implements(node_id: &str) -> bool {
    crate::roles::smith::delivers_code(node_id)
}

/// Whether this Lead node is the failure classifier.
pub fn is_failure_classifier(node_id: &str) -> bool {
    node_id == FAILURE_CLASSIFIER_NODE
}

/// Whether this Lead node's reply may not set a decision.
///
/// Every Lead node but the classifier: the decision is taken once, in the PRE turn, and the implement and post
/// turns run under it, so a reply that restates one there may not overwrite it. Asked of the workflow
/// definition's service binding rather than of a second list of names, so a Lead node added there is covered
/// without an edit here.
fn may_not_set_decision(node_id: &str) -> bool {
    !is_failure_classifier(node_id) && service_for_node(node_id) == Some(LEAD_SERVICE_ID)
}

/// Lead's own reading, supplied to the shared lifecycle as this lane's hooks.
///
/// Three things are Lead's: which node owes a failure class rather than a decision, the rule that a decision is
/// the PRE phase's to set, and the reading that promotes a failed release stage to the class the gate routes on.
/// Code delivery is not: that is inherited from the Smith lane.
pub struct LeadHooks;

impl ForgeRoleHooks for LeadHooks {
    fn adopts_candidate_sha(&self, node_id: &str) -> bool {
        crate::roles::smith::delivers_code(node_id)
    }

    /// The classifier's reply is read differently from any other node's: a release stage in it is the stage that
    /// failed, so the class the reply carried is kept as the classifier's own answer and the stage becomes the
    /// class the gate routes on.
    fn collect_evidence(
        &self,
        node_id: &str,
        evidence: ForgeGateEvidence,
        raw: &str,
        _ports: &RoleEffectPorts,
    ) -> std::result::Result<ForgeGateEvidence, String> {
        let mut next = marker_evidence(raw, &evidence);
        if is_failure_classifier(node_id) {
            if next.failed_release_stage.is_some() {
                if let Some(stage) = next
                    .stage_failure_class
                    .clone()
                    .or_else(|| next.failure_class.clone())
                {
                    next.classifier_failure_class = next.failure_class.clone();
                    next.failure_class = Some(stage);
                }
            }
            return Ok(next);
        }
        if may_not_set_decision(node_id) {
            next.lead_decision = None;
            next.split_count = None;
        }
        Ok(next)
    }

    /// The classifier's deliverable is a class and the PRE phase's is the decision. The implement and post turns
    /// run under that decision and owe the gate nothing of their own.
    fn deliverable_kind(&self, node_id: &str) -> PhaseDeliverableKind {
        if is_failure_classifier(node_id) {
            PhaseDeliverableKind::FailureClass
        } else if node_id == LEAD_DECISION_NODE {
            PhaseDeliverableKind::LeadDecision
        } else {
            PhaseDeliverableKind::None
        }
    }

    /// The classifier owes a class the gate can route, the decision nodes owe a decision it can route, and a
    /// SPLIT owes the count it split into. All three are statements about Lead's own nodes.
    fn routing_decision_missing(
        &self,
        node_id: &str,
        evidence: &ForgeGateEvidence,
    ) -> Option<&'static str> {
        if is_failure_classifier(node_id) {
            return if FAILURE_CLASSES.contains(&evidence.failure_class.as_deref().unwrap_or("")) {
                None
            } else {
                Some("failure_class")
            };
        }
        if !may_not_set_decision(node_id) {
            return None;
        }
        let decision = evidence.lead_decision.as_deref().unwrap_or("");
        if !LEAD_DECISIONS.contains(&decision) {
            return Some("lead_decision");
        }
        if decision == "SPLIT" && evidence.split_count.unwrap_or(0) <= 0 {
            return Some("lead_decision.splitCount");
        }
        None
    }

    /// The solo implement delivers code, so its candidate is judged by Smith's rules — one judgement, one owner.
    fn judge_output(
        &self,
        ctx: &crate::roles::lifecycle::ForgeRoleContext<'_>,
        node_id: &str,
        out: &mut crate::engine::runner::HarnessOutput,
    ) {
        crate::roles::smith::judge_delivered_candidate(ctx.harness, node_id, out);
    }

    fn interpret_turn(
        &self,
        ctx: &ForgeRoleContext<'_>,
        turn: &ForgeRoleTurn<'_>,
        evidence: &mut ForgeGateEvidence,
    ) -> Result<()> {
        crate::roles::smith::read_delivered_work(ctx, turn, evidence)
    }
}

/// Forge-internal service for every Lead workflow node.
pub struct LeadService<'a> {
    runner: &'a dyn ForgeRoleRunner,
}

impl<'a> LeadService<'a> {
    pub fn new(runner: &'a dyn ForgeRoleRunner) -> Self {
        Self { runner }
    }
}

impl AbstractForgeService for LeadService<'_> {
    fn descriptor(&self) -> ForgeServiceDescriptor {
        ForgeServiceDescriptor {
            service_id: LEAD_SERVICE_ID,
            lane: LaneId::Lead,
            description: "Forge orchestration service for Lead decision and assembly lanes",
        }
    }

    fn runner(&self) -> &dyn ForgeRoleRunner {
        self.runner
    }

    fn hooks(&self) -> &dyn ForgeRoleHooks {
        &LeadHooks
    }
}
