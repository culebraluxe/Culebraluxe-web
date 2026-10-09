//! Shared fixture for the FORGE ARCHITECTURE seam suite (`forge_arch_seam__0NN__*.rs`).
//!
//! The FORGE.JOB rails (`forge_job__001..015`) prove each layer's own contract. This suite proves the layers are
//! still CONNECTED the way the ownership model says, as a whole:
//!
//! ```text
//! Workflow:              routing / state / sequencing           (core/workflow + FORGE_SDLC-v6.xml)
//! JobService:            lease / heartbeat / retry / recovery   (forge::engine::job)
//! AbstractForgeService:  shared role lifecycle                  (forge::roles::service + roles::lifecycle)
//! Concrete role:         role-specific interpretation           (forge::roles::<lane> via RoleHooks)
//! OpenCodeHarness:       vendor process / session / model       (forge::engine::opencode*)
//! PostgreSQL:            atomic state transitions
//! ```
//!
//! Everything here is the PRODUCTION seam or an observer wrapped around one: the XML definition through
//! `ForgeRuntime::in_memory_full`, the production durable driver `drive_forge_story_with_jobs`, the production
//! `WorkflowJobService` behind a recording decorator, and the production lane composition `ForgeLaneServices` (the
//! very struct `bin/forge.rs` registers). The only doubles are at the vendor edge: a scripted `ForgeRoleRunner`
//! (no envelope, so a lane hands it the whole turn) or a scripted `RoleHarness` (the port `OpenCodeHarness`
//! implements). No model, no network, no OpenCode, no database.
//!
//! Included with `#[path = "support/forge_arch_seam.rs"] mod support;` — a directory under `tests/` is not a target.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use forge::engine::assay::CommandResult;
use forge::engine::executor::{
    drive_forge_story_with_jobs, DriveForgeStoryOptions, DriveForgeStoryResult,
    DurableForgeExecution, ForgeRoleOutcome, ForgeRoleRunner,
};
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::job::{
    ForgeJobLease, ForgeJobRequest, ForgeJobState, JobService, WorkflowJobService,
};
use forge::engine::runner::{HarnessOutput, RoleHarness};
use forge::engine::runtime::{ActiveForgeRoleTask, ForgeRuntime};
use forge::engine::writer::RecordingWriter;
use forge::roles::ForgeServiceRegistry;
use test_harness::source::{code_of, read};
use workflow::{MemoryStore, Result as WfResult, WorkflowError};

pub const WORKER: &str = "forge-arch-seam-worker";

/// A 40-hex SHA, so every lineage check that normalizes SHAs accepts it as one.
pub const SMITH_CANDIDATE: &str = "5a1700000000000000000000000000000000c0de";
pub const WORKSPACE_HEAD: &str = "eadeadeadeadeadeadeadeadeadeadeadeadead0";

/// One ordered log shared by every observer, so a test can assert the ORDER the layers were crossed in.
#[derive(Clone, Default)]
pub struct Trace(Arc<Mutex<Vec<String>>>);

impl Trace {
    pub fn push(&self, entry: impl Into<String>) {
        self.0.lock().expect("trace lock").push(entry.into());
    }

    pub fn entries(&self) -> Vec<String> {
        self.0.lock().expect("trace lock").clone()
    }

    pub fn with_prefix(&self, prefix: &str) -> Vec<String> {
        self.entries()
            .into_iter()
            .filter(|entry| entry.starts_with(prefix))
            .collect()
    }
}

/// The production `WorkflowJobService`, observed. Every call is delegated unchanged; the decorator only writes the
/// call into the shared trace, so "the driver went through JobService" is a recorded fact rather than an assumption.
pub struct ObservedJobs<'a> {
    pub inner: WorkflowJobService<'a, MemoryStore>,
    pub trace: Trace,
}

impl<'a> ObservedJobs<'a> {
    pub fn new(rt: &'a ForgeRuntime<MemoryStore>, trace: Trace) -> Self {
        Self {
            inner: WorkflowJobService::new(rt.engine()),
            trace,
        }
    }
}

