//! Propose — turn typed Phase 3 improvement signals into candidate harnesses.
//!
//! Harness² proposes in **two separate scopes**, and this module keeps them separate all the way
//! to Compose:
//!
//! * an **invariant** candidate improves the general harness — reasoning, planning, tool use,
//!   context and delegation guidance that applies to every domain;
//! * a **domain** candidate adds knowledge, constraints or procedures that are only true inside
//!   one [`HarnessDomain`].
//!
//! A candidate never mixes the two scopes; Compose is the only place they meet. Every change
//! carries its rationale and cites the Phase 3 evidence ids that motivated it, so a probe result
//! can be traced back to the scorecard weakness, repeated correction, routing weakness, skill
//! underperformance, model-role weakness, HOLD pattern or cost/turn excess that prompted it.
//!
//! Bounds and authority:
//!
//! * the proposal budget is [`EvolutionBudget`]; at most `k` candidates are proposed per scope
//!   (v1 defaults to `k = 1`, and `k > 1` is data, not a different code path);
//! * the proposer never targets a RED surface (`Scripts`/`Plugins`), and it never changes a
//!   component's declared authority — a stronger policy gate (`super::policy`) re-checks the
//!   candidate before any probe runs, but a proposal that could never pass is not generated;
//! * guidance text is deliberately written so it does not introduce the protected topics the
//!   policy scan refuses (authority, QA/Git, production, security, release, workflow XML,
//!   trusted boundaries, deny rules, destructive commands).
//!
//! The proposed [`Candidate`] is a change list against one immutable base revision; the proposer
//! never builds the candidate revision itself. The caller materializes it with
//! [`HarnessRevision::from_changes`], which is the only shape the policy accepts.

use super::model::{
    Candidate, CandidateSet, Component, ComponentChange, ComponentKind, EvolutionBudget,
    EvolutionError, EvolutionMode, HarnessDomain, HarnessRevision, ProposalKind,
};

/// The component kinds an invariant candidate may touch.
///
/// Invariant guidance is about *how* the agent works in general, so it lands in the system
/// prompt, the tool guidance or the subagent/delegation guidance. It never lands in domain-scoped
/// memory or skills — that would smuggle domain knowledge into the generic harness.
pub const INVARIANT_COMPONENT_KINDS: [ComponentKind; 3] = [
    ComponentKind::SystemPrompt,
    ComponentKind::ToolGuidance,
    ComponentKind::Subagents,
];

/// The component kinds a domain candidate may touch.
///
/// Domain knowledge and domain procedures land in memory and skills. `Memory` is the domain's
/// facts/constraints; `Skills` is its procedure. Neither is a general invariant surface.
pub const DOMAIN_COMPONENT_KINDS: [ComponentKind; 2] =
    [ComponentKind::Memory, ComponentKind::Skills];

/// The invariant guidance families. Every invariant candidate names the family it belongs to, so
/// "general harness improvement" never becomes an untyped catch-all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvariantTheme {
    Reasoning,
    Planning,
    ToolUse,
    Context,
    Delegation,
}

impl InvariantTheme {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Reasoning => "reasoning",
            Self::Planning => "planning",
            Self::ToolUse => "tool_use",
            Self::Context => "context",
            Self::Delegation => "delegation",
        }
    }
}

/// The domain guidance families: domain knowledge, domain constraints, domain procedures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainTheme {
    Knowledge,
    Constraints,
    Procedures,
}

impl DomainTheme {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Knowledge => "knowledge",
            Self::Constraints => "constraints",
            Self::Procedures => "procedures",
        }
    }
}

/// One Phase 3 signal the proposer understands.
///
/// Every variant is a *typed* observation from the Phase 3 surfaces (the scorecard, the durable
/// run/decision rows, the HOLD audit), not a free-text "improve the harness" instruction. The
/// fields are the durable identifiers of the observation so a reviewer can find the rows.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "signal")]
pub enum ImprovementSignal {
    /// A `(role, agent)` bucket on the scorecard is weak: low completion, high rework, or a
    /// failure class that keeps reappearing.
    ScorecardWeakness {
        role: String,
        agent: String,
        weakness: String,
    },
    /// The same correction was requested more than once for one lane.
    RepeatedCorrection { lane: String, correction: String },
    /// Routing/handoff sent the work to the wrong lane or asked for the wrong expertise.
    RoutingWeakness { lane: String, detail: String },
    /// A canonical skill was loaded but not applied, or its procedure underperformed.
    SkillUnderperformance { skill: String, detail: String },
    /// A model/role pairing underperformed the lane's alternatives.
    ModelRoleWeakness {
        model: String,
        role: String,
        detail: String,
    },
    /// The same HOLD reason (node + failure class) recurred.
    HoldPattern {
        node: String,
        failure_class: String,
        holds: u32,
    },
    /// Cost or turn count exceeded the lane's own recorded baseline.
    CostTurnExcess {
        lane: String,
        metric: String,
        excess_percent: u32,
    },
}

