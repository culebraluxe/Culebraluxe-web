//! LLM-backed proposer (TD 3) — a read-only advisory seat proposes harness changes as strict JSON.
//!
//! This module is the transport adapter between the existing Phase 2 advisory layer
//! (`crate::engine::advisory_runner`) and the Harness Evolution [`Proposer`] contract. It asks one
//! **read-only** seat (default `exec-chief-of-staff`) for invariant and domain changes, parses the
//! reply as strict JSON under hard size/shape limits, builds changes that can never declare an
//! authority class, and runs [`super::policy::validate_candidate`] on every candidate **before**
//! returning it. A proposal that cannot survive the policy gate is never handed to Probe.
//!
//! Safety properties, in code rather than prose:
//!
//! * the only I/O is [`SeatRunner::run`] — the existing read-only seat transport. This module never
//!   executes a tool, edits a file, publishes, migrates or deploys;
//! * the reply must be one JSON object with exactly `invariant` and `domain` arrays of change
//!   objects; unknown keys, free text, code fences and oversized replies are rejected;
//! * a wire change carrying an `authority` field is refused outright — the proposer may not change
//!   a component's declared authority (that is how automatic adoption would be unlocked);
//! * RED kinds (`Scripts`, `Plugins`) and policy-protected content are refused, and each candidate
//!   revision is materialized with [`HarnessRevision::from_changes`] and re-validated by the policy;
//! * the invariant/domain scopes stay separate and only the kinds `propose.rs` assigns to each
//!   scope are accepted;
//! * the budget bounds the number of candidates per scope; exceeding it is an error, not a silent
//!   truncation (a truncated proposal would hide that the model ignored the budget).
//!
//! Tests use a deterministic fake [`SeatRunner`]; no test makes a live model call.

use std::fmt;

use serde::Deserialize;

use crate::engine::advisory_runner::{SeatRunner, SeatTurn};
use crate::engine::executive_swarm::{ExecutiveAgent, SwarmAgent};
use crate::engine::harness_evolution::experiment::{ProposalContext, Proposer};
use crate::engine::harness_evolution::model::{
    AuthorityClass, Candidate, CandidateSet, ChangeAction, Component, ComponentChange,
    ComponentKind, EvolutionBudget, EvolutionError, EvolutionMode, HarnessRevision, ProposalKind,
};
use crate::engine::harness_evolution::policy;
use crate::engine::harness_evolution::propose::{
    ImprovementTarget, DOMAIN_COMPONENT_KINDS, INVARIANT_COMPONENT_KINDS,
};

/// The default proposing seat: the read-only Chief of Staff. The principal architect seat may be
/// selected with [`LlmProposer::with_agent`].
pub const DEFAULT_PROPOSER_SEAT: SwarmAgent = SwarmAgent::Executive(ExecutiveAgent::ChiefOfStaff);

/// Hard limits applied to one seat reply. These are deliberately small: a harness edit is a short,
/// reviewable change, not an essay, and an oversized reply is more likely to be an injection or a
/// confused model than a good proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LlmProposalLimits {
    /// Maximum accepted reply size in bytes, checked before JSON parsing.
    pub max_response_bytes: usize,
    /// Hard cap on changes per scope, independent of the experiment budget.
    pub max_changes_per_scope: usize,
    /// Maximum `content` length for one change in characters.
    pub max_content_chars: usize,
    /// Maximum `rationale` length for one change in characters.
    pub max_rationale_chars: usize,
}

impl Default for LlmProposalLimits {
    fn default() -> Self {
        Self {
            max_response_bytes: 32_768,
            max_changes_per_scope: 8,
            max_content_chars: 8_192,
            max_rationale_chars: 1_024,
        }
    }
}

/// The strict wire shape the seat must return. `deny_unknown_fields` makes "mostly JSON" a
/// rejection rather than a guess, and the missing arrays are errors too.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireProposal {
    invariant: Vec<WireChange>,
    domain: Vec<WireChange>,
}

/// One proposed change. `authority` exists only so a reply that tries to declare one can be
/// rejected with a precise error (the JSON schema we ask for does not include it).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireChange {
    kind: ComponentKind,
    action: ChangeAction,
    #[serde(default)]
    content: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    authority: Option<AuthorityClass>,
    rationale: String,
}

