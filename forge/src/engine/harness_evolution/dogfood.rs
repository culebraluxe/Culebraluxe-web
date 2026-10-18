//! The first bounded Harness Evolution dogfood experiment.
//!
//! The GPT spec for the first dogfood run is deliberately narrow:
//!
//! * **3–5 historical Rust-backend stories**, loaded read-only from the control plane;
//! * **only prompt and tool guidance editable** (`SystemPrompt`, `ToolGuidance`) — every other
//!   component (guardrails, memory, subagents, skills, scripts, plugins) is protected;
//! * **k = 1**: one invariant + one domain candidate under the default budget;
//! * everything else (authority, QA/Git, production, release, workflow ownership) is untouched.
//!
//! This module turns that spec into a typed, self-validating **plan**: a base revision, the typed
//! improvement targets, the invariant candidate the proposer generates (domain scope is deferred
//! because it would touch memory/skills), the materialized candidate revision after the policy
//! gate, one matched probe contract per historical story, the planned probe command envelope, and
//! the durable evidence ids a real run is expected to write.
//!
//! The dry-run path is honest about what it is: **no model turn, no database read or write, no
//! active revision moved.** It prints the plan and the requirements for a real run instead of
//! pretending to have executed one. `scripts/harness-dogfood.sh` prints it.

use super::model::{
    Candidate, Component, ComponentKind, EnvironmentClass, EvolutionBudget, EvolutionError,
    EvolutionMode, HarnessDomain, HarnessManifest, HarnessRevision,
};
use super::policy;
use super::probe::{ProbeContract, ProbeError, ProbePins};
use super::probe_replay::{
    HistoricalTask, HistoricalTaskLoader, HistoricalTaskRequest, ReplayError,
};
use super::propose::{propose, ImprovementSignal, ImprovementTarget, ProposalRequest};

/// The dogfood experiment's domain: the Rust backend lane.
pub const DOGFOOD_DOMAIN: HarnessDomain = HarnessDomain::RustBackend;

/// The dogfood plan must cover at least this many historical stories.
pub const DOGFOOD_MIN_STORIES: usize = 3;

/// …and at most this many, so the first run stays small enough to review.
pub const DOGFOOD_MAX_STORIES: usize = 5;

/// The only components this dogfood experiment may edit.
pub const DOGFOOD_EDITABLE_COMPONENTS: [ComponentKind; 2] =
    [ComponentKind::SystemPrompt, ComponentKind::ToolGuidance];

/// Everything else is protected for this experiment: the components remain in the base revision
/// untouched, and the plan renders them as protected.
pub const DOGFOOD_PROTECTED_COMPONENTS: [ComponentKind; 6] = [
    ComponentKind::Guardrails,
    ComponentKind::Memory,
    ComponentKind::Subagents,
    ComponentKind::Skills,
    ComponentKind::Scripts,
    ComponentKind::Plugins,
];

/// Where a story slot's data came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DogfoodStorySource {
    /// Loaded read-only from the control plane; the evidence ids are real.
    Loaded {
        packet_sha: Option<String>,
        evidence_refs: Vec<String>,
    },
    /// Dry run: the slot is named but nothing was loaded. The plan says so rather than inventing
    /// packet evidence.
    DryRunPlaceholder,
}

/// One historical story the dogfood experiment would replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DogfoodProbePlan {
    pub story_id: String,
    pub run_id: Option<String>,
    pub environment_class: EnvironmentClass,
    pub source: DogfoodStorySource,
    /// The reference probe id (the base revision's execution of this story).
    pub reference_probe_id: String,
    pub reference_revision: String,
    pub reference_command: String,
    pub candidates: Vec<DogfoodCandidateProbe>,
    /// Durable ids a real run is expected to write: story/run rows, probe rows, contrast rows.
    pub expected_evidence_refs: Vec<String>,
}

/// One candidate probe for one story.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DogfoodCandidateProbe {
    pub candidate_id: String,
    pub revision: String,
    pub probe_id: String,
    /// The matched-pair reproducibility digest the probe contract pins.
    pub contract_digest: String,
    pub command: String,
}

/// The typed plan for the first dogfood experiment.
#[derive(Debug, Clone, PartialEq)]
pub struct DogfoodPlan {
    pub experiment_id: String,
    pub domain: HarnessDomain,
    pub base_revision: String,
    pub base_manifest_hash: String,
    pub mode: EvolutionMode,
    pub budget: EvolutionBudget,
    pub editable_components: Vec<ComponentKind>,
    pub protected_components: Vec<ComponentKind>,
    /// Rendered improvement targets (signal + evidence refs) that drove the proposal.
    pub targets: Vec<String>,
    pub candidates: Vec<Candidate>,
    /// The candidate revisions materialized through `HarnessRevision::from_changes` and accepted by
    /// the policy gate.
    pub candidate_revisions: Vec<HarnessRevision>,
    /// Domain-scope candidates the proposer generated but this experiment defers: they would touch
    /// memory/skills, outside the dogfood editable surface.
    pub deferred_domain_candidates: Vec<String>,
    pub probes: Vec<DogfoodProbePlan>,
    /// What a real run needs that a dry run cannot provide.
    pub requirements: Vec<String>,
    /// What this plan deliberately does not do or claim.
    pub limitations: Vec<String>,
}

