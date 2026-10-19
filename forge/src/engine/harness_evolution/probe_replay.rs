//! Replay ProbeExecutor — run a historical Forge task against two harness revisions in a
//! non-production sandbox.
//!
//! The experiment state machine is transport-agnostic: [`super::probe::ProbeExecutor`] runs one
//! pinned execution and returns durable evidence links. [`super::probe::RecordedProbe`] is the
//! deterministic test double; this module is the **real** replay executor for one historical
//! Forge story:
//!
//! ```text
//! story/run id
//!   -> HistoricalTaskLoader      (read DAO: storyboard_story_run + story packet snapshots)
//!   -> HistoricalTask            (packet text + durable evidence ids only)
//!   -> ReplayHarness             (immutable manifest content; authority is NOT executable)
//!   -> ReplaySandbox             (injected non-production runner)
//!   -> BehaviorEvidence          (counts + evidence ids; no telemetry copy)
//! ```
//!
//! Refusals are structural, not conventions:
//!
//! * a request whose environment class is production is refused **before** any loader or sandbox
//!   is touched ([`ReplayError::ProductionRefused`]);
//! * a request whose side/revision do not belong to the pinned reference/candidate pair is refused
//!   ([`ReplayError::UnmatchedPair`]) — as is a pair that failed
//!   [`super::probe::validate_matched_pair`];
//! * a sandbox reading with no durable evidence id/refs is refused
//!   ([`ReplayError::EvidenceMissing`]);
//! * the v1 default sandbox ([`UnavailableSandbox`]) refuses to execute anything: no live
//!   execution surface is wired until an operator injects one. Tests inject a deterministic fake.
//!
//! A historical task that originally ran in production is legitimate input — the replay executes
//! it in a sandbox, and the task's source environment is metadata, never the execution class.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;

use super::catalog::HarnessCatalog;
use super::contrast::contrast;
use super::model::{
    BehaviorDimension, BehaviorEvidence, ComponentKind, ContrastDirection, ContrastReport,
    DimensionContrast, EnvironmentClass, EvolutionBudget, EvolutionMode, HarnessRevision,
    ProbeSpec,
};
use super::model_turn_sandbox::USAGE_UNMEASURED_REF;
use super::policy;
use super::probe::{
    validate_matched_pair, ProbeContract, ProbeError, ProbeExecutor, ProbeOutcome,
    ProbePairReading, ProbePins, ProbeReading, ProbeRequest, ProbeSide,
};

/// Map the durable `storyboard_story_run.execution_environment` word to the model class.
///
/// An unknown or missing word maps to `Development`, never to `Production`: the mapping must not
/// be able to turn a typo into a production execution. `PROD` maps to `Production` precisely so
/// the replay refuses it.
pub fn environment_class_for(execution_environment: Option<&str>) -> EnvironmentClass {
    match execution_environment
        .map(|value| value.trim().to_ascii_uppercase())
        .as_deref()
    {
        Some("PROD") | Some("PRODUCTION") => EnvironmentClass::Production,
        Some("STAGING") => EnvironmentClass::Staging,
        Some("TEST") | Some("CI") => EnvironmentClass::Ci,
        Some("LOCAL") => EnvironmentClass::Sandbox,
        Some("DEV") | Some("DEVELOPMENT") => EnvironmentClass::Development,
        _ => EnvironmentClass::Development,
    }
}

/// Why a replay could not run or could not be used. Every variant is a refusal, and the typed
/// error is available to callers of [`ReplayProbeExecutor::execute_replay`]; the
/// [`ProbeExecutor`] impl wraps it into a [`ProbeError`] so the experiment seam stays unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    TaskNotFound {
        story_id: String,
        run_id: Option<String>,
    },
    LoaderUnavailable {
        message: String,
    },
    InvalidTask {
        message: String,
    },
    UnknownRevision {
        revision_id: String,
    },
    HarnessMaterialization {
        revision: String,
        message: String,
    },
    ProductionRefused {
        environment_class: EnvironmentClass,
    },
    UnmatchedPair {
        message: String,
    },
    SandboxRefused {
        sandbox: String,
        message: String,
    },
    SandboxFailed {
        probe_id: String,
        message: String,
    },
    EvidenceMissing {
        probe_id: String,
        message: String,
    },
    SandboxReadingRejected {
        probe_id: String,
        message: String,
    },
    /// The two readings could not be contrasted (missing/shared evidence, identity drift, ...).
    ContrastRefused {
        message: String,
    },
    /// The pre-probe policy + lineage gate refused the candidate before any execution.
    PolicyRefused {
        message: String,
    },
}

impl ReplayError {
    /// Stable code for durable rows, logs and tests.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::TaskNotFound { .. } => "replay_task_not_found",
            Self::LoaderUnavailable { .. } => "replay_loader_unavailable",
            Self::InvalidTask { .. } => "replay_invalid_task",
            Self::UnknownRevision { .. } => "replay_unknown_revision",
            Self::HarnessMaterialization { .. } => "replay_harness_materialization",
            Self::ProductionRefused { .. } => "replay_production_refused",
            Self::UnmatchedPair { .. } => "replay_unmatched_pair",
            Self::SandboxRefused { .. } => "replay_sandbox_refused",
            Self::SandboxFailed { .. } => "replay_sandbox_failed",
            Self::EvidenceMissing { .. } => "replay_evidence_missing",
            Self::SandboxReadingRejected { .. } => "replay_reading_rejected",
            Self::ContrastRefused { .. } => "replay_contrast_refused",
            Self::PolicyRefused { .. } => "replay_policy_refused",
        }
    }
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TaskNotFound { story_id, run_id } => match run_id {
                Some(run_id) => write!(
                    f,
                    "historical Forge task not found for story {story_id} run {run_id}"
                ),
                None => write!(f, "historical Forge task not found for story {story_id}"),
            },
            Self::LoaderUnavailable { message } => {
                write!(f, "historical task loader unavailable: {message}")
            }
            Self::InvalidTask { message } => write!(f, "historical task is unusable: {message}"),
            Self::UnknownRevision { revision_id } => {
                write!(
                    f,
                    "harness revision {revision_id} is not available to the replay"
                )
            }
            Self::HarnessMaterialization { revision, message } => {
                write!(
                    f,
                    "cannot materialize harness revision {revision}: {message}"
                )
            }
            Self::ProductionRefused { environment_class } => write!(
                f,
                "replay refuses the {} environment; probes never execute in production",
                environment_class.id()
            ),
            Self::UnmatchedPair { message } => {
                write!(
                    f,
                    "replay refused an unmatched reference/candidate pair: {message}"
                )
            }
            Self::SandboxRefused { sandbox, message } => {
                write!(f, "sandbox {sandbox} refused to execute: {message}")
            }
            Self::SandboxFailed { probe_id, message } => {
                write!(f, "sandbox failed probe {probe_id}: {message}")
            }
            Self::EvidenceMissing { probe_id, message } => {
                write!(f, "probe {probe_id} has no durable evidence: {message}")
            }
            Self::SandboxReadingRejected { probe_id, message } => write!(
                f,
                "probe {probe_id} sandbox reading is inconsistent: {message}"
            ),
            Self::ContrastRefused { message } => {
                write!(f, "replay contrast was refused: {message}")
            }
            Self::PolicyRefused { message } => {
                write!(
                    f,
                    "replay policy/lineage gate refused the candidate: {message}"
                )
            }
        }
    }
}

impl std::error::Error for ReplayError {}

impl From<ReplayError> for ProbeError {
    fn from(error: ReplayError) -> Self {
        match error {
            ReplayError::ProductionRefused { environment_class } => {
                Self::Evolution(super::model::EvolutionError::ProductionProbeForbidden {
                    environment_class,
                })
            }
            ReplayError::EvidenceMissing { probe_id, .. } => Self::MissingEvidence { probe_id },
            ReplayError::SandboxRefused { .. }
            | ReplayError::SandboxFailed { .. }
            | ReplayError::SandboxReadingRejected { .. } => Self::ReadingMismatch {
                probe_id: String::new(),
                message: error.to_string(),
            },
            _ => Self::InvalidContract(error.to_string()),
        }
    }
}

