//! Assay/QA lane. Policy in qa_repair; provenance in qa_assert; collect in assay.
//!
//! Assay is the lane that MEASURES instead of talking, and this service owns that measurement: the declared
//! commands run, their result — not a model's description of it — becomes the evidence, and the lane's own
//! verdict becomes a row of its own. RUST_CONTRACT QA takes the whole turn without a model at all, which is
//! why it is Assay and not Inspector. The execution lifecycle is inherited, not copied (see `roles::lifecycle`).

use crate::engine::assay::{AssayVerdict, CommandResult};
use crate::engine::facts::{marker_evidence, ForgeGateEvidence};
use crate::engine::phase::{lane_deliverable_kind, PhaseDeliverableKind, RoleEffectPorts};
use crate::engine::qa_adjudicate::{
    adjudicate_frozen_judgments, resolve_planned_check, CheckObservation,
};
use crate::engine::qa_plan::{validate_frozen_snapshot, ApprovedAssayPlan, PlanIdentity};
use crate::engine::role_mapping::LaneId;
use crate::engine::runner::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::roles::hooks::ForgeRoleHooks;
use crate::roles::lifecycle::{hold_rejected_deliverable, ForgeRoleContext};
use crate::roles::service::{AbstractForgeService, ForgeServiceDescriptor};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use workflow::{Result, WorkflowError};

pub use crate::engine::assay::{adjudicate_assay, collect_assay_evidence};
pub use crate::engine::qa_assert::assertion_resolution;
pub use crate::engine::qa_repair::route_qa_result;

pub const ASSAY_SERVICE_ID: &str = "forge.assay";

/// The nodes whose turn MEASURES: the verification family, and nothing else.
pub fn is_measurement_node(node_id: &str) -> bool {
    matches!(node_id, "qa_verify" | "fast_qa_verify")
}

/// Assay's own reading, supplied to the shared lifecycle as this lane's hooks.
pub struct AssayHooks;

impl ForgeRoleHooks for AssayHooks {
    /// A measurement node's verdict comes from its frozen plan and observed commands; a model may not state it.
    fn collect_evidence(
        &self,
        node_id: &str,
        evidence: ForgeGateEvidence,
        raw: &str,
        _ports: &RoleEffectPorts,
    ) -> std::result::Result<ForgeGateEvidence, String> {
        let mut next = marker_evidence(node_id, raw, &evidence);
        if is_measurement_node(node_id) {
            next.qa_passed = evidence.qa_passed;
        }
        Ok(next)
    }

    /// The model turn of a measurement node owes nothing: the verdict is produced by the commands, after the turn,
    /// every time. Asking the turn for it (the lane default, `QaVerdict`) called every turn's verdict missing,
    /// self-healed it, and paid for a second model turn on every verification.
    fn deliverable_kind(&self, node_id: &str) -> PhaseDeliverableKind {
        if is_measurement_node(node_id) {
            PhaseDeliverableKind::None
        } else {
            lane_deliverable_kind(node_id)
        }
    }

    /// Measurement is deterministic for every QA verification run. Reconciliation and command
    /// execution happen before any model turn, so a prior durable receipt needs no model call.
    fn turn_without_model(
        &self,
        ctx: &ForgeRoleContext<'_>,
        node_id: &str,
        task: &ActiveForgeRoleTask,
    ) -> Option<Result<ForgeRoleOutcome>> {
        if is_measurement_node(node_id) {
            return Some(run_frozen_assay(
                ctx,
                node_id,
                task,
                ctx.test_mode == Some("RUST_CONTRACT"),
            ));
        }
        None
    }
}

/// Forge-internal service for deterministic QA/Assay verification lanes.
///
/// Assay remains read/test/report only: it writes the measurement and the verdict it produced, and it owns
/// no repair. Repair policy stays in `qa_repair`, provenance in `qa_assert`.
pub struct AssayService<'a> {
    runner: &'a dyn ForgeRoleRunner,
}

