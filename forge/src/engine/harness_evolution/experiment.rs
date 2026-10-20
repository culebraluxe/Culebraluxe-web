//! The experiment state machine and its orchestration seam.
//!
//! One [`HarnessExperiment`] is one Propose → Probe → Contrast → Compose → Forge Validate pass
//! against a pinned base revision. The state machine reuses the model's closed
//! [`RevisionStatus`] lifecycle table, so an experiment cannot skip Probe or Self-adopt.
//!
//! The safety property this module exists for: **an experiment can never move the active
//! harness pointer.** [`HarnessExperiment::run`] takes the catalog mutably but only ever reads it
//! and inserts lineage revisions; the only activation path is
//! [`super::adoption`]. A failure therefore leaves the active revision byte-identical, and the
//! state machine records why the experiment stopped.
//!
//! Propose/Contrast/Compose/Validate are injected as traits. v1 ships no optimizer; the seams
//! let the Smith modules (or the deterministic test doubles) drive one experiment without
//! teaching this file their internals. k = 1 stays the default; [`run_recursion`] implements the
//! k > 1 modes (parallel: one shared evidence phase and k compositions from it; sequential: k
//! chained single passes), bounded by [`RECURSION_K_CEILING`] / [`RECURSION_STEP_CEILING`].
//!
//! A composition travels as Compose returned it: [`ComposedSuccessor`] carries the successor
//! manifest **and** Compose's exact change list, so Forge validation receives the composer's real
//! rationales instead of a diff re-derived from the manifest pair.

use std::collections::BTreeMap;
use std::fmt;

use chrono::{DateTime, Utc};

use super::catalog::{CatalogError, HarnessCatalog};
use super::model::{
    BehaviorDimension, Candidate, CandidateSet, ComponentChange, ComponentKind,
    CompositionDecision, ContrastReport, EvolutionBudget, EvolutionError, EvolutionMode,
    HarnessDomain, HarnessManifest, HarnessRevision, ProposalKind, RevisionStatus,
    ValidationEvidence,
};
use super::policy;
use super::probe::{
    verify_reading_matches_request, ProbeContract, ProbeError, ProbeExecutor, ProbePins,
    ProbeReading,
};

/// The lifecycle an experiment walks. Reusing the model's closed revision lifecycle keeps one
/// transition table for revisions and experiments alike.
pub type ExperimentState = RevisionStatus;

/// The stage an experiment stopped in. Recorded on failure so the durable row says where.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperimentStage {
    Propose,
    Policy,
    Probe,
    Contrast,
    Compose,
    Validate,
}

impl ExperimentStage {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Propose => "propose",
            Self::Policy => "policy",
            Self::Probe => "probe",
            Self::Contrast => "contrast",
            Self::Compose => "compose",
            Self::Validate => "validate",
        }
    }
}

impl fmt::Display for ExperimentStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// Why an experiment stopped. A failure is a refusal, not a degraded success: the active harness
/// is untouched and the experiment cannot be advanced past `Failed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperimentFailure {
    pub stage: ExperimentStage,
    pub code: String,
    pub message: String,
}

impl ExperimentFailure {
    pub fn new(
        stage: ExperimentStage,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            stage,
            code: code.into(),
            message: message.into(),
        }
    }
}

/// A composition as the composer returned it: the component decisions, the successor manifest and
/// the **exact** change list that produced that manifest.
///
/// The change list travels with the composition instead of being re-derived downstream. Compose
/// already carries it (`compose::ComposedHarness::changes`), and re-deriving it from the manifest
/// pair would synthesize generic rationales — which would make the validator's "exact composed
/// changes" claim false.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedSuccessor {
    pub decision: CompositionDecision,
    pub manifest: HarnessManifest,
    /// Compose's canonical change list. Invariant: `base.manifest.apply_changes(changes)` equals
    /// [`ComposedSuccessor::manifest`]; [`HarnessExperiment::record_composition`] proves it.
    pub changes: Vec<ComponentChange>,
}

/// The adapter from Compose's own result to the experiment seam. The change list is carried
/// verbatim; nothing is re-derived.
impl From<super::compose::ComposedHarness> for ComposedSuccessor {
    fn from(composed: super::compose::ComposedHarness) -> Self {
        Self {
            decision: composed.decision,
            manifest: composed.successor.manifest,
            changes: composed.changes,
        }
    }
}

impl From<&super::compose::ComposedHarness> for ComposedSuccessor {
    fn from(composed: &super::compose::ComposedHarness) -> Self {
        Self {
            decision: composed.decision.clone(),
            manifest: composed.successor.manifest.clone(),
            changes: composed.changes.clone(),
        }
    }
}

/// Injected proposal stage (Smith `propose`).
pub trait Proposer {
    fn propose(&self, context: &ProposalContext<'_>) -> Result<CandidateSet, EvolutionError>;
}

/// Injected behavioral-contrast stage (Smith `contrast`).
pub trait Contrastor {
    fn contrast(&self, context: &ContrastContext<'_>) -> Result<ContrastReport, EvolutionError>;
}

/// Injected composition stage (Smith `compose`).
pub trait Composer {
    fn compose(
        &self,
        context: &CompositionContext<'_>,
    ) -> Result<ComposedSuccessor, EvolutionError>;
}

/// Injected Forge validation stage (Smith `validate`).
pub trait Validator {
    fn validate(
        &self,
        context: &ValidationContext<'_>,
    ) -> Result<ValidationEvidence, EvolutionError>;
}

/// Everything the proposal stage is allowed to know. A proposer never sees the active pointer.
pub struct ProposalContext<'a> {
    pub experiment_id: &'a str,
    pub domain: HarnessDomain,
    pub base: &'a HarnessRevision,
    pub mode: EvolutionMode,
    pub budget: EvolutionBudget,
    pub seed: u64,
    pub pins: &'a ProbePins,
}

pub struct ContrastContext<'a> {
    pub experiment_id: &'a str,
    pub domain: HarnessDomain,
    pub base: &'a HarnessRevision,
    pub candidate: &'a HarnessRevision,
    pub contract: &'a ProbeContract,
    pub reference: &'a ProbeReading,
    pub candidate_reading: &'a ProbeReading,
}

pub struct CompositionContext<'a> {
    pub experiment_id: &'a str,
    pub domain: HarnessDomain,
    pub base: &'a HarnessRevision,
    pub candidates: &'a CandidateSet,
    pub candidate_revisions: &'a [HarnessRevision],
    pub contrasts: &'a [ContrastReport],
    pub mode: EvolutionMode,
    pub budget: EvolutionBudget,
    pub seed: u64,
}

pub struct ValidationContext<'a> {
    pub experiment_id: &'a str,
    pub domain: HarnessDomain,
    pub base: &'a HarnessRevision,
    pub successor: Option<&'a HarnessRevision>,
    pub composition: &'a CompositionDecision,
    pub contrasts: &'a [ContrastReport],
    pub changed_kinds: &'a [ComponentKind],
    /// The composed change list, exactly as Compose produced it. Forge validation re-runs the
    /// static policy gate on this list (proving `successor = base + changes`) instead of
    /// synthesizing a diff from the two manifests.
    pub changes: &'a [ComponentChange],
}

/// The five injected stages one experiment needs.
pub struct ExperimentRunner<'a> {
    pub executor: &'a dyn ProbeExecutor,
    pub proposer: &'a dyn Proposer,
    pub contrastor: &'a dyn Contrastor,
    pub composer: &'a dyn Composer,
    pub validator: &'a dyn Validator,
}

/// Why an experiment step was refused. Every variant is a refusal; the experiment is marked
/// `Failed` and the active harness is never touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExperimentError {
    Evolution(EvolutionError),
    Catalog(CatalogError),
    Probe(ProbeError),
    IllegalTransition {
        from: RevisionStatus,
        to: RevisionStatus,
    },
    NotRunFromDraft {
        state: RevisionStatus,
    },
    BaseMissing {
        revision_id: String,
    },
    PinnedBaseMismatch {
        revision_id: String,
        expected: String,
        actual: String,
    },
    DomainMismatch {
        expected: HarnessDomain,
        actual: HarnessDomain,
    },
    PolicyRejected {
        violations: Vec<String>,
    },
    ProbeBudgetExceeded {
        planned: u32,
        budget: u32,
    },
    CompositionMismatch {
        expected: String,
        actual: String,
    },
    UnknownCandidateRevision {
        revision_id: String,
    },
    Failed(ExperimentFailure),
}

impl ExperimentError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Evolution(error) => error.code(),
            Self::Catalog(_) => "catalog_refused",
            Self::Probe(_) => "probe_refused",
            Self::IllegalTransition { .. } => "illegal_transition",
            Self::NotRunFromDraft { .. } => "not_run_from_draft",
            Self::BaseMissing { .. } => "base_missing",
            Self::PinnedBaseMismatch { .. } => "pinned_base_mismatch",
            Self::DomainMismatch { .. } => "domain_mismatch",
            Self::PolicyRejected { .. } => "policy_rejected",
            Self::ProbeBudgetExceeded { .. } => "probe_budget_exceeded",
            Self::CompositionMismatch { .. } => "composition_mismatch",
            Self::UnknownCandidateRevision { .. } => "unknown_candidate_revision",
            Self::Failed(failure) => match failure.stage {
                ExperimentStage::Propose => "propose_failed",
                ExperimentStage::Policy => "policy_rejected",
                ExperimentStage::Probe => "probe_failed",
                ExperimentStage::Contrast => "contrast_failed",
                ExperimentStage::Compose => "compose_failed",
                ExperimentStage::Validate => "validate_failed",
            },
        }
    }

    pub fn stage(&self) -> ExperimentStage {
        match self {
            Self::Probe(_) => ExperimentStage::Probe,
            Self::PolicyRejected { .. } => ExperimentStage::Policy,
            Self::CompositionMismatch { .. } => ExperimentStage::Compose,
            Self::Failed(failure) => failure.stage,
            _ => ExperimentStage::Propose,
        }
    }
}