/// An LLM-backed [`Proposer`].
///
/// The proposer is read-only by construction: its only external call is the advisory seat runner.
/// `targets` are the typed Phase 3 improvement signals the prompt is built from; they are validated
/// (non-empty, evidence-linked, same domain as the base revision) before any seat call.
pub struct LlmProposer<'a> {
    seat: &'a dyn SeatRunner,
    targets: Vec<ImprovementTarget>,
    agent: SwarmAgent,
    limits: LlmProposalLimits,
}

impl<'a> LlmProposer<'a> {
    /// A proposer that asks the Chief of Staff seat for changes motivated by `targets`.
    pub fn new(seat: &'a dyn SeatRunner, targets: Vec<ImprovementTarget>) -> Self {
        Self {
            seat,
            targets,
            agent: DEFAULT_PROPOSER_SEAT,
            limits: LlmProposalLimits::default(),
        }
    }

    /// Choose the advisory seat (e.g. `exec-principal-architect`).
    pub fn with_agent(mut self, agent: SwarmAgent) -> Self {
        self.agent = agent;
        self
    }

    pub fn with_limits(mut self, limits: LlmProposalLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn targets(&self) -> &[ImprovementTarget] {
        &self.targets
    }

    pub fn agent(&self) -> SwarmAgent {
        self.agent
    }

    /// Ask the seat once and return the policy-checked candidate set for `base`.
    ///
    /// This is the entry point the experiment runner uses (via [`Proposer`]). It is public so an
    /// operator surface can preview a proposal without running an experiment.
    pub fn propose_for(
        &self,
        base: &HarnessRevision,
        budget: EvolutionBudget,
        mode: EvolutionMode,
    ) -> Result<CandidateSet, EvolutionError> {
        self.validate_request(base, &budget)?;

        let prompt = self.prompt(base, &budget);
        let turn = self
            .seat
            .run(self.agent, &prompt)
            .map_err(|error| seat_failure(&self.agent, &error))?;
        let proposal = self.parse(&turn)?;

        let per_scope =
            (budget.candidates_per_scope() as usize).min(self.limits.max_changes_per_scope);
        if proposal.invariant.len() > per_scope {
            return Err(EvolutionError::InvalidCandidate(format!(
                "LLM proposer returned {} invariant changes but the budget allows at most {per_scope}",
                proposal.invariant.len()
            )));
        }
        if proposal.domain.len() > per_scope {
            return Err(EvolutionError::InvalidCandidate(format!(
                "LLM proposer returned {} domain changes but the budget allows at most {per_scope}",
                proposal.domain.len()
            )));
        }
        if proposal.invariant.is_empty() && proposal.domain.is_empty() {
            return Err(EvolutionError::InvalidCandidate(
                "LLM proposer returned no candidates".into(),
            ));
        }

        let mut set = CandidateSet::new(mode, budget);
        for (ordinal, wire) in proposal.invariant.iter().enumerate() {
            set.invariant.push(self.build_candidate(
                base,
                ProposalKind::Invariant,
                &INVARIANT_COMPONENT_KINDS,
                wire,
                ordinal,
            )?);
        }
        for (ordinal, wire) in proposal.domain.iter().enumerate() {
            set.domain.push(self.build_candidate(
                base,
                ProposalKind::Domain,
                &DOMAIN_COMPONENT_KINDS,
                wire,
                ordinal,
            )?);
        }
        set.validate()?;
        self.policy_gate(base, &set)?;
        Ok(set)
    }

    fn validate_request(
        &self,
        base: &HarnessRevision,
        budget: &EvolutionBudget,
    ) -> Result<(), EvolutionError> {
        budget.validate()?;
        base.verify_integrity()?;
        if self.targets.is_empty() {
            return Err(EvolutionError::InvalidCandidate(
                "LLM proposer has no improvement targets; a proposal without a Phase 3 signal is a guess"
                    .into(),
            ));
        }
        for target in &self.targets {
            target.validate()?;
            if target.domain != base.domain {
                return Err(EvolutionError::InvalidCandidate(format!(
                    "improvement target for domain {} cannot propose against base domain {}",
                    target.domain, base.domain
                )));
            }
        }
        Ok(())
    }

    fn parse(&self, turn: &SeatTurn) -> Result<WireProposal, EvolutionError> {
        let text = turn.text.trim();
        if text.len() > self.limits.max_response_bytes {
            return Err(EvolutionError::InvalidCandidate(format!(
                "LLM proposer reply is {} bytes, over the {} byte limit",
                text.len(),
                self.limits.max_response_bytes
            )));
        }
        serde_json::from_str::<WireProposal>(text).map_err(|error| {
            EvolutionError::InvalidCandidate(format!(
                "LLM proposer returned malformed JSON: {error}"
            ))
        })
    }

    fn build_candidate(
        &self,
        base: &HarnessRevision,
        scope: ProposalKind,
        allowed_kinds: &[ComponentKind],
        wire: &WireChange,
        ordinal: usize,
    ) -> Result<Candidate, EvolutionError> {
        if wire.authority.is_some() {
            return Err(EvolutionError::InvalidCandidate(format!(
                "LLM proposer declared an authority class for {}; proposals may not change declared authority",
                wire.kind
            )));
        }
        if wire.kind.minimum_authority().is_red() {
            return Err(EvolutionError::InvalidCandidate(format!(
                "LLM proposer targeted {} which is a protected RED surface",
                wire.kind
            )));
        }
        if !allowed_kinds.contains(&wire.kind) {
            return Err(EvolutionError::InvalidCandidate(format!(
                "LLM proposer targeted {} in the {} scope; allowed kinds are {:?}",
                wire.kind,
                scope.id(),
                allowed_kinds
                    .iter()
                    .map(|kind| kind.id())
                    .collect::<Vec<_>>()
            )));
        }
        let rationale = Self::bounded_text(
            &wire.rationale,
            "rationale",
            self.limits.max_rationale_chars,
        )?;
        if rationale.trim().is_empty() {
            return Err(EvolutionError::InvalidCandidate(format!(
                "LLM proposer change for {} has no rationale",
                wire.kind
            )));
        }

        let change = match wire.action {
            ChangeAction::Set => {
                let content =
                    Self::bounded_text(&wire.content, "content", self.limits.max_content_chars)?;
                if content.trim().is_empty() {
                    return Err(EvolutionError::InvalidCandidate(format!(
                        "LLM proposer SET change for {} has no content",
                        wire.kind
                    )));
                }
                let description = if wire.description.trim().is_empty() {
                    format!("{} proposal from the advisory proposer", scope.id())
                } else {
                    wire.description.clone()
                };
                // Preserve the base declaration when the component exists; a new component gets
                // the kind floor. The proposer never gets to pick the authority class.
                let authority = base
                    .manifest
                    .component_authority(wire.kind)
                    .unwrap_or_else(|| wire.kind.minimum_authority());
                let component = Component::new(wire.kind, authority, content, description);
                ComponentChange::set(component, rationale)
            }
            ChangeAction::Remove => ComponentChange::remove(wire.kind, rationale),
        };
        change.validate_shape()?;

        let candidate = Candidate::new(
            format!("{}-llm-{}-{}", base.id, scope.id(), ordinal + 1),
            base.id.clone(),
            scope,
            vec![change],
        );
        candidate.validate()?;
        Ok(candidate)
    }

    fn bounded_text(text: &str, field: &str, max_chars: usize) -> Result<String, EvolutionError> {
        if text.chars().count() > max_chars {
            return Err(EvolutionError::InvalidCandidate(format!(
                "LLM proposer {field} is {} characters, over the {max_chars} limit",
                text.chars().count()
            )));
        }
        Ok(text.to_string())
    }

    /// Re-run the pre-probe policy gate on every proposed candidate. The LLM is not trusted to
    /// respect the RED/protected surfaces just because the prompt asked it to.
    fn policy_gate(
        &self,
        base: &HarnessRevision,
        set: &CandidateSet,
    ) -> Result<(), EvolutionError> {
        for candidate in set.all() {
            let revision = HarnessRevision::from_changes(
                base,
                candidate.id.clone(),
                &candidate.changes,
                "llm-proposer",
            )?;
            if let Err(violations) = policy::validate_candidate(base, &revision, &candidate.changes)
            {
                return Err(EvolutionError::InvalidCandidate(format!(
                    "LLM proposer candidate {} rejected by policy: {}",
                    candidate.id,
                    violations
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; ")
                )));
            }
        }
        Ok(())
    }

    fn prompt(&self, base: &HarnessRevision, budget: &EvolutionBudget) -> String {
        let targets = self
            .targets
            .iter()
            .map(|target| format!("- [{}] {}", target.signal.id(), target.summary()))
            .collect::<Vec<_>>()
            .join("\n");
        let per_scope =
            (budget.candidates_per_scope() as usize).min(self.limits.max_changes_per_scope);
        let invariant_kinds = kind_ids(&INVARIANT_COMPONENT_KINDS);
        let domain_kinds = kind_ids(&DOMAIN_COMPONENT_KINDS);
        format!(
            "You are the read-only Harness Evolution proposer seat `{agent}` for the Forge SDLC.\n\
             DOMAIN: {domain}\n\
             BASE REVISION: {revision} (manifest {hash})\n\n\
             OBSERVED PHASE 3 SIGNALS (a proposal must be motivated by at least one of these and \
             cite its evidence ids in the change rationale):\n{targets}\n\n\
             PROPOSE two separate scopes of harness changes:\n\
             - `invariant`: general reasoning/planning/tool-use/context/delegation guidance; \
             allowed kinds: {invariant_kinds}\n\
             - `domain`: domain knowledge/constraints/procedures for {domain}; allowed kinds: \
             {domain_kinds}\n\n\
             HARD RULES (any violation rejects the whole reply):\n\
             1. Reply with ONLY one JSON object. No prose before or after it, no code fences.\n\
             2. At most {per_scope} change(s) per scope.\n\
             3. Each change object may contain ONLY these keys: kind, action, content, description, \
             rationale. Do NOT include an authority field and do not propose authority changes.\n\
             4. Never target scripts or plugins (protected RED surfaces).\n\
             5. action is `set` or `remove`. For `set`, content is the FULL replacement text. For \
             `remove`, content must be empty or omitted.\n\
             6. rationale is required; cite the evidence id(s) that motivated the change.\n\
             7. Never propose changes to QA/Git boundaries, production permissions, \
             security/entitlements, publication/DEV_OPS, workflow XML ownership, trusted \
             boundaries, OpenCode deny rules or destructive gates.\n\
             8. Keep content short and reviewable (max {content_limit} characters); rationale max \
             {rationale_limit} characters.\n\n\
             JSON SHAPE:\n\
             {{\"invariant\":[{{\"kind\":\"system_prompt\",\"action\":\"set\",\"content\":\"...\",\
             \"description\":\"...\",\"rationale\":\"...\"}}],\"domain\":[]}}\n",
            agent = self.agent.id(),
            domain = base.domain.id(),
            revision = base.id,
            hash = base.manifest_hash,
            targets = targets,
            invariant_kinds = invariant_kinds,
            domain_kinds = domain_kinds,
            per_scope = per_scope,
            content_limit = self.limits.max_content_chars,
            rationale_limit = self.limits.max_rationale_chars,
        )
    }
}

impl Proposer for LlmProposer<'_> {
    fn propose(&self, context: &ProposalContext<'_>) -> Result<CandidateSet, EvolutionError> {
        self.propose_for(context.base, context.budget, context.mode)
    }
}