impl<'a> AssayService<'a> {
    pub fn new(runner: &'a dyn ForgeRoleRunner) -> Self {
        Self { runner }
    }
}

impl AbstractForgeService for AssayService<'_> {
    fn descriptor(&self) -> ForgeServiceDescriptor {
        ForgeServiceDescriptor {
            service_id: ASSAY_SERVICE_ID,
            lane: LaneId::Assay,
            description: "Forge verification service for Assay/QA execution lanes",
        }
    }

    fn runner(&self) -> &dyn ForgeRoleRunner {
        self.runner
    }

    fn hooks(&self) -> &dyn ForgeRoleHooks {
        &AssayHooks
    }
}

/// The Assay lane's model-free road.
///
/// RUST_CONTRACT QA measures the application instead of asking a model to describe it, so no harness
/// turn is spent here — and none may be, or the verdict would depend on a model's willingness to run
/// the command. It is reached through [`AssayHooks::turn_without_model`], which is why it
/// returns the whole outcome: a lane that needs no turn is a lane the shared lifecycle hands the
/// whole turn to.
///
/// It lives in the lane that owns the measurement, and no other lane can reach it.
fn run_frozen_assay(
    ctx: &ForgeRoleContext<'_>,
    node_id: &str,
    task: &ActiveForgeRoleTask,
    rust_contract: bool,
) -> Result<ForgeRoleOutcome> {
    let story_id = task.story_id.as_str();
    if story_id.trim().is_empty() {
        return Err(WorkflowError::generic(format!(
            "role task {} carries no story id; refusing QA",
            task.task_id
        )));
    }

    let current = ctx.current.clone();
    let (mut evidence, verdict) = measure_frozen_plan(ctx, task, node_id, current, rust_contract)?;
    dispose_failure(&mut evidence, &verdict);
    apply_unproven_tristate(&mut evidence, &verdict);
    if let Some(reason) = evidence.deliverable_rejection.as_deref() {
        hold_rejected_deliverable(ctx, task, node_id, reason)?;
    }

    Ok(ForgeRoleOutcome {
        transition_name: Some("complete".into()),
        evidence,
    })
}

/// The route a measured failure takes when the turn named none.
///
/// The QA failure route REPAIRS only on a `disposition`, and that came from nowhere but a model's reply — so a
/// failed command held every story whose QA turn happened not to say REPAIR, and (before the decision fix) the route
/// fell through to Smith without one. Assay owns the measurement, so it owns this reading: a command that FAILED is
/// the implementation's to repair, inside the repair budget; a verdict with nothing measured (UNPROVEN) is not
/// something another Smith turn can fix, so it escalates to a person. A disposition the turn did state stands.
pub fn dispose_failure(evidence: &mut ForgeGateEvidence, verdict: &AssayVerdict) {
    if evidence.qa_passed != Some(false) || evidence.disposition.is_some() {
        return;
    }
    evidence.disposition = Some(
        match verdict {
            AssayVerdict::Fail => "REPAIR",
            AssayVerdict::Pass | AssayVerdict::Unproven => "ESCALATE",
        }
        .into(),
    );
}

/// WO-V1 tri-state persist: UNPROVEN is "measured nothing", and the durable boolean has three
/// states — `Some(true)` proved, `Some(false)` failed, `None` unproven. The collector in
/// `engine::assay` writes `Some(false)` for every non-pass (it only knows the gate boolean), so the
/// lane that OWNS the three-way verdict corrects it here: AFTER `dispose_failure` (which reads the
/// boolean to set ESCALATE) and BEFORE the evidence is returned or persisted. The artifact token
/// stays UNPROVEN throughout and the rejection/disposition survive; only the boolean stops claiming
/// a failure nobody measured — which is what used to make an unmeasured run report
/// `agree (Failed/FAIL)` in the doctor.
pub fn apply_unproven_tristate(evidence: &mut ForgeGateEvidence, verdict: &AssayVerdict) {
    if *verdict == AssayVerdict::Unproven {
        evidence.qa_passed = None;
    }
}