impl ImprovementSignal {
    /// Stable id for durable rows and rationale text.
    pub const fn id(&self) -> &'static str {
        match self {
            Self::ScorecardWeakness { .. } => "scorecard_weakness",
            Self::RepeatedCorrection { .. } => "repeated_correction",
            Self::RoutingWeakness { .. } => "routing_weakness",
            Self::SkillUnderperformance { .. } => "skill_underperformance",
            Self::ModelRoleWeakness { .. } => "model_role_weakness",
            Self::HoldPattern { .. } => "hold_pattern",
            Self::CostTurnExcess { .. } => "cost_turn_excess",
        }
    }

    /// How urgent the signal is. When the budget is `k = 1` the proposer must choose, and it
    /// chooses the signal that shows a run failing over one that shows a run costing more.
    pub const fn priority(&self) -> u8 {
        match self {
            Self::HoldPattern { .. } => 90,
            Self::RepeatedCorrection { .. } => 80,
            Self::ScorecardWeakness { .. } => 70,
            Self::RoutingWeakness { .. } => 60,
            Self::ModelRoleWeakness { .. } => 55,
            Self::SkillUnderperformance { .. } => 50,
            Self::CostTurnExcess { .. } => 30,
        }
    }

    /// A short human sentence naming the observation, used in candidate rationales.
    pub fn subject(&self) -> String {
        match self {
            Self::ScorecardWeakness {
                role,
                agent,
                weakness,
            } => format!("{role}/{agent} scorecard weakness: {weakness}"),
            Self::RepeatedCorrection { lane, correction } => {
                format!("{lane} repeated correction: {correction}")
            }
            Self::RoutingWeakness { lane, detail } => {
                format!("{lane} routing weakness: {detail}")
            }
            Self::SkillUnderperformance { skill, detail } => {
                format!("skill {skill} underperformed: {detail}")
            }
            Self::ModelRoleWeakness {
                model,
                role,
                detail,
            } => format!("model {model} in role {role}: {detail}"),
            Self::HoldPattern {
                node,
                failure_class,
                holds,
            } => format!("{holds} HOLDs at {node} with failure class {failure_class}"),
            Self::CostTurnExcess {
                lane,
                metric,
                excess_percent,
            } => format!("{lane} exceeded its baseline on {metric} by {excess_percent}%"),
        }
    }
}

/// One typed improvement target: the signal, the domain it came from, and the durable evidence
/// ids that support it. A target with no evidence is not a target; it is a guess.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ImprovementTarget {
    pub domain: HarnessDomain,
    pub signal: ImprovementSignal,
    pub evidence_refs: Vec<String>,
}

impl ImprovementTarget {
    pub fn new(
        domain: HarnessDomain,
        signal: ImprovementSignal,
        evidence_refs: Vec<String>,
    ) -> Self {
        Self {
            domain,
            signal,
            evidence_refs,
        }
    }

    /// A target must cite at least one durable evidence row, and every citation must be a real
    /// id rather than blank padding.
    pub fn validate(&self) -> Result<(), EvolutionError> {
        if self.evidence_refs.is_empty() {
            return Err(EvolutionError::InvalidCandidate(format!(
                "improvement target [{}] has no evidence refs",
                self.signal.id()
            )));
        }
        if self
            .evidence_refs
            .iter()
            .any(|reference| reference.trim().is_empty())
        {
            return Err(EvolutionError::InvalidCandidate(format!(
                "improvement target [{}] has a blank evidence ref",
                self.signal.id()
            )));
        }
        Ok(())
    }

    /// The sentence a candidate rationale carries: what was observed and where it is recorded.
    pub fn summary(&self) -> String {
        format!(
            "{} (evidence: {})",
            self.signal.subject(),
            self.evidence_refs.join(", ")
        )
    }
}

/// One proposal request: the signals observed for a base revision, under one budget and mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalRequest {
    pub targets: Vec<ImprovementTarget>,
    pub budget: EvolutionBudget,
    pub mode: EvolutionMode,
}

impl ProposalRequest {
    /// The v1 request: one invariant + one domain candidate from the observed targets.
    pub fn k1(targets: Vec<ImprovementTarget>) -> Self {
        Self {
            targets,
            budget: EvolutionBudget::k1(),
            mode: EvolutionMode::Parallel,
        }
    }

    pub fn validate(&self) -> Result<(), EvolutionError> {
        self.budget.validate()?;
        if self.targets.is_empty() {
            return Err(EvolutionError::InvalidCandidate(
                "proposal request has no improvement targets".into(),
            ));
        }
        for target in &self.targets {
            target.validate()?;
        }
        Ok(())
    }
}