impl fmt::Debug for LlmProposer<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LlmProposer")
            .field("agent", &self.agent)
            .field("targets", &self.targets.len())
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

fn kind_ids(kinds: &[ComponentKind]) -> String {
    kinds
        .iter()
        .map(|kind| kind.id())
        .collect::<Vec<_>>()
        .join(", ")
}

fn seat_failure(agent: &SwarmAgent, error: &str) -> EvolutionError {
    EvolutionError::InvalidCandidate(format!("LLM proposer seat {} failed: {error}", agent.id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::harness_evolution::model::{Component, HarnessDomain, HarnessManifest};
    use crate::engine::harness_evolution::propose::ImprovementSignal;
    use std::cell::RefCell;

    struct FakeSeat {
        reply: Result<String, String>,
        prompts: RefCell<Vec<(SwarmAgent, String)>>,
    }

    impl FakeSeat {
        fn replying(text: impl Into<String>) -> Self {
            Self {
                reply: Ok(text.into()),
                prompts: RefCell::new(Vec::new()),
            }
        }

        fn failing(message: impl Into<String>) -> Self {
            Self {
                reply: Err(message.into()),
                prompts: RefCell::new(Vec::new()),
            }
        }

        fn call_count(&self) -> usize {
            self.prompts.borrow().len()
        }

        fn last_prompt(&self) -> String {
            self.prompts
                .borrow()
                .last()
                .map(|(_, prompt)| prompt.clone())
                .unwrap_or_default()
        }
    }

    impl SeatRunner for FakeSeat {
        fn run(&self, agent: SwarmAgent, prompt: &str) -> Result<SeatTurn, String> {
            self.prompts.borrow_mut().push((agent, prompt.to_string()));
            self.reply
                .as_ref()
                .map(|text| SeatTurn {
                    text: text.clone(),
                    tokens: 123,
                })
                .map_err(|error| error.clone())
        }
    }

    fn base_revision() -> HarnessRevision {
        HarnessRevision::draft(
            "base-1",
            HarnessDomain::RustBackend,
            HarnessManifest::from_components([Component::green(
                ComponentKind::SystemPrompt,
                "baseline prompt",
                "baseline",
            )]),
            "owner",
        )
    }

    fn target() -> ImprovementTarget {
        ImprovementTarget::new(
            HarnessDomain::RustBackend,
            ImprovementSignal::HoldPattern {
                node: "validate".into(),
                failure_class: "missing_test".into(),
                holds: 3,
            },
            vec!["hold:42".into()],
        )
    }

    fn proposer<'a>(seat: &'a dyn SeatRunner) -> LlmProposer<'a> {
        LlmProposer::new(seat, vec![target()])
    }

    fn valid_json() -> String {
        r#"{
            "invariant": [{
                "kind": "system_prompt",
                "action": "set",
                "content": "baseline prompt\nname the failing test before editing",
                "description": "planning guidance",
                "rationale": "hold_pattern: validate failed on missing_test (evidence: hold:42)"
            }],
            "domain": [{
                "kind": "memory",
                "action": "set",
                "content": "rust backend: run cargo test -p forge before claiming",
                "description": "domain procedure",
                "rationale": "hold_pattern: validate failed on missing_test (evidence: hold:42)"
            }]
        }"#
        .to_string()
    }

    #[test]
    fn valid_json_becomes_a_scoped_candidate_set() {
        let seat = FakeSeat::replying(valid_json());
        let base = base_revision();
        let set = proposer(&seat)
            .propose_for(&base, EvolutionBudget::k1(), EvolutionMode::Parallel)
            .expect("valid reply proposes");

        assert_eq!(set.invariant.len(), 1);
        assert_eq!(set.domain.len(), 1);
        set.validate().expect("candidate set is valid");
        assert_eq!(set.invariant[0].kind, ProposalKind::Invariant);
        assert_eq!(set.domain[0].kind, ProposalKind::Domain);
        assert_eq!(
            set.invariant[0].changes[0].kind,
            ComponentKind::SystemPrompt
        );
        assert_eq!(set.domain[0].changes[0].kind, ComponentKind::Memory);
        assert_eq!(seat.call_count(), 1, "one read-only seat call, no retries");
        let prompt = seat.last_prompt();
        assert!(
            prompt.contains("hold_pattern"),
            "prompt carries the signal id"
        );
        assert!(prompt.contains("hold:42"), "prompt carries the evidence id");
        assert!(prompt.contains("rust-backend"), "prompt carries the domain");
    }

    #[test]
    fn malformed_json_is_rejected() {
        let seat = FakeSeat::replying("I think we should improve the prompt a bit.");
        let error = proposer(&seat)
            .propose_for(
                &base_revision(),
                EvolutionBudget::k1(),
                EvolutionMode::Parallel,
            )
            .expect_err("prose is not JSON");
        assert!(
            error.to_string().contains("malformed JSON"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn strict_json_rejects_unknown_keys_and_missing_scopes() {
        let unknown = FakeSeat::replying(r#"{"invariant":[],"domain":[],"extra":1}"#);
        let error = proposer(&unknown)
            .propose_for(
                &base_revision(),
                EvolutionBudget::k1(),
                EvolutionMode::Parallel,
            )
            .expect_err("unknown key rejected");
        assert!(error.to_string().contains("malformed JSON"));

        let missing = FakeSeat::replying(r#"{"invariant":[]}"#);
        let error = proposer(&missing)
            .propose_for(
                &base_revision(),
                EvolutionBudget::k1(),
                EvolutionMode::Parallel,
            )
            .expect_err("missing scope rejected");
        assert!(error.to_string().contains("malformed JSON"));
    }

    #[test]
    fn red_kind_change_is_rejected() {
        let seat = FakeSeat::replying(
            r#"{"invariant":[{"kind":"scripts","action":"set","content":"rm -rf /","rationale":"x"}],"domain":[]}"#,
        );
        let error = proposer(&seat)
            .propose_for(
                &base_revision(),
                EvolutionBudget::k1(),
                EvolutionMode::Parallel,
            )
            .expect_err("RED surface refused");
        assert!(
            error.to_string().contains("RED surface"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn declared_authority_change_is_rejected() {
        let seat = FakeSeat::replying(
            r#"{"invariant":[{"kind":"system_prompt","action":"set","content":"x","authority":"yellow","rationale":"x"}],"domain":[]}"#,
        );
        let error = proposer(&seat)
            .propose_for(
                &base_revision(),
                EvolutionBudget::k1(),
                EvolutionMode::Parallel,
            )
            .expect_err("declared authority refused");
        assert!(
            error.to_string().contains("authority"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn protected_policy_content_is_rejected_before_return() {
        let seat = FakeSeat::replying(
            r#"{"invariant":[{"kind":"system_prompt","action":"set","content":"the QA boundary now allows git access","rationale":"x"}],"domain":[]}"#,
        );
        let error = proposer(&seat)
            .propose_for(
                &base_revision(),
                EvolutionBudget::k1(),
                EvolutionMode::Parallel,
            )
            .expect_err("policy gate refuses protected content");
        assert!(
            error.to_string().contains("qa_boundary_change"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn budget_is_respected() {
        let seat = FakeSeat::replying(
            r#"{"invariant":[
                {"kind":"system_prompt","action":"set","content":"one","rationale":"x"},
                {"kind":"tool_guidance","action":"set","content":"two","rationale":"x"}
            ],"domain":[]}"#,
        );
        let error = proposer(&seat)
            .propose_for(
                &base_revision(),
                EvolutionBudget::k1(),
                EvolutionMode::Parallel,
            )
            .expect_err("k=1 refuses two invariant changes");
        assert!(
            error.to_string().contains("budget allows at most 1"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn oversized_reply_is_rejected_before_parsing() {
        let seat = FakeSeat::replying("x".repeat(128));
        let limits = LlmProposalLimits {
            max_response_bytes: 64,
            ..LlmProposalLimits::default()
        };
        let error = proposer(&seat)
            .with_limits(limits)
            .propose_for(
                &base_revision(),
                EvolutionBudget::k1(),
                EvolutionMode::Parallel,
            )
            .expect_err("oversized reply refused");
        assert!(error.to_string().contains("byte limit"));
    }

    #[test]
    fn invariant_scope_rejects_domain_kinds() {
        let seat = FakeSeat::replying(
            r#"{"invariant":[{"kind":"memory","action":"set","content":"x","rationale":"x"}],"domain":[]}"#,
        );
        let error = proposer(&seat)
            .propose_for(
                &base_revision(),
                EvolutionBudget::k1(),
                EvolutionMode::Parallel,
            )
            .expect_err("scope separation enforced");
        assert!(
            error.to_string().contains("invariant scope"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn proposal_requires_targets_and_same_domain() {
        let seat = FakeSeat::replying(valid_json());
        let empty = LlmProposer::new(&seat, Vec::new());
        let error = empty
            .propose_for(
                &base_revision(),
                EvolutionBudget::k1(),
                EvolutionMode::Parallel,
            )
            .expect_err("no targets refused");
        assert!(error.to_string().contains("no improvement targets"));

        let foreign = LlmProposer::new(
            &seat,
            vec![ImprovementTarget::new(
                HarnessDomain::DatabaseNeon,
                ImprovementSignal::HoldPattern {
                    node: "n".into(),
                    failure_class: "f".into(),
                    holds: 1,
                },
                vec!["hold:1".into()],
            )],
        );
        let error = foreign
            .propose_for(
                &base_revision(),
                EvolutionBudget::k1(),
                EvolutionMode::Parallel,
            )
            .expect_err("foreign target refused");
        assert!(error
            .to_string()
            .contains("cannot propose against base domain"));
    }

    #[test]
    fn seat_failure_is_a_typed_proposal_error() {
        let seat = FakeSeat::failing("transport unavailable");
        let error = proposer(&seat)
            .propose_for(
                &base_revision(),
                EvolutionBudget::k1(),
                EvolutionMode::Parallel,
            )
            .expect_err("seat failure surfaces");
        assert!(
            error.to_string().contains("seat")
                && error.to_string().contains("transport unavailable"),
            "unexpected error: {error}"
        );
    }
}