/// One historical Forge task, loaded from the durable control plane.
///
/// `evidence_refs` point at durable ids (`storyboard_story_run`, `storyboard_story`, the packet
/// snapshot fingerprint). The packet text is reproduced input, not telemetry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalTask {
    pub story_id: String,
    pub run_id: String,
    pub title: String,
    pub packet: String,
    pub run_type: Option<String>,
    pub agent_runtime: Option<String>,
    pub result_status: Option<String>,
    /// The environment the historical run executed in. Metadata only: the replay's execution class
    /// comes from the probe contract and is always non-production.
    pub source_environment: EnvironmentClass,
    pub packet_sha: Option<String>,
    pub evidence_refs: Vec<String>,
}

impl HistoricalTask {
    pub fn new(
        story_id: impl Into<String>,
        run_id: impl Into<String>,
        title: impl Into<String>,
        packet: impl Into<String>,
    ) -> Self {
        let mut task = Self {
            story_id: story_id.into(),
            run_id: run_id.into(),
            title: title.into(),
            packet: packet.into(),
            run_type: None,
            agent_runtime: None,
            result_status: None,
            source_environment: EnvironmentClass::Development,
            packet_sha: None,
            evidence_refs: Vec::new(),
        };
        task.rebuild_evidence_refs();
        task
    }

    pub fn with_source_environment(mut self, environment_class: EnvironmentClass) -> Self {
        self.source_environment = environment_class;
        self
    }

    pub fn with_run_metadata(
        mut self,
        run_type: Option<String>,
        agent_runtime: Option<String>,
        result_status: Option<String>,
    ) -> Self {
        self.run_type = run_type;
        self.agent_runtime = agent_runtime;
        self.result_status = result_status;
        self
    }

    pub fn with_packet_sha(mut self, packet_sha: Option<String>) -> Self {
        self.packet_sha = packet_sha.filter(|sha| !sha.trim().is_empty());
        self.rebuild_evidence_refs();
        self
    }

    /// A task without identity or without packet text cannot be replayed.
    pub fn validate(&self) -> Result<(), ReplayError> {
        if self.story_id.trim().is_empty() {
            return Err(ReplayError::InvalidTask {
                message: "historical task has no story id".into(),
            });
        }
        if self.run_id.trim().is_empty() {
            return Err(ReplayError::InvalidTask {
                message: "historical task has no run id".into(),
            });
        }
        if self.packet.trim().is_empty() {
            return Err(ReplayError::InvalidTask {
                message: format!(
                    "historical task {} has no packet text to replay",
                    self.run_id
                ),
            });
        }
        Ok(())
    }

    fn rebuild_evidence_refs(&mut self) {
        let mut refs = vec![
            format!("storyboard_story_run:{}", self.run_id),
            format!("storyboard_story:{}", self.story_id),
        ];
        if let Some(sha) = self
            .packet_sha
            .as_deref()
            .map(str::trim)
            .filter(|sha| !sha.is_empty())
        {
            refs.push(format!("storyboard_story_run:{}:packet:{sha}", self.run_id));
        }
        self.evidence_refs = refs;
    }
}

/// Which story/run a replay must load. `run_id = None` selects the story's newest run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalTaskRequest {
    pub story_id: String,
    pub run_id: Option<String>,
}

impl HistoricalTaskRequest {
    pub fn new(story_id: impl Into<String>, run_id: Option<String>) -> Self {
        Self {
            story_id: story_id.into(),
            run_id,
        }
    }

    pub fn for_story(story_id: impl Into<String>) -> Self {
        Self::new(story_id, None)
    }
}

/// The historical-task load seam. Production wires [`ForgeHistoricalTaskLoader`]; tests inject a
/// stub that returns a deterministic task (no database).
pub trait HistoricalTaskLoader {
    fn load(&self, request: &HistoricalTaskRequest) -> Result<HistoricalTask, ReplayError>;
}

/// The Forge-backed loader: ONE narrow read of the control plane, no writes, no mutation.
///
/// It uses `ForgeReadDao::historical_harness_task`, whose packet assembly prefers the run's own
/// specification snapshots over the story's current columns.
#[derive(Debug, Clone, Copy, Default)]
pub struct ForgeHistoricalTaskLoader;

impl HistoricalTaskLoader for ForgeHistoricalTaskLoader {
    fn load(&self, request: &HistoricalTaskRequest) -> Result<HistoricalTask, ReplayError> {
        let row = crate::engine::vendor_session::with_shared(|db, runtime| {
            let dao = db::forge_read::ForgeReadDao::new(db.clone());
            runtime.block_on(async {
                dao.historical_harness_task(&request.story_id, request.run_id.as_deref())
                    .await
            })
        })
        .map_err(|message| ReplayError::LoaderUnavailable { message })?
        .map_err(|error| ReplayError::LoaderUnavailable {
            message: error.to_string(),
        })?
        .ok_or_else(|| ReplayError::TaskNotFound {
            story_id: request.story_id.clone(),
            run_id: request.run_id.clone(),
        })?;

        let task = HistoricalTask::new(row.story_id, row.run_id, row.title, row.packet)
            .with_source_environment(environment_class_for(row.execution_environment.as_deref()))
            .with_run_metadata(row.run_type, row.agent_runtime, row.result_status)
            .with_packet_sha(row.packet_sha);
        task.validate()?;
        Ok(task)
    }
}

/// Where the replay gets an immutable harness revision by id. Production wires the catalog;
/// tests wire a small map.
pub trait RevisionSource {
    fn revision(&self, revision_id: &str) -> Result<HarnessRevision, ReplayError>;
}

/// An in-memory revision source (tests, and a catalog-free replay).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapRevisionSource {
    revisions: BTreeMap<String, HarnessRevision>,
}

impl MapRevisionSource {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, revision: HarnessRevision) -> Result<(), ReplayError> {
        revision
            .verify_integrity()
            .map_err(|error| ReplayError::HarnessMaterialization {
                revision: revision.id.clone(),
                message: error.to_string(),
            })?;
        self.revisions.insert(revision.id.clone(), revision);
        Ok(())
    }

    pub fn from_revisions<I>(revisions: I) -> Result<Self, ReplayError>
    where
        I: IntoIterator<Item = HarnessRevision>,
    {
        let mut source = Self::new();
        for revision in revisions {
            source.insert(revision)?;
        }
        Ok(source)
    }
}

impl RevisionSource for MapRevisionSource {
    fn revision(&self, revision_id: &str) -> Result<HarnessRevision, ReplayError> {
        self.revisions
            .get(revision_id)
            .cloned()
            .ok_or_else(|| ReplayError::UnknownRevision {
                revision_id: revision_id.to_string(),
            })
    }
}

/// The catalog is the production revision source: one read, cloned, integrity re-checked by
/// [`ReplayHarness::materialize`].
impl RevisionSource for HarnessCatalog {
    fn revision(&self, revision_id: &str) -> Result<HarnessRevision, ReplayError> {
        HarnessCatalog::revision(self, revision_id)
            .cloned()
            .ok_or_else(|| ReplayError::UnknownRevision {
                revision_id: revision_id.to_string(),
            })
    }
}

/// A harness materialized for execution: **component content only**.
///
/// Authority classes, lifecycle status, parent links and timestamps are deliberately absent. A
/// sandbox cannot be handed an authority field and cannot execute one; it receives the text each
/// component contributes. The manifest hash travels so a reading can be attributed to the exact
/// immutable revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayHarness {
    pub revision: String,
    pub manifest_hash: String,
    pub components: BTreeMap<ComponentKind, String>,
}

impl ReplayHarness {
    pub fn materialize(revision: &HarnessRevision) -> Result<Self, ReplayError> {
        revision
            .verify_integrity()
            .map_err(|error| ReplayError::HarnessMaterialization {
                revision: revision.id.clone(),
                message: error.to_string(),
            })?;
        let components = revision
            .manifest
            .iter()
            .map(|(kind, component)| (*kind, component.content.clone()))
            .collect();
        Ok(Self {
            revision: revision.id.clone(),
            manifest_hash: revision.manifest_hash.clone(),
            components,
        })
    }

    /// The content this sandbox would receive for one component, if present.
    pub fn content(&self, kind: ComponentKind) -> Option<&str> {
        self.components.get(&kind).map(String::as_str)
    }
}