/// One proposed candidate plus the target and evidence it came from.
///
/// The [`Candidate`] carries the change list; this wrapper adds the proposal-level rationale and
/// evidence refs so the experiment can persist *why* the candidate exists without reading prose
/// back out of a component body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedCandidate {
    pub candidate: Candidate,
    /// Index into [`ProposalRequest::targets`] of the signal that produced this candidate.
    pub target_index: usize,
    pub rationale: String,
    pub evidence_refs: Vec<String>,
}

impl ProposedCandidate {
    pub fn scope(&self) -> ProposalKind {
        self.candidate.kind
    }
}

/// What one Propose step produced: the two scopes, still separate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalBatch {
    pub base_revision: String,
    pub domain: HarnessDomain,
    pub invariant: Vec<ProposedCandidate>,
    pub domain_scope: Vec<ProposedCandidate>,
    pub budget: EvolutionBudget,
    pub mode: EvolutionMode,
}

impl ProposalBatch {
    pub fn all(&self) -> impl Iterator<Item = &ProposedCandidate> {
        self.invariant.iter().chain(self.domain_scope.iter())
    }

    pub fn len(&self) -> usize {
        self.invariant.len() + self.domain_scope.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn within_budget(&self) -> bool {
        let per_scope = self.budget.candidates_per_scope() as usize;
        self.invariant.len() <= per_scope && self.domain_scope.len() <= per_scope
    }

    /// The candidate set the probe stage consumes. Invariant and domain candidates are still in
    /// their own buckets; nothing is pre-combined.
    pub fn candidate_set(&self) -> Result<CandidateSet, EvolutionError> {
        self.validate()?;
        let mut set = CandidateSet::new(self.mode, self.budget);
        set.invariant = self
            .invariant
            .iter()
            .map(|proposed| proposed.candidate.clone())
            .collect();
        set.domain = self
            .domain_scope
            .iter()
            .map(|proposed| proposed.candidate.clone())
            .collect();
        set.validate()?;
        Ok(set)
    }

    /// Structural validity of the whole batch: budget, scope separation, one base revision,
    /// well-formed changes, no RED surface, and unique ids.
    pub fn validate(&self) -> Result<(), EvolutionError> {
        self.budget.validate()?;
        if self.base_revision.trim().is_empty() {
            return Err(EvolutionError::InvalidCandidate(
                "proposal batch has no base revision".into(),
            ));
        }
        if self.is_empty() {
            return Err(EvolutionError::InvalidCandidate(
                "proposal batch is empty".into(),
            ));
        }
        if !self.within_budget() {
            return Err(EvolutionError::InvalidCandidate(format!(
                "proposal batch exceeds k={} per scope",
                self.budget.k
            )));
        }
        let mut ids: Vec<&str> = Vec::new();
        validate_scope(
            ProposalKind::Invariant,
            &self.invariant,
            &self.base_revision,
            &INVARIANT_COMPONENT_KINDS,
            self.budget.candidates_per_scope() as usize,
            &mut ids,
        )?;
        validate_scope(
            ProposalKind::Domain,
            &self.domain_scope,
            &self.base_revision,
            &DOMAIN_COMPONENT_KINDS,
            self.budget.candidates_per_scope() as usize,
            &mut ids,
        )?;
        Ok(())
    }
}

fn validate_scope<'a>(
    kind: ProposalKind,
    proposals: &'a [ProposedCandidate],
    base_revision: &str,
    allowed_kinds: &[ComponentKind],
    per_scope: usize,
    ids: &mut Vec<&'a str>,
) -> Result<(), EvolutionError> {
    if proposals.len() > per_scope {
        return Err(EvolutionError::InvalidCandidate(format!(
            "{} scope proposed {} candidates for a budget of {}",
            kind.id(),
            proposals.len(),
            per_scope
        )));
    }
    for proposed in proposals {
        if proposed.candidate.kind != kind {
            return Err(EvolutionError::InvalidCandidate(format!(
                "candidate {} is filed under {} but declares {}",
                proposed.candidate.id,
                kind.id(),
                proposed.candidate.kind.id()
            )));
        }
        if proposed.candidate.base_revision != base_revision {
            return Err(EvolutionError::InvalidCandidate(format!(
                "candidate {} targets {} instead of the batch base {}",
                proposed.candidate.id, proposed.candidate.base_revision, base_revision
            )));
        }
        proposed.candidate.validate()?;
        if proposed.rationale.trim().is_empty() {
            return Err(EvolutionError::InvalidCandidate(format!(
                "candidate {} has no proposal rationale",
                proposed.candidate.id
            )));
        }
        if proposed.evidence_refs.is_empty() {
            return Err(EvolutionError::InvalidCandidate(format!(
                "candidate {} has no evidence refs",
                proposed.candidate.id
            )));
        }
        for change in &proposed.candidate.changes {
            if change.kind.minimum_authority().is_red() {
                return Err(EvolutionError::InvalidCandidate(format!(
                    "{} is a protected RED surface and cannot be proposed",
                    change.kind
                )));
            }
            if !allowed_kinds.contains(&change.kind) {
                return Err(EvolutionError::InvalidCandidate(format!(
                    "{} candidate changes {} outside its scope",
                    kind.id(),
                    change.kind
                )));
            }
        }
        if ids.contains(&proposed.candidate.id.as_str()) {
            return Err(EvolutionError::InvalidCandidate(format!(
                "duplicate candidate id {}",
                proposed.candidate.id
            )));
        }
        ids.push(proposed.candidate.id.as_str());
    }
    Ok(())
}