impl DogfoodPlan {
    /// Structural validation. A plan that fails here must not be handed to a real runner.
    pub fn validate(&self) -> Result<(), DogfoodError> {
        if self.domain != DOGFOOD_DOMAIN {
            return Err(DogfoodError::WrongDomain {
                expected: DOGFOOD_DOMAIN,
                actual: self.domain,
            });
        }
        if !self.budget.is_k1() {
            return Err(DogfoodError::BudgetNotK1 {
                k: self.budget.k,
                max_steps: self.budget.max_steps,
            });
        }
        if self.probes.len() < DOGFOOD_MIN_STORIES || self.probes.len() > DOGFOOD_MAX_STORIES {
            return Err(DogfoodError::StoryCount {
                provided: self.probes.len(),
                min: DOGFOOD_MIN_STORIES,
                max: DOGFOOD_MAX_STORIES,
            });
        }
        if self.editable_components != DOGFOOD_EDITABLE_COMPONENTS.to_vec() {
            return Err(DogfoodError::EditableSurface {
                component: ComponentKind::SystemPrompt,
                candidate: "plan".into(),
                message: "editable component set is not the dogfood set".into(),
            });
        }
        if self.protected_components != DOGFOOD_PROTECTED_COMPONENTS.to_vec() {
            return Err(DogfoodError::EditableSurface {
                component: ComponentKind::Guardrails,
                candidate: "plan".into(),
                message: "protected component set is not the dogfood set".into(),
            });
        }
        if self.candidates.is_empty() {
            return Err(DogfoodError::EmptyCandidateSet);
        }
        for candidate in &self.candidates {
            for change in &candidate.changes {
                if !DOGFOOD_EDITABLE_COMPONENTS.contains(&change.kind) {
                    return Err(DogfoodError::EditableSurface {
                        component: change.kind,
                        candidate: candidate.id.clone(),
                        message: "candidate changes a protected component for this experiment"
                            .into(),
                    });
                }
                if change.kind.minimum_authority().is_red() {
                    return Err(DogfoodError::EditableSurface {
                        component: change.kind,
                        candidate: candidate.id.clone(),
                        message: "candidate targets a RED surface".into(),
                    });
                }
            }
        }
        for probe in &self.probes {
            if probe.candidates.is_empty() {
                return Err(DogfoodError::EmptyProbePair {
                    story_id: probe.story_id.clone(),
                });
            }
            if probe.reference_revision != self.base_revision {
                return Err(DogfoodError::ProbePair {
                    story_id: probe.story_id.clone(),
                    message: "reference probe does not run the plan's base revision".into(),
                });
            }
            for candidate in &probe.candidates {
                if candidate.revision == self.base_revision {
                    return Err(DogfoodError::ProbePair {
                        story_id: probe.story_id.clone(),
                        message: "candidate probe runs the base revision".into(),
                    });
                }
            }
            if probe.environment_class.is_production() {
                return Err(DogfoodError::ProductionRefused {
                    environment_class: probe.environment_class,
                });
            }
        }
        Ok(())
    }

    /// Render the plan as the operator-facing dry-run document.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("HARNESS EVOLUTION — FIRST DOGFOOD EXPERIMENT PLAN (DRY RUN)\n");
        out.push_str("no model turn, no database write, no active revision moved\n");
        out.push_str("======================================================================\n");
        out.push_str(&format!("experiment: {}\n", self.experiment_id));
        out.push_str(&format!("domain: {}\n", self.domain));
        out.push_str(&format!(
            "mode/budget: {} k={} (max_steps={}, max_probes={})\n",
            self.mode.id(),
            self.budget.k,
            self.budget.max_steps,
            self.budget.max_probes
        ));
        out.push_str(&format!(
            "base revision: {} (manifest {})\n",
            self.base_revision, self.base_manifest_hash
        ));
        out.push_str(&format!(
            "editable components: {}\n",
            join_kinds(&self.editable_components)
        ));
        out.push_str(&format!(
            "protected components: {}\n",
            join_kinds(&self.protected_components)
        ));

        out.push_str("\nTARGETS\n");
        for target in &self.targets {
            out.push_str(&format!("  - {target}\n"));
        }

        out.push_str("\nCANDIDATES\n");
        for candidate in &self.candidates {
            out.push_str(&format!("  - {} [{}]\n", candidate.id, candidate.kind.id()));
            for change in &candidate.changes {
                out.push_str(&format!(
                    "      {} {}: {}\n",
                    change.action.id(),
                    change.kind,
                    change.rationale
                ));
            }
        }
        if !self.deferred_domain_candidates.is_empty() {
            out.push_str(&format!(
                "  domain scope deferred ({}): {}\n",
                self.deferred_domain_candidates.len(),
                self.deferred_domain_candidates.join(", ")
            ));
            out.push_str(
                "      reason: domain candidates edit memory/skills, outside the dogfood editable surface\n",
            );
        }
        out.push_str("  materialized revisions (policy-checked):\n");
        for revision in &self.candidate_revisions {
            out.push_str(&format!(
                "      {} parent={:?} manifest={}\n",
                revision.id, revision.parent_revision_id, revision.manifest_hash
            ));
        }

