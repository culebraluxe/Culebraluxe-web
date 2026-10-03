#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use db::NewToolArtifact;
use forge::engine::assay::CommandResult;
use forge::engine::completion::MemoryLedger;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::executor::{
    drive_forge_story_with_jobs, DriveForgeStoryOptions, DriveForgeStoryResult,
    DurableForgeExecution,
};
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::hold::OpenHold;
use forge::engine::job::{ForgeJobBridge, JobService, WorkflowJobService};
use forge::engine::runner::{HarnessOutput, ProductionRoleRunner, RoleHarness};
use forge::engine::runtime::{ActiveForgeRoleTask, ForgeRuntime};
use forge::engine::writer::{ForgeEvidenceReader, ForgeStateWriter};
use forge::roles::ForgeLaneServices;
use workflow::{Job, JobStatus, MemoryStore, Result as WorkflowResult, Value};

pub const STORY: &str = "TST-FORGE-SEAM";
pub const STORY_RUN: &str = "run-forge-seam";
pub const WORK_TYPE: &str = "FEATURE";
pub const WORKER: &str = "forge-seam-worker";
pub const CANDIDATE_SHA: &str = "0123456789abcdef0123456789abcdef01234567";
pub const WRONG_HEAD_SHA: &str = "fedcba9876543210fedcba9876543210fedcba98";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedHold {
    pub process_instance_id: String,
    pub task_id: Option<String>,
    pub story_id: String,
    pub reason: String,
    pub originating_node: Option<String>,
    pub failure_class: Option<String>,
    pub resume_target: Option<String>,
}

#[derive(Default)]
pub struct SeamWriter {
    pub human_holds: Mutex<Vec<(String, String)>>,
    pub completed: Mutex<Vec<String>>,
    pub in_progress: Mutex<Vec<String>>,
    pub candidates: Mutex<Vec<(String, String)>>,
    pub details: Mutex<Vec<(String, String)>>,
    pub opened_holds: Mutex<Vec<CapturedHold>>,
    pub artifacts: Mutex<Vec<NewToolArtifact>>,
}

impl ForgeStateWriter for SeamWriter {
    fn mark_story_human_hold(&self, story_id: &str, reason: &str) -> Result<(), String> {
        self.human_holds
            .lock()
            .expect("human holds")
            .push((story_id.to_string(), reason.to_string()));
        Ok(())
    }

    fn mark_story_complete(&self, story_id: &str) -> Result<(), String> {
        self.completed
            .lock()
            .expect("completed")
            .push(story_id.to_string());
        Ok(())
    }

    fn mark_story_in_progress(&self, story_id: &str) -> Result<(), String> {
        self.in_progress
            .lock()
            .expect("in progress")
            .push(story_id.to_string());
        Ok(())
    }

    fn stamp_run_candidate(&self, run_id: &str, candidate_sha: &str) -> Result<(), String> {
        self.candidates
            .lock()
            .expect("candidates")
            .push((run_id.to_string(), candidate_sha.to_ascii_lowercase()));
        Ok(())
    }

    fn append_run_detail(&self, run_id: &str, detail: &str) -> Result<(), String> {
        self.details
            .lock()
            .expect("details")
            .push((run_id.to_string(), detail.to_string()));
        Ok(())
    }

    fn open_hold(&self, input: &OpenHold) -> Result<String, String> {
        let mut holds = self.opened_holds.lock().expect("opened holds");
        let id = format!("hold-{}", holds.len() + 1);
        holds.push(CapturedHold {
            process_instance_id: input.process_instance_id.clone(),
            task_id: input.task_id.clone(),
            story_id: input.story_id.clone(),
            reason: input.reason.clone(),
            originating_node: input.originating_node.clone(),
            failure_class: input.failure_class.clone(),
            resume_target: input.resume_target.clone(),
        });
        Ok(id)
    }

    fn record_tool_artifact(&self, input: &NewToolArtifact) -> Result<Option<String>, String> {
        let mut artifacts = self.artifacts.lock().expect("artifacts");
        let id = format!("artifact-{}", artifacts.len() + 1);
        artifacts.push(input.clone());
        Ok(Some(id))
    }

    fn record_run_usage(
        &self,
        _run_id: &str,
        _usage: &forge::engine::harness_usage::HarnessUsage,
    ) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Default)]
pub struct SeamHarness {
    calls: Mutex<Vec<String>>,
    commands: Mutex<Vec<String>>,
    self_heals: Mutex<Vec<(String, Option<String>)>>,
}