impl fmt::Display for ExperimentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Evolution(error) => write!(f, "{error}"),
            Self::Catalog(error) => write!(f, "{error}"),
            Self::Probe(error) => write!(f, "{error}"),
            Self::IllegalTransition { from, to } => {
                write!(f, "experiment may not move from {from} to {to}")
            }
            Self::NotRunFromDraft { state } => {
                write!(f, "experiment already ran; state is {state}")
            }
            Self::BaseMissing { revision_id } => {
                write!(
                    f,
                    "pinned base revision {revision_id} is not in the catalog"
                )
            }
            Self::PinnedBaseMismatch {
                revision_id,
                expected,
                actual,
            } => write!(
                f,
                "experiment pinned base {revision_id} at {expected}, but the catalog holds {actual}"
            ),
            Self::DomainMismatch { expected, actual } => {
                write!(
                    f,
                    "experiment domain {expected} does not match revision domain {actual}"
                )
            }
            Self::PolicyRejected { violations } => {
                write!(
                    f,
                    "policy refused the candidate before probing: {}",
                    violations.join("; ")
                )
            }
            Self::ProbeBudgetExceeded { planned, budget } => write!(
                f,
                "experiment would run {planned} probes but the budget allows {budget}"
            ),
            Self::CompositionMismatch { expected, actual } => write!(
                f,
                "composition names successor manifest {expected} but supplied {actual}"
            ),
            Self::UnknownCandidateRevision { revision_id } => {
                write!(f, "candidate revision {revision_id} is not in the catalog")
            }
            Self::Failed(failure) => write!(
                f,
                "experiment failed during {} ({}): {}",
                failure.stage, failure.code, failure.message
            ),
        }
    }
}

impl std::error::Error for ExperimentError {}

impl From<EvolutionError> for ExperimentError {
    fn from(error: EvolutionError) -> Self {
        Self::Evolution(error)
    }
}

impl From<CatalogError> for ExperimentError {
    fn from(error: CatalogError) -> Self {
        Self::Catalog(error)
    }
}

impl From<ProbeError> for ExperimentError {
    fn from(error: ProbeError) -> Self {
        Self::Probe(error)
    }
}

/// States from which a refused step may mark the experiment `Failed`. A completed experiment
/// (`Adoptable`/`Adopted`) is not failed by a second `run` attempt.
fn can_fail(state: RevisionStatus) -> bool {
    matches!(
        state,
        RevisionStatus::Draft
            | RevisionStatus::Proposed
            | RevisionStatus::ProbeReady
            | RevisionStatus::Probing
            | RevisionStatus::Compared
            | RevisionStatus::Composed
            | RevisionStatus::Validating
    )
}

/// A structural component diff between two manifests, as a well-formed [`ComponentChange`]
/// list: `base.apply_changes(&changes)` equals `successor` by construction, one change per
/// component kind, every change carries a rationale.
///
/// This is a general-purpose diff (reports, [`component_changes`]) with synthesized rationales. It
/// is **not** the validation seam: the experiment carries Compose's own list through
/// [`ComposedSuccessor::changes`], because a re-derived diff cannot preserve the composer's
/// evidence-linked rationales.
pub fn manifest_changes(
    base: &HarnessManifest,
    successor: &HarnessManifest,
) -> Vec<ComponentChange> {
    let mut changes = Vec::new();
    for kind in ComponentKind::ALL {
        match (base.get(kind), successor.get(kind)) {
            (None, None) => {}
            (Some(before), Some(after)) if before == after => {}
            (_, Some(after)) => changes.push(ComponentChange::set(
                after.clone(),
                format!("composed change to {kind}"),
            )),
            (Some(_), None) => changes.push(ComponentChange::remove(
                kind,
                format!("composed removal of {kind}"),
            )),
        }
    }
    changes
}

/// [`manifest_changes`] between two revisions.
pub fn component_changes(
    base: &HarnessRevision,
    successor: &HarnessRevision,
) -> Vec<ComponentChange> {
    manifest_changes(&base.manifest, &successor.manifest)
}

