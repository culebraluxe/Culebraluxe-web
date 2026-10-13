use super::*;
use crate::engine::assay::CommandResult;
use crate::engine::qa_plan::{
    AcceptanceJudgment, ApprovedAcceptanceCondition, ApprovedAssayCommand, ApprovedAssayPlan,
    ApprovedAssertionCheck, AssayParser, AssayRunner, CheckAggregation,
};
use crate::engine::runner::{HarnessOutput, ProductionRoleRunner, RoleHarness};
use crate::engine::writer::RecordingWriter;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A harness that runs whatever it is told and counts any turn it is asked for. Its commands pass or fail
/// on demand, so the lane's verdict can be traced back to the measurement rather than to a model.
struct MeasurementHarness {
    turns: AtomicUsize,
    commands_run: AtomicUsize,
    command_passes: bool,
    commands: Vec<String>,
    output: String,
}

impl MeasurementHarness {
    fn new(command_passes: bool) -> Self {
        Self {
            turns: AtomicUsize::new(0),
            commands_run: AtomicUsize::new(0),
            command_passes,
            commands: Vec::new(),
            output: String::new(),
        }
    }

    /// Commands that the packet's Story Run snapshot approves.
    fn with_commands(mut self, commands: &[&str]) -> Self {
        self.commands = commands.iter().map(|cmd| (*cmd).to_string()).collect();
        self
    }

    fn with_output(mut self, output: &str) -> Self {
        self.output = output.into();
        self
    }

    fn turns(&self) -> usize {
        self.turns.load(Ordering::SeqCst)
    }

    fn commands_run(&self) -> usize {
        self.commands_run.load(Ordering::SeqCst)
    }
}

impl RoleHarness for MeasurementHarness {
    fn run_role(
        &self,
        node_id: &str,
        _: &ActiveForgeRoleTask,
        _: Option<&str>,
    ) -> Result<HarnessOutput> {
        self.turns.fetch_add(1, Ordering::SeqCst);
        Ok(HarnessOutput {
            trajectory: None,
            raw: format!("{node_id} described its own work\n"),
            candidate_sha: None,
            assay_commands: self.commands.clone(),
            acceptance_mapped: false,
            refusal: None,
            execution_base: None,
            usage: None,
        })
    }
    fn exists_on_base_ref(&self, _: &str, _: &str) -> bool {
        true
    }
    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }
    fn run_command(&self, command: &str) -> CommandResult {
        self.commands_run.fetch_add(1, Ordering::SeqCst);
        CommandResult {
            cancelled: false,
            command: command.into(),
            exit_code: if self.command_passes { 0 } else { 1 },
            passed: self.command_passes,
            excerpt: self.output.clone(),
            unmeasurable: false,
            output: self.output.clone(),
        }
    }
}

fn qa_task() -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: "task-qa_verify".into(),
        process_instance_id: "proc-1".into(),
        story_id: "TST-ASSAY-001".into(),
        token_id: None,
        node_id: Some("qa_verify".into()),
        status: workflow::TaskStatus::Ready,
        assignee: None,
        candidates: vec!["qa_verify".into()],
        write_surface: None,
    }
}

const RUN_ID: &str = "11111111-2222-3333-4444-555555555555";