impl SeamHarness {
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("calls").clone()
    }

    pub fn count(&self, node: &str) -> usize {
        self.calls()
            .into_iter()
            .filter(|called| called == node)
            .count()
    }

    pub fn commands(&self) -> Vec<String> {
        self.commands.lock().expect("commands").clone()
    }
}

impl RoleHarness for SeamHarness {
    fn run_role(
        &self,
        node_id: &str,
        _task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> WorkflowResult<HarnessOutput> {
        self.calls
            .lock()
            .expect("calls")
            .push(node_id.to_string());
        self.self_heals.lock().expect("self heals").push((
            node_id.to_string(),
            self_heal.map(str::to_string),
        ));

        let (raw, candidate_sha, assay_commands, acceptance_mapped) = match node_id {
            "architect" | "repair_architect" => (
                concat!(
                    "architecture handoff\n",
                    "FORGE_ARCHITECT_HANDOFF: {\"version\":1,\"baseRef\":\"base-seam\",",
                    "\"findings\":[{\"id\":\"F1\",\"required\":true,",
                    "\"summary\":\"Exercise the Forge seam\",",
                    "\"scope\":[\"forge/src/lib.rs\"],\"proofs\":[],\"risks\":[]}]}\n"
                )
                .to_string(),
                None,
                vec![],
                false,
            ),
            "lead_pre" => (
                "FORGE_EVIDENCE_JSON: {\"leadDecision\":\"SMITH\"}\n".to_string(),
                None,
                vec![],
                false,
            ),
            "smith" | "repair_smith" | "fast_smith" => (
                "candidate implemented\n".to_string(),
                Some(CANDIDATE_SHA.to_string()),
                vec![],
                false,
            ),
            "lead_post" => ("integration complete\n".to_string(), None, vec![], false),
            "qa_review" => (
                "FORGE_EVIDENCE_JSON: {\"qaReviewPassed\":true}\n".to_string(),
                None,
                vec![],
                false,
            ),
            "qa_verify" | "fast_qa_verify" => (
                "deterministic assay\n".to_string(),
                None,
                vec!["cargo test --manifest-path Cargo.toml -p forge".to_string()],
                true,
            ),
            other => (format!("{other} complete\n"), None, vec![], false),
        };

        Ok(HarnessOutput {
            raw,
            candidate_sha,
            assay_commands,
            acceptance_mapped,
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
        self.commands
            .lock()
            .expect("commands")
            .push(command.to_string());
        CommandResult {
            command: command.to_string(),
            exit_code: 0,
            passed: true,
            excerpt: String::new(),
            unmeasurable: false,
            output: if command.trim() == "git rev-parse HEAD" {
                WRONG_HEAD_SHA.to_string()
            } else {
                String::new()
            },
        }
    }
}

struct LedgerEvidence(Arc<MemoryLedger>);

impl ForgeEvidenceReader for LedgerEvidence {
    fn read(&self, story_id: &str) -> ForgeGateEvidence {
        self.0.evidence_for(story_id).unwrap_or_default()
    }
}

pub struct SeamFixture {
    pub rt: ForgeRuntime<MemoryStore>,
    pub memory: MemoryStore,
    pub writer: Arc<SeamWriter>,
    pub ledger: Arc<MemoryLedger>,
    pub harness: SeamHarness,
}

pub fn initial_evidence() -> ForgeGateEvidence {
    ForgeGateEvidence {
        work_type: Some(WORK_TYPE.into()),
        scout_required: Some(false),
        architecture_review_required: Some(false),
        qa_review_required: Some(true),
        // A normal FEATURE happy path still exercises Architect/Lead/Smith/Inspector/Assay, then the XML's
        // release-deferred branch closes the process without touching a publisher or external deployment target.
        deployment_deferred_to_batch: Some(1),
        ..Default::default()
    }
}

impl SeamFixture {
    pub fn new() -> Self {
        let memory = MemoryStore::new();
        let writer = Arc::new(SeamWriter::default());
        let ledger = Arc::new(MemoryLedger::new());
        let reader: Arc<dyn ForgeEvidenceReader> = Arc::new(LedgerEvidence(ledger.clone()));
        let rt = ForgeRuntime::from_store(
            memory.clone(),
            writer.clone(),
            None,
            Some(reader),
            ledger.clone(),
            forge_sdlc_definition(),
        )
        .expect("production Forge XML seeds the seam runtime");
        Self {
            rt,
            memory,
            writer,
            ledger,
            harness: SeamHarness::default(),
        }
    }

    pub fn current_evidence(&self) -> ForgeGateEvidence {
        self.ledger
            .evidence_for(STORY)
            .unwrap_or_else(initial_evidence)
    }

    /// Drive exactly one production role turn. Workflow decides which role is READY; the helper never names it.
    /// The next invocation rehydrates the role runner from the completion ledger produced by the previous turn.
    pub fn drive_one_role(&self) -> WorkflowResult<DriveForgeStoryResult> {
        let current = self.current_evidence();
        let runner = ProductionRoleRunner::new(&self.harness, current)
            .with_writer(self.writer.as_ref())
            .with_story_run(Some(STORY_RUN.to_string()));
        let lanes = ForgeLaneServices::new(&runner);
        let registry = lanes.registry()?;
        let jobs = WorkflowJobService::new(self.rt.engine());
        drive_forge_story_with_jobs(
            &self.rt,
            STORY,
            DriveForgeStoryOptions {
                work_type: WORK_TYPE,
                evidence: initial_evidence(),
                runner: None,
                max_steps: 1,
                worker_id: WORKER,
                split_concurrency: 1,
                stop_after: None,
                turn_cap: 16,
            },
            DurableForgeExecution {
                jobs: &jobs,
                registry: &registry,
            },
        )
    }

    pub fn open_role_tasks(&self) -> Vec<ActiveForgeRoleTask> {
        self.rt.list_role_tasks(STORY).expect("list role tasks")
    }

    pub fn one_open_role(&self) -> ActiveForgeRoleTask {
        let tasks = self.open_role_tasks();
        assert_eq!(tasks.len(), 1, "expected one READY role task: {tasks:?}");
        tasks[0].clone()
    }

    pub fn jobs_for_instance(&self, instance_id: &str) -> Vec<Job> {
        self.rt
            .engine()
            .jobs_for_instance(instance_id)
            .expect("jobs for process")
    }

    pub fn job(&self, id: &str) -> Job {
        self.rt.engine().get_job(id).expect("job exists")
    }

    pub fn terminal_jobs(&self, instance_id: &str) -> Vec<Job> {
        self.jobs_for_instance(instance_id)
            .into_iter()
            .filter(|job| {
                matches!(
                    job.status,
                    JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
                )
            })
            .collect()
    }

    pub fn artifacts(&self) -> Vec<NewToolArtifact> {
        self.writer.artifacts.lock().expect("artifacts").clone()
    }

    pub fn assay_artifact(&self) -> Option<NewToolArtifact> {
        self.artifacts()
            .into_iter()
            .find(|artifact| artifact.tool == "assay")
    }

    pub fn candidate_stamps(&self) -> Vec<(String, String)> {
        self.writer.candidates.lock().expect("candidates").clone()
    }

    pub fn holds(&self) -> Vec<CapturedHold> {
        self.writer.opened_holds.lock().expect("holds").clone()
    }

    pub fn job_payloads_by_task(
        &self,
        instance_id: &str,
    ) -> BTreeMap<String, Vec<Value>> {
        let mut out: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for job in self.jobs_for_instance(instance_id) {
            if let Some(task_id) = job.payload.get("taskId").and_then(Value::as_str) {
                out.entry(task_id.to_string())
                    .or_default()
                    .push(job.payload.clone());
            }
        }
        out
    }
}

pub fn enqueue_ready_task(
    fixture: &SeamFixture,
    task: &ActiveForgeRoleTask,
) -> WorkflowResult<(String, String)> {
    let current = fixture.current_evidence();
    let runner = ProductionRoleRunner::new(&fixture.harness, current)
        .with_writer(fixture.writer.as_ref())
        .with_story_run(Some(STORY_RUN.to_string()));
    let lanes = ForgeLaneServices::new(&runner);
    let registry = lanes.registry()?;
    let request = ForgeJobBridge::new(&registry)
        .job_for_ready_task(task)
        .expect("READY role task has a service binding");
    let service_key = request.service_key.clone();
    let jobs = WorkflowJobService::new(fixture.rt.engine());
    let id = jobs.enqueue(&request)?;
    Ok((id, service_key))
}

pub fn count_jobs_for_task(
    fixture: &SeamFixture,
    instance_id: &str,
    task_id: &str,
) -> usize {
    fixture
        .jobs_for_instance(instance_id)
        .into_iter()
        .filter(|job| job.payload.get("taskId").and_then(Value::as_str) == Some(task_id))
        .count()
}
