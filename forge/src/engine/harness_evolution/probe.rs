//! Probe contract: what must be pinned before two executions can be compared, and the refusal
//! guards around running one.
//!
//! Harness² contrasts a reference execution of the base harness with a candidate execution of the
//! same task. The contrast is only meaningful when *everything except the harness revision* is
//! held fixed: task/benchmark, model, model policy, executor, tool surface, environment class and
//! input dataset. [`validate_matched_pair`] is that matched-condition check; a mismatch is a typed
//! refusal, never a "close enough" contrast.
//!
//! Two further refusals are structural, not conventions:
//!
//! * a probe never executes in a live production class ([`EnvironmentClass::allows_probe`]);
//! * the reference and candidate revisions must differ (a probe of a harness against itself is
//!   not a contrast).
//!
//! The model's [`ProbeSpec`] pins the execution identity. [`ProbeContract`] adds the recursion
//! identity Harness² also requires for a reproducible comparison — mode, budget (k, step ceiling,
//! probe ceiling) and seed — and hashes the whole thing into one digest. [`ProbeOutcome`] links
//! durable evidence ids only; it is not a second telemetry capture. [`RecordedProbe`] is the
//! deterministic executor double: same request, same recorded reading, no clocks, no randomness.

use std::cell::RefCell;
use std::fmt;

use sha2::{Digest, Sha256};

use super::model::{
    BehaviorEvidence, EnvironmentClass, EvolutionBudget, EvolutionError, EvolutionMode,
    HarnessRevision, ProbeSpec,
};

/// Domain-separation prefix for the probe-contract digest. Bumping it invalidates stored digests
/// on purpose: a silent encoding change would make two different probe identities collide.
pub const PROBE_DIGEST_DOMAIN: &str = "forge.harness_evolution.probe.v1";

/// Everything the executor must hold fixed across reference and candidate executions. Built once
/// per experiment and applied to every candidate, so no two probes in one experiment can drift.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbePins {
    pub task: String,
    pub model: String,
    pub model_policy: String,
    pub executor: String,
    pub tools: Vec<String>,
    pub environment_class: EnvironmentClass,
    pub input_dataset: String,
    pub test_plan: String,
    pub replay_id: String,
}

impl ProbePins {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        task: impl Into<String>,
        model: impl Into<String>,
        model_policy: impl Into<String>,
        executor: impl Into<String>,
        tools: Vec<String>,
        environment_class: EnvironmentClass,
        input_dataset: impl Into<String>,
        test_plan: impl Into<String>,
        replay_id: impl Into<String>,
    ) -> Self {
        Self {
            task: task.into(),
            model: model.into(),
            model_policy: model_policy.into(),
            executor: executor.into(),
            tools,
            environment_class,
            input_dataset: input_dataset.into(),
            test_plan: test_plan.into(),
            replay_id: replay_id.into(),
        }
    }

    /// The v1 default pins: a sandbox development execution of one task. Tests and the CLI
    /// override the model/executor/dataset; the environment class stays non-production.
    pub fn development(task: impl Into<String>) -> Self {
        Self::new(
            task,
            "deepseek-chat",
            "default",
            "opencode",
            Vec::new(),
            EnvironmentClass::Development,
            "forge-harness-evolution",
            "cargo test -p forge",
            "replay-harness-evolution",
        )
    }

    fn spec_for(&self, base_revision: &str, candidate_revision: &str) -> ProbeSpec {
        ProbeSpec::new(self.task.clone(), base_revision, candidate_revision)
            .with_model(self.model.clone())
            .with_model_policy(self.model_policy.clone())
            .with_executor(self.executor.clone())
            .with_tools(self.tools.clone())
            .with_environment_class(self.environment_class)
            .with_input_dataset(self.input_dataset.clone())
            .with_test_plan(self.test_plan.clone())
            .with_replay_id(self.replay_id.clone())
    }
}

/// Which side of the pair an execution is: the base harness or the candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProbeSide {
    Reference,
    Candidate,
}

impl ProbeSide {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::Candidate => "candidate",
        }
    }
}

