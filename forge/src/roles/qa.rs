//! Assay/QA lane. Policy in qa_repair; provenance in qa_assert; collect in assay.
//!
//! Assay is the lane that MEASURES instead of talking, and this service owns that measurement: the declared
//! commands run, their result — not a model's description of it — becomes the evidence, and the lane's own
//! verdict becomes a row of its own. RUST_CONTRACT QA takes the whole turn without a model at all, which is
//! why it is Assay and not Inspector. The execution lifecycle is inherited, not copied (see `roles::lifecycle`).

use crate::engine::assay::{collect_rust_contract_assay_evidence, AssayEvidence, AssayVerdict};
use crate::engine::executor::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::facts::{marker_evidence, ForgeGateEvidence};
use crate::engine::phase::{lane_deliverable_kind, PhaseDeliverableKind, RoleEffectPorts};
use crate::engine::role_mapping::LaneId;
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::roles::hooks::ForgeRoleHooks;
use crate::roles::lifecycle::{hold_rejected_deliverable, ForgeRoleContext, ForgeRoleTurn};
use crate::roles::service::{AbstractForgeService, ForgeServiceDescriptor};
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
    /// A measurement node's verdict is MEASURED after the turn (`read_assay_measurement`); a model may not state
    /// it. The reply's `qaPassed` is therefore not read — the verdict standing before the turn is kept until the
    /// commands replace it.
    fn collect_evidence(
        &self,
        node_id: &str,
        evidence: ForgeGateEvidence,
        raw: &str,
        _ports: &RoleEffectPorts,
    ) -> std::result::Result<ForgeGateEvidence, String> {
        let mut next = marker_evidence(raw, &evidence);
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

    /// The lane that measures instead of talking: RUST_CONTRACT QA runs the declared commands and reads
    /// the result, so it takes the whole turn and never asks a harness for one.
    fn turn_without_model(
        &self,
        ctx: &ForgeRoleContext<'_>,
        node_id: &str,
        task: &ActiveForgeRoleTask,
    ) -> Option<Result<ForgeRoleOutcome>> {
        if is_measurement_node(node_id) && ctx.test_mode == Some("RUST_CONTRACT") {
            return Some(run_rust_contract_qa(ctx, node_id, task));
        }
        None
    }

    fn interpret_turn(
        &self,
        ctx: &ForgeRoleContext<'_>,
        turn: &ForgeRoleTurn<'_>,
        evidence: &mut ForgeGateEvidence,
    ) -> Result<()> {
        read_assay_measurement(ctx, turn, evidence)
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
fn run_rust_contract_qa(
    ctx: &ForgeRoleContext<'_>,
    node_id: &str,
    task: &ActiveForgeRoleTask,
) -> Result<ForgeRoleOutcome> {
    let story_id = task.story_id.as_str();
    if story_id.trim().is_empty() {
        return Err(WorkflowError::generic(format!(
            "role task {} carries no story id; refusing RUST_CONTRACT QA",
            task.task_id
        )));
    }

    let mut current = ctx.current.clone();
    let head = ctx.harness.run_command("git rev-parse HEAD");
    let sha = head.output.trim();
    if head.passed && sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        current.candidate_sha = Some(sha.to_ascii_lowercase());
    } else {
        current.candidate_sha = None;
    }
    if let Some(base) = ctx.harness.execution_base_commit() {
        current
            .extra
            .insert("recordedBase", workflow::Value::from(base));
    }
    let AssayEvidence {
        mut evidence,
        verdict,
    } = collect_rust_contract_assay_evidence(
        current,
        Some(&|cmd| ctx.harness.run_command(cmd)),
        ctx.contract_assay_commands,
        ctx.contract_acceptance_mapped,
    );
    dispose_failure(&mut evidence, &verdict);

    if let Some(writer) = ctx.writer {
        writer
            .record_tool_artifact(&assay_tool_artifact(
                story_id,
                ctx.story_run_id,
                &evidence,
                verdict,
            ))
            .map_err(|error| {
                WorkflowError::generic(format!("record_tool_artifact({story_id}): {error}"))
            })?;
    }
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
    }
}