/// Propose invariant and domain candidates for one base revision.
///
/// The strongest signals are proposed first; with the default `k = 1` the same top signal drives
/// one invariant candidate and one domain candidate. The returned batch is validated, and
/// [`ProposalBatch::candidate_set`] hands the probe stage an immutable [`CandidateSet`].
pub fn propose(
    base: &HarnessRevision,
    request: &ProposalRequest,
) -> Result<ProposalBatch, EvolutionError> {
    request.validate()?;
    base.verify_integrity()?;

    // Deterministic order: priority desc, then the caller's target order. A stable sort keeps
    // equal priorities in the order the caller listed them, so two identical requests propose
    // identical candidates.
    let mut ordered: Vec<(usize, &ImprovementTarget)> =
        request.targets.iter().enumerate().collect();
    ordered.sort_by(|(left_index, left), (right_index, right)| {
        right
            .signal
            .priority()
            .cmp(&left.signal.priority())
            .then(left_index.cmp(right_index))
    });

    for (_, target) in &ordered {
        if target.domain != base.domain {
            return Err(EvolutionError::InvalidCandidate(format!(
                "improvement target for domain {} cannot propose against base domain {}",
                target.domain, base.domain
            )));
        }
    }

    let per_scope = request.budget.candidates_per_scope() as usize;
    let mut invariant = Vec::with_capacity(per_scope.min(ordered.len()));
    for (ordinal, (target_index, target)) in ordered.iter().take(per_scope).enumerate() {
        invariant.push(build_proposed(
            base,
            target,
            *target_index,
            ProposalKind::Invariant,
            ordinal,
        )?);
    }
    let mut domain_scope = Vec::with_capacity(per_scope.min(ordered.len()));
    for (ordinal, (target_index, target)) in ordered.iter().take(per_scope).enumerate() {
        domain_scope.push(build_proposed(
            base,
            target,
            *target_index,
            ProposalKind::Domain,
            ordinal,
        )?);
    }

    let batch = ProposalBatch {
        base_revision: base.id.clone(),
        domain: base.domain,
        invariant,
        domain_scope,
        budget: request.budget,
        mode: request.mode,
    };
    batch.validate()?;
    Ok(batch)
}

/// Convenience for callers that only need the probe-stage shape.
pub fn propose_candidate_set(
    base: &HarnessRevision,
    request: &ProposalRequest,
) -> Result<CandidateSet, EvolutionError> {
    propose(base, request)?.candidate_set()
}

fn build_proposed(
    base: &HarnessRevision,
    target: &ImprovementTarget,
    target_index: usize,
    scope: ProposalKind,
    ordinal: usize,
) -> Result<ProposedCandidate, EvolutionError> {
    let (kind, addition, theme, fallback_description) = match scope {
        ProposalKind::Invariant => {
            let (kind, addition) = invariant_guidance(&target.signal);
            (
                kind,
                addition.to_string(),
                invariant_theme(&target.signal).id(),
                "invariant harness guidance".to_string(),
            )
        }
        ProposalKind::Domain => {
            let (kind, addition) = domain_guidance(&target.signal, base.domain);
            (
                kind,
                addition,
                domain_theme(&target.signal).id(),
                format!("{} domain guidance", base.domain.id()),
            )
        }
    };

    if kind.minimum_authority().is_red() {
        return Err(EvolutionError::InvalidCandidate(format!(
            "{kind} is a protected RED surface and cannot be proposed"
        )));
    }

    let component = component_with_addition(base, kind, &addition, &fallback_description);
    let rationale = format!(
        "{} proposal [{}:{}]: {}",
        scope.id(),
        theme,
        target.signal.id(),
        target.summary()
    );
    let change = ComponentChange::set(component, rationale.clone());
    let candidate = Candidate::new(
        format!("{}-{}-{}", base.id, scope.id(), ordinal + 1),
        base.id.clone(),
        scope,
        vec![change],
    );
    candidate.validate()?;

    Ok(ProposedCandidate {
        candidate,
        target_index,
        rationale,
        evidence_refs: target.evidence_refs.clone(),
    })
}