/// The artifact a QA lane's own measurement becomes (migration 130, `kind = 'qa-assay-evidence'`).
///
/// The verdict is the lane's **own** reading: `PASS` when the assay passed, `FAIL` when it did not, and `UNPROVEN`
/// when the lane measured nothing at all — three answers, because collapsing "not proven" into "failed" is a verdict
/// nobody measured. `tool` is `assay` and the summary is the blocker text, so a reader gets the reason with the
/// reading.
pub fn assay_tool_artifact(
    story_id: &str,
    story_run_id: Option<&str>,
    evidence: &ForgeGateEvidence,
    verdict: AssayVerdict,
) -> db::NewToolArtifact {
    let verdict = match verdict {
        AssayVerdict::Pass => "PASS",
        AssayVerdict::Fail => "FAIL",
        AssayVerdict::Unproven => "UNPROVEN",
    };
    db::NewToolArtifact {
        story_id: story_id.to_string(),
        story_run_id: story_run_id.map(str::to_string),
        tool: "assay".to_string(),
        kind: "qa-assay-evidence".to_string(),
        verdict: Some(verdict.to_string()),
        summary: evidence
            .deliverable_rejection
            .clone()
            .or_else(|| evidence.last_failure.clone()),
        detail: None,
        // QA reports measurements only. Candidate and release lineage belong to Forge's
        // publisher path and must never be attached to a QA finding.
        sha: None,
        idempotency_key: None,
    }
}