impl fmt::Display for ProbeSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// One experiment's probe identity: base vs candidate plus the recursion identity. The pair's
/// [`ProbeSpec`] is derived from the pins, never hand-assembled by a caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeContract {
    pub experiment_id: String,
    pub base_revision: String,
    pub candidate_revision: String,
    pub pins: ProbePins,
    pub mode: EvolutionMode,
    pub budget: EvolutionBudget,
    pub seed: u64,
    spec: ProbeSpec,
}

impl ProbeContract {
    pub fn new(
        experiment_id: impl Into<String>,
        base: &HarnessRevision,
        candidate: &HarnessRevision,
        pins: ProbePins,
        mode: EvolutionMode,
        budget: EvolutionBudget,
        seed: u64,
    ) -> Result<Self, ProbeError> {
        base.verify_integrity()?;
        candidate.verify_integrity()?;
        if base.domain != candidate.domain {
            return Err(ProbeError::InvalidContract(format!(
                "base revision {} is {} but candidate {} is {}",
                base.id, base.domain, candidate.id, candidate.domain
            )));
        }
        if base.id == candidate.id {
            return Err(ProbeError::Evolution(EvolutionError::SameRevisionProbe {
                revision: base.id.clone(),
            }));
        }
        budget.validate()?;
        let spec = pins.spec_for(&base.id, &candidate.id);
        spec.validate_complete()?;
        Ok(Self {
            experiment_id: experiment_id.into(),
            base_revision: base.id.clone(),
            candidate_revision: candidate.id.clone(),
            pins,
            mode,
            budget,
            seed,
            spec,
        })
    }

    /// The model's pinned execution spec. Reference and candidate executions share this value:
    /// only the revision the executor runs differs.
    pub fn spec(&self) -> &ProbeSpec {
        &self.spec
    }

    pub fn reference_spec(&self) -> ProbeSpec {
        self.spec.clone()
    }

    pub fn candidate_spec(&self) -> ProbeSpec {
        self.spec.clone()
    }

    /// Fail closed before an executor is reached: complete pins, matched identity, non-production
    /// class, different revisions, valid recursion budget.
    pub fn validate(&self) -> Result<(), ProbeError> {
        self.spec.validate_complete()?;
        if self.base_revision == self.candidate_revision {
            return Err(ProbeError::Evolution(EvolutionError::SameRevisionProbe {
                revision: self.base_revision.clone(),
            }));
        }
        self.budget.validate()?;
        Ok(())
    }

    /// The reproducibility digest: every pin plus mode, k (and the probe ceilings) and seed.
    /// Two contracts with the same digest describe the same experiment; any pin or recursion
    /// identity change changes it.
    pub fn digest(&self) -> String {
        let mut out = Vec::new();
        push_field(&mut out, PROBE_DIGEST_DOMAIN);
        push_field(&mut out, &self.experiment_id);
        push_field(&mut out, &self.base_revision);
        push_field(&mut out, &self.candidate_revision);
        push_field(&mut out, &self.pins.task);
        push_field(&mut out, &self.pins.model);
        push_field(&mut out, &self.pins.model_policy);
        push_field(&mut out, &self.pins.executor);
        let mut tools = self.pins.tools.clone();
        tools.sort();
        tools.dedup();
        push_field(&mut out, &tools.join(","));
        push_field(&mut out, self.pins.environment_class.id());
        push_field(&mut out, &self.pins.input_dataset);
        push_field(&mut out, &self.pins.test_plan);
        push_field(&mut out, &self.pins.replay_id);
        push_field(&mut out, self.mode.id());
        push_bytes(&mut out, &self.budget.k.to_le_bytes());
        push_bytes(&mut out, &self.budget.max_steps.to_le_bytes());
        push_bytes(&mut out, &self.budget.max_probes.to_le_bytes());
        push_bytes(&mut out, &self.seed.to_le_bytes());
        sha256_hex(&out)
    }

    pub fn reference_request(&self, probe_id: impl Into<String>) -> ProbeRequest {
        self.request(ProbeSide::Reference, probe_id)
    }