impl JobService for ObservedJobs<'_> {
    fn enqueue(&self, request: &ForgeJobRequest) -> WfResult<String> {
        self.trace.push(format!(
            "job.enqueue {} {}",
            request.service_key, request.node_id
        ));
        self.inner.enqueue(request)
    }
    fn claim(&self, worker_id: &str, limit: usize) -> WfResult<Vec<ForgeJobLease>> {
        self.trace.push("job.claim");
        self.inner.claim(worker_id, limit)
    }
    fn claim_one(&self, job_id: &str, worker_id: &str) -> WfResult<ForgeJobLease> {
        let lease = self.inner.claim_one(job_id, worker_id)?;
        self.trace.push(format!(
            "job.claim_one {} {}",
            lease.service_key, lease.node_id
        ));
        Ok(lease)
    }
    fn heartbeat(&self, job_id: &str, worker_id: &str) -> WfResult<i64> {
        self.trace.push("job.heartbeat");
        self.inner.heartbeat(job_id, worker_id)
    }
    fn inspect(&self, job_id: &str) -> WfResult<ForgeJobState> {
        self.inner.inspect(job_id)
    }
    fn complete(&self, job_id: &str, worker_id: &str) -> WfResult<()> {
        self.trace.push("job.complete");
        self.inner.complete(job_id, worker_id)
    }
    fn fail(&self, job_id: &str, worker_id: &str, error: &str, permanent: bool) -> WfResult<()> {
        self.trace.push(if permanent {
            "job.fail permanent"
        } else {
            "job.fail retryable"
        });
        self.inner.fail(job_id, worker_id, error, permanent)
    }
    fn cancel(&self, job_id: &str, actor: &str) -> WfResult<()> {
        self.inner.cancel(job_id, actor)
    }
    fn requeue(&self, job_id: &str, actor: &str) -> WfResult<()> {
        self.inner.requeue(job_id, actor)
    }
    fn recover_stale(&self, batch: usize) -> WfResult<usize> {
        self.trace.push("job.recover_stale");
        self.inner.recover_stale(batch)
    }
}

/// What a scripted node answers.
#[derive(Clone)]
pub enum Scripted {
    /// `transition_name = Some("complete")` with this evidence — the only transition a lane ever returns.
    Complete(ForgeGateEvidence),
    Fail(fn() -> WorkflowError),
}

/// A role turn with no envelope: `turn_ports()` is `None`, which is the seam `run_lane_turn` reads, so the lane
/// service hands it the whole turn and one call here is one paid turn. Unscripted nodes are a test bug and panic.
pub struct ScriptedRunner {
    pub script: BTreeMap<String, Scripted>,
    pub trace: Trace,
}

impl ScriptedRunner {
    pub fn new(trace: Trace) -> Self {
        Self {
            script: BTreeMap::new(),
            trace,
        }
    }

    pub fn answer(mut self, node: &str, evidence: ForgeGateEvidence) -> Self {
        self.script
            .insert(node.into(), Scripted::Complete(evidence));
        self
    }

    pub fn fail(mut self, node: &str, error: fn() -> WorkflowError) -> Self {
        self.script.insert(node.into(), Scripted::Fail(error));
        self
    }

    pub fn turns(&self) -> Vec<String> {
        self.trace.with_prefix("turn ")
    }
}

impl ForgeRoleRunner for ScriptedRunner {
    fn run(&self, node_id: &str, _task: &ActiveForgeRoleTask) -> WfResult<ForgeRoleOutcome> {
        self.trace.push(format!("turn {node_id}"));
        match self.script.get(node_id) {
            Some(Scripted::Complete(evidence)) => Ok(ForgeRoleOutcome {
                transition_name: Some("complete".into()),
                evidence: evidence.clone(),
            }),
            Some(Scripted::Fail(error)) => Err(error()),
            None => panic!("no scripted answer for node {node_id}"),
        }
    }
}

/// The evidence a FEATURE story is woken with — what `bin/forge.rs` passes.
pub fn feature_evidence() -> ForgeGateEvidence {
    ForgeGateEvidence {
        work_type: Some("FEATURE".into()),
        scout_required: Some(false),
        ..Default::default()
    }
}

/// `feature_evidence()` plus whatever a node reports.
pub fn feature(patch: impl FnOnce(&mut ForgeGateEvidence)) -> ForgeGateEvidence {
    let mut evidence = feature_evidence();
    patch(&mut evidence);
    evidence
}