fn measure_frozen_plan(
    ctx: &ForgeRoleContext<'_>,
    task: &ActiveForgeRoleTask,
    node_id: &str,
    mut evidence: ForgeGateEvidence,
    rust_contract: bool,
) -> Result<(ForgeGateEvidence, AssayVerdict)> {
    let started_at = chrono::Utc::now().to_rfc3339();
    let mut results = Vec::new();
    let mut timings = Vec::new();
    let mut plan: Option<ApprovedAssayPlan> = None;
    let mut identity: Option<PlanIdentity> = None;
    let mut approval = None;
    let mut plan_errors = Vec::new();

    if let (Some(writer), Some(run_id)) = (ctx.writer, ctx.story_run_id) {
        let snapshot_row = writer.read_assay_plan_snapshot(run_id).map_err(|error| {
            WorkflowError::generic(format!("read assay plan snapshot: {error}"))
        })?;
        match snapshot_row {
            None => plan_errors.push("assay plan snapshot missing for Story Run".into()),
            Some(row) if row.story_id != task.story_id => {
                plan_errors.push("assay plan snapshot belongs to another Story".into())
            }
            Some(row) => {
                let commands = row
                    .assay_commands_snapshot
                    .unwrap_or_default()
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                match row.snapshot.as_ref() {
                    None => plan_errors.push("assay plan snapshot absent (legacy run)".into()),
                    Some(snapshot) => {
                        match validate_frozen_snapshot(snapshot, &row.story_id, &commands) {
                            Ok(validated) => {
                                let actual_environment = ctx.harness.assay_environment_identity();
                                let environment_mismatch =
                                    validated.plan.commands.iter().any(|command| {
                                        command.environment_identity != actual_environment
                                    });
                                if environment_mismatch {
                                    plan_errors.push(format!(
                                        "assay plan environment identity does not match executor policy {actual_environment}"
                                    ));
                                } else if rust_contract
                                    && !validated.plan.conditions.iter().any(|condition| {
                                        condition.judgment
                                    == crate::engine::qa_plan::AcceptanceJudgment::TestArtifact
                                    })
                                {
                                    plan_errors.push(
                                        "RUST_CONTRACT plan lacks test-artifact acceptance".into(),
                                    );
                                } else {
                                    identity = Some(validated.identity);
                                    approval = Some((validated.approved_by, validated.approved_at));
                                    plan = Some(validated.plan);
                                }
                            }
                            Err(errors) => plan_errors.extend(errors),
                        }
                    }
                }
            }
        }
    } else {
        plan_errors.push("assay requires a durable Story Run and state writer".into());
    }

    let idempotency_key = ctx.story_run_id.map(|run_id| {
        format!(
            "forge-assay-v2:{run_id}:{}:{node_id}:{}",
            task.task_id,
            identity
                .as_ref()
                .map(|id| id.hash.as_str())
                .unwrap_or("plan-missing")
        )
    });
    if let (Some(writer), Some(key)) = (ctx.writer, idempotency_key.as_deref()) {
        if let Some(receipt) = writer
            .read_assay_receipt(key)
            .map_err(|error| WorkflowError::generic(format!("read assay receipt: {error}")))?
        {
            if receipt.story_id != task.story_id
                || receipt.story_run_id.as_deref() != ctx.story_run_id
                || receipt.idempotency_key != key
                || receipt
                    .detail
                    .as_ref()
                    .and_then(|detail| detail.get("task_id"))
                    .and_then(Value::as_str)
                    != Some(task.task_id.as_str())
                || receipt
                    .detail
                    .as_ref()
                    .and_then(|detail| detail.get("plan_sha256"))
                    .and_then(Value::as_str)
                    != identity.as_ref().map(|identity| identity.hash.as_str())
                || receipt
                    .detail
                    .as_ref()
                    .and_then(|detail| detail.get("measurement_node"))
                    .and_then(Value::as_str)
                    != Some(node_id)
                || receipt
                    .detail
                    .as_ref()
                    .and_then(|detail| detail.get("receipt_schema_version"))
                    .and_then(Value::as_u64)
                    != Some(2)
                || receipt.detail.as_ref().is_some_and(|detail| {
                    detail.get("candidate_sha").is_some()
                        || detail.get("candidate_attestation_source").is_some()
                })
                || receipt.sha.is_some()
            {
                return Err(WorkflowError::generic(
                    "assay receipt idempotency key conflicts with run, plan, or measurement node",
                ));
            }
            let detail = receipt.detail.as_ref().expect("validated receipt detail");
            let verdict = detail
                .get("gate_verdict")
                .and_then(Value::as_str)
                .and_then(parse_verdict)
                .ok_or_else(|| {
                    WorkflowError::generic("stored assay receipt has no valid gate verdict")
                })?;
            let row_verdict = match verdict {
                AssayVerdict::Pass => "PASS",
                AssayVerdict::Fail => "FAIL",
                AssayVerdict::Unproven => "UNPROVEN",
            };
            if receipt.verdict.as_deref() != Some(row_verdict) {
                return Err(WorkflowError::generic(
                    "stored assay receipt detail disagrees with its durable verdict",
                ));
            }
            evidence.qa_passed = Some(verdict == AssayVerdict::Pass);
            if let Some(blockers) = detail.get("blockers").and_then(Value::as_array) {
                let blockers = blockers
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>();
                if verdict != AssayVerdict::Pass {
                    evidence.deliverable_rejection =
                        Some(format!("QA {verdict:?}: {}", blockers.join("; ")));
                }
            }
            if rust_contract
                && detail
                    .get("product_judgment")
                    .and_then(|value| value.get("verdict"))
                    .and_then(Value::as_str)
                    == Some("Fail")
            {
                evidence.last_failure = Some(
                    "RUST_CONTRACT product finding restored from durable assay receipt".into(),
                );
            }
            attach_assay_receipt_identity(
                &mut evidence,
                &receipt.id,
                ctx.story_run_id.ok_or_else(|| {
                    WorkflowError::generic("stored assay receipt has no Story Run identity")
                })?,
                key,
                node_id,
                detail,
            )?;
            return Ok((evidence, verdict));
        }
    }

    if let Some(valid_plan) = plan.as_ref() {
        for command in &valid_plan.commands {
            let began = chrono::Utc::now().to_rfc3339();
            let result = ctx.harness.run_command(&command.command);
            let ended = chrono::Utc::now().to_rfc3339();
            timings.push((command.id.clone(), began, ended));
            results.push(result);
        }
    }

    let mut product_report = None;
    let mut artifact_report = None;
    let mut blockers = plan_errors.clone();
    let mut verdict = AssayVerdict::Unproven;
    if let Some(valid_plan) = plan.as_ref() {
        let judgments = adjudicate_frozen_judgments(valid_plan, &results);
        blockers.extend(
            judgments
                .product
                .blockers
                .clone()
                .into_iter()
                .map(|b| format!("PRODUCT {b}")),
        );
        if rust_contract {
            blockers.extend(
                judgments
                    .test_artifact
                    .blockers
                    .clone()
                    .into_iter()
                    .map(|b| format!("ARTIFACT {b}")),
            );
        }
        let gate = if rust_contract {
            &judgments.test_artifact
        } else {
            &judgments.product
        };
        verdict = gate.verdict.clone();
        product_report = Some(judgments.product);
        artifact_report = rust_contract.then_some(judgments.test_artifact);
    }

    if !plan_errors.is_empty() {
        verdict = AssayVerdict::Unproven;
    }

    evidence.qa_passed = Some(verdict == AssayVerdict::Pass);
    if rust_contract {
        if let Some(product) = product_report.as_ref() {
            if product.verdict == AssayVerdict::Fail {
                evidence.last_failure = Some(format!(
                    "RUST_CONTRACT product finding: {}",
                    product.blockers.join("; ")
                ));
            }
        }
    }
    if verdict != AssayVerdict::Pass {
        evidence.deliverable_rejection = Some(format!(
            "QA {:?}: {}",
            verdict,
            if blockers.is_empty() {
                "acceptance plan did not prove its stated criteria".to_string()
            } else {
                blockers.join("; ")
            }
        ));
    }

    let ended_at = chrono::Utc::now().to_rfc3339();
    let detail = build_assay_receipt(
        task,
        node_id,
        ctx.story_run_id,
        plan.as_ref(),
        identity.as_ref(),
        approval.as_ref(),
        &results,
        &timings,
        product_report.as_ref(),
        artifact_report.as_ref(),
        rust_contract,
        &verdict,
        &blockers,
        &started_at,
        &ended_at,
    );
    let writer = ctx
        .writer
        .ok_or_else(|| WorkflowError::generic("QA measurement requires a durable state writer"))?;
    let run_id = ctx
        .story_run_id
        .ok_or_else(|| WorkflowError::generic("QA measurement requires a durable Story Run"))?;
    let key = idempotency_key.as_deref().ok_or_else(|| {
        WorkflowError::generic("QA measurement requires a durable receipt idempotency key")
    })?;
    let artifact = assay_receipt_artifact(
        &task.story_id,
        Some(run_id),
        &evidence,
        verdict.clone(),
        detail,
        Some(key.to_string()),
    );
    let artifact_id = writer
        .record_tool_artifact(&artifact)
        .map_err(|error| {
            WorkflowError::generic(format!(
                "persist assay receipt for {}: {error}",
                task.story_id
            ))
        })?
        .ok_or_else(|| WorkflowError::generic("assay receipt writer returned no durable row id"))?;
    attach_assay_receipt_identity(
        &mut evidence,
        &artifact_id,
        run_id,
        key,
        node_id,
        artifact.detail.as_ref().expect("receipt detail"),
    )?;
    Ok((evidence, verdict))
}