    pub fn candidate_request(&self, probe_id: impl Into<String>) -> ProbeRequest {
        self.request(ProbeSide::Candidate, probe_id)
    }

    fn request(&self, side: ProbeSide, probe_id: impl Into<String>) -> ProbeRequest {
        ProbeRequest {
            probe_id: probe_id.into(),
            side,
            revision: match side {
                ProbeSide::Reference => self.base_revision.clone(),
                ProbeSide::Candidate => self.candidate_revision.clone(),
            },
            contract_digest: self.digest(),
            spec: self.spec.clone(),
        }
    }
}

fn push_field(out: &mut Vec<u8>, value: &str) {
    push_bytes(out, value.as_bytes());
}

fn push_bytes(out: &mut Vec<u8>, value: &[u8]) {
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value);
}

fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        // Writing to a String cannot fail; avoid unwrap/expect in library code.
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// One executor request. `revision` is the harness the executor must run; `spec` is the pinned
/// identity, which is identical for both sides of a pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeRequest {
    pub probe_id: String,
    pub side: ProbeSide,
    pub revision: String,
    pub contract_digest: String,
    pub spec: ProbeSpec,
}

/// The durable result of one probe execution. It links evidence ids only — it carries no token,
/// cost or timing copy (that telemetry stays in Forge's evidence rows, addressed by these ids).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeOutcome {
    pub probe_id: String,
    pub side: ProbeSide,
    pub revision: String,
    pub contract_digest: String,
    /// Id of the durable `BehaviorEvidence` row/receipt for this execution.
    pub evidence_id: String,
    /// Forge evidence ids this execution points at (execution row, trajectory, QA receipt, ...).
    pub evidence_refs: Vec<String>,
}

/// A probe outcome plus the typed behavioral reading needed to contrast it. The reading is a
/// reference model: it names evidence ids and carries counts, not a telemetry dump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeReading {
    pub outcome: ProbeOutcome,
    pub behavior: BehaviorEvidence,
}

impl ProbeReading {
    /// Pair an outcome with its reading. The evidence id and the evidence links must agree, and
    /// at least one durable reference must exist — a probe that cannot be traced is not evidence.
    pub fn new(outcome: ProbeOutcome, behavior: BehaviorEvidence) -> Result<Self, ProbeError> {
        if outcome.probe_id != behavior.probe_id {
            return Err(ProbeError::ReadingMismatch {
                probe_id: outcome.probe_id.clone(),
                message: format!(
                    "outcome names probe {}, but the behavior reading names probe {}",
                    outcome.probe_id, behavior.probe_id
                ),
            });
        }
        if outcome.evidence_id != behavior.evidence_id {
            return Err(ProbeError::ReadingMismatch {
                probe_id: outcome.probe_id,
                message: format!(
                    "outcome evidence {} does not match reading evidence {}",
                    outcome.evidence_id, behavior.evidence_id
                ),
            });
        }
        if outcome.evidence_refs != behavior.evidence_refs {
            return Err(ProbeError::ReadingMismatch {
                probe_id: outcome.probe_id,
                message: "outcome evidence refs do not match the reading's".into(),
            });
        }
        if outcome.evidence_refs.is_empty() {
            return Err(ProbeError::MissingEvidence {
                probe_id: outcome.probe_id,
            });
        }
        Ok(Self { outcome, behavior })
    }

    pub fn probe_id(&self) -> &str {
        &self.outcome.probe_id
    }

    pub fn evidence_refs(&self) -> &[String] {
        &self.outcome.evidence_refs
    }
}

/// The executor seam. Production wires a Forge-backed executor that records evidence rows; tests
/// wire [`RecordedProbe`]. The trait is deliberately small: a probe executor runs one pinned
/// execution and returns its durable links plus reading.
pub trait ProbeExecutor {
    fn execute(&self, request: &ProbeRequest) -> Result<ProbeReading, ProbeError>;
}

