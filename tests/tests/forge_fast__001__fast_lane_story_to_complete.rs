//! FORGE.FAST — a FAST story goes fast_smith → fast_qa_verify → fast_publish → complete, through the production
//! definition and the durable driver; and a FAST story whose QA keeps failing stops on HOLD when its repair budget is
//! spent instead of looping.
//!
//! 192 production stories took this lane (2026-09-28..10-02) and no test drove it end to end. The second case is the
//! one the definition got wrong: `fast_qa_route` guarded `hold` behind `qaReplanEligible == true`, so a spent budget
//! matched nothing, and the engine took the first branch — back to Smith, without bound.
//!
//! Level: L1, production engine + XML + durable driver on the in-memory store; scripted role harness and release.

use std::path::Path;
use std::sync::{Arc, Mutex};

use forge::engine::assay::CommandResult;
use forge::engine::completion::MemoryLedger;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::executor::{
    drive_forge_story_with_jobs, DriveForgeStoryOptions, DriveForgeStoryResult,
    DurableForgeExecution,
};
use forge::engine::facts::ForgeGateEvidence;
use forge::engine::harness::TurnTermination;
use forge::engine::job::WorkflowJobService;
use forge::engine::qa_plan::{
    AcceptanceJudgment, ApprovedAcceptanceCondition, ApprovedAssayCommand, ApprovedAssayPlan,
    ApprovedAssertionCheck, AssayParser, AssayRunner, CheckAggregation,
};
use forge::engine::runner::{HarnessOutput, ProductionRoleRunner, RoleHarness};
use forge::engine::runtime::{ActiveForgeRoleTask, ForgeRuntime};
use forge::engine::writer::{ForgeEvidenceReader, ForgeReleaseExecutor, RecordingWriter};
use forge::roles::ForgeLaneServices;
use workflow::{
    ApplicationCommandOutcome, ApplicationCommandResult, MemoryStore, ProcessOutcome,
    ProcessStatus, Result as WfResult, Value,
};

const STORY: &str = "TST-FORGE-FAST-001";
const RUN: &str = "TST-FORGE-FAST-RUN-001";
const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CANDIDATE: &str = "0123456789abcdef0123456789abcdef01234567";

/// The roles, scripted: Smith delivers a candidate; Assay measures one command whose result the case decides.
struct FastHarness {
    calls: Mutex<Vec<String>>,
    qa_passes: bool,
}

impl RoleHarness for FastHarness {
    fn run_role(
        &self,
        node: &str,
        _task: &ActiveForgeRoleTask,
        _self_heal: Option<&str>,
    ) -> WfResult<HarnessOutput> {
        self.calls.lock().unwrap().push(node.to_string());
        let (raw, candidate, assay) = match node {
            "fast_smith" | "fast_repair_smith" => {
                ("candidate committed\n", Some(CANDIDATE.to_string()), vec![])
            }
            "fast_qa_verify" => ("verifying\n", None, vec!["cargo test -p forge".to_string()]),
            other => panic!("the FAST lane must not reach {other}"),
        };
        Ok(HarnessOutput {
            raw: raw.into(),
            candidate_sha: candidate,
            assay_commands: assay,
            acceptance_mapped: true,
            refusal: None,
            execution_base: Some(BASE.into()),
            usage: None,
        })
    }
    fn exists_on_base_ref(&self, _: &str, _: &str) -> bool {
        true
    }
    fn assay_cwd(&self) -> &Path {
        Path::new(".")
    }
    fn execution_base_commit(&self) -> Option<&str> {
        Some(BASE)
    }
    fn candidate_probe(&self) -> Option<&dyn forge::engine::runner::CandidateProbe> {
        Some(self)
    }
    fn run_command(&self, command: &str) -> CommandResult {
        let git_diff = command.starts_with("git diff ");
        let passed = git_diff || self.qa_passes;
        CommandResult {
            command: command.into(),
            exit_code: if passed { 0 } else { 101 },
            passed,
            excerpt: String::new(),
            unmeasurable: false,
            output: if git_diff && command.contains("--name-only") {
                "forge/src/example.rs\n".into()
            } else if git_diff {
                "diff --git a/forge/src/example.rs b/forge/src/example.rs\n+change\n".into()
            } else if self.qa_passes {
                "test fast_lane::contract ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n".into()
            } else {
                "test fast_lane::contract ... FAILED\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n".into()
            },

            cancelled: false,
        }
    }

