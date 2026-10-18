//! Agent OS Phase 2 — controlled executive/expert swarm contract.
//!
//! This module deliberately does NOT create a second workflow engine. Forge remains the
//! deterministic delivery machine. The executive swarm is the bounded reasoning layer above
//! it: classify a decision, select a small read-only panel, collect independent verdicts,
//! synthesize one decision, then hand an implementation contract into the existing Forge SDLC.
//!
//! The OpenCode harness may execute these profiles, but it must consume this contract rather
//! than inventing a second agent catalogue or routing table. Executive/expert agents are
//! read-only by construction here. They advise; Forge executes.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Ordinary expert swarms stay small enough to be useful rather than becoming a token furnace.
pub const MAX_SWARM_EXPERTS: usize = 5;
/// Council may add one more domain seat, but is still deliberately bounded.
pub const MAX_COUNCIL_EXPERTS: usize = 6;
/// A primary executive/expert may ask at most three bounded micro-agent questions.
pub const MAX_CHILDREN_PER_EXPERT: usize = 3;
/// Children never spawn grandchildren.
pub const MAX_CHILD_DEPTH: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentAuthority {
    ReadOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExecutiveAgent {
    ChiefOfStaff,
    Advisor,
    PrincipalArchitect,
    Adversary,
}

impl ExecutiveAgent {
    pub const fn id(self) -> &'static str {
        match self {
            Self::ChiefOfStaff => "exec-chief-of-staff",
            Self::Advisor => "exec-advisor",
            Self::PrincipalArchitect => "exec-principal-architect",
            Self::Adversary => "exec-adversary",
        }
    }

    pub const fn authority(self) -> AgentAuthority {
        AgentAuthority::ReadOnly
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExpertAgent {
    Security,
    Database,
    Rust,
    UiMvi,
    DevOps,
    Integration,
    Performance,
    Research,
}

impl ExpertAgent {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Security => "expert-security",
            Self::Database => "expert-database",
            Self::Rust => "expert-rust",
            Self::UiMvi => "expert-ui-mvi",
            Self::DevOps => "expert-devops",
            Self::Integration => "expert-integration",
            Self::Performance => "expert-performance",
            Self::Research => "expert-research",
        }
    }

    pub const fn authority(self) -> AgentAuthority {
        AgentAuthority::ReadOnly
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SwarmAgent {
    Executive(ExecutiveAgent),
    Expert(ExpertAgent),
}

impl SwarmAgent {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Executive(agent) => agent.id(),
            Self::Expert(agent) => agent.id(),
        }
    }

    pub const fn authority(self) -> AgentAuthority {
        AgentAuthority::ReadOnly
    }
}

pub const EXECUTIVE_AGENT_CATALOG: [ExecutiveAgent; 4] = [
    ExecutiveAgent::ChiefOfStaff,
    ExecutiveAgent::Advisor,
    ExecutiveAgent::PrincipalArchitect,
    ExecutiveAgent::Adversary,
];