/// The deterministic test double: a fixed set of recorded readings, keyed by probe id.
///
/// Same request in, same reading out, and the calls are recorded so a test can assert that a
/// policy refusal happened *before* any probe was reached.
#[derive(Debug, Default)]
pub struct RecordedProbe {
    readings: Vec<ProbeReading>,
    calls: RefCell<Vec<String>>,
}

impl RecordedProbe {
    pub fn new(readings: Vec<ProbeReading>) -> Self {
        Self {
            readings,
            calls: RefCell::new(Vec::new()),
        }
    }

    pub fn with_reading(mut self, reading: ProbeReading) -> Self {
        self.readings.push(reading);
        self
    }

    /// Probe ids in execution order. Empty proves the executor was never reached.
    pub fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }

    pub fn was_called(&self) -> bool {
        !self.calls.borrow().is_empty()
    }
}

impl ProbeExecutor for RecordedProbe {
    fn execute(&self, request: &ProbeRequest) -> Result<ProbeReading, ProbeError> {
        self.calls.borrow_mut().push(request.probe_id.clone());
        let reading = self
            .readings
            .iter()
            .find(|reading| reading.outcome.probe_id == request.probe_id)
            .cloned()
            .ok_or_else(|| ProbeError::NotRecorded {
                probe_id: request.probe_id.clone(),
            })?;
        verify_reading_matches_request(request, &reading)?;
        Ok(reading)
    }
}

pub fn verify_reading_matches_request(
    request: &ProbeRequest,
    reading: &ProbeReading,
) -> Result<(), ProbeError> {
    if reading.outcome.side != request.side {
        return Err(ProbeError::SideMismatch {
            probe_id: request.probe_id.clone(),
            expected: request.side,
            actual: reading.outcome.side,
        });
    }
    if reading.outcome.revision != request.revision {
        return Err(ProbeError::RevisionMismatch {
            probe_id: request.probe_id.clone(),
            expected: request.revision.clone(),
            actual: reading.outcome.revision.clone(),
        });
    }
    // A reading is only usable for the probe it belongs to: neither the outcome nor its
    // behavior reading may name a different probe than the one that was requested.
    if reading.outcome.probe_id != request.probe_id
        || reading.behavior.probe_id != request.probe_id
    {
        return Err(ProbeError::ReadingMismatch {
            probe_id: request.probe_id.clone(),
            message: format!(
                "reading is attributed to probe {} (behavior {}), not the requested probe {}",
                reading.outcome.probe_id, reading.behavior.probe_id, request.probe_id
            ),
        });
    }
    if reading.behavior.evidence_id != reading.outcome.evidence_id
        || reading.behavior.evidence_refs != reading.outcome.evidence_refs
    {
        return Err(ProbeError::ReadingMismatch {
            probe_id: request.probe_id.clone(),
            message: "behavior reading and probe outcome disagree on evidence links".into(),
        });
    }
    if !reading.outcome.contract_digest.is_empty()
        && reading.outcome.contract_digest != request.contract_digest
    {
        return Err(ProbeError::DigestMismatch {
            probe_id: request.probe_id.clone(),
            expected: request.contract_digest.clone(),
            actual: reading.outcome.contract_digest.clone(),
        });
    }
    if reading.outcome.evidence_refs.is_empty() {
        return Err(ProbeError::MissingEvidence {
            probe_id: request.probe_id.clone(),
        });
    }
    Ok(())
}