fn attach_assay_receipt_identity(
    evidence: &mut ForgeGateEvidence,
    artifact_id: &str,
    story_run_id: &str,
    idempotency_key: &str,
    measurement_node: &str,
    detail: &Value,
) -> Result<()> {
    let required = |field: &str| {
        detail.get(field).and_then(Value::as_str).ok_or_else(|| {
            WorkflowError::generic(format!("assay receipt detail is missing {field}"))
        })
    };
    let optional = |field: &str| detail.get(field).and_then(Value::as_str);
    evidence.extra.insert(
        "assayReceipt",
        workflow::json!({
            "artifactId": artifact_id,
            "storyRunId": story_run_id,
            "idempotencyKey": idempotency_key,
            "measurementNode": measurement_node,
            "gateVerdict": required("gate_verdict")?,
            "planSha256": optional("plan_sha256"),
        }),
    );
    Ok(())
}

fn build_assay_receipt(
    task: &ActiveForgeRoleTask,
    node_id: &str,
    run_id: Option<&str>,
    plan: Option<&ApprovedAssayPlan>,
    identity: Option<&PlanIdentity>,
    approval: Option<&(String, String)>,
    results: &[CommandResult],
    timings: &[(String, String, String)],
    product: Option<&crate::engine::qa_adjudicate::QaReport>,
    artifact: Option<&crate::engine::qa_adjudicate::QaReport>,
    rust_contract: bool,
    gate_verdict: &AssayVerdict,
    blockers: &[String],
    started_at: &str,
    ended_at: &str,
) -> Value {
    let commands = plan
        .into_iter()
        .flat_map(|plan| plan.commands.iter())
        .map(|planned| {
            let result = results
                .iter()
                .find(|result| result.command == planned.command);
            let timing = timings.iter().find(|(id, _, _)| id == &planned.id);
            let (status, exit_code, excerpt, truncated) = match result {
                None => ("absent", None, String::new(), false),
                Some(result) => {
                    let status = if crate::engine::assay::is_cmd_cancelled(result) {
                        "cancelled"
                    } else if crate::engine::assay::is_cmd_timeout(result) {
                        "timed_out"
                    } else if crate::engine::assay::is_build_failure_output(&result.output)
                        || crate::engine::assay::is_build_failure_output(&result.excerpt)
                    {
                        "build_failed"
                    } else if result.unmeasurable {
                        "unmeasurable"
                    } else if result.passed {
                        "passed"
                    } else {
                        "failed"
                    };
                    let excerpt = redact_assay_output(&result.excerpt);
                    (
                        status,
                        Some(result.exit_code),
                        excerpt,
                        result.output != result.excerpt,
                    )
                }
            };
            json!({
                "command_id": planned.id,
                "command_display": redact_assay_output(&planned.command),
                "command_sha256": hash_text(&planned.command),
                "runner": runner_name(planned.runner),
                "parser": parser_name(planned.parser),
                "working_directory": planned.working_directory,
                "environment_identity": planned.environment_identity,
                "started_at": timing.map(|(_, started, _)| started.as_str()),
                "ended_at": timing.map(|(_, _, ended)| ended.as_str()),
                "status": status,
                "cancelled_by_harness": result.is_some_and(|result| result.cancelled),
                "exit_code": exit_code,
                "output_excerpt": excerpt,
                "output_truncated": truncated,
            })
        })
        .collect::<Vec<_>>();
    let checks = plan
        .into_iter()
        .flat_map(|plan| plan.checks.iter().map(move |check| (plan, check)))
        .map(|(plan, check)| {
            let command = plan
                .commands
                .iter()
                .find(|command| command.id == check.command_id);
            let result = command.and_then(|command| {
                results
                    .iter()
                    .find(|result| result.command == command.command)
            });
            let observation = command
                .map(|command| resolve_planned_check(command, check, result))
                .unwrap_or_else(|| CheckObservation::Absent("planned_command_missing".into()));
            json!({
                "check_id": check.id,
                "command_id": check.command_id,
                "assertion": check.assertion,
                "observation": observation_name(&observation),
                "detail": format!("{observation:?}"),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "receipt_schema_version": 2,
        "story_id": task.story_id,
        "story_run_id": run_id,
        "process_instance_id": task.process_instance_id,
        "task_id": task.task_id,
        "measurement_node": node_id,
        "gate_verdict": format!("{gate_verdict:?}"),
        "plan_id": identity.map(|id| id.plan_id.as_str()),
        "plan_version": identity.map(|id| id.plan_version),
        "plan_schema_version": identity.map(|id| id.schema_version),
        "plan_sha256": identity.map(|id| id.hash.as_str()),
        "plan_approved_by": approval.map(|(by, _)| by.as_str()),
        "plan_approved_at": approval.map(|(_, at)| at.as_str()),
        "assertion_parser_version": "forge-qa-assert-v1",
        "started_at": started_at,
        "ended_at": ended_at,
        "test_artifact_judgment": {
            "applicable": rust_contract,
            "verdict": artifact.map(|report| format!("{:?}", report.verdict)),
            "blockers": artifact.map(|report| report.blockers.clone()).unwrap_or_default(),
            "negative_control_confirmed": artifact.is_some_and(|report| report.negative_control_confirmed),
        },
        "product_judgment": {
            "verdict": product.map(|report| format!("{:?}", report.verdict)),
            "blockers": product.map(|report| report.blockers.clone()).unwrap_or_default(),
            "negative_control_confirmed": product.is_some_and(|report| report.negative_control_confirmed),
        },
        "blockers": blockers,
        "commands": commands,
        "checks": checks,
        "evidence_truncated": results.iter().any(|result| result.output != result.excerpt),
        "redactions_applied": results.iter().any(|result| redact_assay_output(&result.excerpt) != result.excerpt),
        "redaction_policy": "forge-assay-redaction-v1",
    })
}

fn observation_name(observation: &CheckObservation) -> &'static str {
    match observation {
        CheckObservation::Passed => "passed",
        CheckObservation::Failed => "failed",
        CheckObservation::Skipped => "skipped",
        CheckObservation::Absent(_) => "absent",
        CheckObservation::BuildFailed => "build_failed",
        CheckObservation::TimedOut => "timed_out",
        CheckObservation::Cancelled => "cancelled",
        CheckObservation::Unmeasurable => "unmeasurable",
    }
}

fn parse_verdict(raw: &str) -> Option<AssayVerdict> {
    match raw {
        "Pass" => Some(AssayVerdict::Pass),
        "Fail" => Some(AssayVerdict::Fail),
        "Unproven" => Some(AssayVerdict::Unproven),
        _ => None,
    }
}

fn runner_name(runner: crate::engine::qa_plan::AssayRunner) -> &'static str {
    use crate::engine::qa_plan::AssayRunner;
    match runner {
        AssayRunner::RustLibtest => "rust_libtest",
        AssayRunner::Tap => "tap",
        AssayRunner::Junit => "junit",
        AssayRunner::CommandExit => "command_exit",
    }
}