        out.push_str("\nPROBES (matched pairs, one contract per story)\n");
        for probe in &self.probes {
            let run = probe.run_id.as_deref().unwrap_or("<latest-run>");
            out.push_str(&format!(
                "  story {} (run {run}, env {})\n",
                probe.story_id,
                probe.environment_class.id()
            ));
            match &probe.source {
                DogfoodStorySource::Loaded {
                    packet_sha,
                    evidence_refs,
                } => {
                    out.push_str(&format!(
                        "      source: loaded read-only (packet_sha {}, evidence {})\n",
                        packet_sha.as_deref().unwrap_or("none"),
                        evidence_refs.join(", ")
                    ));
                }
                DogfoodStorySource::DryRunPlaceholder => {
                    out.push_str(
                        "      source: dry-run placeholder (not loaded; no database read in dry-run)\n",
                    );
                }
            }
            out.push_str(&format!(
                "      reference probe {} -> revision {} (contract {})\n",
                probe.reference_probe_id, probe.reference_revision, probe.reference_command
            ));
            for candidate in &probe.candidates {
                out.push_str(&format!(
                    "      candidate probe {} -> revision {} (contract {})\n",
                    candidate.probe_id, candidate.revision, candidate.command
                ));
                out.push_str(&format!(
                    "          contract digest {}\n",
                    candidate.contract_digest
                ));
            }
            out.push_str(&format!(
                "      expected evidence: {}\n",
                probe.expected_evidence_refs.join(", ")
            ));
        }

        out.push_str("\nREQUIREMENTS BEFORE A REAL RUN\n");
        for requirement in &self.requirements {
            out.push_str(&format!("  - {requirement}\n"));
        }

        out.push_str("\nLIMITATIONS / REFUSALS\n");
        for limitation in &self.limitations {
            out.push_str(&format!("  - {limitation}\n"));
        }
        out.push_str(
            "  - production execution is refused by construction (development environment only)\n",
        );
        out.push_str("\nEND OF DRY-RUN PLAN\n");
        out
    }
}

fn join_kinds(kinds: &[ComponentKind]) -> String {
    kinds
        .iter()
        .map(|kind| kind.id())
        .collect::<Vec<_>>()
        .join(", ")
}

/// A dogfood request: the pinned base revision, the probe pins, the typed targets and the 3–5
/// historical story slots.
#[derive(Debug, Clone, PartialEq)]
pub struct DogfoodRequest {
    pub experiment_id: String,
    pub base: HarnessRevision,
    pub pins: ProbePins,
    pub targets: Vec<ImprovementTarget>,
    pub stories: Vec<HistoricalTaskRequest>,
}

impl DogfoodRequest {
    pub fn new(
        experiment_id: impl Into<String>,
        base: &HarnessRevision,
        pins: ProbePins,
        targets: Vec<ImprovementTarget>,
        stories: Vec<HistoricalTaskRequest>,
    ) -> Self {
        Self {
            experiment_id: experiment_id.into(),
            base: base.clone(),
            pins,
            targets,
            stories,
        }
    }
}

/// Why a dogfood plan was refused. Every variant is a refusal, not a degraded plan.
#[derive(Debug, Clone, PartialEq)]
pub enum DogfoodError {
    ProductionRefused {
        environment_class: EnvironmentClass,
    },
    StoryCount {
        provided: usize,
        min: usize,
        max: usize,
    },
    DuplicateStory {
        story_id: String,
    },
    InvalidStory {
        story_id: String,
    },
    WrongDomain {
        expected: HarnessDomain,
        actual: HarnessDomain,
    },
    EditableSurface {
        component: ComponentKind,
        candidate: String,
        message: String,
    },
    BudgetNotK1 {
        k: u32,
        max_steps: u32,
    },
    EmptyCandidateSet,
    EmptyProbePair {
        story_id: String,
    },
    ProbePair {
        story_id: String,
        message: String,
    },
    PolicyViolation {
        violations: Vec<String>,
    },
    Evolution(EvolutionError),
    Probe(ProbeError),
    Loader(ReplayError),
}

impl DogfoodError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ProductionRefused { .. } => "dogfood_production_refused",
            Self::StoryCount { .. } => "dogfood_story_count",
            Self::DuplicateStory { .. } => "dogfood_duplicate_story",
            Self::InvalidStory { .. } => "dogfood_invalid_story",
            Self::WrongDomain { .. } => "dogfood_wrong_domain",
            Self::EditableSurface { .. } => "dogfood_editable_surface",
            Self::BudgetNotK1 { .. } => "dogfood_budget_not_k1",
            Self::EmptyCandidateSet => "dogfood_empty_candidate_set",
            Self::EmptyProbePair { .. } => "dogfood_empty_probe_pair",
            Self::ProbePair { .. } => "dogfood_probe_pair",
            Self::PolicyViolation { .. } => "dogfood_policy_violation",
            Self::Evolution(error) => error.code(),
            Self::Probe(_) => "dogfood_probe_refused",
            Self::Loader(_) => "dogfood_loader_refused",
        }
    }
}