/// The matched-condition check Harness² requires: same task, model, model policy, executor,
/// tools, environment class and input dataset; same base/candidate pairing; different harness
/// revisions. Anything else is refused before a contrast can be computed.
pub fn validate_matched_pair(
    reference: &ProbeSpec,
    candidate: &ProbeSpec,
) -> Result<(), ProbeError> {
    reference.validate_complete()?;
    candidate.validate_complete()?;

    if reference.task != candidate.task {
        return Err(ProbeError::Evolution(EvolutionError::InvalidProbe(format!(
            "reference probe task {} differs from candidate probe task {}",
            reference.task, candidate.task
        ))));
    }
    if reference.test_plan != candidate.test_plan {
        return Err(ProbeError::Evolution(EvolutionError::InvalidProbe(
            "reference and candidate probes must share one test plan".into(),
        )));
    }
    if reference.replay_id != candidate.replay_id {
        return Err(ProbeError::Evolution(EvolutionError::InvalidProbe(
            "reference and candidate probes must share one replay id".into(),
        )));
    }
    if reference.base_revision != candidate.base_revision
        || reference.candidate_revision != candidate.candidate_revision
    {
        return Err(ProbeError::Evolution(EvolutionError::InvalidProbe(
            "reference and candidate probes name different revisions".into(),
        )));
    }
    if reference.base_revision == reference.candidate_revision {
        return Err(ProbeError::Evolution(EvolutionError::SameRevisionProbe {
            revision: reference.base_revision.clone(),
        }));
    }
    if !reference.identity().matches(&candidate.identity()) {
        return Err(ProbeError::Evolution(EvolutionError::ProbeIdentityMismatch {
            reference: reference.identity().summary(),
            candidate: candidate.identity().summary(),
        }));
    }
    Ok(())
}

/// Execute both sides of one candidate pair. The contract is validated first; a pair that is not
/// matched never reaches the executor.
pub fn run_pair(
    executor: &dyn ProbeExecutor,
    contract: &ProbeContract,
    reference_probe_id: &str,
    candidate_probe_id: &str,
) -> Result<ProbePairReading, ProbeError> {
    contract.validate()?;
    validate_matched_pair(&contract.reference_spec(), &contract.candidate_spec())?;

    let reference = executor.execute(&contract.reference_request(reference_probe_id))?;
    let candidate = executor.execute(&contract.candidate_request(candidate_probe_id))?;
    verify_reading_matches_request(
        &contract.reference_request(reference_probe_id),
        &reference,
    )?;
    verify_reading_matches_request(
        &contract.candidate_request(candidate_probe_id),
        &candidate,
    )?;
    Ok(ProbePairReading {
        contract_digest: contract.digest(),
        reference,
        candidate,
    })
}

/// Both sides of one matched probe pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbePairReading {
    pub contract_digest: String,
    pub reference: ProbeReading,
    pub candidate: ProbeReading,
}

/// Why a probe could not run or could not be used. Every variant is a refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeError {
    Evolution(EvolutionError),
    InvalidContract(String),
    NotRecorded {
        probe_id: String,
    },
    SideMismatch {
        probe_id: String,
        expected: ProbeSide,
        actual: ProbeSide,
    },
    RevisionMismatch {
        probe_id: String,
        expected: String,
        actual: String,
    },
    DigestMismatch {
        probe_id: String,
        expected: String,
        actual: String,
    },
    MissingEvidence {
        probe_id: String,
    },
    ReadingMismatch {
        probe_id: String,
        message: String,
    },
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Evolution(error) => write!(f, "{error}"),
            Self::InvalidContract(message) => write!(f, "invalid probe contract: {message}"),
            Self::NotRecorded { probe_id } => {
                write!(f, "no recorded reading for probe {probe_id}")
            }
            Self::SideMismatch {
                probe_id,
                expected,
                actual,
            } => write!(
                f,
                "probe {probe_id} ran as {actual}, but the request was the {expected} side"
            ),
            Self::RevisionMismatch {
                probe_id,
                expected,
                actual,
            } => write!(
                f,
                "probe {probe_id} ran revision {actual}, not the pinned {expected}"
            ),
            Self::DigestMismatch {
                probe_id,
                expected,
                actual,
            } => write!(
                f,
                "probe {probe_id} identity digest {actual} does not match the contract digest {expected}"
            ),
            Self::MissingEvidence { probe_id } => {
                write!(f, "probe {probe_id} links no durable evidence")
            }
            Self::ReadingMismatch { probe_id, message } => {
                write!(f, "probe {probe_id} reading is inconsistent: {message}")
            }
        }
    }
}

impl std::error::Error for ProbeError {}