/// One harness experiment against one pinned base revision.
#[derive(Debug, Clone, PartialEq)]
pub struct HarnessExperiment {
    pub id: String,
    pub domain: HarnessDomain,
    /// The base revision id, pinned at creation.
    pub base_revision: String,
    /// The base manifest hash, pinned at creation: a base that changed underneath is refused.
    pub base_manifest_hash: String,
    pub mode: EvolutionMode,
    pub budget: EvolutionBudget,
    pub seed: u64,
    pub pins: ProbePins,
    pub state: RevisionStatus,
    pub candidates: Option<CandidateSet>,
    pub candidate_revisions: Vec<String>,
    pub reference: Option<ProbeReading>,
    /// Candidate revision id -> its matched reading.
    pub readings: BTreeMap<String, ProbeReading>,
    pub contrasts: Vec<ContrastReport>,
    pub composition: Option<CompositionDecision>,
    /// The canonical component diff the composition applied (base manifest -> successor
    /// manifest). Carried so adoption can re-run the policy gate on exactly what was composed
    /// instead of trusting the successor revision's shape.
    pub composed_changes: Vec<ComponentChange>,
    pub successor_revision: Option<String>,
    pub validation: Option<ValidationEvidence>,
    pub failure: Option<ExperimentFailure>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl HarnessExperiment {
    /// Create an experiment pinned to one base revision. The base is read-only here; the caller
    /// must have inserted it in the catalog first. k = 1 / one step is the defaults path
    /// ([`HarnessExperiment::new_k1`]).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        domain: HarnessDomain,
        base: &HarnessRevision,
        pins: ProbePins,
        mode: EvolutionMode,
        budget: EvolutionBudget,
        seed: u64,
    ) -> Result<Self, ExperimentError> {
        base.verify_integrity()?;
        budget.validate()?;
        let now = Utc::now();
        Ok(Self {
            id: id.into(),
            domain,
            base_revision: base.id.clone(),
            base_manifest_hash: base.manifest_hash.clone(),
            mode,
            budget,
            seed,
            pins,
            state: RevisionStatus::Draft,
            candidates: None,
            candidate_revisions: Vec::new(),
            reference: None,
            readings: BTreeMap::new(),
            contrasts: Vec::new(),
            composition: None,
            composed_changes: Vec::new(),
            successor_revision: None,
            validation: None,
            failure: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// The v1 experiment: parallel k = 1, seed 0. The base is pinned exactly as in `new`.
    pub fn new_k1(
        id: impl Into<String>,
        domain: HarnessDomain,
        base: &HarnessRevision,
        pins: ProbePins,
    ) -> Result<Self, ExperimentError> {
        Self::new(
            id,
            domain,
            base,
            pins,
            EvolutionMode::Parallel,
            EvolutionBudget::k1(),
            0,
        )
    }

    pub fn state(&self) -> RevisionStatus {
        self.state
    }

    pub fn is_terminal(&self) -> bool {
        self.state.is_terminal() || self.state == RevisionStatus::Adopted
    }

    fn transition(&mut self, next: RevisionStatus) -> Result<(), ExperimentError> {
        if !self.state.can_transition_to(next) {
            return Err(ExperimentError::IllegalTransition {
                from: self.state,
                to: next,
            });
        }
        self.state = next;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Re-read and re-check the pinned base. A base whose manifest no longer hashes to the value
    /// pinned at creation is refused; the experiment may not silently run against a different
    /// harness than the one it claims.
    fn check_base(&self, base: &HarnessRevision) -> Result<(), ExperimentError> {
        if base.id != self.base_revision {
            return Err(ExperimentError::BaseMissing {
                revision_id: self.base_revision.clone(),
            });
        }
        if base.domain != self.domain {
            return Err(ExperimentError::DomainMismatch {
                expected: self.domain,
                actual: base.domain,
            });
        }
        if base.manifest_hash != self.base_manifest_hash {
            return Err(ExperimentError::PinnedBaseMismatch {
                revision_id: base.id.clone(),
                expected: self.base_manifest_hash.clone(),
                actual: base.manifest_hash.clone(),
            });
        }
        base.verify_integrity()?;
        Ok(())
    }

    fn require_base(&self, catalog: &HarnessCatalog) -> Result<HarnessRevision, ExperimentError> {
        let base = catalog
            .revision(&self.base_revision)
            .cloned()
            .ok_or_else(|| ExperimentError::BaseMissing {
                revision_id: self.base_revision.clone(),
            })?;
        self.check_base(&base)?;
        Ok(base)
    }

    /// Proposed → record candidates. This is the **policy gate**: every candidate revision is
    /// built from the base through the model's only sanctioned constructor and passed through
    /// [`policy::validate_candidate`] *before* anything is persisted or probed. A RED / protected
    /// candidate stops the experiment here.
    pub fn record_proposal(
        &mut self,
        catalog: &mut HarnessCatalog,
        candidates: CandidateSet,
    ) -> Result<(), ExperimentError> {
        if self.state != RevisionStatus::Draft {
            return Err(ExperimentError::NotRunFromDraft { state: self.state });
        }
        candidates.validate()?;
        let base = self.require_base(catalog)?;

        let mut revisions: Vec<HarnessRevision> = Vec::new();
        for candidate in candidates.all() {
            if candidate.base_revision != self.base_revision {
                return Err(ExperimentError::Evolution(
                    EvolutionError::InvalidCandidate(format!(
                        "candidate {} targets {}, not the pinned base {}",
                        candidate.id, candidate.base_revision, self.base_revision
                    )),
                ));
            }
            let revision = HarnessRevision::from_changes(
                &base,
                format!("{}-{}", self.id, candidate.id),
                &candidate.changes,
                "harness-evolution",
            )?;
            if let Err(violations) =
                policy::validate_candidate(&base, &revision, &candidate.changes)
            {
                return Err(ExperimentError::PolicyRejected {
                    violations: violations.iter().map(ToString::to_string).collect(),
                });
            }
            revisions.push(revision);
        }

        for revision in &revisions {
            catalog.insert(revision.clone())?;
        }
        self.candidate_revisions = revisions.into_iter().map(|revision| revision.id).collect();
        self.candidates = Some(candidates);
        self.transition(RevisionStatus::Proposed)
    }

    pub fn mark_probe_ready(&mut self) -> Result<(), ExperimentError> {
        self.transition(RevisionStatus::ProbeReady)
    }

    pub fn begin_probing(&mut self) -> Result<(), ExperimentError> {
        self.transition(RevisionStatus::Probing)
    }

    /// Execute the reference run once and one matched candidate run per candidate, then contrast
    /// each pair. Reference and candidate executions are matched-condition only when the contract
    /// validates; the reference run is shared across candidates (Harness² starts with k reference
    /// executions of the base harness).
    fn execute_probes(
        &mut self,
        catalog: &HarnessCatalog,
        runner: &ExperimentRunner<'_>,
    ) -> Result<(), ExperimentError> {
        let base = self.require_base(catalog)?;
        let candidates = self
            .candidates
            .clone()
            .ok_or_else(|| ExperimentError::BaseMissing {
                revision_id: self.base_revision.clone(),
            })?;
        let mut candidate_revisions: Vec<HarnessRevision> = Vec::new();
        for revision_id in &self.candidate_revisions {
            let revision = catalog.revision(revision_id).cloned().ok_or_else(|| {
                ExperimentError::UnknownCandidateRevision {
                    revision_id: revision_id.clone(),
                }
            })?;
            candidate_revisions.push(revision);
        }
        let planned = 1 + candidate_revisions.len() as u32;
        if planned > self.budget.max_probes {
            return Err(ExperimentError::ProbeBudgetExceeded {
                planned,
                budget: self.budget.max_probes,
            });
        }

        let pairs: Vec<(Candidate, HarnessRevision)> =
            candidates.all().cloned().zip(candidate_revisions).collect();
        let reference_probe_id = format!("{}-reference", self.id);
        let mut reference: Option<ProbeReading> = None;
        let mut readings: BTreeMap<String, ProbeReading> = BTreeMap::new();
        let mut contrasts: Vec<ContrastReport> = Vec::new();

        for (candidate, revision) in pairs {
            let contract = ProbeContract::new(
                self.id.clone(),
                &base,
                &revision,
                self.pins.clone(),
                self.mode,
                self.budget,
                self.seed,
            )?;
            contract.validate()?;

            if reference.is_none() {
                let request = contract.reference_request(reference_probe_id.clone());
                let reading = runner.executor.execute(&request)?;
                verify_reading_matches_request(&request, &reading)?;
                reference = Some(reading);
            }
            let request =
                contract.candidate_request(format!("{}-{}-candidate", self.id, candidate.id));
            let candidate_reading = runner.executor.execute(&request)?;
            verify_reading_matches_request(&request, &candidate_reading)?;

            let report = {
                let reference_reading = reference.as_ref().expect("reference recorded above");
                let context = ContrastContext {
                    experiment_id: &self.id,
                    domain: self.domain,
                    base: &base,
                    candidate: &revision,
                    contract: &contract,
                    reference: reference_reading,
                    candidate_reading: &candidate_reading,
                };
                runner.contrastor.contrast(&context)?
            };
            report.validate()?;
            if report.base_revision != base.id
                || report.candidate_revision != revision.id
                || !report.identities_match()
            {
                return Err(ExperimentError::Evolution(
                    EvolutionError::ProbeIdentityMismatch {
                        reference: report.reference_identity.summary(),
                        candidate: report.candidate_identity.summary(),
                    },
                ));
            }
            contrasts.push(report);
            readings.insert(revision.id.clone(), candidate_reading);
        }

        self.reference = reference;
        self.readings = readings;
        self.contrasts = contrasts;
        self.transition(RevisionStatus::Compared)
    }

    pub fn mark_compared(&mut self) -> Result<(), ExperimentError> {
        self.transition(RevisionStatus::Compared)
    }

    /// Record the composed successor. The successor manifest must be exactly the manifest the
    /// decision names; a mismatch is refused. The base revision is never overwritten: the
    /// successor is a new child revision (or, legally, the unchanged base).
    pub fn record_composition(
        &mut self,
        catalog: &mut HarnessCatalog,
        composed: ComposedSuccessor,
    ) -> Result<(), ExperimentError> {
        if self.state != RevisionStatus::Compared {
            return Err(ExperimentError::IllegalTransition {
                from: self.state,
                to: RevisionStatus::Composed,
            });
        }
        composed.decision.validate()?;
        let base = self.require_base(catalog)?;
        if composed.decision.base_revision != base.id {
            return Err(ExperimentError::Evolution(
                EvolutionError::InvalidComposition(format!(
                    "composition targets {} but the experiment base is {}",
                    composed.decision.base_revision, base.id
                )),
            ));
        }
        let actual_hash = composed.manifest.manifest_hash();
        if composed.decision.successor_manifest_hash != actual_hash {
            return Err(ExperimentError::CompositionMismatch {
                expected: composed.decision.successor_manifest_hash,
                actual: actual_hash,
            });
        }

        // Compose's own change list is the seam's source of truth. Re-deriving a diff here would
        // synthesize rationales and break the validator's "exact composed changes" contract, so
        // the list is carried through and proven against the manifest instead.
        for change in &composed.changes {
            change.validate_shape()?;
        }
        let reconstructed = base.manifest.apply_changes(&composed.changes)?;
        if reconstructed != composed.manifest {
            return Err(ExperimentError::CompositionMismatch {
                expected: composed.manifest.manifest_hash(),
                actual: reconstructed.manifest_hash(),
            });
        }
        self.composed_changes = composed.changes.clone();
        let successor = if composed.manifest == base.manifest {
            // Composing the unchanged base is legal; no new revision is invented for it.
            base.id.clone()
        } else {
            let revision = HarnessRevision::draft(
                format!("{}-successor", self.id),
                self.domain,
                composed.manifest,
                "harness-evolution",
            )
            .with_parent(Some(base.id.clone()))
            .with_status(RevisionStatus::Composed);
            catalog.insert(revision.clone())?;
            revision.id
        };
        self.composition = Some(composed.decision);
        self.successor_revision = Some(successor);
        self.transition(RevisionStatus::Composed)
    }

    pub fn begin_validation(
        &mut self,
        catalog: &mut HarnessCatalog,
    ) -> Result<(), ExperimentError> {
        if self.state != RevisionStatus::Composed {
            return Err(ExperimentError::IllegalTransition {
                from: self.state,
                to: RevisionStatus::Validating,
            });
        }
        if let Some(successor) = self
            .successor_revision
            .as_deref()
            .filter(|successor| *successor != self.base_revision)
        {
            catalog.set_status(successor, RevisionStatus::Validating)?;
        }
        self.transition(RevisionStatus::Validating)
    }

    /// Record Forge validation. A failed gate rejects; a clean validation makes the experiment
    /// `Adoptable`; an incomplete one leaves it `Validating` (more evidence is required — a
    /// behavioral contrast alone is never enough).
    pub fn record_validation(
        &mut self,
        catalog: &mut HarnessCatalog,
        evidence: ValidationEvidence,
    ) -> Result<(), ExperimentError> {
        if self.state != RevisionStatus::Validating {
            return Err(ExperimentError::IllegalTransition {
                from: self.state,
                to: RevisionStatus::Validating,
            });
        }
        let successor = self
            .successor_revision
            .clone()
            .filter(|successor| *successor != self.base_revision);
        if !evidence.failed_gates().is_empty() {
            if let Some(successor) = &successor {
                catalog.set_status(successor, RevisionStatus::Rejected)?;
            }
            self.validation = Some(evidence);
            self.transition(RevisionStatus::Rejected)?;
            return Ok(());
        }
        if evidence.is_clean() {
            if let Some(successor) = &successor {
                catalog.set_status(successor, RevisionStatus::Adoptable)?;
            }
            self.validation = Some(evidence);
            self.transition(RevisionStatus::Adoptable)?;
            return Ok(());
        }
        // Incomplete: stay Validating; no adoption path exists from here.
        self.validation = Some(evidence);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Mark the experiment adopted after adoption.rs has applied the decision. This does not move
    /// the catalog pointer; it records that the pointer move happened.
    pub fn mark_adopted(&mut self) -> Result<(), ExperimentError> {
        self.transition(RevisionStatus::Adopted)
    }

    pub fn mark_rejected(&mut self, reason: impl Into<String>) -> Result<(), ExperimentError> {
        self.failure.get_or_insert_with(|| {
            ExperimentFailure::new(ExperimentStage::Validate, "rejected", reason.into())
        });
        self.transition(RevisionStatus::Rejected)
    }

    pub fn mark_superseded(&mut self) -> Result<(), ExperimentError> {
        self.transition(RevisionStatus::Superseded)
    }

    pub fn mark_rolled_back(&mut self) -> Result<(), ExperimentError> {
        self.transition(RevisionStatus::RolledBack)
    }

    /// Run the whole pass. The only catalog writes are lineage inserts and lifecycle statuses;
    /// there is no activation call in this file. Any error marks the experiment `Failed` and
    /// leaves the active pointer exactly where it was.
    pub fn run(
        &mut self,
        catalog: &mut HarnessCatalog,
        runner: &ExperimentRunner<'_>,
    ) -> Result<(), ExperimentError> {
        let result = self.run_inner(catalog, runner);
        if let Err(error) = &result {
            if can_fail(self.state) {
                self.state = RevisionStatus::Failed;
            }
            if self.failure.is_none() {
                self.failure = Some(ExperimentFailure::new(
                    error.stage(),
                    error.code(),
                    error.to_string(),
                ));
            }
            self.updated_at = Utc::now();
            // Best-effort durable failure status; never the active pointer.
            for revision_id in self
                .candidate_revisions
                .iter()
                .chain(self.successor_revision.iter())
            {
                if revision_id != &self.base_revision {
                    let _ = catalog.set_status(revision_id, RevisionStatus::Failed);
                }
            }
        }
        result
    }

    /// Run the Propose + Probe + Contrast phases: `Draft` -> `Compared`.
    ///
    /// Public so the k > 1 recursion layer can share ONE evidence phase across several
    /// compositions (Harness² parallel recursion) without proposing or probing again: the
    /// candidates, the shared reference execution and every per-candidate contrast stay fixed
    /// while each composition is attempted from them.
    pub fn run_proposals_and_probes(
        &mut self,
        catalog: &mut HarnessCatalog,
        runner: &ExperimentRunner<'_>,
    ) -> Result<(), ExperimentError> {
        if self.state != RevisionStatus::Draft {
            return Err(ExperimentError::NotRunFromDraft { state: self.state });
        }
        let base = self.require_base(catalog)?;

        // 1. Propose. Policy is enforced inside record_proposal, before any probe exists.
        let proposal = {
            let context = ProposalContext {
                experiment_id: &self.id,
                domain: self.domain,
                base: &base,
                mode: self.mode,
                budget: self.budget,
                seed: self.seed,
                pins: &self.pins,
            };
            runner.proposer.propose(&context)
        };
        let candidates = proposal.map_err(|error| {
            self.ensure_failure(ExperimentStage::Propose, ExperimentError::Evolution(error))
        })?;
        if let Err(error) = self.record_proposal(catalog, candidates) {
            let stage = if matches!(error, ExperimentError::PolicyRejected { .. }) {
                ExperimentStage::Policy
            } else {
                ExperimentStage::Propose
            };
            return Err(self.ensure_failure(stage, error));
        }

        // 2. Probe + contrast.
        self.mark_probe_ready()
            .map_err(|error| self.ensure_failure(ExperimentStage::Probe, error))?;
        self.begin_probing()
            .map_err(|error| self.ensure_failure(ExperimentStage::Probe, error))?;
        if let Err(error) = self.execute_probes(catalog, runner) {
            return Err(self.ensure_failure(ExperimentStage::Probe, error));
        }
        Ok(())
    }

    /// Run the Compose + Forge Validate phases: `Compared` -> `Adoptable` / `Validating` /
    /// `Rejected`.
    ///
    /// This is where a composition becomes a successor revision and where Forge validation decides
    /// whether it may be adopted. The active harness pointer is never written here; only lineage
    /// revisions and lifecycle statuses are.
    pub fn run_composition_and_validation(
        &mut self,
        catalog: &mut HarnessCatalog,
        runner: &ExperimentRunner<'_>,
    ) -> Result<(), ExperimentError> {
        if self.state != RevisionStatus::Compared {
            return Err(ExperimentError::IllegalTransition {
                from: self.state,
                to: RevisionStatus::Composed,
            });
        }
        let base = self.require_base(catalog)?;

        // 3. Compose. The successor is a new child revision; the base stays untouched.
        let composition = {
            let candidate_revisions: Vec<HarnessRevision> = self
                .candidate_revisions
                .iter()
                .filter_map(|revision_id| catalog.revision(revision_id).cloned())
                .collect();
            let context = CompositionContext {
                experiment_id: &self.id,
                domain: self.domain,
                base: &base,
                candidates: self.candidates.as_ref().ok_or_else(|| {
                    ExperimentError::BaseMissing {
                        revision_id: self.base_revision.clone(),
                    }
                })?,
                candidate_revisions: &candidate_revisions,
                contrasts: &self.contrasts,
                mode: self.mode,
                budget: self.budget,
                seed: self.seed,
            };
            runner.composer.compose(&context)
        };
        let composed = composition.map_err(|error| {
            self.ensure_failure(ExperimentStage::Compose, ExperimentError::Evolution(error))
        })?;
        self.record_composition(catalog, composed)
            .map_err(|error| self.ensure_failure(ExperimentStage::Compose, error))?;

        // 4. Forge Validate.
        self.begin_validation(catalog)
            .map_err(|error| self.ensure_failure(ExperimentStage::Validate, error))?;
        let changed_kinds = self.changed_kinds(catalog);
        let validation_result = {
            let successor = self
                .successor_revision
                .as_deref()
                .and_then(|revision_id| catalog.revision(revision_id))
                .filter(|revision| revision.id != self.base_revision);
            let composition =
                self.composition
                    .as_ref()
                    .ok_or_else(|| ExperimentError::BaseMissing {
                        revision_id: self.base_revision.clone(),
                    })?;
            let context = ValidationContext {
                experiment_id: &self.id,
                domain: self.domain,
                base: &base,
                successor,
                composition,
                contrasts: &self.contrasts,
                changed_kinds: &changed_kinds,
                changes: &self.composed_changes,
            };
            runner.validator.validate(&context)
        };
        let validation = validation_result.map_err(|error| {
            self.ensure_failure(ExperimentStage::Validate, ExperimentError::Evolution(error))
        })?;
        self.record_validation(catalog, validation)
            .map_err(|error| self.ensure_failure(ExperimentStage::Validate, error))?;
        Ok(())
    }

    fn run_inner(
        &mut self,
        catalog: &mut HarnessCatalog,
        runner: &ExperimentRunner<'_>,
    ) -> Result<(), ExperimentError> {
        self.run_proposals_and_probes(catalog, runner)?;
        self.run_composition_and_validation(catalog, runner)
    }

    /// Component kinds whose manifest entry differs between the base and the successor.
    fn changed_kinds(&self, catalog: &HarnessCatalog) -> Vec<ComponentKind> {
        let Some(successor_id) = self.successor_revision.as_deref() else {
            return Vec::new();
        };
        let Some(base) = catalog.revision(&self.base_revision) else {
            return Vec::new();
        };
        let Some(successor) = catalog.revision(successor_id) else {
            return Vec::new();
        };
        let mut kinds: Vec<ComponentKind> = Vec::new();
        for kind in ComponentKind::ALL {
            if base.manifest.get(kind) != successor.manifest.get(kind) {
                kinds.push(kind);
            }
        }
        kinds
    }

    fn ensure_failure(
        &mut self,
        stage: ExperimentStage,
        error: ExperimentError,
    ) -> ExperimentError {
        let failure = ExperimentFailure::new(stage, error.code(), error.to_string());
        if self.failure.is_none() {
            if can_fail(self.state) {
                self.state = RevisionStatus::Failed;
            }
            self.updated_at = Utc::now();
            self.failure = Some(failure.clone());
        }
        ExperimentError::Failed(failure)
    }
}

/// The widest parallel recursion this build will express: k invariant + k domain candidates per
/// evidence phase. A larger `k` is refused before any stage runs — recursion multiplies probes, so
/// a typo in `k` must not launch a large experiment.
pub const RECURSION_K_CEILING: u32 = 4;

/// The deepest sequential recursion this build will express.
pub const RECURSION_STEP_CEILING: u32 = 4;

/// The per-step budget of one Propose–Probe–Compose pass: one invariant + one domain candidate,
/// one shared reference execution and two candidate probes.
pub const SINGLE_PASS_BUDGET: EvolutionBudget = EvolutionBudget {
    k: 1,
    max_steps: 1,
    max_probes: 3,
};

/// Validate a recursion budget against the mode's shape and this build's ceilings.
///
/// * `Parallel` — one evidence phase: `max_steps` is 1, and the budget must cover one shared
///   reference run plus `k` invariant and `k` domain candidate probes (`1 + 2k`).
/// * `Sequential` — `k` successive single passes, so `k` *is* the step count (`k == max_steps`) and
///   the budget must cover `3` probes per step.
///
/// Refusals happen before a catalog write or a probe, so an over-budget request costs nothing.
pub fn validate_recursion_budget(
    mode: EvolutionMode,
    budget: &EvolutionBudget,
) -> Result<(), ExperimentError> {
    budget.validate()?;
    if budget.k > RECURSION_K_CEILING {
        return Err(ExperimentError::Evolution(EvolutionError::InvalidBudget(
            format!(
                "k={} exceeds the recursion ceiling of {RECURSION_K_CEILING}",
                budget.k
            ),
        )));
    }
    match mode {
        EvolutionMode::Parallel => {
            if budget.max_steps != 1 {
                return Err(ExperimentError::Evolution(EvolutionError::InvalidBudget(
                    "parallel recursion is one shared evidence step; max_steps must be 1".into(),
                )));
            }
            let planned = 1 + 2 * budget.k;
            if planned > budget.max_probes {
                return Err(ExperimentError::ProbeBudgetExceeded {
                    planned,
                    budget: budget.max_probes,
                });
            }
        }
        EvolutionMode::Sequential => {
            if budget.max_steps > RECURSION_STEP_CEILING {
                return Err(ExperimentError::Evolution(EvolutionError::InvalidBudget(
                    format!("sequential recursion is capped at {RECURSION_STEP_CEILING} steps"),
                )));
            }
            if budget.k != budget.max_steps {
                return Err(ExperimentError::Evolution(EvolutionError::InvalidBudget(
                    "sequential recursion's k is its step count; k must equal max_steps".into(),
                )));
            }
            let planned = SINGLE_PASS_BUDGET.max_probes * budget.max_steps;
            if planned > budget.max_probes {
                return Err(ExperimentError::ProbeBudgetExceeded {
                    planned,
                    budget: budget.max_probes,
                });
            }
        }
    }
    Ok(())
}

/// One step of a k > 1 recursion.
///
/// * parallel — one independent composition attempt over the shared evidence phase;
/// * sequential — one full Propose–Probe–Compose–Validate pass whose refined harness is carried
///   into the next step.
#[derive(Debug, Clone, PartialEq)]
pub struct RecursionStep {
    pub index: u32,
    /// The base revision this step's evidence was measured against.
    pub base_revision: String,
    pub experiment: HarnessExperiment,
    pub successor_revision: Option<String>,
    /// True only when Forge validation finished clean (`Adoptable`); adoption still decides.
    pub adoptable: bool,
}

/// What one k > 1 recursion produced.
#[derive(Debug, Clone, PartialEq)]
pub struct RecursionOutcome {
    pub mode: EvolutionMode,
    pub budget: EvolutionBudget,
    /// Parallel only: the ONE shared propose + probe + contrast phase every composition used.
    /// Sequential carries its evidence inside each step's experiment instead.
    pub evidence_phase: Option<HarnessExperiment>,
    pub steps: Vec<RecursionStep>,
    /// The revision a next recursion would carry forward: the first adoptable parallel composition,
    /// or the last adoptable sequential step.
    pub final_successor: Option<String>,
    /// Set when sequential recursion stopped early because a step was not adoptable (the chain
    /// refuses to carry an unvalidated harness forward).
    pub stopped_reason: Option<String>,
}

impl RecursionOutcome {
    pub fn adoptable_count(&self) -> usize {
        self.steps.iter().filter(|step| step.adoptable).count()
    }

    pub fn is_fully_adoptable(&self) -> bool {
        !self.steps.is_empty() && self.steps.iter().all(|step| step.adoptable)
    }

    pub fn step(&self, index: u32) -> Option<&RecursionStep> {
        self.steps.iter().find(|step| step.index == index)
    }
}

/// One k > 1 recursion request: the pinned base, the probe pins and the recursion budget.
#[derive(Debug, Clone, PartialEq)]
pub struct RecursionRequest {
    pub experiment_id: String,
    pub domain: HarnessDomain,
    pub base: HarnessRevision,
    pub pins: ProbePins,
    pub mode: EvolutionMode,
    pub budget: EvolutionBudget,
    pub seed: u64,
}

impl RecursionRequest {
    pub fn new(
        experiment_id: impl Into<String>,
        base: &HarnessRevision,
        pins: ProbePins,
        mode: EvolutionMode,
        budget: EvolutionBudget,
        seed: u64,
    ) -> Self {
        Self {
            experiment_id: experiment_id.into(),
            domain: base.domain,
            base: base.clone(),
            pins,
            mode,
            budget,
            seed,
        }
    }

    /// The v1 default: parallel k = 1, one step.
    pub fn k1(experiment_id: impl Into<String>, base: &HarnessRevision, pins: ProbePins) -> Self {
        Self::new(
            experiment_id,
            base,
            pins,
            EvolutionMode::Parallel,
            EvolutionBudget::k1(),
            0,
        )
    }
}

/// Run a k > 1 recursion. The active harness pointer is never written: steps only insert lineage
/// revisions and set lifecycle statuses, exactly like a single pass. Adoption remains the Lead's
/// separate, policy-gated step.
///
/// * **Parallel** (`k` > 1): one shared evidence phase proposes `k` invariant + `k` domain
///   candidates and probes them once; then `k` independent compositions/validations are attempted
///   from that same evidence. Every attempt is its own [`RecursionStep`].
/// * **Sequential** (`k` steps): `k` successive single passes; each step's adoptable successor
///   becomes the next step's base (and a step that is not adoptable stops the chain).
pub fn run_recursion(
    request: &RecursionRequest,
    catalog: &mut HarnessCatalog,
    runner: &ExperimentRunner<'_>,
) -> Result<RecursionOutcome, ExperimentError> {
    request.base.verify_integrity()?;
    if request.base.domain != request.domain {
        return Err(ExperimentError::DomainMismatch {
            expected: request.domain,
            actual: request.base.domain,
        });
    }
    validate_recursion_budget(request.mode, &request.budget)?;
    match request.mode {
        EvolutionMode::Parallel => run_parallel_recursion(request, catalog, runner),
        EvolutionMode::Sequential => run_sequential_recursion(request, catalog, runner),
    }
}

fn run_parallel_recursion(
    request: &RecursionRequest,
    catalog: &mut HarnessCatalog,
    runner: &ExperimentRunner<'_>,
) -> Result<RecursionOutcome, ExperimentError> {
    let mut evidence = HarnessExperiment::new(
        request.experiment_id.clone(),
        request.domain,
        &request.base,
        request.pins.clone(),
        EvolutionMode::Parallel,
        request.budget,
        request.seed,
    )?;
    evidence.run_proposals_and_probes(catalog, runner)?;

    let mut steps = Vec::with_capacity(request.budget.k as usize);
    for index in 1..=request.budget.k {
        // Each composition is an independent attempt over the SAME evidence: clone the
        // evidence-complete experiment, then compose + validate. Cloning is what keeps the
        // composition attempts from consuming one another's state.
        let mut attempt = evidence.clone();
        attempt.id = format!("{}-composition-{index}", request.experiment_id);
        attempt.run_composition_and_validation(catalog, runner)?;
        let adoptable = attempt.state == RevisionStatus::Adoptable;
        steps.push(RecursionStep {
            index,
            base_revision: attempt.base_revision.clone(),
            successor_revision: attempt.successor_revision.clone(),
            adoptable,
            experiment: attempt,
        });
    }

    let final_successor = steps
        .iter()
        .find(|step| step.adoptable)
        .and_then(|step| step.successor_revision.clone());
    Ok(RecursionOutcome {
        mode: EvolutionMode::Parallel,
        budget: request.budget,
        evidence_phase: Some(evidence),
        steps,
        final_successor,
        stopped_reason: None,
    })
}

fn run_sequential_recursion(
    request: &RecursionRequest,
    catalog: &mut HarnessCatalog,
    runner: &ExperimentRunner<'_>,
) -> Result<RecursionOutcome, ExperimentError> {
    let mut steps = Vec::with_capacity(request.budget.max_steps as usize);
    let mut current_base_id = request.base.id.clone();
    let mut final_successor: Option<String> = None;
    let mut stopped_reason: Option<String> = None;

    for index in 1..=request.budget.max_steps {
        let base = catalog.revision(&current_base_id).cloned().ok_or_else(|| {
            ExperimentError::BaseMissing {
                revision_id: current_base_id.clone(),
            }
        })?;
        let mut experiment = HarnessExperiment::new(
            format!("{}-step-{index}", request.experiment_id),
            request.domain,
            &base,
            request.pins.clone(),
            EvolutionMode::Sequential,
            SINGLE_PASS_BUDGET,
            request.seed,
        )?;
        // A full single pass: the same policy-before-probe and adoption rails as k = 1.
        experiment.run(catalog, runner)?;
        let adoptable = experiment.state == RevisionStatus::Adoptable;
        let end_state = experiment.state;
        let successor = experiment.successor_revision.clone();
        steps.push(RecursionStep {
            index,
            base_revision: current_base_id.clone(),
            successor_revision: successor.clone(),
            adoptable,
            experiment,
        });
        if !adoptable {
            stopped_reason = Some(format!(
                "step {index} ended {end_state} instead of adoptable; sequential recursion will not carry an unvalidated harness"
            ));
            break;
        }
        final_successor = successor.clone();
        if let Some(successor) = successor {
            current_base_id = successor;
        }
        // A composition that legally chose the unchanged base leaves current_base_id alone; the
        // next step then refines from the same base. That is a real (no-op) step, not a failure.
    }

    Ok(RecursionOutcome {
        mode: EvolutionMode::Sequential,
        budget: request.budget,
        evidence_phase: None,
        steps,
        final_successor,
        stopped_reason,
    })
}

/// Behavioral dimensions a v1 experiment reports, exposed so a test/report can assert that the
/// contrast is behavioral and not stylistic. (The Smith contrast stage owns the real reading.)
pub const CONTRAST_DIMENSIONS: [BehaviorDimension; 16] = BehaviorDimension::ALL;

/// The proposal kinds kept separate until Compose.
pub const PROPOSAL_KINDS: [ProposalKind; 2] = [ProposalKind::Invariant, ProposalKind::Domain];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::harness_evolution::model::{
        BehaviorEvidence, Component, ComponentDecision, ComponentDecisionRecord, DimensionContrast,
        EnvironmentClass, QaVerdict,
    };
    use crate::engine::harness_evolution::probe::{ProbeOutcome, ProbeSide, RecordedProbe};
    use std::cell::RefCell;

    fn prompt(content: &str) -> Component {
        Component::green(
            ComponentKind::SystemPrompt,
            content,
            "experiment test prompt",
        )
    }

    fn base_revision() -> HarnessRevision {
        HarnessRevision::draft(
            "base-1",
            HarnessDomain::RustBackend,
            HarnessManifest::from_components([prompt("base prompt")]),
            "owner",
        )
        .with_status(RevisionStatus::Adopted)
    }

    fn pins() -> ProbePins {
        ProbePins::new(
            "task-1",
            "deepseek-chat",
            "default",
            "opencode",
            Vec::new(),
            EnvironmentClass::Development,
            "dataset-1",
            "cargo test",
            "replay-1",
        )
    }

    fn experiment(base: &HarnessRevision) -> HarnessExperiment {
        HarnessExperiment::new_k1("exp-1", HarnessDomain::RustBackend, base, pins()).expect("new")
    }

    struct FixedProposer {
        candidate: Candidate,
    }

    impl Proposer for FixedProposer {
        fn propose(&self, _context: &ProposalContext<'_>) -> Result<CandidateSet, EvolutionError> {
            let mut set = CandidateSet::k1();
            set.invariant.push(self.candidate.clone());
            Ok(set)
        }
    }

    struct FixedContrastor;

    impl Contrastor for FixedContrastor {
        fn contrast(
            &self,
            context: &ContrastContext<'_>,
        ) -> Result<ContrastReport, EvolutionError> {
            Ok(ContrastReport {
                id: format!("contrast-{}-{}", context.base.id, context.candidate.id),
                base_revision: context.base.id.clone(),
                candidate_revision: context.candidate.id.clone(),
                reference_identity: context.contract.spec().identity(),
                candidate_identity: context.contract.spec().identity(),
                dimensions: vec![DimensionContrast::from_evidence(
                    BehaviorDimension::Success,
                    &context.reference.behavior,
                    &context.candidate_reading.behavior,
                    "success contrast",
                )],
                summary: "candidate behavior".into(),
                evidence_refs: context.reference.outcome.evidence_refs.clone(),
            })
        }
    }

    /// Compose's canonical change list for the deterministic fixture. The validator test compares
    /// against THIS list (and against a re-derived diff) rather than against the experiment field
    /// it is checking, so the assertion is not tautological.
    fn fixed_composer_changes() -> Vec<ComponentChange> {
        vec![ComponentChange::set(
            prompt("composed prompt"),
            "compose adoption: candidate guidance beat the base on the behavioral contrast",
        )]
    }

    struct FixedComposer;

    impl Composer for FixedComposer {
        fn compose(
            &self,
            context: &CompositionContext<'_>,
        ) -> Result<ComposedSuccessor, EvolutionError> {
            let changes = fixed_composer_changes();
            let manifest = context.base.manifest.clone().apply_changes(&changes)?;
            Ok(ComposedSuccessor {
                decision: CompositionDecision {
                    id: "composition-1".into(),
                    base_revision: context.base.id.clone(),
                    candidates: context
                        .candidates
                        .all()
                        .map(|candidate| candidate.id.clone())
                        .collect(),
                    contrast_ids: context
                        .contrasts
                        .iter()
                        .map(|report| report.id.clone())
                        .collect(),
                    decisions: vec![ComponentDecisionRecord::new(
                        ComponentKind::SystemPrompt,
                        ComponentDecision::Adopt,
                        "the candidate was better",
                    )
                    .with_evidence_for(vec!["evidence-1".into()])
                    .with_confidence(80)],
                    successor_manifest_hash: manifest.manifest_hash(),
                    summary: "compose".into(),
                    evidence_refs: vec!["evidence-1".into()],
                },
                manifest,
                changes,
            })
        }
    }

    struct FixedValidator {
        evidence: RefCell<Option<ValidationEvidence>>,
    }

    impl FixedValidator {
        fn clean() -> Self {
            Self {
                evidence: RefCell::new(Some(ValidationEvidence::all_passed(vec![
                    "validation-1".into()
                ]))),
            }
        }

        fn incomplete() -> Self {
            Self {
                evidence: RefCell::new(Some(ValidationEvidence::pending())),
            }
        }

        fn failed() -> Self {
            Self {
                evidence: RefCell::new(Some(ValidationEvidence {
                    static_policy: crate::engine::harness_evolution::model::GateStatus::Failed,
                    regression_rails: crate::engine::harness_evolution::model::GateStatus::Passed,
                    replay: crate::engine::harness_evolution::model::GateStatus::Passed,
                    phase3_evidence: crate::engine::harness_evolution::model::GateStatus::Passed,
                    evidence_refs: vec!["validation-1".into()],
                })),
            }
        }
    }

    impl Validator for FixedValidator {
        fn validate(
            &self,
            _context: &ValidationContext<'_>,
        ) -> Result<ValidationEvidence, EvolutionError> {
            Ok(self
                .evidence
                .borrow()
                .clone()
                .expect("validator configured"))
        }
    }

    fn candidate(base: &HarnessRevision) -> Candidate {
        Candidate::new(
            "c1",
            base.id.clone(),
            ProposalKind::Invariant,
            vec![crate_engine_change()],
        )
    }

    fn crate_engine_change() -> crate::engine::harness_evolution::model::ComponentChange {
        crate::engine::harness_evolution::model::ComponentChange::set(
            prompt("candidate prompt"),
            "candidate updates the system prompt",
        )
    }

    fn reading(probe_id: &str, side: ProbeSide, revision: &str) -> ProbeReading {
        let mut behavior = BehaviorEvidence::new(format!("evidence-{probe_id}"), probe_id);
        behavior.success = true;
        behavior.qa = QaVerdict::Pass;
        behavior.evidence_refs = vec![format!("exec-{probe_id}")];
        let outcome = ProbeOutcome {
            probe_id: probe_id.into(),
            side,
            revision: revision.into(),
            contract_digest: String::new(),
            evidence_id: behavior.evidence_id.clone(),
            evidence_refs: behavior.evidence_refs.clone(),
        };
        ProbeReading::new(outcome, behavior).expect("reading")
    }

    fn recorded_for(experiment: &HarnessExperiment) -> RecordedProbe {
        RecordedProbe::new(vec![
            reading(
                &format!("{}-reference", experiment.id),
                ProbeSide::Reference,
                "base-1",
            ),
            reading(
                &format!("{}-c1-candidate", experiment.id),
                ProbeSide::Candidate,
                &format!("{}-c1", experiment.id),
            ),
        ])
    }

    fn catalog_with(base: &HarnessRevision) -> HarnessCatalog {
        let mut catalog = HarnessCatalog::new();
        catalog.insert(base.clone()).unwrap();
        catalog
            .activate(HarnessDomain::RustBackend, &base.id, "bootstrap", "owner")
            .unwrap();
        catalog
    }

    struct Harness2Parts {
        proposer: FixedProposer,
        executor: RecordedProbe,
        contrastor: FixedContrastor,
        composer: FixedComposer,
        validator: FixedValidator,
    }

    fn parts(base: &HarnessRevision) -> Harness2Parts {
        let experiment = experiment(base);
        Harness2Parts {
            proposer: FixedProposer {
                candidate: candidate(base),
            },
            executor: recorded_for(&experiment),
            contrastor: FixedContrastor,
            composer: FixedComposer,
            validator: FixedValidator::clean(),
        }
    }

    fn run_experiment(
        experiment: &mut HarnessExperiment,
        catalog: &mut HarnessCatalog,
        parts: &Harness2Parts,
    ) -> Result<(), ExperimentError> {
        let runner = ExperimentRunner {
            executor: &parts.executor,
            proposer: &parts.proposer,
            contrastor: &parts.contrastor,
            composer: &parts.composer,
            validator: &parts.validator,
        };
        experiment.run(catalog, &runner)
    }

    #[test]
    fn creation_pins_the_base_and_defaults_to_k1() {
        let base = base_revision();
        let experiment = experiment(&base);
        assert_eq!(experiment.base_revision, "base-1");
        assert_eq!(experiment.base_manifest_hash, base.manifest_hash);
        assert_eq!(experiment.budget, EvolutionBudget::k1());
        assert!(experiment.budget.is_k1());
        assert_eq!(experiment.state, RevisionStatus::Draft);
    }

    #[test]
    fn a_base_that_changed_underneath_the_experiment_is_refused() {
        let base = base_revision();
        let experiment = experiment(&base);
        let mut impostor = base.clone();
        impostor.manifest = HarnessManifest::from_components([prompt("someone else's prompt")]);
        impostor.manifest_hash = impostor.manifest.manifest_hash();
        // A drifted base has both a new manifest and a new hash; the pinned hash is what refuses it.
        let error = experiment.check_base(&impostor).expect_err("drifted base");
        assert!(matches!(error, ExperimentError::PinnedBaseMismatch { .. }));
    }

    #[test]
    fn the_happy_path_reaches_adoptable_without_touching_the_active_pointer() {
        let base = base_revision();
        let mut catalog = catalog_with(&base);
        let mut experiment = experiment(&base);
        let parts = parts(&base);

        run_experiment(&mut experiment, &mut catalog, &parts).expect("run");

        assert_eq!(experiment.state, RevisionStatus::Adoptable);
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1"),
            "an experiment must never move the active pointer"
        );
        assert_eq!(experiment.contrasts.len(), 1);
        assert_eq!(
            experiment.composed_changes.len(),
            1,
            "the composition's exact changes are carried with the experiment"
        );
        let successor = experiment.successor_revision.as_deref().expect("successor");
        assert_eq!(successor, "exp-1-successor");
        assert_eq!(
            catalog.revision(successor).unwrap().status,
            RevisionStatus::Adoptable
        );
        assert_eq!(
            catalog
                .revision(successor)
                .unwrap()
                .parent_revision_id
                .as_deref(),
            Some("base-1")
        );
        assert_eq!(
            experiment.reference.as_ref().unwrap().evidence_refs().len(),
            1
        );
    }

    #[test]
    fn a_protected_candidate_fails_before_any_probe_runs() {
        let base = base_revision();
        let mut catalog = catalog_with(&base);
        let mut experiment = experiment(&base);
        let red = Candidate::new(
            "c1",
            base.id.clone(),
            ProposalKind::Invariant,
            vec![
                crate::engine::harness_evolution::model::ComponentChange::set(
                    Component::red(ComponentKind::Scripts, "rm -rf /", "publication surface"),
                    "try to take over scripts",
                ),
            ],
        );
        let executor = RecordedProbe::new(Vec::new());
        let proposer = FixedProposer { candidate: red };
        let parts = Harness2Parts {
            proposer,
            executor,
            contrastor: FixedContrastor,
            composer: FixedComposer,
            validator: FixedValidator::clean(),
        };
        let error = run_experiment(&mut experiment, &mut catalog, &parts).unwrap_err();
        assert_eq!(error.stage(), ExperimentStage::Policy);
        assert!(
            !parts.executor.was_called(),
            "no probe may run after a policy refusal"
        );
        assert_eq!(experiment.state, RevisionStatus::Failed);
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1")
        );
    }

    #[test]
    fn a_probe_failure_is_isolated_to_the_experiment() {
        let base = base_revision();
        let mut catalog = catalog_with(&base);
        let mut experiment = experiment(&base);
        let executor = RecordedProbe::new(Vec::new());
        let proposer = FixedProposer {
            candidate: candidate(&base),
        };
        let parts = Harness2Parts {
            proposer,
            executor,
            contrastor: FixedContrastor,
            composer: FixedComposer,
            validator: FixedValidator::clean(),
        };
        let error = run_experiment(&mut experiment, &mut catalog, &parts).unwrap_err();
        assert_eq!(error.stage(), ExperimentStage::Probe);
        assert_eq!(experiment.state, RevisionStatus::Failed);
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1"),
            "a failed probe cannot mutate the active harness"
        );
        assert_eq!(
            catalog.revision("base-1").unwrap().status,
            RevisionStatus::Adopted
        );
    }

    #[test]
    fn a_failing_composer_is_isolated_to_the_experiment() {
        struct FailingComposer;
        impl Composer for FailingComposer {
            fn compose(
                &self,
                _context: &CompositionContext<'_>,
            ) -> Result<ComposedSuccessor, EvolutionError> {
                Err(EvolutionError::InvalidComposition("no composition".into()))
            }
        }
        let base = base_revision();
        let mut catalog = catalog_with(&base);
        let mut experiment = experiment(&base);
        let proposer = FixedProposer {
            candidate: candidate(&base),
        };
        let executor = recorded_for(&experiment);
        let runner = ExperimentRunner {
            executor: &executor,
            proposer: &proposer,
            contrastor: &FixedContrastor,
            composer: &FailingComposer,
            validator: &FixedValidator::clean(),
        };
        let error = experiment.run(&mut catalog, &runner).unwrap_err();
        assert_eq!(error.stage(), ExperimentStage::Compose);
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1")
        );
    }