/// One sandbox execution request. Everything the sandbox may know: the historical task, the
/// content-only harness, and the pinned identity. No authority field is present by construction.
#[derive(Debug)]
pub struct SandboxRequest<'a> {
    pub probe_id: &'a str,
    pub side: ProbeSide,
    pub environment_class: EnvironmentClass,
    pub task: &'a HistoricalTask,
    pub harness: &'a ReplayHarness,
    pub model: &'a str,
    pub model_policy: &'a str,
    pub executor: &'a str,
    pub tools: &'a [String],
    pub test_plan: &'a str,
}

/// What a sandbox observed. The sandbox fills the behavioral counts and names the durable evidence
/// it produced; the executor stamps the probe id and merges the task's durable refs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxReading {
    pub behavior: BehaviorEvidence,
}

impl SandboxReading {
    pub fn new(behavior: BehaviorEvidence) -> Self {
        Self { behavior }
    }
}

/// The sandbox seam. Implementations execute the harness against the historical task in a
/// non-production environment and return references to durable evidence rows — never a telemetry
/// copy, and never a publication/commit.
pub trait ReplaySandbox {
    fn run(&self, request: &SandboxRequest<'_>) -> Result<SandboxReading, ReplayError>;
}

/// Refuse a production (or otherwise non-probe) execution class. Sandbox implementations should
/// call this too; the executor calls it before the sandbox is reached.
pub fn assert_non_production_environment(
    environment_class: EnvironmentClass,
) -> Result<(), ReplayError> {
    if environment_class.is_production() || !environment_class.allows_probe() {
        return Err(ReplayError::ProductionRefused { environment_class });
    }
    Ok(())
}

/// The v1 default sandbox: it executes nothing.
///
/// No live non-production execution surface is wired in this batch. That is a *refusal*, not a
/// silent no-op: a caller that forgot to inject a sandbox gets a typed error instead of an empty
/// behavior reading that could look like a clean run.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnavailableSandbox;

impl ReplaySandbox for UnavailableSandbox {
    fn run(&self, request: &SandboxRequest<'_>) -> Result<SandboxReading, ReplayError> {
        assert_non_production_environment(request.environment_class)?;
        Err(ReplayError::SandboxRefused {
            sandbox: "unavailable".into(),
            message:
                "no non-production sandbox runtime is wired in this build; inject a ReplaySandbox"
                    .into(),
        })
    }
}

/// The replay [`ProbeExecutor`]. It loads the historical task once (cached), materializes the
/// requested revision's content, runs the injected sandbox, and returns a reading that links the
/// task's durable ids plus the ids the sandbox produced.
pub struct ReplayProbeExecutor<'a> {
    loader: &'a dyn HistoricalTaskLoader,
    sandbox: &'a dyn ReplaySandbox,
    revisions: &'a dyn RevisionSource,
    task_request: HistoricalTaskRequest,
    task: RefCell<Option<HistoricalTask>>,
}

impl<'a> ReplayProbeExecutor<'a> {
    pub fn new(
        loader: &'a dyn HistoricalTaskLoader,
        sandbox: &'a dyn ReplaySandbox,
        revisions: &'a dyn RevisionSource,
        task_request: HistoricalTaskRequest,
    ) -> Self {
        Self {
            loader,
            sandbox,
            revisions,
            task_request,
            task: RefCell::new(None),
        }
    }

    /// The historical task, loaded at most once so the reference and candidate executions compare
    /// the same durable input.
    pub fn load_task(&self) -> Result<HistoricalTask, ReplayError> {
        if let Some(task) = self.task.borrow().as_ref() {
            return Ok(task.clone());
        }
        let task = self.loader.load(&self.task_request)?;
        task.validate()?;
        *self.task.borrow_mut() = Some(task.clone());
        Ok(task)
    }

    /// Run one side of the pair with the typed error preserved.
    pub fn execute_replay(&self, request: &ProbeRequest) -> Result<ProbeReading, ReplayError> {
        assert_non_production_environment(request.spec.environment_class)?;
        check_request_belongs_to_pair(request)?;

        let task = self.load_task()?;
        let revision = self.revisions.revision(&request.revision)?;
        let harness = ReplayHarness::materialize(&revision)?;

        let sandbox_request = SandboxRequest {
            probe_id: &request.probe_id,
            side: request.side,
            environment_class: request.spec.environment_class,
            task: &task,
            harness: &harness,
            model: &request.spec.model,
            model_policy: &request.spec.model_policy,
            executor: &request.spec.executor,
            tools: &request.spec.tools,
            test_plan: &request.spec.test_plan,
        };
        let sandbox_reading = self.sandbox.run(&sandbox_request)?;
        finish_reading(request, &task, sandbox_reading)
    }

    /// Execute the matched reference/candidate pair. The pair is validated **before** the loader or
    /// sandbox is reached: a mismatched or production pair costs no execution.
    pub fn execute_pair(&self, contract: &ProbeContract) -> Result<ProbePairReading, ReplayError> {
        assert_non_production_environment(contract.spec().environment_class)?;
        validate_matched_pair(&contract.reference_spec(), &contract.candidate_spec()).map_err(
            |error| ReplayError::UnmatchedPair {
                message: error.to_string(),
            },
        )?;
        contract
            .validate()
            .map_err(|error| ReplayError::UnmatchedPair {
                message: error.to_string(),
            })?;

        let reference = self.execute_replay(
            &contract.reference_request(format!("{}-reference", contract.experiment_id)),
        )?;
        let candidate = self.execute_replay(
            &contract.candidate_request(format!("{}-candidate", contract.experiment_id)),
        )?;
        Ok(ProbePairReading {
            contract_digest: contract.digest(),
            reference,
            candidate,
        })
    }
}

impl ProbeExecutor for ReplayProbeExecutor<'_> {
    fn execute(&self, request: &ProbeRequest) -> Result<ProbeReading, ProbeError> {
        self.execute_replay(request).map_err(ProbeError::from)
    }
}

/// A request is only executable when its side and revision are the ones the pinned spec names.
/// This is the executor-side half of "refuse a non-matched pair": a hand-assembled request cannot
/// smuggle the base revision into the candidate slot.
fn check_request_belongs_to_pair(request: &ProbeRequest) -> Result<(), ReplayError> {
    request
        .spec
        .validate_complete()
        .map_err(|error| ReplayError::UnmatchedPair {
            message: error.to_string(),
        })?;
    if request.spec.base_revision == request.spec.candidate_revision {
        return Err(ReplayError::UnmatchedPair {
            message: "reference and candidate revisions are the same revision".into(),
        });
    }
    if request.probe_id.trim().is_empty() {
        return Err(ReplayError::UnmatchedPair {
            message: "probe request has no probe id".into(),
        });
    }
    let expected = match request.side {
        ProbeSide::Reference => &request.spec.base_revision,
        ProbeSide::Candidate => &request.spec.candidate_revision,
    };
    if &request.revision != expected {
        return Err(ReplayError::UnmatchedPair {
            message: format!(
                "request ran {} revision {}, but the pinned {} revision is {expected}",
                request.side, request.revision, request.side
            ),
        });
    }
    Ok(())
}