    // This scripted harness has no subprocess to kill; each turn returns immediately. It
    // advertises the durable runner's interruption port and correctly reports no live process.
    fn supports_interrupt(&self) -> bool {
        true
    }

    fn interrupt_execution(&self, _reason: &str) -> WfResult<Option<TurnTermination>> {
        Ok(Some(TurnTermination::none()))
    }
}

impl forge::engine::runner::CandidateProbe for FastHarness {
    fn git(&self, args: &[&str]) -> Option<String> {
        if args.contains(&"rev-parse") {
            Some(CANDIDATE.into())
        } else if args.first() == Some(&"merge-base") || args.first() == Some(&"status") {
            Some(String::new())
        } else if args.first() == Some(&"diff") {
            Some("forge/src/example.rs".into())
        } else {
            None
        }
    }

    fn declared_test_mode(&self) -> Option<&str> {
        None
    }
}

/// The release, scripted: publishing the candidate succeeds and says so in the evidence the decisions read.
#[derive(Default)]
struct Release {
    evidence: Arc<Mutex<ForgeGateEvidence>>,
    commands: Mutex<Vec<String>>,
}

impl ForgeReleaseExecutor for Release {
    fn execute(&self, command_type: &str, input: &Value) -> ApplicationCommandResult {
        self.commands.lock().unwrap().push(command_type.to_string());
        if command_type == "forge.publish_candidate" {
            let mut evidence = self.evidence.lock().unwrap();
            evidence.publish_succeeded = Some(true);
            evidence.published_sha = Some(CANDIDATE.into());
        }
        ApplicationCommandResult {
            command_id: input
                .get("commandId")
                .and_then(Value::as_str)
                .unwrap_or(command_type)
                .to_string(),
            outcome: ApplicationCommandOutcome::Success,
            message: None,
        }
    }
}

/// What the decisions read: the roles' completed evidence, with the release's on top.
struct Reader {
    ledger: Arc<MemoryLedger>,
    release: Arc<Mutex<ForgeGateEvidence>>,
}

impl ForgeEvidenceReader for Reader {
    fn read(&self, story_id: &str) -> Result<ForgeGateEvidence, db::DbFailure> {
        let mut roles = self.ledger.evidence_for(story_id).unwrap_or_default();
        // The story's repair counter, as production's reader now loads it from `storyboard_story`.
        roles.repair_attempts = Some(self.ledger.repairs(story_id));
        roles.replan_attempts = Some(self.ledger.replans(story_id));
        Ok(self.release.lock().unwrap().merge_over(&roles))
    }
}

fn fast_evidence() -> ForgeGateEvidence {
    ForgeGateEvidence {
        work_type: Some("FAST".into()),
        scout_required: Some(false),
        ..Default::default()
    }
}

fn approved_fast_assay_snapshot(writer: &RecordingWriter) {
    let command = "cargo test -p forge";
    let plan = ApprovedAssayPlan {
        schema_version: 1,
        plan_id: "plan-fast-lane-test".into(),
        plan_version: 1,
        commands: vec![ApprovedAssayCommand {
            id: "cmd-check".into(),
            command: command.into(),
            runner: AssayRunner::RustLibtest,
            parser: AssayParser::RustLibtest,
            working_directory: "lane_root".into(),
            environment_identity: "forge-inherited-shell-v1".into(),
        }],
        checks: vec![ApprovedAssertionCheck {
            id: "check-fast-lane".into(),
            command_id: "cmd-check".into(),
            assertion: "fast_lane::contract".into(),
        }],
        conditions: vec![ApprovedAcceptanceCondition {
            id: "AC-product".into(),
            check_ids: vec!["check-fast-lane".into()],
            aggregation: CheckAggregation::AllRequired,
            judgment: AcceptanceJudgment::Product,
        }],
        negative_control: None,
    };
    let identity = plan.identity().expect("plan hashes");
    writer.assay_plan_snapshots.lock().unwrap().insert(
        RUN.into(),
        db::forge_assay::AssayPlanSnapshotRow {
            story_run_id: RUN.into(),
            story_id: STORY.into(),
            assay_commands_snapshot: Some(command.into()),
            snapshot: Some(serde_json::json!({
                "plan": plan,
                "approved_by": "test-operator",
                "approved_at": "2026-10-09T12:00:00Z",
                "approved_hash": identity.hash,
            })),
        },
    );
}

