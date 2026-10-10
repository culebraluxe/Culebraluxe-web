//! Assay/QA lane. Policy in qa_repair; provenance in qa_assert; collect in assay.
//!
//! Assay is the lane that MEASURES instead of talking, and this service owns that measurement: the declared
//! commands run, their result — not a model's description of it — becomes the evidence, and the lane's own
//! verdict becomes a row of its own. RUST_CONTRACT QA takes the whole turn without a model at all, which is
//! why it is Assay and not Inspector. The execution lifecycle is inherited, not copied (see `roles::lifecycle`).

use crate::engine::assay::{AssayVerdict, CommandResult};
use crate::engine::executor::drive::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::facts::{marker_evidence, ForgeGateEvidence};
use crate::engine::phase::{lane_deliverable_kind, PhaseDeliverableKind, RoleEffectPorts};
use crate::engine::qa_adjudicate::{
    adjudicate_frozen_judgments, resolve_planned_check, CheckObservation,
};
use crate::engine::qa_plan::{validate_frozen_snapshot, ApprovedAssayPlan, PlanIdentity};
use crate::engine::role_mapping::LaneId;
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

    let mut current = ctx.current.clone();
    if let Some(base) = ctx.harness.execution_base_commit() {
        current
            .extra
            .insert("recordedBase", workflow::Value::from(base));
    }
    let (mut evidence, verdict) = measure_frozen_plan(ctx, task, node_id, current, rust_contract)?;
    dispose_failure(&mut evidence, &verdict);
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
        sha: evidence.candidate_sha.clone(),
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

    let candidate = normalize_candidate_sha(evidence.candidate_sha.as_deref());
    let mut candidate_workspace_matches = false;
    let idempotency_key = ctx.story_run_id.map(|run_id| {
        format!(
            "forge-assay-v1:{run_id}:{}:{node_id}:{}:{}",
            task.task_id,
            identity
                .as_ref()
                .map(|id| id.hash.as_str())
                .unwrap_or("plan-missing"),
            candidate.as_deref().unwrap_or("candidate-missing")
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
                    .and_then(|detail| detail.get("candidate_sha"))
                    .and_then(Value::as_str)
                    != candidate.as_deref()
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
                    != Some(1)
                || normalize_candidate_sha(receipt.sha.as_deref()) != candidate
            {
                return Err(WorkflowError::generic(
                    "assay receipt idempotency key conflicts with run, plan, candidate, or measurement node",
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
            evidence.qa_verified_sha = (verdict == AssayVerdict::Pass)
                .then(|| candidate.clone())
                .flatten();
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

    // The candidate checkout authorizes a NEW measurement only. A valid durable receipt above is
    // self-contained evidence and can be replayed after its worktree has advanced or disappeared.
    if let Some(candidate) = candidate.as_deref() {
        match ctx.harness.candidate_probe() {
            Some(probe) => {
                let actual = probe
                    .workspace_head(ctx.harness.assay_cwd())
                    .and_then(|sha| normalize_candidate_sha(Some(&sha)));
                candidate_workspace_matches = actual.as_deref() == Some(candidate);
                if !candidate_workspace_matches {
                    plan_errors.push(match actual {
                        Some(actual) => format!(
                            "candidate workspace HEAD {actual} does not match reviewed candidate {candidate}"
                        ),
                        None => "candidate workspace HEAD could not be verified".into(),
                    });
                }
            }
            None => plan_errors.push("candidate workspace Git probe is unavailable".into()),
        }
    }

    if candidate_workspace_matches {
        if let Some(valid_plan) = plan.as_ref() {
            for command in &valid_plan.commands {
                let began = chrono::Utc::now().to_rfc3339();
                let result = ctx.harness.run_command(&command.command);
                let ended = chrono::Utc::now().to_rfc3339();
                timings.push((command.id.clone(), began, ended));
                results.push(result);
            }
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

    if candidate.is_none() {
        blockers.push("CANDIDATE_IDENTITY_MISSING".into());
        verdict = AssayVerdict::Unproven;
    }
    if !plan_errors.is_empty() {
        verdict = AssayVerdict::Unproven;
    }

    evidence.qa_passed = Some(verdict == AssayVerdict::Pass);
    evidence.qa_verified_sha = (verdict == AssayVerdict::Pass)
        .then(|| candidate.clone())
        .flatten();
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
                "acceptance plan did not prove the candidate".to_string()
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
        candidate.as_deref(),
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
            "candidateSha": optional("candidate_sha"),
        }),
    );
    Ok(())
}

fn build_assay_receipt(
    task: &ActiveForgeRoleTask,
    node_id: &str,
    run_id: Option<&str>,
    candidate_sha: Option<&str>,
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
        "receipt_schema_version": 1,
        "story_id": task.story_id,
        "story_run_id": run_id,
        "process_instance_id": task.process_instance_id,
        "task_id": task.task_id,
        "measurement_node": node_id,
        "gate_verdict": format!("{gate_verdict:?}"),
        "candidate_sha": candidate_sha,
        "candidate_attestation_source": "durable_evidence_matched_to_assay_workspace_head",
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
    artifact.sha = detail
        .get("candidate_sha")
        .and_then(Value::as_str)
        .map(str::to_owned);
    artifact.detail = Some(detail);
    artifact.idempotency_key = idempotency_key;
    artifact
}

fn normalize_candidate_sha(candidate: Option<&str>) -> Option<String> {
    candidate
        .map(str::trim)
        .filter(|sha| sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::assay::CommandResult;
    use crate::engine::qa_plan::{
        AcceptanceJudgment, ApprovedAcceptanceCondition, ApprovedAssayCommand, ApprovedAssayPlan,
        ApprovedAssertionCheck, AssayParser, AssayRunner, CheckAggregation,
    };
    use crate::engine::runner::{CandidateProbe, HarnessOutput, ProductionRoleRunner, RoleHarness};
    use crate::engine::writer::RecordingWriter;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    /// A harness that runs whatever it is told and counts any turn it is asked for. Its commands pass or fail
    /// on demand, so the lane's verdict can be traced back to the measurement rather than to a model.
    struct MeasurementHarness {
        turns: AtomicUsize,
        commands_run: AtomicUsize,
        command_passes: bool,
        commands: Vec<String>,
        output: String,
        workspace_sha: std::sync::Mutex<String>,
        candidate_probes: AtomicUsize,
        probe_available: AtomicBool,
    }

    impl MeasurementHarness {
        fn new(command_passes: bool) -> Self {
            Self {
                turns: AtomicUsize::new(0),
                commands_run: AtomicUsize::new(0),
                command_passes,
                commands: Vec::new(),
                output: String::new(),
                workspace_sha: std::sync::Mutex::new("a".repeat(40)),
                candidate_probes: AtomicUsize::new(0),
                probe_available: AtomicBool::new(true),
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

        fn with_workspace_sha(mut self, sha: &str) -> Self {
            *self.workspace_sha.get_mut().unwrap() = sha.into();
            self
        }

        fn disable_candidate_probe(&self) {
            self.probe_available.store(false, Ordering::SeqCst);
        }

        fn enable_candidate_probe(&self) {
            self.probe_available.store(true, Ordering::SeqCst);
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
        fn candidate_probe(&self) -> Option<&dyn CandidateProbe> {
            self.probe_available
                .load(Ordering::SeqCst)
                .then_some(self as &dyn CandidateProbe)
        }
    }

    impl CandidateProbe for MeasurementHarness {
        fn git(&self, _: &[&str]) -> Option<String> {
            self.candidate_probes.fetch_add(1, Ordering::SeqCst);
            Some(self.workspace_sha.lock().unwrap().clone())
        }

        fn declared_test_mode(&self) -> Option<&str> {
            Some("RUST_CONTRACT")
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
                environment_identity: "forge-inherited-shell-v1".into(),
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
        evidence.candidate_sha = Some("a".repeat(40));
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
    fn a_mismatched_workspace_candidate_is_neither_measured_nor_accepted() {
        let harness = Arc::new(MeasurementHarness::new(true).with_workspace_sha(&"b".repeat(40)));
        let (turns, verdict, summary) = measure(&harness, false);
        assert_eq!(turns, 0);
        assert_eq!(harness.commands_run(), 0);
        assert_eq!(verdict, "UNPROVEN");
        assert!(summary.contains("does not match reviewed candidate"));
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
        evidence.candidate_sha = Some("a".repeat(40));
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
        evidence.candidate_sha = Some("A".repeat(40));
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
        assert_eq!(harness.candidate_probes.load(Ordering::SeqCst), 1);
        let artifact = writer.artifacts.lock().unwrap()[0].clone();
        let key = artifact.idempotency_key.clone().expect("receipt key");
        let detail = artifact.detail.clone().expect("durable detail");
        assert_eq!(detail["receipt_schema_version"], 1);
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

        harness.disable_candidate_probe();
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
        assert_eq!(
            harness.candidate_probes.load(Ordering::SeqCst),
            1,
            "durable receipt replay succeeds after the workspace probe becomes unavailable"
        );
        assert_eq!(writer.artifacts.lock().unwrap().len(), 1);

        let mut retry_task = qa_task();
        retry_task.task_id = "task-qa-retry".into();
        harness.enable_candidate_probe();
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
            evidence.candidate_sha = Some("a".repeat(40));
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
}