/// Stamp the probe/outcome identity onto the sandbox reading and link the task's durable evidence.
fn finish_reading(
    request: &ProbeRequest,
    task: &HistoricalTask,
    sandbox_reading: SandboxReading,
) -> Result<ProbeReading, ReplayError> {
    let mut behavior = sandbox_reading.behavior;

    if behavior.evidence_id.trim().is_empty() {
        return Err(ReplayError::EvidenceMissing {
            probe_id: request.probe_id.clone(),
            message: "sandbox returned no durable evidence id".into(),
        });
    }
    if behavior.evidence_refs.is_empty() {
        // The task's own refs prove the INPUT; the execution needs its own durable reference or
        // the reading cannot be traced to a run.
        return Err(ReplayError::EvidenceMissing {
            probe_id: request.probe_id.clone(),
            message: "sandbox returned no durable execution evidence refs".into(),
        });
    }
    if !behavior.probe_id.trim().is_empty() && behavior.probe_id != request.probe_id {
        return Err(ReplayError::SandboxReadingRejected {
            probe_id: request.probe_id.clone(),
            message: format!(
                "sandbox attributed its reading to probe {}, not the requested {}",
                behavior.probe_id, request.probe_id
            ),
        });
    }

    let mut evidence_refs: Vec<String> = task
        .evidence_refs
        .iter()
        .chain(behavior.evidence_refs.iter())
        .map(|reference| reference.trim().to_string())
        .filter(|reference| !reference.is_empty())
        .collect();
    evidence_refs.sort();
    evidence_refs.dedup();
    if evidence_refs.is_empty() {
        return Err(ReplayError::EvidenceMissing {
            probe_id: request.probe_id.clone(),
            message: "sandbox returned no durable evidence refs".into(),
        });
    }

    behavior.probe_id = request.probe_id.clone();
    behavior.evidence_refs = evidence_refs.clone();

    let outcome = ProbeOutcome {
        probe_id: request.probe_id.clone(),
        side: request.side,
        revision: request.revision.clone(),
        contract_digest: request.contract_digest.clone(),
        evidence_id: behavior.evidence_id.clone(),
        evidence_refs,
    };
    ProbeReading::new(outcome, behavior).map_err(|error| ReplayError::SandboxReadingRejected {
        probe_id: request.probe_id.clone(),
        message: error.to_string(),
    })
}

/// The pinned spec a test or CLI uses for a local replay: one task, development class, complete
/// pins. The environment class is explicit so a caller can never inherit "production by accident".
pub fn replay_spec_for(
    task_name: impl Into<String>,
    base_revision: &str,
    candidate_revision: &str,
    environment_class: EnvironmentClass,
) -> ProbeSpec {
    ProbeSpec::new(task_name, base_revision, candidate_revision)
        .with_model("deepseek-chat")
        .with_model_policy("default")
        .with_executor("opencode")
        .with_tools(Vec::new())
        .with_environment_class(environment_class)
        .with_input_dataset("forge-harness-evolution")
        .with_test_plan("cargo test -p forge")
        .with_replay_id("replay-harness-evolution")
}

/// One recorded replay probe request: the operator surface's O2 input.
///
/// It pins exactly what a reproducible probe needs (the [`ProbePins`] identity plus the recursion
/// mode/budget/seed) and names the two durable revisions under test. Nothing here grants authority:
/// the sandbox still executes in a non-production class and the executor still refuses production.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedReplayRequest {
    pub experiment_id: String,
    pub pins: ProbePins,
    pub mode: EvolutionMode,
    pub budget: EvolutionBudget,
    pub seed: u64,
    pub base_revision: String,
    pub candidate_revision: String,
    pub task_request: HistoricalTaskRequest,
}

/// What a dry run produces: the loaded task, both revisions and the pinned contract. No sandbox is
/// touched, nothing is written.
#[derive(Debug)]
pub struct RecordedReplayPlan {
    pub task: HistoricalTask,
    pub base: HarnessRevision,
    pub candidate: HarnessRevision,
    pub contract: ProbeContract,
}

impl RecordedReplayPlan {
    pub fn contract_digest(&self) -> String {
        self.contract.digest()
    }
}

/// What a completed replay produces: both readings and the behavioral contrast. The caller records
/// these rows through the durable DAO; this module never writes.
#[derive(Debug)]
pub struct RecordedReplayOutcome {
    pub task: HistoricalTask,
    pub base: HarnessRevision,
    pub candidate: HarnessRevision,
    pub contract_digest: String,
    /// The pinned spec both executions ran under (persisted as the probe row's `spec`).
    pub spec: ProbeSpec,
    pub reference: ProbeReading,
    pub candidate_reading: ProbeReading,
    pub contrast: ContrastReport,
}

/// The ONE pre-probe policy + lineage gate every probe path must pass before an execution.
///
/// Both the dry run (`plan_recorded_replay`) and the real run (CLI ordered runner) call this, so
/// they cannot drift: the candidate must be a direct child of the base revision, in the same
/// domain, and must pass `policy::validate_candidate` with the canonical base→candidate change
/// list (RED/authority/QA/protected-content changes are refused HERE, before any probe exists).
pub fn validate_probe_candidate(
    base: &HarnessRevision,
    candidate: &HarnessRevision,
) -> Result<(), ReplayError> {
    if candidate.parent_revision_id.as_deref() != Some(base.id.as_str()) {
        return Err(ReplayError::PolicyRefused {
            message: format!(
                "candidate {} must be a direct child of base {}; parent is {:?}",
                candidate.id, base.id, candidate.parent_revision_id
            ),
        });
    }
    if candidate.domain != base.domain {
        return Err(ReplayError::PolicyRefused {
            message: format!(
                "candidate {} is {}, but base {} is {}",
                candidate.id, candidate.domain, base.id, base.domain
            ),
        });
    }
    let changes = policy::changes_between(base, candidate);
    policy::validate_candidate(base, candidate, &changes).map_err(|violations| {
        ReplayError::PolicyRefused {
            message: violations
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; "),
        }
    })
}

fn recorded_contract(
    request: &RecordedReplayRequest,
    revisions: &dyn RevisionSource,
) -> Result<(HarnessRevision, HarnessRevision, ProbeContract), ReplayError> {
    // Production is refused before a revision is read, let alone a sandbox reached.
    assert_non_production_environment(request.pins.environment_class)?;
    let base = revisions.revision(&request.base_revision)?;
    let candidate = revisions.revision(&request.candidate_revision)?;
    // Shared pre-probe gate: policy + lineage BEFORE a contract or sandbox exists.
    validate_probe_candidate(&base, &candidate)?;
    let contract = ProbeContract::new(
        request.experiment_id.clone(),
        &base,
        &candidate,
        request.pins.clone(),
        request.mode,
        request.budget,
        request.seed,
    )
    .map_err(|error| ReplayError::UnmatchedPair {
        message: error.to_string(),
    })?;
    contract
        .validate()
        .map_err(|error| ReplayError::UnmatchedPair {
            message: error.to_string(),
        })?;
    Ok((base, candidate, contract))
}

/// Dry run: load the historical task and build the pinned contract, executing nothing.
pub fn plan_recorded_replay(
    loader: &dyn HistoricalTaskLoader,
    revisions: &dyn RevisionSource,
    request: &RecordedReplayRequest,
) -> Result<RecordedReplayPlan, ReplayError> {
    let (base, candidate, contract) = recorded_contract(request, revisions)?;
    let task = loader.load(&request.task_request)?;
    task.validate()?;
    Ok(RecordedReplayPlan {
        task,
        base,
        candidate,
        contract,
    })
}

impl RecordedReplayOutcome {
    /// True when either side's execution failed (`BehaviorEvidence.success == false`). A failed
    /// pair is still evidence, but it must be recorded as a failed experiment, not a comparison.
    pub fn probe_failed(&self) -> bool {
        !self.reference.behavior.success || !self.candidate_reading.behavior.success
    }
}

/// True when the sandbox explicitly labelled this execution's usage as unmeasured. The label lives
/// in the durable evidence refs, so the honesty rule survives persistence.
fn usage_unmeasured(reading: &ProbeReading) -> bool {
    reading
        .outcome
        .evidence_refs
        .iter()
        .any(|reference| reference == USAGE_UNMEASURED_REF)
}

/// Recompute the human summary from the (possibly normalized) dimensions.
fn summarize_dimensions(dimensions: &[DimensionContrast]) -> String {
    let mut improved = 0;
    let mut regressed = 0;
    let mut unchanged = 0;
    let mut unknown = 0;
    for contrast in dimensions {
        match contrast.direction {
            ContrastDirection::Improved => improved += 1,
            ContrastDirection::Regressed => regressed += 1,
            ContrastDirection::Unchanged => unchanged += 1,
            ContrastDirection::Unknown => unknown += 1,
        }
    }
    format!(
        "{improved} improved, {regressed} regressed, {unchanged} unchanged, {unknown} not captured"
    )
}