/// Assay's own reading: the QA lane MEASURES, and its measurement — not a model's description of it —
/// becomes the evidence. Deterministic verification, which is why the measurement nodes are this lane's
/// and not the Inspector's.
pub fn read_assay_measurement(
    ctx: &ForgeRoleContext<'_>,
    turn: &ForgeRoleTurn<'_>,
    evidence: &mut ForgeGateEvidence,
) -> Result<()> {
    if !is_measurement_node(turn.node_id) {
        return Ok(());
    }
    let collected = if ctx.test_mode == Some("RUST_CONTRACT") {
        collect_rust_contract_assay_evidence(
            std::mem::take(evidence),
            Some(&|cmd| ctx.harness.run_command(cmd)),
            &turn.out.assay_commands,
            ctx.contract_acceptance_mapped,
        )
    } else {
        collect_assay_evidence(
            std::mem::take(evidence),
            Some(&|cmd| ctx.harness.run_command(cmd)),
            &turn.out.assay_commands,
            turn.out.acceptance_mapped,
        )
    };
    let AssayEvidence {
        evidence: measured,
        verdict,
    } = collected;
    *evidence = measured;
    dispose_failure(evidence, &verdict);
    // The lane's own measurement becomes a row (migration 130). It is written the moment it exists, not at
    // the end of the story, because the next question anyone asks about a QA lane is what it measured — and
    // this is the only moment the measurement is in hand. A write that fails fails the lane, like every other
    // state write here: a measurement nobody can read is not evidence.
    if let Some(writer) = ctx.writer {
        writer
            .record_tool_artifact(&assay_tool_artifact(
                turn.story_id,
                ctx.story_run_id,
                evidence,
                verdict,
            ))
            .map_err(|error| {
                WorkflowError::generic(format!("record_tool_artifact({}): {error}", turn.story_id))
            })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::assay::CommandResult;
    use crate::engine::runner::{HarnessOutput, ProductionRoleRunner, RoleHarness};
    use crate::engine::writer::RecordingWriter;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A harness that runs whatever it is told and counts any turn it is asked for. Its commands pass or fail
    /// on demand, so the lane's verdict can be traced back to the measurement rather than to a model.
    struct MeasurementHarness {
        turns: AtomicUsize,
        command_passes: bool,
    }

    impl MeasurementHarness {
        fn new(command_passes: bool) -> Self {
            Self {
                turns: AtomicUsize::new(0),
                command_passes,
            }
        }

        fn turns(&self) -> usize {
            self.turns.load(Ordering::SeqCst)
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
                assay_commands: vec![],
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
            CommandResult {
                command: command.into(),
                exit_code: if self.command_passes { 0 } else { 1 },
                passed: self.command_passes,
                excerpt: String::new(),
                unmeasurable: false,
                output: String::new(),
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
        }
    }

    /// Run the measurement lane once and read back what it recorded: (model turns spent, verdict, summary).
    fn measure(harness: &MeasurementHarness, acceptance_mapped: bool) -> (usize, String, String) {
        let writer = RecordingWriter::default();
        let runner = ProductionRoleRunner::new(harness, ForgeGateEvidence::default())
            .with_writer(&writer)
            .with_test_mode(Some("RUST_CONTRACT".into()))
            .with_contract_assay_commands(vec!["cargo check --all-targets".into()])
            .with_contract_acceptance_mapped(acceptance_mapped);

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

    /// Assay is DETERMINISTIC, and this is what that word buys: the verdict is a function of what the
    /// commands did, never of a model's willingness to describe them. The lane spends no model turn at all,
    /// and its answer moves with its measurement — not proven while the acceptance is unmapped, FAILED once
    /// an authoring check actually failed. Collapsing "not proven" into "failed" would be a verdict nobody
    /// measured, which is why UNPROVEN is an answer of its own.
    #[test]
    fn the_verdict_is_the_measurement_and_not_a_models_description() {
        let passing = MeasurementHarness::new(true);
        let (turns, verdict, summary) = measure(&passing, false);
        assert_eq!(turns, 0, "the measurement lane spends no model turn");
        assert_eq!(
            verdict, "UNPROVEN",
            "an unmapped acceptance is not proven, and must not be reported as a failure"
        );
        assert!(summary.contains("acceptance mapping"), "{summary}");

        let failing = MeasurementHarness::new(false);
        let (turns, verdict, summary) = measure(&failing, true);
        assert_eq!(turns, 0, "a failing measurement still needs no model turn");
        assert_eq!(
            verdict, "FAIL",
            "a failing authoring check is a measured failure"
        );
        assert!(summary.contains("authoring checks failed"), "{summary}");
    }

    /// The lane's model-free road is the measurement lane's, and only under the contract mode: every other
    /// verification turn is a model turn like any other, and its measurement is read out of what the model's
    /// own commands produced.
    #[test]
    fn the_model_free_road_is_only_for_the_measurement_nodes() {
        assert!(is_measurement_node("qa_verify"));
        assert!(is_measurement_node("fast_qa_verify"));
        assert!(
            !is_measurement_node("qa_review"),
            "the review lane reviews; it does not measure"
        );

        let harness = MeasurementHarness::new(true);
        let writer = RecordingWriter::default();
        let runner =
            ProductionRoleRunner::new(&harness, ForgeGateEvidence::default()).with_writer(&writer);

        let _ = AssayService::new(&runner).execute("qa_verify", &qa_task());

        assert!(
            harness.turns() >= 1,
            "with no contract mode declared, this lane's work comes from a model turn"
        );
    }
}