/// Build the changed component from the base revision. The declared authority is never changed:
/// an existing component keeps its class, and a new one gets the kind's floor. Changing the class
/// is how a proposer would try to unlock automatic adoption, so this module cannot do it.
fn component_with_addition(
    base: &HarnessRevision,
    kind: ComponentKind,
    addition: &str,
    fallback_description: &str,
) -> Component {
    let existing = base.manifest.get(kind);
    let authority = existing
        .map(|component| component.authority)
        .unwrap_or_else(|| kind.minimum_authority());
    let mut content = existing
        .map(|component| component.content.clone())
        .unwrap_or_default();
    if !content.trim().is_empty() {
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push('\n');
    }
    content.push_str(addition.trim());

    let description = existing
        .map(|component| component.description.clone())
        .filter(|description| !description.trim().is_empty())
        .unwrap_or_else(|| fallback_description.to_string());

    Component::new(kind, authority, content, description)
}

/// Which invariant family a signal belongs to.
fn invariant_theme(signal: &ImprovementSignal) -> InvariantTheme {
    match signal {
        ImprovementSignal::ScorecardWeakness { .. }
        | ImprovementSignal::RepeatedCorrection { .. } => InvariantTheme::Planning,
        ImprovementSignal::RoutingWeakness { .. } => InvariantTheme::Delegation,
        ImprovementSignal::SkillUnderperformance { .. }
        | ImprovementSignal::CostTurnExcess { .. } => InvariantTheme::ToolUse,
        ImprovementSignal::ModelRoleWeakness { .. } => InvariantTheme::Context,
        ImprovementSignal::HoldPattern { .. } => InvariantTheme::Reasoning,
    }
}

/// Which domain family a signal belongs to.
fn domain_theme(signal: &ImprovementSignal) -> DomainTheme {
    match signal {
        ImprovementSignal::ScorecardWeakness { .. }
        | ImprovementSignal::RepeatedCorrection { .. }
        | ImprovementSignal::HoldPattern { .. } => DomainTheme::Constraints,
        ImprovementSignal::RoutingWeakness { .. }
        | ImprovementSignal::SkillUnderperformance { .. }
        | ImprovementSignal::CostTurnExcess { .. } => DomainTheme::Procedures,
        ImprovementSignal::ModelRoleWeakness { .. } => DomainTheme::Knowledge,
    }
}

/// Invariant guidance: how the agent works, independent of the domain.
///
/// Deliberately contains no protected-topic phrases — the policy's content scan must never fire on
/// a proposal the proposer itself generated.
fn invariant_guidance(signal: &ImprovementSignal) -> (ComponentKind, &'static str) {
    match signal {
        ImprovementSignal::ScorecardWeakness { .. } => (
            ComponentKind::SystemPrompt,
            "Before acting, restate the task's acceptance checks and name the single command that proves each one. Do not report the work finished until those commands have run.",
        ),
        ImprovementSignal::RepeatedCorrection { .. } => (
            ComponentKind::SystemPrompt,
            "If the same correction is requested twice, stop editing and rewrite the plan first: what failed, what you will change, and how you will verify it. A second identical attempt is not progress.",
        ),
        ImprovementSignal::RoutingWeakness { .. } => (
            ComponentKind::Subagents,
            "Delegate only work that is independent of the current step. Name the exact question, the files and the deliverable for each delegate; never delegate the decision you are being asked to make.",
        ),
        ImprovementSignal::SkillUnderperformance { .. } => (
            ComponentKind::ToolGuidance,
            "Load one relevant skill before the first tool call and follow its procedure literally. If the procedure does not fit the task, say why and continue with the plain workflow instead of improvising.",
        ),
        ImprovementSignal::ModelRoleWeakness { .. } => (
            ComponentKind::SystemPrompt,
            "Match planning depth to the role: state assumptions explicitly, separate facts from inference, and prefer one small verified step over a broad unverified change.",
        ),
        ImprovementSignal::HoldPattern { .. } => (
            ComponentKind::ToolGuidance,
            "When the next step is blocked, stop and report the blocker with the exact failing command, the error text and what was already tried. Do not retry the same command with cosmetic changes.",
        ),
        ImprovementSignal::CostTurnExcess { .. } => (
            ComponentKind::ToolGuidance,
            "Prefer one decisive tool call over several speculative ones: read the smallest file that answers the question, batch independent reads, and stop once the acceptance checks pass.",
        ),
    }
}