/// An unmeasured token count is NOT a zero reading. `BehaviorEvidence` carries `u64` token counts,
/// so the sandbox's `model-turn:usage:unmeasured` label is the only reliable signal; when it is
/// present on either side, the tokens dimension becomes `Unknown` with `None` values instead of a
/// fabricated `0.0` comparison.
fn normalize_unmeasured_usage(
    report: &mut ContrastReport,
    reference: &ProbeReading,
    candidate: &ProbeReading,
) {
    if !usage_unmeasured(reference) && !usage_unmeasured(candidate) {
        return;
    }
    for dimension in &mut report.dimensions {
        if dimension.dimension != BehaviorDimension::Tokens {
            continue;
        }
        dimension.direction = ContrastDirection::Unknown;
        dimension.reference_value = None;
        dimension.candidate_value = None;
        dimension.note =
            "usage unmeasured by the sandbox; token counts are not a reading".to_string();
    }
    report.summary = summarize_dimensions(&report.dimensions);
}

/// One complete recorded replay: both matched executions plus the behavioral contrast.
///
/// The pair is validated before the loader/sandbox runs (via `execute_pair`); a sandbox that
/// returns no durable evidence is refused by the executor; a pair whose readings cannot be
/// contrasted is a [`ReplayError::ContrastRefused`], not a silent skip.
pub fn run_recorded_replay(
    loader: &dyn HistoricalTaskLoader,
    sandbox: &dyn ReplaySandbox,
    revisions: &dyn RevisionSource,
    request: &RecordedReplayRequest,
) -> Result<RecordedReplayOutcome, ReplayError> {
    let (base, candidate, contract) = recorded_contract(request, revisions)?;
    let executor =
        ReplayProbeExecutor::new(loader, sandbox, revisions, request.task_request.clone());
    let pair = executor.execute_pair(&contract)?;
    let reference_identity = contract.reference_spec().identity();
    let candidate_identity = contract.candidate_spec().identity();
    let mut report = contrast(
        format!("{}-contrast", contract.experiment_id),
        base.id.clone(),
        candidate.id.clone(),
        &pair.reference.behavior,
        &pair.candidate.behavior,
        &reference_identity,
        &candidate_identity,
    )
    .map_err(|error| ReplayError::ContrastRefused {
        message: error.to_string(),
    })?;
    normalize_unmeasured_usage(&mut report, &pair.reference, &pair.candidate);
    Ok(RecordedReplayOutcome {
        task: executor.load_task()?,
        base,
        candidate,
        contract_digest: pair.contract_digest,
        spec: contract.spec().clone(),
        reference: pair.reference,
        candidate_reading: pair.candidate,
        contrast: report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::harness_evolution::model::{
        Component, ComponentChange, EvolutionBudget, EvolutionMode, HarnessDomain, HarnessManifest,
        QaVerdict,
    };
    use crate::engine::harness_evolution::probe::ProbePins;
    use std::cell::Cell;

    fn revision(id: &str, content: &str) -> HarnessRevision {
        HarnessRevision::draft(
            id,
            HarnessDomain::RustBackend,
            HarnessManifest::from_components([Component::green(
                ComponentKind::SystemPrompt,
                content,
                "replay test prompt",
            )]),
            "owner",
        )
    }

    fn candidate_revision() -> HarnessRevision {
        // A prober-visible candidate must be the base's direct child (the lineage gate enforces
        // this before any probe); keep the fixture honest about that.
        revision("cand-1", "candidate prompt").with_parent(Some("base-1".into()))
    }

    fn gate_revision(
        base: &HarnessRevision,
        id: &str,
        changes: &[ComponentChange],
    ) -> HarnessRevision {
        HarnessRevision::from_changes(base, id, changes, "owner")
            .expect("test candidate change set is applicable to the base")
    }

    fn policy_refused(error: ReplayError) -> String {
        match error {
            ReplayError::PolicyRefused { message } => message,
            other => panic!("expected PolicyRefused, got {other:?}"),
        }
    }

    fn task() -> HistoricalTask {
        HistoricalTask::new("story-1", "run-1", "Historical story", "GOAL:\nfix the bug")
            .with_source_environment(EnvironmentClass::Production)
            .with_run_metadata(
                Some("smith".into()),
                Some("opencode".into()),
                Some("Complete".into()),
            )
            .with_packet_sha(Some("abc123".into()))
    }

    /// A deterministic sandbox: pops readings in order, records what it was asked to run.
    #[derive(Default)]
    struct FakeSandbox {
        readings: RefCell<Vec<SandboxReading>>,
        calls: RefCell<Vec<(ProbeSide, String, String)>>,
    }

    impl FakeSandbox {
        fn with_readings(readings: Vec<SandboxReading>) -> Self {
            Self {
                readings: RefCell::new(readings),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<(ProbeSide, String, String)> {
            self.calls.borrow().clone()
        }
    }

    impl ReplaySandbox for FakeSandbox {
        fn run(&self, request: &SandboxRequest<'_>) -> Result<SandboxReading, ReplayError> {
            self.calls.borrow_mut().push((
                request.side,
                request.harness.revision.clone(),
                request.probe_id.to_string(),
            ));
            if request.environment_class.is_production() {
                return Err(ReplayError::SandboxRefused {
                    sandbox: "fake".into(),
                    message: "production is not a sandbox class".into(),
                });
            }
            let mut readings = self.readings.borrow_mut();
            if readings.is_empty() {
                return Err(ReplayError::SandboxFailed {
                    probe_id: request.probe_id.to_string(),
                    message: "no reading queued".into(),
                });
            }
            Ok(readings.remove(0))
        }
    }

    struct RefusingSandbox;

    impl ReplaySandbox for RefusingSandbox {
        fn run(&self, request: &SandboxRequest<'_>) -> Result<SandboxReading, ReplayError> {
            Err(ReplayError::SandboxRefused {
                sandbox: "refusing".into(),
                message: format!("refused probe {}", request.probe_id),
            })
        }
    }

    #[derive(Default)]
    struct StubLoader {
        task: Option<HistoricalTask>,
        error: Option<ReplayError>,
        loads: Cell<usize>,
    }

    impl StubLoader {
        fn returning(task: HistoricalTask) -> Self {
            Self {
                task: Some(task),
                error: None,
                loads: Cell::new(0),
            }
        }

        fn failing(error: ReplayError) -> Self {
            Self {
                task: None,
                error: Some(error),
                loads: Cell::new(0),
            }
        }

        fn loads(&self) -> usize {
            self.loads.get()
        }
    }

    impl HistoricalTaskLoader for StubLoader {
        fn load(&self, _request: &HistoricalTaskRequest) -> Result<HistoricalTask, ReplayError> {
            self.loads.set(self.loads.get() + 1);
            if let Some(error) = &self.error {
                return Err(error.clone());
            }
            self.task.clone().ok_or(ReplayError::TaskNotFound {
                story_id: "story-1".into(),
                run_id: None,
            })
        }
    }

    fn behavior(evidence_id: &str, refs: Vec<&str>) -> BehaviorEvidence {
        let mut behavior = BehaviorEvidence::new(evidence_id, "");
        behavior.success = true;
        behavior.tests_passed = 3;
        behavior.qa = QaVerdict::Pass;
        behavior.steps = 4;
        behavior.evidence_refs = refs.into_iter().map(str::to_string).collect();
        behavior
    }

    fn revisions() -> MapRevisionSource {
        MapRevisionSource::from_revisions([revision("base-1", "base prompt"), candidate_revision()])
            .expect("revisions")
    }

    fn contract(environment_class: EnvironmentClass) -> ProbeContract {
        let base = revision("base-1", "base prompt");
        let candidate = candidate_revision();
        let mut pins = ProbePins::development("task-1");
        pins.environment_class = environment_class;
        ProbeContract::new(
            "exp-1",
            &base,
            &candidate,
            pins,
            EvolutionMode::Parallel,
            EvolutionBudget::k1(),
            0,
        )
        .expect("contract")
    }

    #[test]
    fn environment_mapping_never_invents_production() {
        assert_eq!(
            environment_class_for(Some("PROD")),
            EnvironmentClass::Production
        );
        assert_eq!(
            environment_class_for(Some(" prod ")),
            EnvironmentClass::Production
        );
        assert_eq!(
            environment_class_for(Some("DEV")),
            EnvironmentClass::Development
        );
        assert_eq!(
            environment_class_for(Some("LOCAL")),
            EnvironmentClass::Sandbox
        );
        assert_eq!(environment_class_for(Some("TEST")), EnvironmentClass::Ci);
        assert_eq!(environment_class_for(None), EnvironmentClass::Development);
        assert_eq!(
            environment_class_for(Some("a-typo")),
            EnvironmentClass::Development
        );
        assert!(!environment_class_for(None).is_production());
    }

    #[test]
    fn historical_task_links_durable_ids_only() {
        let task = task();
        assert_eq!(
            task.evidence_refs,
            vec![
                "storyboard_story_run:run-1".to_string(),
                "storyboard_story:story-1".to_string(),
                "storyboard_story_run:run-1:packet:abc123".to_string(),
            ]
        );
        // The historical environment is metadata, not the execution class.
        assert!(task.source_environment.is_production());
        task.validate().expect("valid task");
        assert!(HistoricalTask::new("story-1", "run-1", "", " ")
            .validate()
            .is_err());
    }

    #[test]
    fn production_environment_is_refused_before_loader_or_sandbox() {
        let mut pins = ProbePins::development("task-1");
        pins.environment_class = EnvironmentClass::Production;
        // A production contract cannot even be constructed; build the request directly to prove
        // the executor is the second line of defence.
        let spec = replay_spec_for("task-1", "base-1", "cand-1", EnvironmentClass::Production);
        let request = ProbeRequest {
            probe_id: "probe-prod".into(),
            side: ProbeSide::Reference,
            revision: "base-1".into(),
            contract_digest: "digest".into(),
            spec,
        };
        let loader = StubLoader::returning(task());
        let sandbox = FakeSandbox::default();
        let revisions = revisions();
        let executor = ReplayProbeExecutor::new(
            &loader,
            &sandbox,
            &revisions,
            HistoricalTaskRequest::for_story("story-1"),
        );
        let error = executor.execute_replay(&request).unwrap_err();
        assert_eq!(error.code(), "replay_production_refused");
        assert_eq!(loader.loads(), 0, "no loader call for a production probe");
        assert!(
            sandbox.calls().is_empty(),
            "no sandbox call for a production probe"
        );
        // The same refusal survives the ProbeExecutor seam as a typed Evolution error.
        let probe_error = ProbeExecutor::execute(&executor, &request).unwrap_err();
        assert_eq!(
            probe_error.to_string(),
            "probe may not target the production environment"
        );
    }

    #[test]
    fn a_historical_task_that_ran_in_production_replays_in_a_sandbox() {
        let spec = replay_spec_for("task-1", "base-1", "cand-1", EnvironmentClass::Development);
        let request = ProbeRequest {
            probe_id: "probe-ref".into(),
            side: ProbeSide::Reference,
            revision: "base-1".into(),
            contract_digest: "digest".into(),
            spec,
        };
        let loader = StubLoader::returning(task());
        let sandbox = FakeSandbox::with_readings(vec![SandboxReading::new(behavior(
            "sandbox-evidence-1",
            vec!["sandbox:exec:1"],
        ))]);
        let revisions = revisions();
        let executor = ReplayProbeExecutor::new(
            &loader,
            &sandbox,
            &revisions,
            HistoricalTaskRequest::for_story("story-1"),
        );
        let reading = executor.execute_replay(&request).expect("sandbox replay");
        assert_eq!(reading.outcome.evidence_id, "sandbox-evidence-1");
        assert_eq!(reading.behavior.probe_id, "probe-ref");
        assert_eq!(
            reading.outcome.evidence_refs,
            reading.behavior.evidence_refs
        );
        assert!(reading
            .outcome
            .evidence_refs
            .contains(&"storyboard_story_run:run-1".to_string()));
        assert!(reading
            .outcome
            .evidence_refs
            .contains(&"sandbox:exec:1".to_string()));
        assert_eq!(sandbox.calls().len(), 1);
        assert_eq!(sandbox.calls()[0].0, ProbeSide::Reference);
        assert_eq!(sandbox.calls()[0].1, "base-1");
    }

    #[test]
    fn the_historical_task_is_loaded_once_for_both_sides() {
        let spec = replay_spec_for("task-1", "base-1", "cand-1", EnvironmentClass::Development);
        let reference = ProbeRequest {
            probe_id: "probe-ref".into(),
            side: ProbeSide::Reference,
            revision: "base-1".into(),
            contract_digest: "digest".into(),
            spec: spec.clone(),
        };
        let candidate = ProbeRequest {
            probe_id: "probe-cand".into(),
            side: ProbeSide::Candidate,
            revision: "cand-1".into(),
            contract_digest: "digest".into(),
            spec,
        };
        let loader = StubLoader::returning(task());
        let sandbox = FakeSandbox::with_readings(vec![
            SandboxReading::new(behavior("sandbox-evidence-ref", vec!["sandbox:exec:ref"])),
            SandboxReading::new(behavior("sandbox-evidence-cand", vec!["sandbox:exec:cand"])),
        ]);
        let revisions = revisions();
        let executor = ReplayProbeExecutor::new(
            &loader,
            &sandbox,
            &revisions,
            HistoricalTaskRequest::for_story("story-1"),
        );
        executor.execute_replay(&reference).unwrap();
        executor.execute_replay(&candidate).unwrap();
        assert_eq!(loader.loads(), 1, "the durable task must be loaded once");
    }

    #[test]
    fn unmatched_side_or_revision_is_refused_before_any_execution() {
        let spec = replay_spec_for("task-1", "base-1", "cand-1", EnvironmentClass::Development);
        // A candidate request pointing at the base revision is not the pinned candidate.
        let request = ProbeRequest {
            probe_id: "probe-cand".into(),
            side: ProbeSide::Candidate,
            revision: "base-1".into(),
            contract_digest: "digest".into(),
            spec,
        };
        let loader = StubLoader::returning(task());
        let sandbox = FakeSandbox::default();
        let revisions = revisions();
        let executor = ReplayProbeExecutor::new(
            &loader,
            &sandbox,
            &revisions,
            HistoricalTaskRequest::for_story("story-1"),
        );
        let error = executor.execute_replay(&request).unwrap_err();
        assert_eq!(error.code(), "replay_unmatched_pair");
        assert_eq!(loader.loads(), 0);
        assert!(sandbox.calls().is_empty());
    }

    #[test]
    fn candidate_harness_is_content_only_authority_is_not_executable() {
        // Same content, different declared authority: the sandbox receives identical content.
        let green = HarnessRevision::draft(
            "rev-green",
            HarnessDomain::RustBackend,
            HarnessManifest::from_components([Component::green(
                ComponentKind::SystemPrompt,
                "same prompt",
                "prompt",
            )]),
            "owner",
        );
        let red = HarnessRevision::draft(
            "rev-red",
            HarnessDomain::RustBackend,
            HarnessManifest::from_components([Component::red(
                ComponentKind::SystemPrompt,
                "same prompt",
                "prompt",
            )]),
            "owner",
        );
        let green_harness = ReplayHarness::materialize(&green).unwrap();
        let red_harness = ReplayHarness::materialize(&red).unwrap();
        assert_eq!(green_harness.components, red_harness.components);
        assert_ne!(green_harness.manifest_hash, red_harness.manifest_hash);
        // The only thing the sandbox can read for a component is its content.
        assert_eq!(
            green_harness.content(ComponentKind::SystemPrompt),
            Some("same prompt")
        );
    }

    #[test]
    fn a_sandbox_reading_without_durable_evidence_is_refused() {
        let spec = replay_spec_for("task-1", "base-1", "cand-1", EnvironmentClass::Development);
        let request = ProbeRequest {
            probe_id: "probe-ref".into(),
            side: ProbeSide::Reference,
            revision: "base-1".into(),
            contract_digest: "digest".into(),
            spec,
        };
        let loader = StubLoader::returning(task());
        let sandbox = FakeSandbox::with_readings(vec![SandboxReading::new(behavior("", vec![]))]);
        let revisions = revisions();
        let executor = ReplayProbeExecutor::new(
            &loader,
            &sandbox,
            &revisions,
            HistoricalTaskRequest::for_story("story-1"),
        );
        assert_eq!(
            executor.execute_replay(&request).unwrap_err().code(),
            "replay_evidence_missing"
        );
    }

    #[test]
    fn a_sandbox_reading_without_refs_is_refused() {
        let spec = replay_spec_for("task-1", "base-1", "cand-1", EnvironmentClass::Development);
        let request = ProbeRequest {
            probe_id: "probe-ref".into(),
            side: ProbeSide::Reference,
            revision: "base-1".into(),
            contract_digest: "digest".into(),
            spec,
        };
        let loader = StubLoader::returning(task());
        let sandbox =
            FakeSandbox::with_readings(vec![SandboxReading::new(behavior("evidence-1", vec![]))]);
        let revisions = revisions();
        let executor = ReplayProbeExecutor::new(
            &loader,
            &sandbox,
            &revisions,
            HistoricalTaskRequest::for_story("story-1"),
        );
        assert_eq!(
            executor.execute_replay(&request).unwrap_err().code(),
            "replay_evidence_missing"
        );
    }

    #[test]
    fn sandbox_refusal_is_typed_and_survives_the_probe_seam() {
        let spec = replay_spec_for("task-1", "base-1", "cand-1", EnvironmentClass::Development);
        let request = ProbeRequest {
            probe_id: "probe-ref".into(),
            side: ProbeSide::Reference,
            revision: "base-1".into(),
            contract_digest: "digest".into(),
            spec,
        };
        let loader = StubLoader::returning(task());
        let sandbox = RefusingSandbox;
        let revisions = revisions();
        let executor = ReplayProbeExecutor::new(
            &loader,
            &sandbox,
            &revisions,
            HistoricalTaskRequest::for_story("story-1"),
        );
        let error = executor.execute_replay(&request).unwrap_err();
        assert_eq!(error.code(), "replay_sandbox_refused");
        let probe_error = ProbeExecutor::execute(&executor, &request).unwrap_err();
        assert!(probe_error.to_string().contains("refused to execute"));
    }

    #[test]
    fn the_default_sandbox_refuses_instead_of_silently_succeeding() {
        let spec = replay_spec_for("task-1", "base-1", "cand-1", EnvironmentClass::Development);
        let request = ProbeRequest {
            probe_id: "probe-ref".into(),
            side: ProbeSide::Reference,
            revision: "base-1".into(),
            contract_digest: "digest".into(),
            spec,
        };
        let loader = StubLoader::returning(task());
        let sandbox = UnavailableSandbox;
        let revisions = revisions();
        let executor = ReplayProbeExecutor::new(
            &loader,
            &sandbox,
            &revisions,
            HistoricalTaskRequest::for_story("story-1"),
        );
        assert_eq!(
            executor.execute_replay(&request).unwrap_err().code(),
            "replay_sandbox_refused"
        );
    }

    #[test]
    fn loader_failure_is_typed_and_never_reaches_the_sandbox() {
        let request = ProbeRequest {
            probe_id: "probe-ref".into(),
            side: ProbeSide::Reference,
            revision: "base-1".into(),
            contract_digest: "digest".into(),
            spec: replay_spec_for("task-1", "base-1", "cand-1", EnvironmentClass::Development),
        };
        let loader = StubLoader::failing(ReplayError::TaskNotFound {
            story_id: "story-1".into(),
            run_id: Some("run-1".into()),
        });
        let sandbox = FakeSandbox::default();
        let revisions = revisions();
        let executor = ReplayProbeExecutor::new(
            &loader,
            &sandbox,
            &revisions,
            HistoricalTaskRequest::new("story-1", Some("run-1".into())),
        );
        let error = executor.execute_replay(&request).unwrap_err();
        assert_eq!(error.code(), "replay_task_not_found");
        assert!(sandbox.calls().is_empty());
    }

    #[test]
    fn unknown_revision_is_refused_before_the_sandbox() {
        let request = ProbeRequest {
            probe_id: "probe-cand".into(),
            side: ProbeSide::Candidate,
            revision: "cand-1".into(),
            contract_digest: "digest".into(),
            spec: replay_spec_for("task-1", "base-1", "cand-1", EnvironmentClass::Development),
        };
        let loader = StubLoader::returning(task());
        let sandbox = FakeSandbox::with_readings(vec![SandboxReading::new(behavior(
            "sandbox-evidence-1",
            vec!["sandbox:exec:1"],
        ))]);
        // Only the base revision is available.
        let revisions = MapRevisionSource::from_revisions([revision("base-1", "base prompt")])
            .expect("revisions");
        let executor = ReplayProbeExecutor::new(
            &loader,
            &sandbox,
            &revisions,
            HistoricalTaskRequest::for_story("story-1"),
        );
        assert_eq!(
            executor.execute_replay(&request).unwrap_err().code(),
            "replay_unknown_revision"
        );
        assert!(sandbox.calls().is_empty());
    }

    #[test]
    fn execute_pair_runs_reference_then_candidate_and_links_one_contract() {
        let contract = contract(EnvironmentClass::Development);
        let loader = StubLoader::returning(task());
        let sandbox = FakeSandbox::with_readings(vec![
            SandboxReading::new(behavior("sandbox-evidence-ref", vec!["sandbox:exec:ref"])),
            SandboxReading::new(behavior("sandbox-evidence-cand", vec!["sandbox:exec:cand"])),
        ]);
        let revisions = revisions();
        let executor = ReplayProbeExecutor::new(
            &loader,
            &sandbox,
            &revisions,
            HistoricalTaskRequest::for_story("story-1"),
        );
        let pair = executor.execute_pair(&contract).expect("pair");
        assert_eq!(pair.contract_digest, contract.digest());
        assert_eq!(pair.reference.outcome.side, ProbeSide::Reference);
        assert_eq!(pair.reference.outcome.revision, "base-1");
        assert_eq!(pair.candidate.outcome.side, ProbeSide::Candidate);
        assert_eq!(pair.candidate.outcome.revision, "cand-1");
        assert_ne!(
            pair.reference.outcome.evidence_id,
            pair.candidate.outcome.evidence_id
        );
        let calls = sandbox.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, ProbeSide::Reference);
        assert_eq!(calls[1].0, ProbeSide::Candidate);
        assert_eq!(loader.loads(), 1);
    }

    #[test]
    fn a_sandbox_that_attributes_the_reading_to_another_probe_is_refused() {
        let request = ProbeRequest {
            probe_id: "probe-ref".into(),
            side: ProbeSide::Reference,
            revision: "base-1".into(),
            contract_digest: "digest".into(),
            spec: replay_spec_for("task-1", "base-1", "cand-1", EnvironmentClass::Development),
        };
        let mut wrong = behavior("sandbox-evidence-1", vec!["sandbox:exec:1"]);
        wrong.probe_id = "someone-else".into();
        let loader = StubLoader::returning(task());
        let sandbox = FakeSandbox::with_readings(vec![SandboxReading::new(wrong)]);
        let revisions = revisions();
        let executor = ReplayProbeExecutor::new(
            &loader,
            &sandbox,
            &revisions,
            HistoricalTaskRequest::for_story("story-1"),
        );
        assert_eq!(
            executor.execute_replay(&request).unwrap_err().code(),
            "replay_reading_rejected"
        );
    }

    #[test]
    fn non_production_guard_is_explicit_for_sandbox_authors() {
        assert!(assert_non_production_environment(EnvironmentClass::Development).is_ok());
        assert!(assert_non_production_environment(EnvironmentClass::Sandbox).is_ok());
        assert_eq!(
            assert_non_production_environment(EnvironmentClass::Production)
                .unwrap_err()
                .code(),
            "replay_production_refused"
        );
    }

    fn recorded_request(pins: ProbePins) -> RecordedReplayRequest {
        RecordedReplayRequest {
            experiment_id: "exp-1".into(),
            pins,
            mode: EvolutionMode::Parallel,
            budget: EvolutionBudget::k1(),
            seed: 0,
            base_revision: "base-1".into(),
            candidate_revision: "cand-1".into(),
            task_request: HistoricalTaskRequest::for_story("story-1"),
        }
    }

    fn recorded_sandbox() -> FakeSandbox {
        FakeSandbox::with_readings(vec![
            SandboxReading::new(behavior("evidence-reference", vec!["sandbox:reference"])),
            SandboxReading::new(behavior("evidence-candidate", vec!["sandbox:candidate"])),
        ])
    }

    #[test]
    fn recorded_plan_is_read_only_and_loads_without_executing() {
        let loader = StubLoader::returning(task());
        let revisions = revisions();
        let request = recorded_request(ProbePins::development("task-1"));
        let plan = plan_recorded_replay(&loader, &revisions, &request).expect("plan");
        assert_eq!(plan.task.story_id, "story-1");
        assert_eq!(plan.base.id, "base-1");
        assert_eq!(plan.candidate.id, "cand-1");
        assert_eq!(plan.contract_digest().len(), 64);
        assert_eq!(loader.loads(), 1, "the plan loads the historical task");
    }

    #[test]
    fn recorded_plan_refuses_production_before_loader_or_revision() {
        let mut pins = ProbePins::development("task-1");
        pins.environment_class = EnvironmentClass::Production;
        let loader = StubLoader::returning(task());
        let revisions = revisions();
        let request = recorded_request(pins);
        let error = plan_recorded_replay(&loader, &revisions, &request).unwrap_err();
        assert_eq!(error.code(), "replay_production_refused");
        assert_eq!(loader.loads(), 0, "production is refused before the loader");
    }

    #[test]
    fn the_shared_gate_accepts_a_valid_direct_child_candidate() {
        let base = revision("base-1", "baseline prompt");
        let candidate = candidate_revision();
        validate_probe_candidate(&base, &candidate).expect("valid child candidate passes");
    }

    #[test]
    fn the_shared_gate_refuses_a_candidate_with_an_unrelated_parent() {
        let base = revision("base-1", "baseline prompt");
        let candidate = candidate_revision().with_parent(Some("some-other-revision".into()));
        let error = validate_probe_candidate(&base, &candidate).expect_err("lineage must match");
        let message = policy_refused(error);
        assert!(message.contains("direct child"), "{message}");
    }

    #[test]
    fn the_shared_gate_refuses_red_authority_qa_and_authority_expansion_candidates() {
        // RED declared by the proposer: never composable/adoptable.
        let base = revision("base-1", "baseline prompt");
        let red = gate_revision(
            &base,
            "gate-red",
            &[ComponentChange::set(
                Component::red(
                    ComponentKind::SystemPrompt,
                    "self-declared protected prompt",
                    "red",
                ),
                "should never pass",
            )],
        );
        let message = policy_refused(
            validate_probe_candidate(&base, &red).expect_err("RED candidate must be refused"),
        );
        assert!(
            message.to_lowercase().contains("red") || message.to_lowercase().contains("protected"),
            "{message}"
        );

        // QA boundary content change.
        let qa = gate_revision(
            &base,
            "gate-qa",
            &[ComponentChange::set(
                Component::green(
                    ComponentKind::SystemPrompt,
                    "the QA boundary now allows git access",
                    "qa",
                ),
                "attempt to move the QA boundary",
            )],
        );
        let message = policy_refused(
            validate_probe_candidate(&base, &qa).expect_err("QA boundary change must be refused"),
        );
        assert!(
            message.to_lowercase().contains("qa") || message.to_lowercase().contains("boundary"),
            "{message}"
        );

        // Authority expansion (GREEN -> YELLOW on the same kind).
        let expansion = gate_revision(
            &base,
            "gate-authority",
            &[ComponentChange::set(
                Component::yellow(
                    ComponentKind::SystemPrompt,
                    "expanded authority",
                    "authority",
                ),
                "authority expansion attempt",
            )],
        );
        let message = policy_refused(
            validate_probe_candidate(&base, &expansion)
                .expect_err("authority expansion must be refused"),
        );
        assert!(message.to_lowercase().contains("authority"), "{message}");
    }

    #[test]
    fn the_shared_gate_refuses_before_a_dry_run_plan_or_a_real_probe() {
        // Dry-run path: plan_recorded_replay builds no contract/execution, but the gate still runs.
        let loader = StubLoader::returning(task());
        let mut revisions = revisions();
        revisions
            .insert(candidate_revision().with_parent(Some("unrelated".into())))
            .expect("insert bad candidate");
        let request = recorded_request(ProbePins::development("task-1"));
        let error = plan_recorded_replay(&loader, &revisions, &request).unwrap_err();
        assert!(matches!(error, ReplayError::PolicyRefused { .. }));

        // Real-run path: no sandbox call is made when the gate refuses.
        let sandbox = recorded_sandbox();
        let error = run_recorded_replay(&loader, &sandbox, &revisions, &request).unwrap_err();
        assert!(matches!(error, ReplayError::PolicyRefused { .. }));
        assert!(sandbox.calls.borrow().is_empty(), "no probe may execute");
        assert_eq!(
            loader.loads(),
            0,
            "the gate runs before even the task loader"
        );
    }

    #[test]
    fn a_recorded_run_pairs_readings_and_builds_a_behavioral_contrast() {
        let loader = StubLoader::returning(task());
        let sandbox = recorded_sandbox();
        let revisions = revisions();
        let request = recorded_request(ProbePins::development("task-1"));
        let outcome =
            run_recorded_replay(&loader, &sandbox, &revisions, &request).expect("recorded run");
        assert_eq!(outcome.contrast.base_revision, "base-1");
        assert_eq!(outcome.contrast.candidate_revision, "cand-1");
        assert_eq!(outcome.contrast.dimensions.len(), 16);
        assert_eq!(outcome.contract_digest.len(), 64);
        assert_eq!(
            outcome.reference.outcome.probe_id,
            "exp-1-reference".to_string()
        );
        assert_eq!(
            outcome.candidate_reading.outcome.probe_id,
            "exp-1-candidate".to_string()
        );
        for evidence in [
            "storyboard_story_run:run-1",
            "storyboard_story:story-1",
            "sandbox:reference",
            "sandbox:candidate",
        ] {
            assert!(
                outcome
                    .contrast
                    .evidence_refs
                    .contains(&evidence.to_string()),
                "contrast must link {evidence}: {:?}",
                outcome.contrast.evidence_refs
            );
        }
        assert_eq!(sandbox.calls().len(), 2, "reference and candidate executed");
    }

    #[test]
    fn unmeasured_usage_is_not_persisted_as_a_zero_token_comparison() {
        let loader = StubLoader::returning(task());
        let mut failed = behavior("evidence-reference", vec![USAGE_UNMEASURED_REF]);
        failed.success = false;
        let mut failed_candidate = behavior("evidence-candidate", vec![USAGE_UNMEASURED_REF]);
        failed_candidate.success = false;
        let sandbox = FakeSandbox::with_readings(vec![
            SandboxReading::new(failed),
            SandboxReading::new(failed_candidate),
        ]);
        let revisions = revisions();
        let request = recorded_request(ProbePins::development("task-1"));
        let outcome =
            run_recorded_replay(&loader, &sandbox, &revisions, &request).expect("recorded run");
        let tokens = outcome
            .contrast
            .dimensions
            .iter()
            .find(|dimension| dimension.dimension == BehaviorDimension::Tokens)
            .expect("tokens dimension");
        assert_eq!(
            tokens.direction,
            ContrastDirection::Unknown,
            "unmeasured usage is not a zero comparison"
        );
        assert_eq!(tokens.reference_value, None);
        assert_eq!(tokens.candidate_value, None);
        assert!(
            outcome.contrast.summary.contains("not captured"),
            "recomputed summary must account for the unmeasured dimension: {}",
            outcome.contrast.summary
        );
        assert!(outcome.probe_failed(), "both executions failed");
    }

    #[test]
    fn a_recorded_run_refuses_an_unknown_revision_before_the_sandbox() {
        let loader = StubLoader::returning(task());
        let sandbox = FakeSandbox::default();
        let revisions = revisions();
        let mut request = recorded_request(ProbePins::development("task-1"));
        request.base_revision = "missing-revision".into();
        let error = run_recorded_replay(&loader, &sandbox, &revisions, &request).unwrap_err();
        assert_eq!(error.code(), "replay_unknown_revision");
        assert!(sandbox.calls().is_empty(), "no execution without revisions");
    }

    #[test]
    fn a_recorded_run_surfaces_a_sandbox_refusal_instead_of_a_partial_contrast() {
        let loader = StubLoader::returning(task());
        let sandbox = FakeSandbox::default(); // no readings queued
        let revisions = revisions();
        let request = recorded_request(ProbePins::development("task-1"));
        let error = run_recorded_replay(&loader, &sandbox, &revisions, &request).unwrap_err();
        assert_eq!(error.code(), "replay_sandbox_failed");
    }
}