/// The XML definition on the in-memory store, with a recording Story Board writer.
pub fn runtime() -> (ForgeRuntime<MemoryStore>, Arc<RecordingWriter>) {
    let writer = Arc::new(RecordingWriter::default());
    let rt = ForgeRuntime::in_memory_full(writer.clone(), None, None)
        .expect("the XML Forge definition seeds");
    (rt, writer)
}

/// One durable generation through the production driver, exactly as `bin/forge.rs` calls it: `runner: None`, so
/// the ONLY way a turn can run is JobService + ForgeServiceRegistry.
pub fn drive(
    rt: &ForgeRuntime<MemoryStore>,
    story: &str,
    jobs: &dyn JobService,
    registry: &ForgeServiceRegistry<'_>,
    max_steps: usize,
) -> WfResult<DriveForgeStoryResult> {
    drive_forge_story_with_jobs(
        rt,
        story,
        DriveForgeStoryOptions {
            work_type: "FEATURE",
            evidence: feature_evidence(),
            runner: None,
            max_steps,
            worker_id: WORKER,
            within_story_concurrency: 1,
            stop_after: None,
            turn_cap: 40,
        },
        DurableForgeExecution { jobs, registry },
    )
}

/// A scripted vendor edge: the port `OpenCodeHarness` implements. Every turn and every command is traced; `git
/// rev-parse HEAD` answers `head`, every other command passes.
pub struct ScriptedHarness {
    pub trace: Trace,
    pub raw: String,
    pub candidate_sha: Option<String>,
    pub head: String,
}

impl ScriptedHarness {
    pub fn new(trace: Trace) -> Self {
        Self {
            trace,
            raw: String::new(),
            candidate_sha: None,
            head: WORKSPACE_HEAD.into(),
        }
    }
}

impl RoleHarness for ScriptedHarness {
    fn run_role(
        &self,
        node_id: &str,
        _task: &ActiveForgeRoleTask,
        _self_heal: Option<&str>,
    ) -> WfResult<HarnessOutput> {
        self.trace.push(format!("harness.turn {node_id}"));
        Ok(HarnessOutput {
            raw: self.raw.clone(),
            candidate_sha: self.candidate_sha.clone(),
            assay_commands: vec![],
            acceptance_mapped: false,
            refusal: None,
            execution_base: None,
            usage: None,
        })
    }
    fn exists_on_base_ref(&self, _base_ref: &str, _path: &str) -> bool {
        true
    }
    fn assay_cwd(&self) -> &Path {
        Path::new(".")
    }
    fn run_command(&self, command: &str) -> CommandResult {
        self.trace.push(format!("harness.command {command}"));
        let output = if command.trim() == "git rev-parse HEAD" {
            format!("{}\n", self.head)
        } else {
            String::new()
        };
        CommandResult {
            command: command.into(),
            exit_code: 0,
            cancelled: false,
            passed: true,
            excerpt: String::new(),
            unmeasurable: false,
            output,
        }
    }
}

pub fn task(story: &str, node: &str) -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: format!("task-{node}"),
        process_instance_id: format!("proc-{story}"),
        story_id: story.into(),
        token_id: None,
        node_id: Some(node.into()),
        status: workflow::TaskStatus::Ready,
        assignee: None,
        candidates: vec![node.into()],
        write_surface: None,
    }
}

/// The production part of a Rust source file: everything above its first `#[cfg(test)]`, comments stripped, blank
/// lines dropped, 1-based line numbers kept for the failure message.
pub fn production_code(path: &Path) -> Vec<(usize, String)> {
    read(path)
        .lines()
        .take_while(|line| !line.trim_start().starts_with("#[cfg(test)]"))
        .enumerate()
        .map(|(index, line)| (index + 1, code_of(line).to_string()))
        .filter(|(_, code)| !code.trim().is_empty())
        .collect()
}

/// The production code of `path` joined into one string, for ordering and containment checks.
pub fn production_text(path: &Path) -> String {
    production_code(path)
        .into_iter()
        .map(|(_, code)| code)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every production line of `path` containing any of `needles` (plain substring), as `file:line: needle`.
pub fn lines_naming(path: &Path, needles: &[&str]) -> Vec<String> {
    let mut hits = Vec::new();
    for (line, code) in production_code(path) {
        for needle in needles {
            if code.contains(needle) {
                hits.push(format!(
                    "{}:{line}: `{needle}` in `{}`",
                    test_harness::source::relative(path),
                    code.trim()
                ));
            }
        }
    }
    hits
}