struct Run {
    out: DriveForgeStoryResult,
    status: ProcessStatus,
    outcome: Option<ProcessOutcome>,
    calls: Vec<String>,
    commands: Vec<String>,
}

fn drive_fast(qa_passes: bool) -> Run {
    let memory = MemoryStore::new();
    let ledger = Arc::new(MemoryLedger::new());
    let writer = Arc::new(RecordingWriter::default());
    approved_fast_assay_snapshot(&writer);
    let release = Arc::new(Release::default());
    let reader: Arc<dyn ForgeEvidenceReader> = Arc::new(Reader {
        ledger: ledger.clone(),
        release: release.evidence.clone(),
    });
    let rt = ForgeRuntime::from_store(
        memory,
        writer.clone(),
        Some(release.clone() as Arc<dyn ForgeReleaseExecutor>),
        Some(reader.clone()),
        ledger,
        forge_sdlc_definition(),
    )
    .expect("the production definition seeds");
    let harness = Arc::new(FastHarness {
        calls: Mutex::new(Vec::new()),
        qa_passes,
    });
    let runner = ProductionRoleRunner::new(harness.clone(), fast_evidence())
        .with_evidence_reader(Some(reader))
        .with_story_run(Some(RUN.into()))
        .with_writer(writer.clone());
    let lanes = ForgeLaneServices::new(&runner);
    let registry = lanes.registry().expect("the production lane composition");
    let jobs = WorkflowJobService::new(rt.engine());
    let out = drive_forge_story_with_jobs(
        &rt,
        STORY,
        DriveForgeStoryOptions {
            work_type: "FAST",
            evidence: fast_evidence(),
            runner: None,
            max_steps: 40,
            worker_id: "forge-fast-worker",
            within_story_concurrency: 1,
            stop_after: None,
            turn_cap: 40,
        },
        DurableForgeExecution {
            jobs: &jobs,
            registry: &registry,
        },
    )
    .expect("the FAST story drives");
    let instance = rt
        .engine()
        .get_process_instance(&out.instance_id)
        .expect("the instance reads");
    let calls = harness.calls.lock().unwrap().clone();
    let commands = release.commands.lock().unwrap().clone();
    Run {
        out,
        status: instance.status,
        outcome: instance.outcome,
        calls,
        commands,
    }
}

#[test]
fn a_fast_story_goes_smith_to_qa_to_publish_to_complete() {
    let run = drive_fast(true);
    assert_eq!(run.calls, vec!["fast_smith"], "{:?}", run.out);
    assert_eq!(run.out.steps, vec!["fast_smith", "fast_qa_verify"]);
    assert_eq!(run.commands, vec!["forge.publish_candidate"]);
    assert_eq!(run.status, ProcessStatus::Completed, "{:?}", run.out);
    assert_eq!(run.outcome, Some(ProcessOutcome::Completed));
    assert!(!run.out.needs_human);
}

#[test]
fn a_fast_story_whose_qa_keeps_failing_holds_when_the_repair_budget_is_spent() {
    let run = drive_fast(false);
    let repairs = run
        .calls
        .iter()
        .filter(|node| *node == "fast_repair_smith")
        .count();
    assert!(
        repairs >= 1,
        "a failing QA is repaired first: {:?}",
        run.calls
    );
    assert!(
        repairs <= 3,
        "the repair budget bounds the loop instead of the step cap: {repairs} repairs, {:?}",
        run.calls
    );
    assert!(
        run.commands.is_empty(),
        "a failing candidate is never published"
    );
    assert!(
        run.out.needs_human,
        "the spent budget ends on HOLD: {:?}",
        run.out
    );
}