impl std::fmt::Display for DogfoodError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProductionRefused { environment_class } => write!(
                f,
                "dogfood refuses the {} environment; it only runs in a non-production class",
                environment_class.id()
            ),
            Self::StoryCount { provided, min, max } => write!(
                f,
                "dogfood needs between {min} and {max} historical stories; {provided} provided"
            ),
            Self::DuplicateStory { story_id } => {
                write!(f, "dogfood story {story_id} is listed more than once")
            }
            Self::InvalidStory { story_id } => {
                write!(f, "dogfood story {story_id} is not a usable story id")
            }
            Self::WrongDomain { expected, actual } => {
                write!(f, "dogfood domain {actual} does not match the required {expected}")
            }
            Self::EditableSurface {
                component,
                candidate,
                message,
            } => write!(
                f,
                "candidate {candidate} touches {component}, outside the dogfood editable surface: {message}"
            ),
            Self::BudgetNotK1 { k, max_steps } => write!(
                f,
                "dogfood is a k=1 experiment; budget is k={k} max_steps={max_steps}"
            ),
            Self::EmptyCandidateSet => write!(f, "dogfood produced no candidate to probe"),
            Self::EmptyProbePair { story_id } => {
                write!(f, "dogfood story {story_id} has no candidate probe")
            }
            Self::ProbePair { story_id, message } => {
                write!(f, "dogfood probe pair for {story_id} is invalid: {message}")
            }
            Self::PolicyViolation { violations } => {
                write!(f, "policy refused a dogfood candidate: {}", violations.join("; "))
            }
            Self::Evolution(error) => write!(f, "{error}"),
            Self::Probe(error) => write!(f, "{error}"),
            Self::Loader(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for DogfoodError {}

impl From<EvolutionError> for DogfoodError {
    fn from(error: EvolutionError) -> Self {
        Self::Evolution(error)
    }
}

impl From<ProbeError> for DogfoodError {
    fn from(error: ProbeError) -> Self {
        Self::Probe(error)
    }
}

impl From<ReplayError> for DogfoodError {
    fn from(error: ReplayError) -> Self {
        Self::Loader(error)
    }
}

/// Build the dogfood plan. This performs **no** database read and writes nothing: the story slots
/// are placeholders until [`build_plan_loaded`] supplies real historical tasks.
pub fn build_plan(request: &DogfoodRequest) -> Result<DogfoodPlan, DogfoodError> {
    build_plan_with_tasks(request, &[])
}

/// Build the plan with historical tasks loaded read-only through `loader`. A production execution
/// class is refused **before** the loader is called.
pub fn build_plan_loaded(
    request: &DogfoodRequest,
    loader: &dyn HistoricalTaskLoader,
) -> Result<DogfoodPlan, DogfoodError> {
    refuse_production(&request.pins)?;
    check_story_slots(&request.stories)?;
    let mut tasks = Vec::with_capacity(request.stories.len());
    for story in &request.stories {
        tasks.push(loader.load(story)?);
    }
    build_plan_with_tasks(request, &tasks)
}

fn build_plan_with_tasks(
    request: &DogfoodRequest,
    loaded_tasks: &[HistoricalTask],
) -> Result<DogfoodPlan, DogfoodError> {
    if request.experiment_id.trim().is_empty() {
        return Err(DogfoodError::Evolution(EvolutionError::InvalidCandidate(
            "dogfood experiment has no id".into(),
        )));
    }
    refuse_production(&request.pins)?;
    check_story_slots(&request.stories)?;
    request.base.verify_integrity()?;
    if request.base.domain != DOGFOOD_DOMAIN {
        return Err(DogfoodError::WrongDomain {
            expected: DOGFOOD_DOMAIN,
            actual: request.base.domain,
        });
    }

    // 1. Propose under the v1 budget: exactly one invariant candidate (k = 1).
    let proposal_request = ProposalRequest::k1(request.targets.clone());
    let batch = propose(&request.base, &proposal_request)?;
    if batch.invariant.is_empty() {
        return Err(DogfoodError::EmptyCandidateSet);
    }
    let deferred_domain_candidates: Vec<String> = batch
        .domain_scope
        .iter()
        .map(|proposed| proposed.candidate.id.clone())
        .collect();

    // 2. Prompt/tool-guidance only. A candidate that strays outside the dogfood surface is refused,
    //    not silently trimmed.
    let candidates: Vec<Candidate> = batch
        .invariant
        .iter()
        .map(|proposed| proposed.candidate.clone())
        .collect();
    for candidate in &candidates {
        for change in &candidate.changes {
            if !DOGFOOD_EDITABLE_COMPONENTS.contains(&change.kind) {
                return Err(DogfoodError::EditableSurface {
                    component: change.kind,
                    candidate: candidate.id.clone(),
                    message: "the dogfood experiment may only edit prompt and tool guidance".into(),
                });
            }
        }
    }

    // 3. Materialize each candidate through the only sanctioned constructor and run the policy gate
    //    before anything is probed.
    let mut candidate_revisions = Vec::with_capacity(candidates.len());
    for candidate in &candidates {
        let revision = HarnessRevision::from_changes(
            &request.base,
            format!("{}-{}", request.experiment_id, candidate.id),
            &candidate.changes,
            "harness-evolution-dogfood",
        )?;
        if let Err(violations) =
            policy::validate_candidate(&request.base, &revision, &candidate.changes)
        {
            return Err(DogfoodError::PolicyViolation {
                violations: violations.iter().map(ToString::to_string).collect(),
            });
        }
        candidate_revisions.push(revision);
    }

    // 4. One matched probe contract per (story, candidate), with the story id pinned as the task.
    let mut probes = Vec::with_capacity(request.stories.len());
    for story in &request.stories {
        let mut story_pins = request.pins.clone();
        story_pins.task = story.story_id.clone();
        let environment_class = story_pins.environment_class;
        let reference_probe_id = format!("{}-{}-reference", request.experiment_id, story.story_id);
        let mut candidate_probes = Vec::with_capacity(candidate_revisions.len());
        for (candidate, revision) in candidates.iter().zip(candidate_revisions.iter()) {
            let contract = ProbeContract::new(
                request.experiment_id.clone(),
                &request.base,
                revision,
                story_pins.clone(),
                EvolutionMode::Parallel,
                EvolutionBudget::k1(),
                0,
            )?;
            contract.validate()?;
            let probe_id = format!(
                "{}-{}-{}-candidate",
                request.experiment_id, story.story_id, candidate.id
            );
            candidate_probes.push(DogfoodCandidateProbe {
                candidate_id: candidate.id.clone(),
                revision: revision.id.clone(),
                probe_id: probe_id.clone(),
                contract_digest: contract.digest(),
                command: probe_command(
                    &request.experiment_id,
                    story,
                    "candidate",
                    &revision.id,
                    environment_class,
                ),
            });
        }
        let expected_evidence_refs = expected_evidence(
            &request.experiment_id,
            story,
            &reference_probe_id,
            &candidate_probes,
        );
        let source = match loaded_tasks.iter().find(|task| {
            task.story_id == story.story_id
                && story
                    .run_id
                    .as_deref()
                    .is_none_or(|run_id| task.run_id == run_id)
        }) {
            Some(task) => DogfoodStorySource::Loaded {
                packet_sha: task.packet_sha.clone(),
                evidence_refs: task.evidence_refs.clone(),
            },
            None => DogfoodStorySource::DryRunPlaceholder,
        };
        probes.push(DogfoodProbePlan {
            story_id: story.story_id.clone(),
            run_id: story.run_id.clone(),
            environment_class,
            source,
            reference_command: probe_command(
                &request.experiment_id,
                story,
                "reference",
                &request.base.id,
                environment_class,
            ),
            reference_probe_id,
            reference_revision: request.base.id.clone(),
            candidates: candidate_probes,
            expected_evidence_refs,
        });
    }

    let plan = DogfoodPlan {
        experiment_id: request.experiment_id.clone(),
        domain: request.base.domain,
        base_revision: request.base.id.clone(),
        base_manifest_hash: request.base.manifest_hash.clone(),
        mode: EvolutionMode::Parallel,
        budget: EvolutionBudget::k1(),
        editable_components: DOGFOOD_EDITABLE_COMPONENTS.to_vec(),
        protected_components: DOGFOOD_PROTECTED_COMPONENTS.to_vec(),
        targets: request
            .targets
            .iter()
            .map(ImprovementTarget::summary)
            .collect(),
        candidates,
        candidate_revisions,
        deferred_domain_candidates,
        probes,
        requirements: vec![
            "a real ReplaySandbox that runs a model turn (this build has sandbox_command::CommandSandbox as a stand-in and UnavailableSandbox as a refusal)".into(),
            "a live advisory model to drive proposer_llm for a real proposal (this plan uses typed targets; the dry run calls no model)".into(),
            "DATABASE_URL_DEV plus migration 294 applied, and a DB-backed catalog/probe writer for experiment/probe/contrast rows (the dry run writes nothing)".into(),
            "human review before adoption: automatic adoption stays OFF (effective-GREEN policy only)".into(),
        ],
        limitations: vec![
            "DRY RUN: no model turn, no database read/write, no active revision moved".into(),
            "candidate set is the invariant scope only; the domain scope is deferred because it edits memory/skills".into(),
            "probe command envelopes are the planned runner requests; `forge harness-evolution probe` is not a CLI entrypoint yet".into(),
            "expected evidence ids are what a real run will write; this plan does not claim they exist".into(),
        ],
    };
    plan.validate()?;
    Ok(plan)
}

fn refuse_production(pins: &ProbePins) -> Result<(), DogfoodError> {
    if pins.environment_class.is_production() || !pins.environment_class.allows_probe() {
        return Err(DogfoodError::ProductionRefused {
            environment_class: pins.environment_class,
        });
    }
    Ok(())
}

fn check_story_slots(stories: &[HistoricalTaskRequest]) -> Result<(), DogfoodError> {
    if stories.len() < DOGFOOD_MIN_STORIES || stories.len() > DOGFOOD_MAX_STORIES {
        return Err(DogfoodError::StoryCount {
            provided: stories.len(),
            min: DOGFOOD_MIN_STORIES,
            max: DOGFOOD_MAX_STORIES,
        });
    }
    let mut seen: Vec<&str> = Vec::new();
    for story in stories {
        if story.story_id.trim().is_empty() {
            return Err(DogfoodError::InvalidStory {
                story_id: "<empty>".into(),
            });
        }
        if seen.contains(&story.story_id.as_str()) {
            return Err(DogfoodError::DuplicateStory {
                story_id: story.story_id.clone(),
            });
        }
        seen.push(&story.story_id);
    }
    Ok(())
}

/// The expected durable ids a real run writes for one story. These are *expected*, not claimed to
/// exist: the probe/contrast rows are written by the real executor wiring.
fn expected_evidence(
    experiment_id: &str,
    story: &HistoricalTaskRequest,
    reference_probe_id: &str,
    candidate_probes: &[DogfoodCandidateProbe],
) -> Vec<String> {
    let mut refs = vec![match story.run_id.as_deref() {
        Some(run_id) => format!("storyboard_story_run:{run_id}"),
        None => format!("storyboard_story:{}", story.story_id),
    }];
    refs.push(format!("harness_probe:{reference_probe_id}"));
    for candidate in candidate_probes {
        refs.push(format!("harness_probe:{}", candidate.probe_id));
    }
    refs.push(format!(
        "harness_contrast:{experiment_id}-{}",
        story.story_id
    ));
    refs.push(format!("harness_validation:{experiment_id}"));
    refs
}

/// The planned probe command envelope. It is printed, never executed; the CLI entrypoint does not
/// exist yet, and the plan says so.
fn probe_command(
    experiment_id: &str,
    story: &HistoricalTaskRequest,
    side: &str,
    revision: &str,
    environment_class: EnvironmentClass,
) -> String {
    format!(
        "harness-evolution probe --experiment {experiment_id} --story {} --run {} --side {side} --revision {revision} --env {}",
        story.story_id,
        story.run_id.as_deref().unwrap_or("<latest-run>"),
        environment_class.id()
    )
}

/// The deterministic dry-run fixture: a Rust-backend base revision with prompt and tool guidance,
/// typed Phase 3-style targets and three historical story slots. No database, no model.
pub fn dry_run_fixture() -> DogfoodRequest {
    let manifest = HarnessManifest::from_components([
        Component::green(
            ComponentKind::SystemPrompt,
            "You are the Forge Rust backend lane. State the acceptance checks before editing.",
            "baseline prompt",
        ),
        Component::yellow(
            ComponentKind::ToolGuidance,
            "Use the focused test command before the full suite.",
            "baseline tool guidance",
        ),
        Component::yellow(
            ComponentKind::Guardrails,
            "Do not edit generated files; report the blocker instead.",
            "baseline guardrails",
        ),
        Component::green(
            ComponentKind::Memory,
            "Rust backend notes: the forge crate owns the engine.",
            "baseline memory",
        ),
    ]);
    let base = HarnessRevision::draft("rust-backend-baseline", DOGFOOD_DOMAIN, manifest, "owner");
    let pins = ProbePins::development("dogfood-history");
    let targets = vec![
        ImprovementTarget::new(
            DOGFOOD_DOMAIN,
            ImprovementSignal::HoldPattern {
                node: "smith-execute".into(),
                failure_class: "compile_error".into(),
                holds: 3,
            },
            vec!["forge_hold_record:hold-1".into()],
        ),
        ImprovementTarget::new(
            DOGFOOD_DOMAIN,
            ImprovementSignal::RepeatedCorrection {
                lane: "smith".into(),
                correction: "run the focused test before handoff".into(),
            },
            vec!["storyboard_story_run:run-a".into()],
        ),
        ImprovementTarget::new(
            DOGFOOD_DOMAIN,
            ImprovementSignal::ScorecardWeakness {
                role: "smith".into(),
                agent: "opencode-harness".into(),
                weakness: "rework rate 60%".into(),
            },
            vec!["scorecard:bucket:smith/opencode-harness".into()],
        ),
    ];
    let stories = vec![
        HistoricalTaskRequest::new(
            "smith-100",
            Some("11111111-1111-1111-1111-111111111111".into()),
        ),
        HistoricalTaskRequest::new(
            "smith-101",
            Some("22222222-2222-2222-2222-222222222222".into()),
        ),
        HistoricalTaskRequest::new(
            "smith-102",
            Some("33333333-3333-3333-3333-333333333333".into()),
        ),
    ];
    DogfoodRequest::new("dogfood-rust-backend-1", &base, pins, targets, stories)
}

/// The dry-run entry point used by `scripts/harness-dogfood.sh`: build the fixture plan and render
/// it. No model, no database, no active revision.
pub fn dry_run_for_rust_backend() -> Result<String, DogfoodError> {
    Ok(build_plan(&dry_run_fixture())?.render())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::harness_evolution::probe_replay::{
        HistoricalTask, HistoricalTaskLoader, ReplayError,
    };
    use std::cell::Cell;

    struct StubLoader {
        tasks: Vec<HistoricalTask>,
        error: Option<ReplayError>,
        loads: Cell<usize>,
    }

    impl StubLoader {
        fn new(tasks: Vec<HistoricalTask>) -> Self {
            Self {
                tasks,
                error: None,
                loads: Cell::new(0),
            }
        }

        fn failing(error: ReplayError) -> Self {
            Self {
                tasks: Vec::new(),
                error: Some(error),
                loads: Cell::new(0),
            }
        }
    }

    impl HistoricalTaskLoader for StubLoader {
        fn load(&self, request: &HistoricalTaskRequest) -> Result<HistoricalTask, ReplayError> {
            self.loads.set(self.loads.get() + 1);
            if let Some(error) = &self.error {
                return Err(error.clone());
            }
            self.tasks
                .iter()
                .find(|task| task.story_id == request.story_id)
                .cloned()
                .ok_or_else(|| ReplayError::TaskNotFound {
                    story_id: request.story_id.clone(),
                    run_id: request.run_id.clone(),
                })
        }
    }

    fn fixture_tasks() -> Vec<HistoricalTask> {
        vec![
            HistoricalTask::new(
                "smith-100",
                "11111111-1111-1111-1111-111111111111",
                "Story 100",
                "GOAL:\nfix 100",
            )
            .with_packet_sha(Some("sha-100".into())),
            HistoricalTask::new(
                "smith-101",
                "22222222-2222-2222-2222-222222222222",
                "Story 101",
                "GOAL:\nfix 101",
            ),
            HistoricalTask::new(
                "smith-102",
                "33333333-3333-3333-3333-333333333333",
                "Story 102",
                "GOAL:\nfix 102",
            ),
        ]
    }

    #[test]
    fn the_dry_run_plan_is_bounded_to_the_dogfood_spec() {
        let plan = build_plan(&dry_run_fixture()).expect("plan");
        assert_eq!(plan.domain, HarnessDomain::RustBackend);
        assert!(plan.budget.is_k1());
        assert_eq!(plan.mode, EvolutionMode::Parallel);
        assert_eq!(plan.probes.len(), 3);
        assert_eq!(
            plan.editable_components,
            vec![ComponentKind::SystemPrompt, ComponentKind::ToolGuidance]
        );
        assert_eq!(plan.protected_components.len(), 6);
        assert!(!plan.candidates.is_empty());
        plan.validate().expect("plan validates");
    }

    #[test]
    fn every_candidate_change_is_prompt_or_tool_guidance_only() {
        let plan = build_plan(&dry_run_fixture()).expect("plan");
        for candidate in &plan.candidates {
            for change in &candidate.changes {
                assert!(
                    DOGFOOD_EDITABLE_COMPONENTS.contains(&change.kind),
                    "{} edited {}",
                    candidate.id,
                    change.kind
                );
                assert!(!change.kind.minimum_authority().is_red());
            }
        }
        // The proposal existed through the real proposer and passed the real policy gate. Each
        // materialized revision is a content-addressed child of the plan's base, built by the only
        // sanctioned constructor (`HarnessRevision::from_changes`).
        assert_eq!(plan.candidate_revisions.len(), plan.candidates.len());
        for (candidate, revision) in plan.candidates.iter().zip(plan.candidate_revisions.iter()) {
            assert_eq!(
                revision.parent_revision_id.as_deref(),
                Some(plan.base_revision.as_str())
            );
            revision
                .verify_integrity()
                .expect("content-addressed revision");
            assert_eq!(revision.manifest_hash, revision.manifest.manifest_hash());
            assert!(!candidate.changes.is_empty());
        }
    }

    #[test]
    fn the_domain_scope_is_deferred_with_a_reason() {
        let plan = build_plan(&dry_run_fixture()).expect("plan");
        assert!(!plan.deferred_domain_candidates.is_empty());
        let rendered = plan.render();
        assert!(rendered.contains("domain scope deferred"));
        assert!(rendered.contains("memory/skills"));
    }

    #[test]
    fn rendering_names_the_base_targets_probes_and_expected_evidence() {
        let plan = build_plan(&dry_run_fixture()).expect("plan");
        let rendered = plan.render();
        assert!(rendered.contains("DRY RUN"));
        assert!(rendered.contains("no model turn, no database write, no active revision moved"));
        assert!(rendered.contains(&plan.base_revision));
        assert!(rendered.contains(&plan.base_manifest_hash));
        for probe in &plan.probes {
            assert!(rendered.contains(&probe.story_id));
            assert!(rendered.contains(&probe.reference_probe_id));
            for candidate in &probe.candidates {
                assert!(rendered.contains(&candidate.probe_id));
                assert!(rendered.contains("contract digest"));
            }
        }
        assert!(rendered.contains("REQUIREMENTS BEFORE A REAL RUN"));
        assert!(rendered.contains("LIMITATIONS / REFUSALS"));
        assert!(rendered.contains("production execution is refused by construction"));
    }

    #[test]
    fn probe_commands_are_development_only_and_never_production() {
        let plan = build_plan(&dry_run_fixture()).expect("plan");
        for probe in &plan.probes {
            assert_eq!(probe.environment_class, EnvironmentClass::Development);
            for command in std::iter::once(&probe.reference_command)
                .chain(probe.candidates.iter().map(|candidate| &candidate.command))
            {
                assert!(command.contains("--env development"));
                assert!(!command.contains("production"));
            }
        }
    }

    #[test]
    fn production_pins_are_refused_before_the_loader_runs() {
        let mut request = dry_run_fixture();
        request.pins.environment_class = EnvironmentClass::Production;
        let loader = StubLoader::new(fixture_tasks());
        let error = build_plan_loaded(&request, &loader).unwrap_err();
        assert_eq!(error.code(), "dogfood_production_refused");
        assert_eq!(
            loader.loads.get(),
            0,
            "no read happens for a production plan"
        );
    }

    #[test]
    fn a_real_load_attaches_durable_evidence_to_the_plan() {
        let request = dry_run_fixture();
        let loader = StubLoader::new(fixture_tasks());
        let plan = build_plan_loaded(&request, &loader).expect("loaded plan");
        assert_eq!(loader.loads.get(), 3);
        for probe in &plan.probes {
            match &probe.source {
                DogfoodStorySource::Loaded { evidence_refs, .. } => {
                    assert!(!evidence_refs.is_empty());
                    assert!(evidence_refs
                        .iter()
                        .any(|reference| reference.starts_with("storyboard_story_run:")));
                }
                DogfoodStorySource::DryRunPlaceholder => {
                    panic!("story {} should have been loaded", probe.story_id)
                }
            }
        }
        let rendered = plan.render();
        assert!(rendered.contains("source: loaded read-only"));
    }

    #[test]
    fn a_loader_failure_is_a_typed_refusal() {
        let request = dry_run_fixture();
        let loader = StubLoader::failing(ReplayError::TaskNotFound {
            story_id: "smith-100".into(),
            run_id: None,
        });
        let error = build_plan_loaded(&request, &loader).unwrap_err();
        assert_eq!(error.code(), "dogfood_loader_refused");
    }

    #[test]
    fn story_count_is_bounded() {
        let mut too_few = dry_run_fixture();
        too_few.stories.truncate(2);
        assert_eq!(
            build_plan(&too_few).unwrap_err().code(),
            "dogfood_story_count"
        );

        let mut too_many = dry_run_fixture();
        for index in 3..6 {
            too_many
                .stories
                .push(HistoricalTaskRequest::new(format!("smith-{index}"), None));
        }
        assert_eq!(
            build_plan(&too_many).unwrap_err().code(),
            "dogfood_story_count"
        );
    }

    #[test]
    fn duplicate_stories_are_refused() {
        let mut request = dry_run_fixture();
        request.stories[2].story_id = request.stories[0].story_id.clone();
        assert_eq!(
            build_plan(&request).unwrap_err().code(),
            "dogfood_duplicate_story"
        );
    }

    #[test]
    fn a_non_rust_backend_base_is_refused() {
        let mut request = dry_run_fixture();
        request.base.domain = HarnessDomain::YewMvi;
        assert_eq!(
            build_plan(&request).unwrap_err().code(),
            "dogfood_wrong_domain"
        );
    }

    #[test]
    fn a_target_that_would_edit_a_protected_component_is_refused() {
        // RoutingWeakness is invariant delegation guidance, which lands on `Subagents` — outside the
        // dogfood editable surface.
        let mut request = dry_run_fixture();
        request.targets = vec![ImprovementTarget::new(
            DOGFOOD_DOMAIN,
            ImprovementSignal::RoutingWeakness {
                lane: "lead".into(),
                detail: "handoff".into(),
            },
            vec!["storyboard_story_run:run-routing".into()],
        )];
        let error = build_plan(&request).unwrap_err();
        assert_eq!(error.code(), "dogfood_editable_surface");
        assert!(error.to_string().contains("subagents"));
    }

    #[test]
    fn the_plan_is_deterministic() {
        let first = build_plan(&dry_run_fixture()).expect("first");
        let second = build_plan(&dry_run_fixture()).expect("second");
        assert_eq!(first.render(), second.render());
        assert_eq!(first.candidates, second.candidates);
        assert_eq!(first.probes, second.probes);
        // Revisions are content-addressed; only their creation timestamp may differ.
        let projection = |plan: &DogfoodPlan| -> Vec<(String, Option<String>, String, String)> {
            plan.candidate_revisions
                .iter()
                .map(|revision| {
                    (
                        revision.id.clone(),
                        revision.parent_revision_id.clone(),
                        revision.manifest_hash.clone(),
                        revision.status.id().to_string(),
                    )
                })
                .collect()
        };
        assert_eq!(projection(&first), projection(&second));
    }

    #[test]
    fn the_script_entrypoint_renders_the_plan_without_side_effects() {
        let text = dry_run_for_rust_backend().expect("dry run");
        assert!(text.starts_with("HARNESS EVOLUTION — FIRST DOGFOOD EXPERIMENT PLAN"));
        assert!(text.contains("dogfood-rust-backend-1"));
        assert!(text.contains("END OF DRY-RUN PLAN"));
    }

    /// The test `scripts/harness-dogfood.sh` runs with `--nocapture` to print the plan.
    #[test]
    fn print_dry_run_plan() {
        let text = dry_run_for_rust_backend().expect("dry run");
        println!("{text}");
        assert!(text.contains("DRY RUN"));
    }
}