impl From<EvolutionError> for ProbeError {
    fn from(error: EvolutionError) -> Self {
        Self::Evolution(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::harness_evolution::model::{
        Component, ComponentKind, HarnessDomain, HarnessManifest, QaVerdict,
    };

    fn revision(id: &str, content: &str) -> HarnessRevision {
        HarnessRevision::draft(
            id,
            HarnessDomain::RustBackend,
            HarnessManifest::from_components([Component::green(
                ComponentKind::SystemPrompt,
                content,
                "probe test prompt",
            )]),
            "owner",
        )
    }

    fn contract() -> ProbeContract {
        let base = revision("rev-1", "base prompt");
        let candidate = revision("rev-2", "candidate prompt");
        ProbeContract::new(
            "exp-1",
            &base,
            &candidate,
            ProbePins::development("task-1"),
            EvolutionMode::Parallel,
            EvolutionBudget::k1(),
            7,
        )
        .expect("contract")
    }

    fn reading(probe_id: &str, side: ProbeSide, revision_id: &str, digest: &str) -> ProbeReading {
        let mut behavior = BehaviorEvidence::new(format!("evidence-{probe_id}"), probe_id);
        behavior.success = true;
        behavior.tests_passed = 3;
        behavior.qa = QaVerdict::Pass;
        behavior.evidence_refs = vec![format!("exec-{probe_id}"), format!("qa-{probe_id}")];
        let outcome = ProbeOutcome {
            probe_id: probe_id.to_string(),
            side,
            revision: revision_id.to_string(),
            contract_digest: digest.to_string(),
            evidence_id: behavior.evidence_id.clone(),
            evidence_refs: behavior.evidence_refs.clone(),
        };
        ProbeReading::new(outcome, behavior).expect("reading")
    }

    #[test]
    fn a_matched_pair_is_accepted() {
        let contract = contract();
        validate_matched_pair(&contract.reference_spec(), &contract.candidate_spec())
            .expect("matched pair");
        contract.validate().expect("valid contract");
    }

    #[test]
    fn every_identity_pin_mismatch_invalidates_the_pair() {
        let contract = contract();
        let reference = contract.reference_spec();
        type SpecMutation = Box<dyn Fn(&mut ProbeSpec)>;
        let mutations: Vec<(&str, SpecMutation)> = vec![
            ("task", Box::new(|spec: &mut ProbeSpec| spec.task = "other".into())),
            (
                "model",
                Box::new(|spec: &mut ProbeSpec| spec.model = "other-model".into()),
            ),
            (
                "model policy",
                Box::new(|spec: &mut ProbeSpec| spec.model_policy = "other".into()),
            ),
            (
                "executor",
                Box::new(|spec: &mut ProbeSpec| spec.executor = "codex".into()),
            ),
            (
                "tools",
                Box::new(|spec: &mut ProbeSpec| spec.tools = vec!["shell".into()]),
            ),
            (
                "environment",
                Box::new(|spec: &mut ProbeSpec| spec.environment_class = EnvironmentClass::Ci),
            ),
            (
                "dataset",
                Box::new(|spec: &mut ProbeSpec| spec.input_dataset = "other".into()),
            ),
            (
                "test plan",
                Box::new(|spec: &mut ProbeSpec| spec.test_plan = "other".into()),
            ),
            (
                "replay",
                Box::new(|spec: &mut ProbeSpec| spec.replay_id = "other".into()),
            ),
        ];
        for (name, mutate) in mutations {
            let mut candidate = reference.clone();
            mutate(&mut candidate);
            let error = validate_matched_pair(&reference, &candidate)
                .expect_err(&format!("{name} mismatch must be refused"));
            assert!(
                matches!(error, ProbeError::Evolution(_)),
                "{name} produced the wrong refusal: {error}"
            );
        }
    }

    #[test]
    fn a_production_environment_is_refused() {
        let base = revision("rev-1", "base");
        let candidate = revision("rev-2", "candidate");
        let mut pins = ProbePins::development("task-1");
        pins.environment_class = EnvironmentClass::Production;
        let error = ProbeContract::new(
            "exp-1",
            &base,
            &candidate,
            pins,
            EvolutionMode::Parallel,
            EvolutionBudget::k1(),
            7,
        )
        .expect_err("production must be refused");
        match error {
            ProbeError::Evolution(EvolutionError::ProductionProbeForbidden { .. }) => {}
            other => panic!("expected production refusal, got {other}"),
        }
    }

    #[test]
    fn a_same_revision_probe_is_refused() {
        let spec = ProbeSpec::new("task-1", "rev-1", "rev-1")
            .with_model("model")
            .with_executor("opencode")
            .with_input_dataset("dataset")
            .with_test_plan("plan")
            .with_replay_id("replay");
        let other = spec.clone();
        let error = validate_matched_pair(&spec, &other).expect_err("same revision refused");
        assert!(
            matches!(
                error,
                ProbeError::Evolution(EvolutionError::SameRevisionProbe { .. })
            ),
            "wrong refusal: {error}"
        );
    }

    #[test]
    fn the_contract_digest_pins_the_recursion_identity() {
        let base = revision("rev-1", "base");
        let candidate = revision("rev-2", "candidate");
        let make = |mode, budget, seed| {
            ProbeContract::new(
                "exp-1",
                &base,
                &candidate,
                ProbePins::development("task-1"),
                mode,
                budget,
                seed,
            )
            .expect("contract")
        };
        let one = make(EvolutionMode::Parallel, EvolutionBudget::k1(), 7);
        let same = make(EvolutionMode::Parallel, EvolutionBudget::k1(), 7);
        assert_eq!(one.digest(), same.digest(), "same identity, same digest");
        assert_ne!(
            one.digest(),
            make(EvolutionMode::Sequential, EvolutionBudget::k1(), 7).digest(),
            "mode is part of the identity"
        );
        assert_ne!(
            one.digest(),
            make(
                EvolutionMode::Parallel,
                EvolutionBudget {
                    k: 2,
                    max_steps: 1,
                    max_probes: 4,
                },
                7,
            )
            .digest(),
            "k is part of the identity"
        );
        assert_ne!(
            one.digest(),
            make(EvolutionMode::Parallel, EvolutionBudget::k1(), 8).digest(),
            "seed is part of the identity"
        );
    }

    #[test]
    fn an_invalid_budget_is_refused() {
        let base = revision("rev-1", "base");
        let candidate = revision("rev-2", "candidate");
        let error = ProbeContract::new(
            "exp-1",
            &base,
            &candidate,
            ProbePins::development("task-1"),
            EvolutionMode::Parallel,
            EvolutionBudget {
                k: 0,
                max_steps: 1,
                max_probes: 3,
            },
            0,
        )
        .expect_err("k=0 refused");
        assert!(matches!(error, ProbeError::Evolution(_)), "{error}");
    }

    #[test]
    fn recorded_probe_is_deterministic_and_records_calls() {
        let contract = contract();
        let reference_id = "exp-1-reference";
        let candidate_id = "exp-1-candidate";
        let digest = contract.digest();
        let recorded = RecordedProbe::new(vec![
            reading(reference_id, ProbeSide::Reference, "rev-1", &digest),
            reading(candidate_id, ProbeSide::Candidate, "rev-2", &digest),
        ]);

        let first = run_pair(&recorded, &contract, reference_id, candidate_id).expect("pair");
        let second = run_pair(&recorded, &contract, reference_id, candidate_id).expect("pair");
        assert_eq!(first, second, "same request, same reading");
        assert_eq!(
            recorded.calls(),
            vec![
                reference_id.to_string(),
                candidate_id.to_string(),
                reference_id.to_string(),
                candidate_id.to_string(),
            ]
        );
    }

    #[test]
    fn an_unrecorded_probe_is_refused_not_improvised() {
        let contract = contract();
        let recorded = RecordedProbe::new(Vec::new());
        let error = recorded
            .execute(&contract.reference_request("missing"))
            .expect_err("no recording");
        assert!(matches!(error, ProbeError::NotRecorded { .. }), "{error}");
    }

    #[test]
    fn a_reading_with_the_wrong_digest_is_refused() {
        let contract = contract();
        let recorded = RecordedProbe::new(vec![reading(
            "probe-1",
            ProbeSide::Reference,
            "rev-1",
            "not-the-digest",
        )]);
        let error = recorded
            .execute(&contract.reference_request("probe-1"))
            .expect_err("digest mismatch");
        assert!(matches!(error, ProbeError::DigestMismatch { .. }), "{error}");
    }

    #[test]
    fn an_outcome_links_evidence_ids_only_and_requires_at_least_one() {
        let contract = contract();
        let digest = contract.digest();
        let valid = reading("probe-1", ProbeSide::Reference, "rev-1", &digest);
        assert_eq!(
            valid.outcome.evidence_refs, valid.behavior.evidence_refs,
            "the outcome links the reading's evidence; it does not copy telemetry"
        );
        assert!(valid.outcome.evidence_refs.iter().all(|reference| !reference.is_empty()));

        let mut empty = valid.behavior.clone();
        empty.probe_id = "probe-2".into();
        empty.evidence_refs.clear();
        let outcome = ProbeOutcome {
            probe_id: "probe-2".into(),
            side: ProbeSide::Reference,
            revision: "rev-1".into(),
            contract_digest: digest,
            evidence_id: empty.evidence_id.clone(),
            evidence_refs: Vec::new(),
        };
        assert!(matches!(
            ProbeReading::new(outcome, empty),
            Err(ProbeError::MissingEvidence { .. })
        ));
    }

    #[test]
    fn a_reading_cannot_be_attributed_to_a_different_probe() {
        let mut behavior = BehaviorEvidence::new("evidence-1", "other-probe");
        behavior.evidence_refs = vec!["exec-1".into()];
        let outcome = ProbeOutcome {
            probe_id: "probe-1".into(),
            side: ProbeSide::Reference,
            revision: "rev-1".into(),
            contract_digest: String::new(),
            evidence_id: behavior.evidence_id.clone(),
            evidence_refs: behavior.evidence_refs.clone(),
        };
        let error = ProbeReading::new(outcome, behavior).expect_err("probe id mismatch");
        match error {
            ProbeError::ReadingMismatch { message, .. } => {
                assert!(message.contains("other-probe"), "{message}");
            }
            other => panic!("expected a probe attribution refusal, got {other}"),
        }
    }

    #[test]
    fn the_executor_refuses_a_reading_whose_behavior_names_another_probe() {
        let contract = contract();
        let mut behavior = BehaviorEvidence::new("evidence-1", "other-probe");
        behavior.evidence_refs = vec!["exec-1".into()];
        let reading = ProbeReading {
            outcome: ProbeOutcome {
                probe_id: "probe-1".into(),
                side: ProbeSide::Reference,
                revision: "rev-1".into(),
                contract_digest: String::new(),
                evidence_id: behavior.evidence_id.clone(),
                evidence_refs: behavior.evidence_refs.clone(),
            },
            behavior,
        };
        // Bypassing `ProbeReading::new` must not bypass the executor's check.
        let recorded = RecordedProbe::new(vec![reading]);
        let error = recorded
            .execute(&contract.reference_request("probe-1"))
            .expect_err("behavior names another probe");
        assert!(
            matches!(error, ProbeError::ReadingMismatch { .. }),
            "{error}"
        );
        assert!(!recorded.calls().is_empty());
    }

    #[test]
    fn run_pair_refuses_a_contract_whose_pins_are_incomplete() {
        let base = revision("rev-1", "base");
        let candidate = revision("rev-2", "candidate");
        let mut pins = ProbePins::development("task-1");
        pins.model.clear();
        let error = ProbeContract::new(
            "exp-1",
            &base,
            &candidate,
            pins,
            EvolutionMode::Parallel,
            EvolutionBudget::k1(),
            7,
        )
        .expect_err("incomplete pins");
        assert!(matches!(error, ProbeError::Evolution(_)), "{error}");
    }
}