    #[test]
    fn failed_validation_rejects_the_experiment_and_successor() {
        let base = base_revision();
        let mut catalog = catalog_with(&base);
        let mut experiment = experiment(&base);
        let parts = Harness2Parts {
            proposer: FixedProposer {
                candidate: candidate(&base),
            },
            executor: recorded_for(&experiment),
            contrastor: FixedContrastor,
            composer: FixedComposer,
            validator: FixedValidator::failed(),
        };
        run_experiment(&mut experiment, &mut catalog, &parts).expect("run completes");
        assert_eq!(experiment.state, RevisionStatus::Rejected);
        assert_eq!(
            catalog.revision("exp-1-successor").unwrap().status,
            RevisionStatus::Rejected
        );
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1")
        );
    }

    #[test]
    fn incomplete_validation_leaves_the_experiment_validating_not_adoptable() {
        let base = base_revision();
        let mut catalog = catalog_with(&base);
        let mut experiment = experiment(&base);
        let parts = Harness2Parts {
            proposer: FixedProposer {
                candidate: candidate(&base),
            },
            executor: recorded_for(&experiment),
            contrastor: FixedContrastor,
            composer: FixedComposer,
            validator: FixedValidator::incomplete(),
        };
        run_experiment(&mut experiment, &mut catalog, &parts).expect("run completes");
        assert_eq!(experiment.state, RevisionStatus::Validating);
        assert_eq!(
            catalog.revision("exp-1-successor").unwrap().status,
            RevisionStatus::Validating
        );
        assert!(
            experiment.mark_adopted().is_err(),
            "more evidence is required"
        );
    }

    #[test]
    fn illegal_transitions_and_reruns_are_refused() {
        let base = base_revision();
        let mut experiment = experiment(&base);
        assert!(matches!(
            experiment.mark_probe_ready(),
            Err(ExperimentError::IllegalTransition { .. })
        ));
        let mut catalog = catalog_with(&base);
        let parts = parts(&base);
        run_experiment(&mut experiment, &mut catalog, &parts).expect("first run");
        let error = run_experiment(&mut experiment, &mut catalog, &parts).unwrap_err();
        assert!(matches!(error, ExperimentError::NotRunFromDraft { .. }));
    }

    #[test]
    fn a_failed_experiment_cannot_be_revived() {
        let base = base_revision();
        let mut catalog = catalog_with(&base);
        let mut experiment = experiment(&base);
        let parts = Harness2Parts {
            proposer: FixedProposer {
                candidate: candidate(&base),
            },
            executor: RecordedProbe::new(Vec::new()),
            contrastor: FixedContrastor,
            composer: FixedComposer,
            validator: FixedValidator::clean(),
        };
        run_experiment(&mut experiment, &mut catalog, &parts).expect_err("probe fails");
        assert_eq!(experiment.state, RevisionStatus::Failed);
        assert!(experiment.mark_probe_ready().is_err());
        assert!(experiment.mark_adopted().is_err());
    }

    #[test]
    fn proposal_kinds_stay_separate_until_compose() {
        assert_eq!(
            PROPOSAL_KINDS,
            [ProposalKind::Invariant, ProposalKind::Domain]
        );
        assert_eq!(CONTRAST_DIMENSIONS.len(), 16);
    }

    struct RecordingValidator {
        evidence: ValidationEvidence,
        seen_changes: RefCell<Vec<ComponentChange>>,
        seen_successor: RefCell<Option<String>>,
    }

    impl RecordingValidator {
        fn clean() -> Self {
            Self {
                evidence: ValidationEvidence::all_passed(vec!["validation-1".into()]),
                seen_changes: RefCell::new(Vec::new()),
                seen_successor: RefCell::new(None),
            }
        }
    }

    impl Validator for RecordingValidator {
        fn validate(
            &self,
            context: &ValidationContext<'_>,
        ) -> Result<ValidationEvidence, EvolutionError> {
            *self.seen_changes.borrow_mut() = context.changes.to_vec();
            *self.seen_successor.borrow_mut() =
                context.successor.map(|successor| successor.id.clone());
            Ok(self.evidence.clone())
        }
    }

    #[test]
    fn validator_receives_the_exact_composed_changes() {
        let base = base_revision();
        let mut catalog = catalog_with(&base);
        let mut experiment = experiment(&base);
        let fixed = parts(&base);
        let recording = RecordingValidator::clean();
        let runner = ExperimentRunner {
            executor: &fixed.executor,
            proposer: &fixed.proposer,
            contrastor: &fixed.contrastor,
            composer: &fixed.composer,
            validator: &recording,
        };

        experiment.run(&mut catalog, &runner).expect("run");

        // The expectation is the COMPOSER's own list, not the experiment field under test: this
        // assertion would still fail if the seam re-derived a diff.
        let composer_changes = fixed_composer_changes();
        assert_eq!(
            *recording.seen_changes.borrow(),
            composer_changes,
            "Forge validation must receive Compose's exact changes, rationales included"
        );
        assert_eq!(experiment.composed_changes, composer_changes);

        // A re-derived diff exists and differs: it has a synthesized rationale, which is exactly
        // what the seam must not substitute for Compose's evidence-linked one.
        let successor_id = experiment
            .successor_revision
            .clone()
            .expect("composed successor");
        let base_manifest = catalog
            .revision(&experiment.base_revision)
            .expect("base in catalog")
            .manifest
            .clone();
        let successor_manifest = catalog
            .revision(&successor_id)
            .expect("successor in catalog")
            .manifest
            .clone();
        let re_derived = manifest_changes(&base_manifest, &successor_manifest);
        assert_ne!(
            composer_changes, re_derived,
            "the composer's rationale must survive; a re-derived diff replaces it"
        );

        assert_eq!(
            recording.seen_successor.borrow().as_deref(),
            Some(successor_id.as_str()),
            "the validator sees the composed successor revision"
        );
        assert_eq!(experiment.state, RevisionStatus::Adoptable);
    }

    #[test]
    fn composition_changes_are_the_adapter_carried_list_not_a_rederived_diff() {
        // The experiment must consume Compose's ComposedHarness verbatim (the From adapter), not
        // re-derive a diff. Construct a real ComposedHarness and prove the mapping.
        use crate::engine::harness_evolution::compose::ComposedHarness;

        let base = base_revision();
        let changes = vec![ComponentChange::set(
            prompt("adapted prompt"),
            "compose rationale",
        )];
        let manifest = base
            .manifest
            .clone()
            .apply_changes(&changes)
            .expect("fixture");
        let successor =
            HarnessRevision::draft("successor-adapted", base.domain, manifest.clone(), "smith")
                .with_parent(Some(base.id.clone()));
        let decision = CompositionDecision {
            id: "composition-adapted".into(),
            base_revision: base.id.clone(),
            candidates: vec!["c1".into()],
            contrast_ids: vec!["contrast-c1".into()],
            decisions: vec![ComponentDecisionRecord::new(
                ComponentKind::SystemPrompt,
                ComponentDecision::Adopt,
                "compose rationale",
            )
            .with_evidence_for(vec!["evidence-1".into()])
            .with_confidence(80)],
            successor_manifest_hash: manifest.manifest_hash(),
            summary: "adapted".into(),
            evidence_refs: vec!["evidence-1".into()],
        };
        let composed_harness = ComposedHarness {
            decision: decision.clone(),
            successor,
            changes: changes.clone(),
        };

        let borrowed = ComposedSuccessor::from(&composed_harness);
        assert_eq!(borrowed.decision, decision);
        assert_eq!(borrowed.manifest, manifest);
        assert_eq!(borrowed.changes, changes);

        let owned = ComposedSuccessor::from(composed_harness);
        assert_eq!(owned.changes, changes);
        assert_eq!(owned.manifest, manifest);
    }

    #[test]
    fn a_composition_whose_changes_do_not_reconstruct_its_manifest_is_refused() {
        let base = base_revision();
        let mut catalog = catalog_with(&base);
        let mut experiment = experiment(&base);
        let parts = parts(&base);
        let runner = ExperimentRunner {
            executor: &parts.executor,
            proposer: &parts.proposer,
            contrastor: &parts.contrastor,
            composer: &parts.composer,
            validator: &parts.validator,
        };
        experiment
            .run_proposals_and_probes(&mut catalog, &runner)
            .expect("evidence phase");

        // Manifest claims the prompt changed, but the carried change list is empty.
        let manifest = base
            .manifest
            .clone()
            .with_component(prompt("composed prompt"));
        let decision = CompositionDecision {
            id: "composition-1".into(),
            base_revision: base.id.clone(),
            candidates: vec!["c1".into()],
            contrast_ids: vec!["contrast-c1".into()],
            decisions: vec![ComponentDecisionRecord::new(
                ComponentKind::SystemPrompt,
                ComponentDecision::Adopt,
                "compose",
            )
            .with_evidence_for(vec!["evidence-1".into()])
            .with_confidence(80)],
            successor_manifest_hash: manifest.manifest_hash(),
            summary: "claim".into(),
            evidence_refs: vec!["evidence-1".into()],
        };
        let composed = ComposedSuccessor {
            decision,
            manifest,
            changes: Vec::new(),
        };
        let error = experiment
            .record_composition(&mut catalog, composed)
            .expect_err("inconsistent change list");
        assert_eq!(error.code(), "composition_mismatch");
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1")
        );
    }
    // ---------------------------------------------------------------------------------------
    // k > 1 recursion (task-9)
    // ---------------------------------------------------------------------------------------

    /// Proposes one candidate against whatever base the context names, so sequential steps can
    /// each target the previous step's successor.
    struct ContextProposer;

    impl Proposer for ContextProposer {
        fn propose(&self, context: &ProposalContext<'_>) -> Result<CandidateSet, EvolutionError> {
            let mut set = CandidateSet::k1();
            set.invariant.push(Candidate::new(
                "c1",
                context.base.id.clone(),
                ProposalKind::Invariant,
                vec![ComponentChange::set(
                    prompt(&format!("candidate at {}", context.base.id)),
                    "candidate update",
                )],
            ));
            Ok(set)
        }
    }

    /// Composes a manifest that depends on the step's base, so a carried successor is a genuinely
    /// new revision and the chain is observable.
    struct StepComposer;

    impl Composer for StepComposer {
        fn compose(
            &self,
            context: &CompositionContext<'_>,
        ) -> Result<ComposedSuccessor, EvolutionError> {
            let content = format!("composed at {}", context.base.id);
            let changes = vec![ComponentChange::set(
                prompt(&content),
                "step compose: the step's contrast supported adopting the candidate guidance",
            )];
            let manifest = context.base.manifest.clone().apply_changes(&changes)?;
            Ok(ComposedSuccessor {
                decision: CompositionDecision {
                    id: format!("composition-{}", context.base.id),
                    base_revision: context.base.id.clone(),
                    candidates: context
                        .candidates
                        .all()
                        .map(|candidate| candidate.id.clone())
                        .collect(),
                    contrast_ids: context
                        .contrasts
                        .iter()
                        .map(|report| report.id.clone())
                        .collect(),
                    decisions: vec![ComponentDecisionRecord::new(
                        ComponentKind::SystemPrompt,
                        ComponentDecision::Adopt,
                        "the step improved",
                    )
                    .with_evidence_for(vec!["evidence-1".into()])
                    .with_confidence(80)],
                    successor_manifest_hash: manifest.manifest_hash(),
                    summary: "step compose".into(),
                    evidence_refs: vec!["evidence-1".into()],
                },
                manifest,
                changes,
            })
        }
    }

    /// Proposes k invariant + k domain candidates, the parallel-recursion width.
    struct TwoPerScopeProposer;

    impl Proposer for TwoPerScopeProposer {
        fn propose(&self, context: &ProposalContext<'_>) -> Result<CandidateSet, EvolutionError> {
            let mut set = CandidateSet::new(context.mode, context.budget);
            for index in 1..=context.budget.k {
                set.invariant.push(Candidate::new(
                    format!("inv-{index}"),
                    context.base.id.clone(),
                    ProposalKind::Invariant,
                    vec![ComponentChange::set(
                        prompt(&format!("invariant {index}")),
                        "invariant guidance",
                    )],
                ));
                set.domain.push(Candidate::new(
                    format!("dom-{index}"),
                    context.base.id.clone(),
                    ProposalKind::Domain,
                    vec![ComponentChange::set(
                        Component::green(
                            ComponentKind::Memory,
                            format!("domain {index}"),
                            "domain memory",
                        ),
                        "domain knowledge",
                    )],
                ));
            }
            set.validate()?;
            Ok(set)
        }
    }

    #[test]
    fn recursion_default_k1_is_one_parallel_step() {
        let base = base_revision();
        let mut catalog = catalog_with(&base);
        let request = RecursionRequest::k1("exp-1", &base, pins());
        let parts = parts(&base);
        let runner = ExperimentRunner {
            executor: &parts.executor,
            proposer: &parts.proposer,
            contrastor: &parts.contrastor,
            composer: &parts.composer,
            validator: &parts.validator,
        };
        let outcome = run_recursion(&request, &mut catalog, &runner).expect("k1 recursion");
        assert_eq!(outcome.mode, EvolutionMode::Parallel);
        assert_eq!(outcome.steps.len(), 1);
        assert!(outcome.steps[0].adoptable);
        assert_eq!(outcome.adoptable_count(), 1);
        assert_eq!(
            outcome.steps[0].successor_revision.as_deref(),
            Some("exp-1-composition-1-successor")
        );
        assert_eq!(outcome.final_successor, outcome.steps[0].successor_revision);
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1"),
            "recursion never moves the active pointer"
        );
    }

    #[test]
    fn parallel_k2_runs_one_evidence_phase_and_two_compositions() {
        let base = base_revision();
        let mut catalog = catalog_with(&base);
        let budget = EvolutionBudget {
            k: 2,
            max_steps: 1,
            max_probes: 5,
        };
        let request =
            RecursionRequest::new("rec-1", &base, pins(), EvolutionMode::Parallel, budget, 0);
        let executor = RecordedProbe::new(vec![
            reading("rec-1-reference", ProbeSide::Reference, "base-1"),
            reading("rec-1-inv-1-candidate", ProbeSide::Candidate, "rec-1-inv-1"),
            reading("rec-1-inv-2-candidate", ProbeSide::Candidate, "rec-1-inv-2"),
            reading("rec-1-dom-1-candidate", ProbeSide::Candidate, "rec-1-dom-1"),
            reading("rec-1-dom-2-candidate", ProbeSide::Candidate, "rec-1-dom-2"),
        ]);
        let runner = ExperimentRunner {
            executor: &executor,
            proposer: &TwoPerScopeProposer,
            contrastor: &FixedContrastor,
            composer: &FixedComposer,
            validator: &FixedValidator::clean(),
        };
        let outcome = run_recursion(&request, &mut catalog, &runner).expect("parallel k2");

        let evidence = outcome
            .evidence_phase
            .as_ref()
            .expect("shared evidence phase");
        assert_eq!(
            evidence.candidate_revisions.len(),
            4,
            "k invariant + k domain"
        );
        assert_eq!(evidence.contrasts.len(), 4);
        assert_eq!(
            outcome.steps.len(),
            2,
            "k compositions from the shared evidence"
        );
        assert_eq!(outcome.adoptable_count(), 2);
        assert!(outcome.is_fully_adoptable());
        for step in &outcome.steps {
            assert!(step.adoptable);
            assert_eq!(step.base_revision, "base-1");
            assert_eq!(step.experiment.state, RevisionStatus::Adoptable);
            // Every composition saw exactly the one shared evidence set.
            assert_eq!(step.experiment.contrasts, evidence.contrasts);
            assert_eq!(
                step.experiment.candidate_revisions,
                evidence.candidate_revisions
            );
        }
        let first = outcome.steps[0].successor_revision.clone();
        let second = outcome.steps[1].successor_revision.clone();
        assert_ne!(
            first, second,
            "each composition is its own successor revision"
        );
        assert_eq!(outcome.final_successor, first);
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1")
        );
    }

    #[test]
    fn sequential_k2_carries_the_adopted_successor_into_the_next_step() {
        let base = base_revision();
        let mut catalog = catalog_with(&base);
        let budget = EvolutionBudget {
            k: 2,
            max_steps: 2,
            max_probes: 6,
        };
        let request =
            RecursionRequest::new("seq-1", &base, pins(), EvolutionMode::Sequential, budget, 0);
        let executor = RecordedProbe::new(vec![
            reading("seq-1-step-1-reference", ProbeSide::Reference, "base-1"),
            reading(
                "seq-1-step-1-c1-candidate",
                ProbeSide::Candidate,
                "seq-1-step-1-c1",
            ),
            reading(
                "seq-1-step-2-reference",
                ProbeSide::Reference,
                "seq-1-step-1-successor",
            ),
            reading(
                "seq-1-step-2-c1-candidate",
                ProbeSide::Candidate,
                "seq-1-step-2-c1",
            ),
        ]);
        let runner = ExperimentRunner {
            executor: &executor,
            proposer: &ContextProposer,
            contrastor: &FixedContrastor,
            composer: &StepComposer,
            validator: &FixedValidator::clean(),
        };
        let outcome = run_recursion(&request, &mut catalog, &runner).expect("sequential k2");

        assert_eq!(outcome.mode, EvolutionMode::Sequential);
        assert_eq!(outcome.steps.len(), 2);
        assert!(outcome.is_fully_adoptable());
        assert!(outcome.stopped_reason.is_none());
        assert_eq!(
            outcome.steps[0].successor_revision.as_deref(),
            Some("seq-1-step-1-successor")
        );
        assert_eq!(outcome.steps[1].base_revision, "seq-1-step-1-successor");
        assert_eq!(
            outcome.final_successor.as_deref(),
            Some("seq-1-step-2-successor")
        );
        // The carried base is the previous step's successor, not the original base.
        assert_ne!(outcome.steps[1].base_revision, "base-1");
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1"),
            "sequential recursion never moves the active pointer"
        );
    }

    #[test]
    fn a_failed_sequential_step_is_isolated_from_the_active_harness() {
        let base = base_revision();
        let mut catalog = catalog_with(&base);
        let budget = EvolutionBudget {
            k: 2,
            max_steps: 2,
            max_probes: 6,
        };
        let request =
            RecursionRequest::new("seq-1", &base, pins(), EvolutionMode::Sequential, budget, 0);
        // Step 1 can run; step 2's reference probe is missing, so step 2 fails.
        let executor = RecordedProbe::new(vec![
            reading("seq-1-step-1-reference", ProbeSide::Reference, "base-1"),
            reading(
                "seq-1-step-1-c1-candidate",
                ProbeSide::Candidate,
                "seq-1-step-1-c1",
            ),
        ]);
        let runner = ExperimentRunner {
            executor: &executor,
            proposer: &ContextProposer,
            contrastor: &FixedContrastor,
            composer: &StepComposer,
            validator: &FixedValidator::clean(),
        };
        let error = run_recursion(&request, &mut catalog, &runner).unwrap_err();
        assert_eq!(error.stage(), ExperimentStage::Probe);
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1"),
            "a failed step cannot mutate the active harness"
        );
        // Step 1's successor is lineage, not activation.
        let successor = catalog
            .revision("seq-1-step-1-successor")
            .expect("step 1 successor recorded");
        assert_eq!(successor.status, RevisionStatus::Adoptable);
        assert_ne!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("seq-1-step-1-successor")
        );
    }

    #[test]
    fn a_non_adoptable_step_stops_sequential_recursion() {
        let base = base_revision();
        let mut catalog = catalog_with(&base);
        let budget = EvolutionBudget {
            k: 2,
            max_steps: 2,
            max_probes: 6,
        };
        let request =
            RecursionRequest::new("seq-1", &base, pins(), EvolutionMode::Sequential, budget, 0);
        let executor = RecordedProbe::new(vec![
            reading("seq-1-step-1-reference", ProbeSide::Reference, "base-1"),
            reading(
                "seq-1-step-1-c1-candidate",
                ProbeSide::Candidate,
                "seq-1-step-1-c1",
            ),
        ]);
        let runner = ExperimentRunner {
            executor: &executor,
            proposer: &ContextProposer,
            contrastor: &FixedContrastor,
            composer: &StepComposer,
            validator: &FixedValidator::incomplete(),
        };
        let outcome = run_recursion(&request, &mut catalog, &runner).expect("recursion completes");
        assert_eq!(
            outcome.steps.len(),
            1,
            "the chain stops after an unvalidated step"
        );
        assert!(!outcome.steps[0].adoptable);
        assert!(outcome.stopped_reason.is_some());
        assert_eq!(outcome.final_successor, None);
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1")
        );
    }

    #[test]
    fn a_budget_above_the_recursion_ceiling_is_refused_before_any_stage() {
        let base = base_revision();
        let mut catalog = catalog_with(&base);
        let budget = EvolutionBudget {
            k: RECURSION_K_CEILING + 1,
            max_steps: 1,
            max_probes: 100,
        };
        let request =
            RecursionRequest::new("rec-1", &base, pins(), EvolutionMode::Parallel, budget, 0);
        let executor = RecordedProbe::new(Vec::new());
        let validator = FixedValidator::clean();
        let runner = ExperimentRunner {
            executor: &executor,
            proposer: &TwoPerScopeProposer,
            contrastor: &FixedContrastor,
            composer: &FixedComposer,
            validator: &validator,
        };
        let error = run_recursion(&request, &mut catalog, &runner).unwrap_err();
        assert_eq!(error.code(), "invalid_budget");
        assert!(
            !executor.was_called(),
            "an over-budget request costs no probe"
        );
        assert_eq!(
            catalog.active_revision_id(HarnessDomain::RustBackend),
            Some("base-1")
        );
    }

    #[test]
    fn recursion_budget_shape_is_mode_specific() {
        // Parallel: one evidence step only.
        assert!(matches!(
            validate_recursion_budget(
                EvolutionMode::Parallel,
                &EvolutionBudget {
                    k: 2,
                    max_steps: 2,
                    max_probes: 10,
                },
            ),
            Err(ExperimentError::Evolution(EvolutionError::InvalidBudget(_)))
        ));
        // Parallel: the budget must cover 1 + 2k probes.
        assert!(matches!(
            validate_recursion_budget(
                EvolutionMode::Parallel,
                &EvolutionBudget {
                    k: 2,
                    max_steps: 1,
                    max_probes: 4,
                },
            ),
            Err(ExperimentError::ProbeBudgetExceeded {
                planned: 5,
                budget: 4
            })
        ));
        // Sequential: k is the step count.
        assert!(matches!(
            validate_recursion_budget(
                EvolutionMode::Sequential,
                &EvolutionBudget {
                    k: 1,
                    max_steps: 2,
                    max_probes: 6,
                },
            ),
            Err(ExperimentError::Evolution(EvolutionError::InvalidBudget(_)))
        ));
        // Sequential: 3 probes per step.
        assert!(matches!(
            validate_recursion_budget(
                EvolutionMode::Sequential,
                &EvolutionBudget {
                    k: 2,
                    max_steps: 2,
                    max_probes: 5,
                },
            ),
            Err(ExperimentError::ProbeBudgetExceeded {
                planned: 6,
                budget: 5
            })
        ));
        // The ceilings themselves are accepted.
        assert!(validate_recursion_budget(
            EvolutionMode::Sequential,
            &EvolutionBudget {
                k: 2,
                max_steps: 2,
                max_probes: 6,
            },
        )
        .is_ok());
    }
}