fn parser_name(parser: crate::engine::qa_plan::AssayParser) -> &'static str {
    use crate::engine::qa_plan::AssayParser;
    match parser {
        AssayParser::RustLibtest => "rust_libtest",
        AssayParser::Tap => "tap",
        AssayParser::Junit => "junit",
        AssayParser::CommandExit => "command_exit",
    }
}

fn redact_assay_output(output: &str) -> String {
    output
        .lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if lower.contains("authorization:")
                || lower.contains("bearer ")
                || lower.contains("database_url=")
                || lower.contains(concat!("postgres", "://"))
                || lower.contains(concat!("postgresql", "://"))
                || lower.contains("api_key=")
                || lower.contains("token=")
            {
                "[REDACTED: credential-like output]"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn hash_text(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn assay_receipt_artifact(
    story_id: &str,
    story_run_id: Option<&str>,
    evidence: &ForgeGateEvidence,
    verdict: AssayVerdict,
    detail: Value,
    idempotency_key: Option<String>,
) -> db::NewToolArtifact {
    let mut artifact = assay_tool_artifact(story_id, story_run_id, evidence, verdict);
    artifact.sha = None;
    artifact.detail = Some(detail);
    artifact.idempotency_key = idempotency_key;
    artifact
}

#[cfg(test)]
mod tests;