/// Domain guidance: knowledge, constraints and procedures that only belong to one domain.
fn domain_guidance(
    signal: &ImprovementSignal,
    domain: HarnessDomain,
) -> (ComponentKind, String) {
    let domain_id = domain.id();
    match signal {
        ImprovementSignal::ScorecardWeakness { role, .. } => (
            ComponentKind::Memory,
            format!(
                "DOMAIN {domain_id}: the {role} lane owns this work. Record the lane's acceptance checks and the evidence this domain expects before starting, and cite them in the handoff."
            ),
        ),
        ImprovementSignal::RepeatedCorrection { correction, .. } => (
            ComponentKind::Memory,
            format!(
                "DOMAIN {domain_id}: a recurring correction here is \"{correction}\". Apply it as a constraint before the first edit instead of rediscovering it."
            ),
        ),
        ImprovementSignal::RoutingWeakness { detail, .. } => (
            ComponentKind::Skills,
            format!(
                "DOMAIN {domain_id}: when this work needs routing, use this procedure: {detail}. State the chosen lane and why before delegating."
            ),
        ),
        ImprovementSignal::SkillUnderperformance { skill, .. } => (
            ComponentKind::Skills,
            format!(
                "DOMAIN {domain_id}: the {skill} procedure applies here. Run its verification steps in order and record which step produced the verdict."
            ),
        ),
        ImprovementSignal::ModelRoleWeakness { role, .. } => (
            ComponentKind::Memory,
            format!(
                "DOMAIN {domain_id}: the {role} role in this domain needs the domain facts stated up front. List the invariant facts of the domain before reasoning about the change."
            ),
        ),
        ImprovementSignal::HoldPattern {
            node,
            failure_class,
            ..
        } => (
            ComponentKind::Memory,
            format!(
                "DOMAIN {domain_id}: the {node} step has repeatedly reached {failure_class}. Prefer the domain's mature path and escalate with the exact failing input instead of inventing a workaround."
            ),
        ),
        ImprovementSignal::CostTurnExcess { lane, .. } => (
            ComponentKind::Skills,
            format!(
                "DOMAIN {domain_id}: the {lane} lane has a cheaper verified path. Use the domain's shortest end-to-end procedure and skip the exploratory sweep when the acceptance checks are already known."
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::harness_evolution::model::{AuthorityClass, HarnessManifest, ProposalKind};
    use crate::engine::harness_evolution::policy;

    fn base_revision() -> HarnessRevision {
        let manifest = HarnessManifest::empty()
            .with_component(Component::green(
                ComponentKind::SystemPrompt,
                "baseline prompt",
                "base prompt",
            ))
            .with_component(Component::green(
                ComponentKind::Memory,
                "baseline memory",
                "base memory",
            ));
        HarnessRevision::draft("base-1", HarnessDomain::RustBackend, manifest, "owner")
    }

    fn target(signal: ImprovementSignal) -> ImprovementTarget {
        ImprovementTarget::new(
            HarnessDomain::RustBackend,
            signal,
            vec!["scorecard:run-1".into()],
        )
    }

    fn hold_target() -> ImprovementTarget {
        target(ImprovementSignal::HoldPattern {
            node: "smith-execute".into(),
            failure_class: "compile_error".into(),
            holds: 3,
        })
    }

    fn cost_target() -> ImprovementTarget {
        target(ImprovementSignal::CostTurnExcess {
            lane: "smith".into(),
            metric: "turns".into(),
            excess_percent: 40,
        })
    }

    fn all_signals() -> Vec<ImprovementSignal> {
        vec![
            ImprovementSignal::ScorecardWeakness {
                role: "smith".into(),
                agent: "opencode".into(),
                weakness: "rework".into(),
            },
            ImprovementSignal::RepeatedCorrection {
                lane: "smith".into(),
                correction: "run the focused test".into(),
            },
            ImprovementSignal::RoutingWeakness {
                lane: "lead".into(),
                detail: "handoff to QA".into(),
            },
            ImprovementSignal::SkillUnderperformance {
                skill: "rust-testing".into(),
                detail: "skipped".into(),
            },
            ImprovementSignal::ModelRoleWeakness {
                model: "flash".into(),
                role: "architect".into(),
                detail: "shallow plan".into(),
            },
            ImprovementSignal::HoldPattern {
                node: "qa-verify".into(),
                failure_class: "flaky".into(),
                holds: 2,
            },
            ImprovementSignal::CostTurnExcess {
                lane: "research".into(),
                metric: "tokens".into(),
                excess_percent: 25,
            },
        ]
    }

    #[test]
    fn k1_proposes_one_candidate_per_scope() {
        let base = base_revision();
        let batch = propose(&base, &ProposalRequest::k1(vec![hold_target()])).unwrap();
        assert_eq!(batch.invariant.len(), 1);
        assert_eq!(batch.domain_scope.len(), 1);
        assert_eq!(batch.len(), 2);
        assert!(batch.validate().is_ok());
        assert!(batch.within_budget());
    }

    #[test]
    fn invariant_and_domain_candidates_stay_separate() {
        let base = base_revision();
        let batch = propose(&base, &ProposalRequest::k1(vec![hold_target()])).unwrap();
        for proposed in &batch.invariant {
            assert_eq!(proposed.candidate.kind, ProposalKind::Invariant);
            for change in &proposed.candidate.changes {
                assert!(
                    INVARIANT_COMPONENT_KINDS.contains(&change.kind),
                    "invariant candidate touched {}",
                    change.kind
                );
            }
        }
        for proposed in &batch.domain_scope {
            assert_eq!(proposed.candidate.kind, ProposalKind::Domain);
            for change in &proposed.candidate.changes {
                assert!(
                    DOMAIN_COMPONENT_KINDS.contains(&change.kind),
                    "domain candidate touched {}",
                    change.kind
                );
            }
        }
        let set = batch.candidate_set().unwrap();
        assert_eq!(set.invariant.len(), 1);
        assert_eq!(set.domain.len(), 1);
    }

    #[test]
    fn every_change_carries_the_target_rationale_and_evidence() {
        let base = base_revision();
        let batch = propose(&base, &ProposalRequest::k1(vec![hold_target()])).unwrap();
        for proposed in batch.all() {
            assert!(proposed.rationale.contains("hold_pattern"));
            assert!(proposed.rationale.contains("scorecard:run-1"));
            assert_eq!(proposed.evidence_refs, vec!["scorecard:run-1".to_string()]);
            for change in &proposed.candidate.changes {
                assert!(change.rationale.contains("hold_pattern"));
                assert!(!change.rationale.trim().is_empty());
            }
        }
    }

    #[test]
    fn proposer_never_targets_a_red_surface() {
        let base = base_revision();
        let targets = all_signals().into_iter().map(target).collect();
        let batch = propose(&base, &ProposalRequest::k1(targets)).unwrap();
        for proposed in batch.all() {
            for change in &proposed.candidate.changes {
                assert!(!change.kind.minimum_authority().is_red());
                assert!(!policy::is_protected_kind(change.kind));
            }
        }
    }

    #[test]
    fn base_authority_is_preserved_and_new_components_get_the_floor() {
        let base = base_revision();

        // SystemPrompt exists as GREEN in the base: the proposal keeps that class.
        let invariant_batch = propose(
            &base,
            &ProposalRequest::k1(vec![target(ImprovementSignal::ScorecardWeakness {
                role: "smith".into(),
                agent: "opencode".into(),
                weakness: "rework".into(),
            })]),
        )
        .unwrap();
        let change = &invariant_batch.invariant[0].candidate.changes[0];
        let component = change.component.as_ref().unwrap();
        assert_eq!(component.kind, ComponentKind::SystemPrompt);
        assert_eq!(component.authority, AuthorityClass::Green);

        // Skills does not exist in the base: the proposal creates it at the kind's floor.
        let domain_batch = propose(
            &base,
            &ProposalRequest::k1(vec![target(ImprovementSignal::SkillUnderperformance {
                skill: "rust-testing".into(),
                detail: "skipped".into(),
            })]),
        )
        .unwrap();
        let change = &domain_batch.domain_scope[0].candidate.changes[0];
        let component = change.component.as_ref().unwrap();
        assert_eq!(component.kind, ComponentKind::Skills);
        assert_eq!(component.authority, ComponentKind::Skills.minimum_authority());
    }

    #[test]
    fn budget_caps_candidates_per_scope() {
        let base = base_revision();
        let targets: Vec<ImprovementTarget> =
            all_signals().into_iter().map(target).collect();
        let request = ProposalRequest {
            targets,
            budget: EvolutionBudget {
                k: 2,
                max_steps: 1,
                max_probes: 4,
            },
            mode: EvolutionMode::Parallel,
        };
        let batch = propose(&base, &request).unwrap();
        assert_eq!(batch.invariant.len(), 2);
        assert_eq!(batch.domain_scope.len(), 2);
        assert!(batch.validate().is_ok());
        let set = batch.candidate_set().unwrap();
        assert_eq!(set.len(), 4);
    }

    #[test]
    fn higher_priority_signal_wins_when_budget_is_one() {
        let base = base_revision();
        let targets = vec![cost_target(), hold_target()];
        let batch = propose(&base, &ProposalRequest::k1(targets)).unwrap();
        // HoldPattern (90) outranks CostTurnExcess (30); both scopes use the same top target.
        assert_eq!(batch.invariant[0].target_index, 1);
        assert_eq!(batch.domain_scope[0].target_index, 1);
        assert!(batch.invariant[0]
            .candidate
            .changes
            .iter()
            .all(|change| change.rationale.contains("hold_pattern")));
    }

    #[test]
    fn target_domain_mismatch_is_refused() {
        let base = base_revision();
        let foreign = ImprovementTarget::new(
            HarnessDomain::YewMvi,
            ImprovementSignal::CostTurnExcess {
                lane: "lead".into(),
                metric: "tokens".into(),
                excess_percent: 10,
            },
            vec!["roi:row-1".into()],
        );
        let error = propose(&base, &ProposalRequest::k1(vec![foreign])).unwrap_err();
        assert_eq!(error.code(), "invalid_candidate");
        assert!(error.to_string().contains("domain"));
    }

    #[test]
    fn target_without_evidence_is_refused() {
        let base = base_revision();
        let bare = ImprovementTarget::new(
            HarnessDomain::RustBackend,
            ImprovementSignal::CostTurnExcess {
                lane: "lead".into(),
                metric: "tokens".into(),
                excess_percent: 10,
            },
            vec![],
        );
        let error = propose(&base, &ProposalRequest::k1(vec![bare])).unwrap_err();
        assert_eq!(error.code(), "invalid_candidate");
        assert!(error.to_string().contains("no evidence refs"));
    }

    #[test]
    fn empty_request_and_zero_budget_are_refused() {
        let base = base_revision();
        let empty = ProposalRequest::k1(vec![]);
        assert_eq!(
            propose(&base, &empty).unwrap_err().code(),
            "invalid_candidate"
        );
        let zero = ProposalRequest {
            targets: vec![hold_target()],
            budget: EvolutionBudget {
                k: 0,
                max_steps: 1,
                max_probes: 1,
            },
            mode: EvolutionMode::Parallel,
        };
        assert_eq!(propose(&base, &zero).unwrap_err().code(), "invalid_budget");
    }

    #[test]
    fn proposal_is_deterministic() {
        let base = base_revision();
        let first = propose(&base, &ProposalRequest::k1(vec![hold_target(), cost_target()])).unwrap();
        let second = propose(&base, &ProposalRequest::k1(vec![hold_target(), cost_target()])).unwrap();
        assert_eq!(first, second);
        let set = first.candidate_set().unwrap();
        let again = second.candidate_set().unwrap();
        assert_eq!(set, again);
    }

    #[test]
    fn candidates_target_one_base_revision_and_validate_as_a_set() {
        let base = base_revision();
        let batch = propose(&base, &ProposalRequest::k1(vec![hold_target()])).unwrap();
        for proposed in batch.all() {
            assert_eq!(proposed.candidate.base_revision, base.id);
        }
        let set = batch.candidate_set().unwrap();
        set.validate().unwrap();
        assert_eq!(set.budget, EvolutionBudget::k1());
    }

    #[test]
    fn proposed_content_does_not_introduce_protected_topics() {
        let base = base_revision();
        let targets: Vec<ImprovementTarget> =
            all_signals().into_iter().map(target).collect();
        let batch = propose(&base, &ProposalRequest::k1(targets)).unwrap();
        for proposed in batch.all() {
            for change in &proposed.candidate.changes {
                let component = change.component.as_ref().unwrap();
                let base_surfaces = base
                    .manifest
                    .get(component.kind)
                    .map(|existing| policy::detect_protected_surfaces(&existing.content))
                    .unwrap_or_default();
                let proposed_surfaces = policy::detect_protected_surfaces(&component.content);
                assert_eq!(
                    proposed_surfaces, base_surfaces,
                    "{} introduced a protected topic",
                    component.kind
                );
            }
        }
    }

    #[test]
    fn invariant_candidates_name_their_guidance_family() {
        let base = base_revision();
        let themes: Vec<&str> = vec![
            "reasoning",
            "planning",
            "tool_use",
            "context",
            "delegation",
        ];
        let batch = propose(&base, &ProposalRequest::k1(vec![hold_target()])).unwrap();
        for proposed in &batch.invariant {
            assert!(
                themes.iter().any(|theme| proposed.rationale.contains(theme)),
                "invariant rationale names no guidance family: {}",
                proposed.rationale
            );
        }
        // HoldPattern is reasoning guidance, and the invariant change lands on tool guidance.
        assert!(batch.invariant[0].rationale.contains("reasoning"));
        assert_eq!(
            batch.invariant[0].candidate.changes[0].kind,
            ComponentKind::ToolGuidance
        );
    }

    #[test]
    fn domain_candidates_name_their_guidance_family() {
        let base = base_revision();
        let batch = propose(&base, &ProposalRequest::k1(vec![hold_target()])).unwrap();
        let rationale = &batch.domain_scope[0].rationale;
        assert!(rationale.contains("constraints"));
        assert!(rationale.contains("hold_pattern"));
    }

    #[test]
    fn domain_candidates_are_domain_scoped() {
        let base = base_revision();
        let batch = propose(&base, &ProposalRequest::k1(vec![hold_target()])).unwrap();
        let domain = &batch.domain_scope[0];
        let change = &domain.candidate.changes[0];
        let content = &change.component.as_ref().unwrap().content;
        assert!(content.contains(base.domain.id()));
        for proposed in &batch.invariant {
            let change = &proposed.candidate.changes[0];
            let content = &change.component.as_ref().unwrap().content;
            assert!(!content.starts_with("DOMAIN "));
        }
    }
}