fn approved_plan_snapshot(writer: &RecordingWriter, command: &str) {
    let plan = ApprovedAssayPlan {
        schema_version: 1,
        plan_id: "qa-test-plan".into(),
        plan_version: 1,
        commands: vec![ApprovedAssayCommand {
            id: "cmd-check".into(),
            command: command.into(),
            runner: AssayRunner::CommandExit,
            parser: AssayParser::CommandExit,
            working_directory: "lane_root".into(),
            environment_identity: "forge-sanitized-assay-v2".into(),
        }],
        checks: vec![ApprovedAssertionCheck {
            id: "check-build".into(),
            command_id: "cmd-check".into(),
            assertion: "exit_code_zero".into(),
        }],
        conditions: vec![
            ApprovedAcceptanceCondition {
                id: "AC-product".into(),
                check_ids: vec!["check-build".into()],
                aggregation: CheckAggregation::AllRequired,
                judgment: AcceptanceJudgment::Product,
            },
            ApprovedAcceptanceCondition {
                id: "AC-artifact".into(),
                check_ids: vec!["check-build".into()],
                aggregation: CheckAggregation::AllRequired,
                judgment: AcceptanceJudgment::TestArtifact,
            },
        ],
        negative_control: None,
    };
    let identity = plan.identity().unwrap();
    writer.assay_plan_snapshots.lock().unwrap().insert(
        RUN_ID.into(),
        db::forge_assay::AssayPlanSnapshotRow {
            story_run_id: RUN_ID.into(),
            story_id: "TST-ASSAY-001".into(),
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

/// Run the measurement lane once and read back what it recorded: (model turns spent, verdict, summary).
fn measure(
    harness: &Arc<MeasurementHarness>,
    _acceptance_mapped: bool,
) -> (usize, String, String) {
    let writer = Arc::new(RecordingWriter::default());
    approved_plan_snapshot(&writer, "cargo check --all-targets");
    let mut evidence = ForgeGateEvidence::default();
    let runner = ProductionRoleRunner::new(harness.clone(), evidence)
        .with_writer(writer.clone())
        .with_story_run(Some(RUN_ID.into()))
        .with_test_mode(Some("RUST_CONTRACT".into()))
        .with_contract_assay_commands(vec!["cargo check --all-targets".into()])
        .with_contract_acceptance_mapped(false);

    let _ = AssayService::new(&runner).execute("qa_verify", &qa_task());

    let artifacts = writer.artifacts.lock().unwrap();
    let row = artifacts
        .iter()
        .find(|a| a.kind == "qa-assay-evidence")
        .expect("the lane's own measurement is a row");
    (
        harness.turns(),
        row.verdict.clone().unwrap_or_default(),
        row.summary.clone().unwrap_or_default(),
    )
}


/// WO-V1, end to end through the collector and the lane's own disposition: an unmapped
/// acceptance measures UNPROVEN, and the durable boolean must be NO verdict — `None`, never
/// `Some(false)`, which the doctor reads as a FAIL the Failed run agrees with. The lane's order
/// is `dispose_failure` (which READS the boolean to choose the route) and then the tri-state
/// (which corrects it), so the ESCALATE disposition and the UNPROVEN rejection survive.
#[test]
fn unproven_is_persisted_as_no_verdict_not_as_failed() {
    use crate::engine::assay::{collect_rust_contract_assay_evidence, AssayEvidence};
    let run = |command: &str| CommandResult {
        command: command.into(),
        exit_code: 0,
        passed: true,
        excerpt: String::new(),
        unmeasurable: false,
        cancelled: false,
        output: String::new(),
    };
    let AssayEvidence {
        mut evidence,
        verdict,
    } = collect_rust_contract_assay_evidence(
        ForgeGateEvidence::default(),
        Some(&run),
        &["cargo check --all-targets".into()],
        false,
    );
    assert_eq!(
        verdict,
        crate::engine::assay::AssayVerdict::Unproven,
        "an unmapped acceptance measures nothing — the bug's precondition"
    );
    assert_eq!(
        evidence.qa_passed,
        Some(false),
        "the collector only knows the gate boolean, so this is what it writes"
    );

    dispose_failure(&mut evidence, &verdict);
    apply_unproven_tristate(&mut evidence, &verdict);
    assert_eq!(
        evidence.qa_passed, None,
        "UNPROVEN is unmeasured, and an unmeasured run must not persist qa_passed=false"
    );
    assert_eq!(
        evidence.disposition.as_deref(),
        Some("ESCALATE"),
        "an unmeasured run is not another Smith turn's to repair"
    );
    assert!(
        evidence
            .deliverable_rejection
            .as_deref()
            .unwrap_or("")
            .contains("UNPROVEN"),
        "the rejection still names the state: {:?}",
        evidence.deliverable_rejection
    );
}

/// WO-V1 guard: a MEASURED failure still persists `Some(false)` with a REPAIR disposition.
/// Only the unmeasured reading changes state.
#[test]
fn measured_fail_is_still_persisted_as_failed_with_repair() {
    let harness = Arc::new(MeasurementHarness::new(false));
    let writer = Arc::new(RecordingWriter::default());
    approved_plan_snapshot(&writer, "cargo check --all-targets");
    let runner = ProductionRoleRunner::new(harness.clone(), ForgeGateEvidence::default())
        .with_writer(writer.clone())
        .with_story_run(Some(RUN_ID.into()))
        .with_test_mode(Some("RUST_CONTRACT".into()))
        .with_contract_assay_commands(vec!["cargo check --all-targets".into()])
        .with_contract_acceptance_mapped(true);

    let outcome = AssayService::new(&runner)
        .execute("qa_verify", &qa_task())
        .expect("the measurement lane runs");
    assert_eq!(outcome.evidence.qa_passed, Some(false));
    assert_eq!(outcome.evidence.disposition.as_deref(), Some("REPAIR"));
}

/// WO-V1 guard: a MEASURED pass still persists `Some(true)` — driven through the collector
/// plus the tri-state, since the turn-level rig carries no base/candidate SHAs and therefore
/// cannot reach PASS (it stops at "execution base is missing", which is itself a FAIL).
#[test]
fn measured_pass_is_still_persisted_as_passed() {
    use crate::engine::assay::{collect_rust_contract_assay_evidence, AssayEvidence};
    let sha = "a".repeat(40);
    let mut evidence = ForgeGateEvidence {
        candidate_sha: Some(sha.clone()),
        ..ForgeGateEvidence::default()
    };
    evidence
        .extra
        .insert("recordedBase", workflow::Value::from(sha.clone()));
    let run = |command: &str| CommandResult {
        command: command.into(),
        exit_code: 0,
        passed: true,
        excerpt: String::new(),
        unmeasurable: false,
        cancelled: false,
        output: String::new(),
    };
    let AssayEvidence { mut evidence, verdict } = collect_rust_contract_assay_evidence(
        evidence,
        Some(&run),
        &["cargo check --all-targets".into()],
        true,
    );
    assert_eq!(verdict, crate::engine::assay::AssayVerdict::Pass);
    dispose_failure(&mut evidence, &verdict);
    apply_unproven_tristate(&mut evidence, &verdict);
    assert_eq!(evidence.qa_passed, Some(true));
}

/// WO-V1 unit: the tri-state clears only the boolean, never the record around it — the
/// ESCALATE disposition and the UNPROVEN rejection survive, and Pass/Fail are untouched.
#[test]
fn unproven_tristate_clears_only_the_boolean_not_the_record() {
    use crate::engine::assay::AssayVerdict;
    let mut evidence = ForgeGateEvidence {
        qa_passed: Some(false),
        deliverable_rejection: Some("QA UNPROVEN: RUST_CONTRACT acceptance mapping is missing.".into()),
        disposition: Some("ESCALATE".into()),
        ..ForgeGateEvidence::default()
    };
    apply_unproven_tristate(&mut evidence, &AssayVerdict::Unproven);
    assert_eq!(evidence.qa_passed, None);
    assert_eq!(evidence.disposition.as_deref(), Some("ESCALATE"));
    assert!(evidence.deliverable_rejection.as_deref().unwrap_or("").contains("UNPROVEN"));

    let mut pass = ForgeGateEvidence {
        qa_passed: Some(true),
        ..ForgeGateEvidence::default()
    };
    apply_unproven_tristate(&mut pass, &AssayVerdict::Pass);
    assert_eq!(pass.qa_passed, Some(true));

    let mut fail = ForgeGateEvidence {
        qa_passed: Some(false),
        ..ForgeGateEvidence::default()
    };
    apply_unproven_tristate(&mut fail, &AssayVerdict::Fail);
    assert_eq!(fail.qa_passed, Some(false));
}

/// The verdict follows the approved plan and measured command result; legacy booleans do not affect it.
#[test]
fn the_verdict_is_the_measurement_and_not_a_models_description() {
    let passing = Arc::new(MeasurementHarness::new(true));
    let (turns, verdict, summary) = measure(&passing, false);
    assert_eq!(turns, 0, "the measurement lane spends no model turn");
    assert_eq!(
        verdict, "PASS",
        "the typed frozen plan controls acceptance, not the legacy bool"
    );
    assert_eq!(summary, "");

    let failing = Arc::new(MeasurementHarness::new(false));
    let (turns, verdict, summary) = measure(&failing, true);
    assert_eq!(turns, 0, "a failing measurement still needs no model turn");
    assert_eq!(
        verdict, "FAIL",
        "a failing authoring check is a measured failure"
    );
    assert!(summary.contains("CMD_FAIL"), "{summary}");
}

#[test]
fn qa_measurement_does_not_consult_candidate_identity() {
    let harness = Arc::new(MeasurementHarness::new(true));
    let (turns, verdict, summary) = measure(&harness, false);
    assert_eq!(turns, 0);
    assert_eq!(harness.commands_run(), 1);
    assert_eq!(verdict, "PASS");
    assert!(summary.is_empty());
}

/// Verification is deterministic and model-free under both product and test-authoring plans.
#[test]
fn the_model_free_road_is_for_measurement_nodes_in_every_mode() {
    assert!(is_measurement_node("qa_verify"));
    assert!(is_measurement_node("fast_qa_verify"));
    assert!(
        !is_measurement_node("qa_review"),
        "the review lane reviews; it does not measure"
    );

    let harness = Arc::new(MeasurementHarness::new(true));
    let writer = Arc::new(RecordingWriter::default());
    let runner = ProductionRoleRunner::new(harness.clone(), ForgeGateEvidence::default())
        .with_writer(writer.clone());

    let _ = AssayService::new(&runner).execute("qa_verify", &qa_task());

    assert_eq!(
        harness.turns(),
        0,
        "QA reads its approved frozen plan and spends no model turn"
    );
}

#[test]
fn legacy_boolean_without_an_approved_plan_stays_unproven() {
    let harness = Arc::new(MeasurementHarness::new(true));
    let writer = Arc::new(RecordingWriter::default());
    let mut evidence = ForgeGateEvidence::default();
    let runner = ProductionRoleRunner::new(harness, evidence)
        .with_writer(writer.clone())
        .with_story_run(Some(RUN_ID.into()))
        .with_contract_assay_commands(vec!["cargo check --all-targets".into()])
        .with_contract_acceptance_mapped(true);

    let _ = AssayService::new(&runner).execute("qa_verify", &qa_task());

    let artifacts = writer.artifacts.lock().unwrap();
    assert_eq!(artifacts[0].verdict.as_deref(), Some("UNPROVEN"));
    assert!(artifacts[0].detail.as_ref().unwrap()["blockers"]
        .to_string()
        .contains("assay plan snapshot missing"));
}

#[test]
fn durable_receipt_redacts_output_and_replays_without_rerunning_commands() {
    let harness = Arc::new(
        MeasurementHarness::new(true).with_output("TOKEN=secret-value\ncheck completed\n"),
    );
    let writer = Arc::new(RecordingWriter::default());
    approved_plan_snapshot(&writer, "cargo check --all-targets");
    let mut evidence = ForgeGateEvidence::default();
    let runner = ProductionRoleRunner::new(harness.clone(), evidence)
        .with_writer(writer.clone())
        .with_story_run(Some(RUN_ID.into()));

    let first = AssayService::new(&runner)
        .execute("qa_verify", &qa_task())
        .expect("first measurement completes");
    assert_eq!(first.evidence.qa_passed, Some(true));
    assert_eq!(
        first
            .evidence
            .extra
            .get("assayReceipt")
            .and_then(|receipt| receipt.get("artifactId"))
            .and_then(workflow::Value::as_str),
        Some("artifact-1")
    );
    assert_eq!(
        first
            .evidence
            .extra
            .get("assayReceipt")
            .and_then(|receipt| receipt.get("idempotencyKey"))
            .and_then(workflow::Value::as_str),
        writer.artifacts.lock().unwrap()[0]
            .idempotency_key
            .as_deref()
    );
    assert_eq!(harness.commands_run(), 1);
    let artifact = writer.artifacts.lock().unwrap()[0].clone();
    let key = artifact.idempotency_key.clone().expect("receipt key");
    let detail = artifact.detail.clone().expect("durable detail");
    assert_eq!(detail["receipt_schema_version"], 2);
    assert!(detail.get("candidate_sha").is_none());
    assert!(artifact.sha.is_none());
    assert_eq!(detail["plan_approved_by"], "test-operator");
    assert_eq!(detail["redactions_applied"], true);
    assert!(!detail.to_string().contains("secret-value"));
    assert!(detail.to_string().contains("REDACTED"));
    writer.assay_receipts.lock().unwrap().insert(
        key.clone(),
        db::forge_assay::AssayReceiptRow {
            id: "receipt-1".into(),
            story_id: artifact.story_id.clone(),
            story_run_id: artifact.story_run_id.clone(),
            verdict: artifact.verdict.clone(),
            detail: artifact.detail.clone(),
            sha: artifact.sha.clone(),
            idempotency_key: key,
        },
    );

    let replay = AssayService::new(&runner)
        .execute("qa_verify", &qa_task())
        .expect("stored receipt reconciles");
    assert_eq!(replay.evidence.qa_passed, Some(true));
    assert_eq!(
        replay
            .evidence
            .extra
            .get("assayReceipt")
            .and_then(|receipt| receipt.get("artifactId"))
            .and_then(workflow::Value::as_str),
        Some("receipt-1"),
        "replay carries the identity of the durable row it read"
    );
    assert_eq!(
        harness.commands_run(),
        1,
        "reconciliation does not rerun a command"
    );
    assert_eq!(writer.artifacts.lock().unwrap().len(), 1);

    let mut retry_task = qa_task();
    retry_task.task_id = "task-qa-retry".into();
    AssayService::new(&runner)
        .execute("qa_verify", &retry_task)
        .expect("a new task invocation measures again");
    assert_eq!(harness.commands_run(), 2);
    let artifacts = writer.artifacts.lock().unwrap();
    assert_eq!(artifacts.len(), 2);
    assert_ne!(
        artifacts[0].idempotency_key, artifacts[1].idempotency_key,
        "a new workflow task has a new measurement receipt key"
    );
}

/// OFF THE CONTRACT PATH THE MAP COMES FROM THE ROW, NOT FROM A MODEL (2026-10-03).
///
/// The non-contract branch read `turn.out.acceptance_mapped` — a transport field NOTHING in the product ever
/// sets true — so a story whose commands ALL passed could only ever be ruled UNPROVEN. ENG-FORGE-C1-BUILD-INFO-01
/// hit exactly that: four commands green, `failed=[]`, no PASS, and therefore nothing published; every
/// non-contract story has hit it since 2026-09-19, the last time one passed. The row already carries the rule
/// the contract path measures (every assay command appears in the acceptance criteria), so this branch reads it
/// too — while UNPROVEN stays the answer when the row does not map the commands, because that is a finding
/// about the story and not a failure to paper over.
#[test]
fn a_non_contract_story_requires_the_frozen_plan_not_a_boolean() {
    fn run(acceptance_mapped: bool) -> (String, String) {
        let harness = Arc::new(
            MeasurementHarness::new(true).with_commands(&["cargo check --all-targets"]),
        );
        let writer = Arc::new(RecordingWriter::default());
        approved_plan_snapshot(&writer, "cargo check --all-targets");
        let mut evidence = ForgeGateEvidence::default();
        let runner = ProductionRoleRunner::new(harness.clone(), evidence)
            .with_writer(writer.clone())
            .with_story_run(Some(RUN_ID.into()))
            .with_contract_assay_commands(vec!["cargo check --all-targets".into()])
            .with_contract_acceptance_mapped(acceptance_mapped);

        let _ = AssayService::new(&runner).execute("qa_verify", &qa_task());

        let artifacts = writer.artifacts.lock().unwrap();
        let row = artifacts
            .iter()
            .find(|a| a.kind == "qa-assay-evidence")
            .expect("the lane's own measurement is a row");
        (
            row.verdict.clone().unwrap_or_default(),
            row.summary.clone().unwrap_or_default(),
        )
    }

    let (verdict, summary) = run(true);
    assert_eq!(
        verdict, "PASS",
        "an approved frozen plan and its measured check prove acceptance: {summary}"
    );

    let (verdict, _) = run(false);
    assert_eq!(verdict, "PASS", "legacy booleans do not decide acceptance");
}