pub const EXPERT_AGENT_CATALOG: [ExpertAgent; 8] = [
    ExpertAgent::Security,
    ExpertAgent::Database,
    ExpertAgent::Rust,
    ExpertAgent::UiMvi,
    ExpertAgent::DevOps,
    ExpertAgent::Integration,
    ExpertAgent::Performance,
    ExpertAgent::Research,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionSignal {
    Strategy,
    Architecture,
    Security,
    Database,
    Rust,
    UiMvi,
    Deployment,
    Integration,
    Performance,
    Research,
    BuildVsBuy,
    Platform,
    CrossDomain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwarmMode {
    Advisory,
    Council,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CouncilGate {
    pub credible_paths: u8,
    pub material_consequence: bool,
    pub difficult_to_reverse: bool,
}

impl CouncilGate {
    pub const fn warranted(self) -> bool {
        self.credible_paths >= 2 && self.material_consequence && self.difficult_to_reverse
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwarmRequest {
    pub mode: SwarmMode,
    pub signals: Vec<DecisionSignal>,
    pub council_gate: Option<CouncilGate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwarmPlan {
    /// Chief of Staff owns coordination and recording, but is not counted as an expert seat.
    pub coordinator: ExecutiveAgent,
    pub participants: Vec<SwarmAgent>,
    /// Chief of Staff synthesizes the panel; it never takes an implementation seat.
    pub synthesis_owner: ExecutiveAgent,
    pub max_children_per_expert: usize,
    pub max_child_depth: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwarmRoutingError {
    NoDecisionSignal,
    CouncilNotWarranted,
    TooManyParticipants { requested: usize, maximum: usize },
}

impl fmt::Display for SwarmRoutingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDecisionSignal => write!(f, "swarm routing requires at least one decision signal"),
            Self::CouncilNotWarranted => write!(
                f,
                "Council requires at least two credible paths, material consequences, and difficult reversibility"
            ),
            Self::TooManyParticipants { requested, maximum } => write!(
                f,
                "swarm requested {requested} expert seats but the mode permits at most {maximum}"
            ),
        }
    }
}

impl std::error::Error for SwarmRoutingError {}

fn push_unique(participants: &mut Vec<SwarmAgent>, participant: SwarmAgent) {
    if !participants.contains(&participant) {
        participants.push(participant);
    }
}

fn add_signal_participants(signal: DecisionSignal, participants: &mut Vec<SwarmAgent>) {
    let exec = |agent| SwarmAgent::Executive(agent);
    let expert = |agent| SwarmAgent::Expert(agent);

    match signal {
        DecisionSignal::Strategy => {
            push_unique(participants, exec(ExecutiveAgent::Advisor));
        }
        DecisionSignal::Architecture => {
            push_unique(participants, exec(ExecutiveAgent::PrincipalArchitect));
        }
        DecisionSignal::Security => {
            push_unique(participants, exec(ExecutiveAgent::PrincipalArchitect));
            push_unique(participants, expert(ExpertAgent::Security));
        }
        DecisionSignal::Database => {
            push_unique(participants, exec(ExecutiveAgent::PrincipalArchitect));
            push_unique(participants, expert(ExpertAgent::Database));
        }
        DecisionSignal::Rust => {
            push_unique(participants, exec(ExecutiveAgent::PrincipalArchitect));
            push_unique(participants, expert(ExpertAgent::Rust));
        }
        DecisionSignal::UiMvi => {
            push_unique(participants, exec(ExecutiveAgent::PrincipalArchitect));
            push_unique(participants, expert(ExpertAgent::UiMvi));
        }
        DecisionSignal::Deployment => {
            push_unique(participants, exec(ExecutiveAgent::PrincipalArchitect));
            push_unique(participants, expert(ExpertAgent::DevOps));
        }
        DecisionSignal::Integration => {
            push_unique(participants, exec(ExecutiveAgent::PrincipalArchitect));
            push_unique(participants, expert(ExpertAgent::Integration));
        }
        DecisionSignal::Performance => {
            push_unique(participants, exec(ExecutiveAgent::PrincipalArchitect));
            push_unique(participants, expert(ExpertAgent::Performance));
            push_unique(participants, expert(ExpertAgent::Rust));
        }
        DecisionSignal::Research => {
            push_unique(participants, expert(ExpertAgent::Research));
        }
        DecisionSignal::BuildVsBuy => {
            push_unique(participants, exec(ExecutiveAgent::Advisor));
            push_unique(participants, exec(ExecutiveAgent::PrincipalArchitect));
            push_unique(participants, expert(ExpertAgent::Research));
        }
        DecisionSignal::Platform => {
            push_unique(participants, exec(ExecutiveAgent::Advisor));
            push_unique(participants, exec(ExecutiveAgent::PrincipalArchitect));
            push_unique(participants, expert(ExpertAgent::DevOps));
        }
        DecisionSignal::CrossDomain => {
            push_unique(participants, exec(ExecutiveAgent::Advisor));
            push_unique(participants, exec(ExecutiveAgent::PrincipalArchitect));
        }
    }
}

/// Deterministic panel selection. The router never silently drops an expert to fit a cap;
/// over-broad requests fail so the coordinator can narrow the question or split the decision.
pub fn plan_swarm(request: &SwarmRequest) -> Result<SwarmPlan, SwarmRoutingError> {
    if request.signals.is_empty() {
        return Err(SwarmRoutingError::NoDecisionSignal);
    }

    if request.mode == SwarmMode::Council
        && !request.council_gate.is_some_and(CouncilGate::warranted)
    {
        return Err(SwarmRoutingError::CouncilNotWarranted);
    }

    let mut participants = Vec::new();
    for signal in &request.signals {
        add_signal_participants(*signal, &mut participants);
    }

    if request.mode == SwarmMode::Council {
        push_unique(
            &mut participants,
            SwarmAgent::Executive(ExecutiveAgent::Adversary),
        );
    }

    let maximum = match request.mode {
        SwarmMode::Advisory => MAX_SWARM_EXPERTS,
        SwarmMode::Council => MAX_COUNCIL_EXPERTS,
    };
    if participants.len() > maximum {
        return Err(SwarmRoutingError::TooManyParticipants {
            requested: participants.len(),
            maximum,
        });
    }

    Ok(SwarmPlan {
        coordinator: ExecutiveAgent::ChiefOfStaff,
        participants,
        synthesis_owner: ExecutiveAgent::ChiefOfStaff,
        max_children_per_expert: MAX_CHILDREN_PER_EXPERT,
        max_child_depth: MAX_CHILD_DEPTH,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    Proceed,
    Reject,
    Conditional,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpertVerdict {
    pub agent: SwarmAgent,
    pub verdict: Verdict,
    pub recommendation: String,
    pub confidence: u8,
    pub key_reasons: Vec<String>,
    pub primary_risk: String,
    pub assumptions: Vec<String>,
    pub evidence: Vec<String>,
    pub what_would_change_my_mind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerdictError {
    ConfidenceOutOfRange,
    MissingRecommendation,
    MissingPrimaryRisk,
}

pub fn validate_verdict(verdict: &ExpertVerdict) -> Result<(), VerdictError> {
    if verdict.confidence > 100 {
        return Err(VerdictError::ConfidenceOutOfRange);
    }
    if verdict.recommendation.trim().is_empty() {
        return Err(VerdictError::MissingRecommendation);
    }
    if verdict.primary_risk.trim().is_empty() {
        return Err(VerdictError::MissingPrimaryRisk);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionSynthesis {
    pub decision_id: String,
    pub question: String,
    pub recommendation: String,
    pub confidence: u8,
    pub consensus: Vec<String>,
    pub dissent: Vec<String>,
    pub accepted_risks: Vec<String>,
    pub next_action: String,
    pub source_agents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForgeDecisionHandoff {
    /// Optional because strategic decisions may not yet have a Story Board id.
    pub story_id: Option<String>,
    pub decision_id: String,
    pub objective: String,
    pub recommendation: String,
    pub architecture_contract: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub dissent_to_preserve: Vec<String>,
    pub source_agents: Vec<String>,
}

/// This is the only Phase-2 output shape intended to cross into implementation.
/// It contains a decision/contract, never permission for the swarm itself to edit or publish.
pub fn handoff_to_forge(synthesis: &DecisionSynthesis) -> ForgeDecisionHandoff {
    ForgeDecisionHandoff {
        story_id: None,
        decision_id: synthesis.decision_id.clone(),
        objective: synthesis.question.clone(),
        recommendation: synthesis.recommendation.clone(),
        architecture_contract: Vec::new(),
        acceptance_criteria: Vec::new(),
        dissent_to_preserve: synthesis.dissent.clone(),
        source_agents: synthesis.source_agents.clone(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EscalationKind {
    ExecutionSequencing,
    ArchitectureBoundary,
    StrategicTradeoff,
    MultiDomainIrreversible,
    ProductIntent,
    ProductionAuthorization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EscalationTarget {
    Lead,
    PrincipalArchitect,
    Advisor,
    Council,
    Chris,
}

pub const fn escalation_target(kind: EscalationKind) -> EscalationTarget {
    match kind {
        EscalationKind::ExecutionSequencing => EscalationTarget::Lead,
        EscalationKind::ArchitectureBoundary => EscalationTarget::PrincipalArchitect,
        EscalationKind::StrategicTradeoff => EscalationTarget::Advisor,
        EscalationKind::MultiDomainIrreversible => EscalationTarget::Council,
        EscalationKind::ProductIntent | EscalationKind::ProductionAuthorization => {
            EscalationTarget::Chris
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executive_and_expert_profiles_are_read_only() {
        for agent in EXECUTIVE_AGENT_CATALOG {
            assert_eq!(agent.authority(), AgentAuthority::ReadOnly);
        }
        for agent in EXPERT_AGENT_CATALOG {
            assert_eq!(agent.authority(), AgentAuthority::ReadOnly);
        }
    }

    #[test]
    fn security_routes_to_architecture_and_security_without_council() {
        let plan = plan_swarm(&SwarmRequest {
            mode: SwarmMode::Advisory,
            signals: vec![DecisionSignal::Security],
            council_gate: None,
        })
        .expect("security advisory should route");

        assert_eq!(
            plan.participants,
            vec![
                SwarmAgent::Executive(ExecutiveAgent::PrincipalArchitect),
                SwarmAgent::Expert(ExpertAgent::Security),
            ]
        );
        assert_eq!(plan.coordinator, ExecutiveAgent::ChiefOfStaff);
    }

    #[test]
    fn build_vs_buy_routes_advisor_architect_and_research() {
        let plan = plan_swarm(&SwarmRequest {
            mode: SwarmMode::Advisory,
            signals: vec![DecisionSignal::BuildVsBuy],
            council_gate: None,
        })
        .expect("build-vs-buy advisory should route");

        assert_eq!(
            plan.participants,
            vec![
                SwarmAgent::Executive(ExecutiveAgent::Advisor),
                SwarmAgent::Executive(ExecutiveAgent::PrincipalArchitect),
                SwarmAgent::Expert(ExpertAgent::Research),
            ]
        );
    }

    #[test]
    fn council_requires_the_full_gate() {
        let error = plan_swarm(&SwarmRequest {
            mode: SwarmMode::Council,
            signals: vec![DecisionSignal::Platform],
            council_gate: Some(CouncilGate {
                credible_paths: 2,
                material_consequence: true,
                difficult_to_reverse: false,
            }),
        })
        .expect_err("reversible choice should not invoke Council");

        assert_eq!(error, SwarmRoutingError::CouncilNotWarranted);
    }

    #[test]
    fn council_adds_mandatory_adversary() {
        let plan = plan_swarm(&SwarmRequest {
            mode: SwarmMode::Council,
            signals: vec![DecisionSignal::Platform],
            council_gate: Some(CouncilGate {
                credible_paths: 2,
                material_consequence: true,
                difficult_to_reverse: true,
            }),
        })
        .expect("platform Council should route");

        assert!(plan
            .participants
            .contains(&SwarmAgent::Executive(ExecutiveAgent::Adversary)));
        assert!(plan.participants.len() <= MAX_COUNCIL_EXPERTS);
    }

    #[test]
    fn router_refuses_to_silently_drop_experts_to_fit_cap() {
        let error = plan_swarm(&SwarmRequest {
            mode: SwarmMode::Advisory,
            signals: vec![
                DecisionSignal::Security,
                DecisionSignal::Database,
                DecisionSignal::Deployment,
                DecisionSignal::Integration,
                DecisionSignal::Performance,
            ],
            council_gate: None,
        })
        .expect_err("over-broad panel must be narrowed, not silently truncated");

        assert!(matches!(
            error,
            SwarmRoutingError::TooManyParticipants {
                maximum: MAX_SWARM_EXPERTS,
                ..
            }
        ));
    }

    #[test]
    fn child_fanout_is_bounded_and_has_no_grandchildren() {
        let plan = plan_swarm(&SwarmRequest {
            mode: SwarmMode::Advisory,
            signals: vec![DecisionSignal::Architecture],
            council_gate: None,
        })
        .expect("architecture advisory should route");

        assert_eq!(plan.max_children_per_expert, 3);
        assert_eq!(plan.max_child_depth, 1);
    }

    #[test]
    fn verdict_rejects_invalid_confidence_and_empty_recommendation() {
        let bad = ExpertVerdict {
            agent: SwarmAgent::Executive(ExecutiveAgent::Advisor),
            verdict: Verdict::Conditional,
            recommendation: String::new(),
            confidence: 101,
            key_reasons: vec![],
            primary_risk: "risk".into(),
            assumptions: vec![],
            evidence: vec![],
            what_would_change_my_mind: String::new(),
        };
        assert_eq!(
            validate_verdict(&bad),
            Err(VerdictError::ConfidenceOutOfRange)
        );
    }

    #[test]
    fn escalation_ladder_is_deterministic() {
        assert_eq!(
            escalation_target(EscalationKind::ExecutionSequencing),
            EscalationTarget::Lead
        );
        assert_eq!(
            escalation_target(EscalationKind::ArchitectureBoundary),
            EscalationTarget::PrincipalArchitect
        );
        assert_eq!(
            escalation_target(EscalationKind::MultiDomainIrreversible),
            EscalationTarget::Council
        );
        assert_eq!(
            escalation_target(EscalationKind::ProductionAuthorization),
            EscalationTarget::Chris
        );
    }

    #[test]
    fn forge_handoff_preserves_dissent_and_never_grants_execution_authority() {
        let synthesis = DecisionSynthesis {
            decision_id: "DEC-1".into(),
            question: "Which platform?".into(),
            recommendation: "Use A".into(),
            confidence: 88,
            consensus: vec!["A is simpler".into()],
            dissent: vec!["Portability risk remains".into()],
            accepted_risks: vec!["Vendor coupling".into()],
            next_action: "Create Forge story".into(),
            source_agents: vec!["exec-advisor".into(), "exec-adversary".into()],
        };

        let handoff = handoff_to_forge(&synthesis);
        assert_eq!(handoff.dissent_to_preserve, synthesis.dissent);
        assert_eq!(
            ExecutiveAgent::Advisor.authority(),
            AgentAuthority::ReadOnly
        );
    }
}
