//! The shared Forge role-turn lifecycle.
//!
//! Placement in the stack, stated once so no layer has to guess:
//!
//! ```text
//! Workflow            = state and routing (which logical role is ready)
//! JobService          = execution reliability (durable claim/lease/retry)
//! AbstractForgeService= the common agent lifecycle (THIS module)
//! concrete service    = the role's own intelligence (Smith, Architect, Lead, …)
//! OpenCodeHarness     = model/runtime mechanics
//! ```
//!
//! `ProductionRoleRunner` grew the whole turn sequence because it was the only place a turn ran.
//! That sequence is not any role's intelligence — it is the *same* sequence for every lane: the
//! execution-target guard, the bounded attempt loop with its self-heal directive, the spend meter,
//! the bench-intent cap, and the deliverable gate that ends in a hold. It lives here so a concrete
//! `AbstractForgeService` can inherit it instead of seven services copying it.
//!
//! WHAT THIS MODULE DELIBERATELY DOES NOT KNOW: Smith, Architect, Lead, Scout, Inspector, Assay and
//! DevOps. A lane's own reading arrives through [`ForgeRoleHooks`] (`roles/hooks.rs`: how its reply
//! becomes evidence, what its node owes, the decision its node must carry, and the reading that may
//! write its record), and every default there is structural — a lane that overrides nothing gets the
//! shared lifecycle and nothing role-specific. That is what makes "role intelligence lives in the role
//! service" a checkable property rather than a slogan: there is no `match node_id` anywhere in this
//! file, so a test can run a Smith-family node through the default hooks and prove no Smith behavior.

use crate::engine::execution_target::{assert_forge_execution_target, env_pairs_from_process};
use crate::engine::executor::drive::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::facts::ForgeGateEvidence;
use crate::engine::harness::HarnessUsage;
use crate::engine::hold::{
    deliverable_enforcement_enabled, parse_deliverable_reprompt_budget, OpenHold,
};
use crate::engine::observer::record_forge_observer;
use crate::engine::phase::RoleEffectPorts;
use crate::engine::runner::{ForgeTurnPorts, HarnessOutput, RoleHarness};
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::engine::self_heal::{attempt_budget, build_self_heal_directive};
use crate::engine::service_binding::lane_for_node;
use crate::engine::writer::ForgeStateWriter;
use crate::roles::hooks::ForgeRoleHooks;
use workflow::{Result, WorkflowError};

/// The execution envelope a Forge lane runs under.
///
/// A borrowed view rather than a bundle of owned state: `ProductionRoleRunner`'s fields are public
/// and assigned by callers (including tests outside this crate), so the lifecycle reads them where
/// they already live instead of taking ownership of them.
pub struct ForgeRoleContext<'a> {
    pub harness: &'a dyn RoleHarness,
    pub current: &'a ForgeGateEvidence,
    pub writer: Option<&'a dyn ForgeStateWriter>,
    /// The Story Run this lane is executing (the row a claim opened). `None` on the hand-run lane
    /// (`forge --story …`, no `--work-item`), which opens no run row.
    pub story_run_id: Option<&'a str>,
    pub bench_intent: Option<&'a str>,
    pub test_mode: Option<&'a str>,
    pub contract_assay_commands: &'a [String],
    pub contract_acceptance_mapped: bool,
    pub require_prod: bool,
}

impl<'a> ForgeRoleContext<'a> {
    /// The envelope, read off the ports whoever hosts the turn exposes.
    ///
    /// This is the join between the two halves of the extraction: the runner owns the ports, the lane
    /// service owns the turn, and the lifecycle — which knows neither — reads them here. A field added to
    /// the envelope must be added to both, which is why they sit this close together.
    pub fn from_ports(ports: &'a dyn ForgeTurnPorts) -> Self {
        Self {
            harness: ports.harness(),
            current: ports.current(),
            writer: ports.writer(),
            story_run_id: ports.story_run_id(),
            bench_intent: ports.bench_intent(),
            test_mode: ports.test_mode(),
            contract_assay_commands: ports.contract_assay_commands(),
            contract_acceptance_mapped: ports.contract_acceptance_mapped(),
            require_prod: ports.require_prod(),
        }
    }
}

/// One turn's result, as the lane's own reading sees it.
pub struct ForgeRoleTurn<'a> {
    pub node_id: &'a str,
    /// The story this lane was listed for. Never empty here: the lifecycle refuses a task that
    /// carries no story id *before* any hook runs, because the writes a lane makes are
    /// identity-bearing (`forge_hold_record.story_id` and `forge_tool_artifact.story_id` are foreign
    /// keys to `storyboard_story(id)`).
    pub story_id: &'a str,
    pub task: &'a ActiveForgeRoleTask,
    /// The merged turn: the last attempt's output carrying the FIRST attempt's execution base,
    /// because a retry's own pre-turn HEAD already contains the earlier attempts' commits.
    pub out: &'a HarnessOutput,
}

/// Run one lane's turn through the runner that hosts its ports.
///
/// THE SERVICE BOUNDARY's single entry point: a concrete service says which runner its turns run through
/// and which reading is its own, and gets the shared sequence by inheriting it — never by copying it.
///
/// A runner that exposes no ports (a double, the synthetic default) has no envelope to read a turn under,
/// so the whole turn is delegated to it. That keeps every established seam — the CLI's composition, the
/// service tests' scrubbers, the durable job lane — behaving exactly as it did before the split.
pub fn run_lane_turn(
    runner: &dyn ForgeRoleRunner,
    node_id: &str,
    task: &ActiveForgeRoleTask,
    hooks: &dyn ForgeRoleHooks,
) -> Result<ForgeRoleOutcome> {
    match runner.turn_ports() {
        Some(ports) => {
            // Each turn starts from the story's evidence as it stands NOW, not as the process was woken with.
            let mut current = ports.current_for(&task.story_id)?;
            // A rejection is the verdict on the turn that produced it. Carried into the next turn's starting
            // evidence it reads as this turn's own refusal: a repair Smith after a QA FAIL was re-prompted and paid
            // twice before it had answered once.
            current.deliverable_rejection = None;
            let ctx = ForgeRoleContext {
                current: &current,
                ..ForgeRoleContext::from_ports(ports)
            };
            run_forge_role_turn(&ctx, node_id, task, hooks)
        }
        None => runner.run(node_id, task),
    }
}

/// The envelope a lane's effects are read under.
///
/// Built here because the ports are read at more than one point in a turn, and a field wired at only
/// one of them is a half-wired rail (2026-09-29).
pub fn effect_ports(ctx: &ForgeRoleContext<'_>) -> RoleEffectPorts {
    RoleEffectPorts {
        bench_intent: ctx.bench_intent.map(str::to_string),
        ..RoleEffectPorts::default()
    }
}

/// Run one Forge role turn: the sequence every lane runs, and only that sequence.
///
/// Called by every concrete `AbstractForgeService` (and, while it is still being emptied, by
/// `ProductionRoleRunner`, which supplies its own hooks so no behavior changes as the ownership
/// moves). The role-specific steps are the three hook calls; everything else here is the same for
/// Smith as it is for DevOps.
pub fn run_forge_role_turn(
    ctx: &ForgeRoleContext<'_>,
    node_id: &str,
    task: &ActiveForgeRoleTask,
    hooks: &dyn ForgeRoleHooks,
) -> Result<ForgeRoleOutcome> {
    if ctx.require_prod {
        let env = env_pairs_from_process();
        crate::engine::execution_target::assert_forge_lane_may_start(&env)
            .map_err(|e| WorkflowError::generic(e.0))?;
        if let Ok(declared) = std::env::var("EXECUTION_ENV") {
            assert_forge_execution_target(Some(&declared), None)
                .map_err(|e| WorkflowError::generic(e.0))?;
        }
    }

    // A lane that needs no model turn takes its own road out, before any harness turn is asked for.
    if let Some(outcome) = hooks.turn_without_model(ctx, node_id, task) {
        return outcome;
    }

    let enforce = deliverable_enforcement_enabled(
        std::env::var("FORGE_ENFORCE_DELIVERABLES").ok().as_deref(),
    );
    let budget = attempt_budget(
        enforce,
        parse_deliverable_reprompt_budget(
            std::env::var("FORGE_DELIVERABLE_RETRIES").ok().as_deref(),
        ),
    );
    let mut prior_reply: Option<String> = None;
    // The corrective directive for the next attempt. Set below when this attempt missed something, and handed
    // to `run_role` so the retry names the omission instead of repeating the prompt (2026-09-29).
    let mut self_heal: Option<String> = None;
    let mut evidence = ctx.current.clone();
    let mut last_raw = String::new();
    let mut last_out_sha = None;
    let mut last_assay = vec![];
    let mut last_mapped = false;
    let mut last_refusal = None;
    // The FIRST attempt's base: a retry's own pre-turn HEAD already contains the earlier attempts' commits,
    // and a patch taken from it would silently drop them.
    let mut first_execution_base: Option<String> = None;
    let mut total_usage: Option<HarnessUsage> = None;
    // A node the definition binds no agent service to has no lane, so no lane can be asked what it owes — and a
    // refusal belongs before a turn is paid for rather than after. This is the check the deleted
    // `ForgePhaseAgent::new` made at the top of every collect.
    lane_for_node(node_id).map_err(WorkflowError::generic)?;

    // Reject empty story_id BEFORE any harness turn runs. The writes below are
    // identity-bearing (forge_hold_record.story_id, forge_tool_artifact.story_id),
    // and a task with no story id must not spend money on a turn. The same error
    // string, just earlier — before the for attempt loop.
    let story_id = task.story_id.as_str();
    if story_id.trim().is_empty() {
        return Err(WorkflowError::generic(format!(
            "role task {} carries no story id; refusing to write identity-bearing Forge records against \
             the process-instance id",
            task.task_id
        )));
    }

    for attempt in 0..budget {
        let mut out = ctx.harness.run_role(node_id, task, self_heal.as_deref())?;
        // The harness reported facts; the lane judges them before anything below reads them.
        hooks.judge_output(ctx, node_id, &mut out);
        // Recorded per attempt, the moment it is known: a later attempt that errors out must not take the
        // spend of the earlier ones down with it.
        if let Some(usage) = out.usage.as_ref() {
            record_turn_usage(ctx, node_id, &task.story_id, usage)?;
            match total_usage.as_mut() {
                Some(total) => total.absorb(usage),
                None => total_usage = Some(usage.clone()),
            }
        }
        last_raw = out.raw.clone();
        last_out_sha = out.candidate_sha.clone();
        last_assay = out.assay_commands.clone();
        last_mapped = out.acceptance_mapped;
        last_refusal = out.refusal.clone();
        if first_execution_base.is_none() {
            first_execution_base = out.execution_base.clone();
        }
        let ports = effect_ports(ctx);
        // The lane's OWN reading of the reply, in place of the engine's deleted collect switch: what the
        // marker says is shared, what a lane adds or refuses is not.
        evidence = hooks
            .collect_evidence(node_id, ctx.current.clone(), &out.raw, &ports)
            .map_err(WorkflowError::generic)?;
        // A lane that DELIVERS a candidate says so here; the lifecycle does not know which lanes those are.
        if hooks.adopts_candidate_sha(node_id) {
            evidence.candidate_sha = out.candidate_sha.clone();
        }
        // The bench intent the dispatch carried, applied the moment the proposal is read, so a decision outside
        // the Cap is a rejected deliverable on the same attempt rather than a surprise at settle time.
        apply_bench_intent(ctx, &mut evidence);
        if attempt + 1 < budget {
            let missing = crate::engine::phase::missing_deliverables(
                hooks.deliverable_kind(node_id),
                &evidence,
                &out.raw,
                !out.raw.is_empty(),
                evidence.findings.is_some(),
            );
            if missing.is_empty() {
                break;
            }
            let directive = build_self_heal_directive(
                node_id,
                &missing.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                None,
                evidence
                    .deliverable_rejection
                    .as_deref()
                    .map(|s| vec![s.to_string()])
                    .unwrap_or_default()
                    .as_slice(),
                prior_reply.as_deref(),
            );
            prior_reply = Some(out.raw);
            self_heal = Some(directive);
            continue;
        }
        break;
    }
    let out_raw = last_raw;
    let out = HarnessOutput {
        raw: out_raw.clone(),
        candidate_sha: last_out_sha,
        assay_commands: last_assay,
        acceptance_mapped: last_mapped,
        refusal: last_refusal,
        execution_base: first_execution_base,
        usage: total_usage,
    };
    // The same envelope the attempts ran under, and the same cap: this is the evidence the writes below act on,
    // so a decision outside the bench intent must be refused here even if the last attempt broke out early.
    apply_bench_intent(ctx, &mut evidence);

    // ONE identity, taken from the task this lane was listed with. It is read here, before any write this turn
    // makes, because the writes below are identity-bearing: `forge_hold_record.story_id` and
    // `forge_tool_artifact.story_id` are foreign keys to `storyboard_story(id)`, so a process-instance UUID
    // substituted here is a row the database refuses. `runtime::list_role_tasks` fills it from the story that
    // owns the instance; a task that carries none is refused rather than given one.
    // Empty story_id was already rejected above, before any harness turn ran.
    let story_id = task.story_id.as_str();

    // The lane's OWN reading of the turn — the only role-specific step in this function, and the only
    // one that may write a role's record. It runs before the gate below, so a refusal a role reads
    // here is a refusal the gate acts on.
    hooks.interpret_turn(
        ctx,
        &ForgeRoleTurn {
            node_id,
            story_id,
            task,
            out: &out,
        },
        &mut evidence,
    )?;
    let missing = crate::engine::phase::missing_deliverables(
        hooks.deliverable_kind(node_id),
        &evidence,
        &out.raw,
        !out.raw.is_empty(),
        evidence.findings.is_some(),
    );
    if !missing.is_empty() && evidence.deliverable_rejection.is_none() {
        evidence.deliverable_rejection =
            Some(format!("role did not deliver {}", missing.join(", ")));
    }
    let route_missing = hooks.routing_decision_missing(node_id, &evidence);
    if let Some(route) = route_missing {
        if evidence.deliverable_rejection.is_none() {
            evidence.deliverable_rejection = Some(format!("routing decision missing: {route}"));
        }
    }

    // Trace recording is diagnostic (see `engine::observer`) and is deliberately contained, so its failure is
    // not a lane failure. It is written only for a run that has a state writer: a writer-less run (tests, a
    // machine with no PROD URL) must not write trace rows into whichever pool is installed.
    if ctx.writer.is_some() {
        let _ = record_forge_observer(
            &task.process_instance_id,
            story_id,
            &task.task_id,
            node_id,
            "role.completed",
            &format!("node={node_id}"),
        );
    }

    if let Some(reason) = evidence.deliverable_rejection.as_deref() {
        hold_rejected_deliverable(ctx, task, node_id, reason)?;
    }
    // A node that owes a ROUTING decision and gave none cannot route forward: the gateway after it would match no
    // condition and fall through to its first branch (Lead's `execution_shape` fell to SOLO on every story). Every
    // agent node in the definition has a `hold` transition, and this is what it is for. Other rejections keep
    // `complete`, because the workflow routes them itself (an Assay FAIL goes to repair, not to a human).
    let transition = if route_missing.is_some() {
        "hold"
    } else {
        "complete"
    };
    Ok(ForgeRoleOutcome {
        transition_name: Some(transition.into()),
        evidence,
    })
}

/// Record a rejected deliverable as the story's hold: the board's `Hold` and a `DELIVERABLE_REJECTED` hold record,
/// against the story the task was listed for. One home for the write every lane makes when it refuses a turn —
/// the lifecycle's own gate, Assay's model-free road, DevOps' production check.
///
/// A hold that cannot be recorded is not a hold that was silently skipped: both writes propagate, so a gate that
/// failed to record itself is visible as a failed lane. A writer-less run (tests, a machine with no PROD URL)
/// records nothing.
pub fn hold_rejected_deliverable(
    ctx: &ForgeRoleContext<'_>,
    task: &ActiveForgeRoleTask,
    node_id: &str,
    reason: &str,
) -> Result<()> {
    let Some(writer) = ctx.writer else {
        return Ok(());
    };
    let story_id = task.story_id.as_str();
    writer
        .mark_story_human_hold(story_id, reason)
        .map_err(|error| {
            WorkflowError::generic(format!("mark_story_human_hold({story_id}): {error}"))
        })?;
    writer
        .open_hold(&OpenHold {
            process_instance_id: task.process_instance_id.clone(),
            task_id: Some(task.task_id.clone()),
            story_id: story_id.to_string(),
            reason: reason.to_string(),
            originating_node: Some(node_id.into()),
            failure_class: Some("DELIVERABLE_REJECTED".into()),
            resume_target: None,
        })
        .map_err(|error| {
            WorkflowError::generic(format!("forge_hold_record({story_id}): {error}"))
        })?;
    Ok(())
}

/// Put one model turn's spend on the record: always on stderr, and on the Story Run row when this lane has one.
///
/// The hand-run lane (`forge --story …`, no `--work-item`) opens no run row, so its spend has nowhere durable to
/// go; the stderr line is then the only reading, and it is printed for every lane so the burn is never silent.
/// A write that fails fails the lane, like every other state write here.
fn record_turn_usage(
    ctx: &ForgeRoleContext<'_>,
    node_id: &str,
    story_id: &str,
    usage: &HarnessUsage,
) -> Result<()> {
    eprintln!(
        "spend story={story_id} node={node_id} run={} tokens_in={} tokens_out={} cost_usd={:.6} session={}",
        ctx.story_run_id.unwrap_or("(none)"),
        usage.tokens_input,
        usage.tokens_output,
        usage.cost_usd,
        usage.session_id
    );
    if let (Some(writer), Some(run_id)) = (ctx.writer, ctx.story_run_id) {
        writer.record_run_usage(run_id, usage).map_err(|error| {
            WorkflowError::generic(format!("record_run_usage({story_id}, {run_id}): {error}"))
        })?;
    }
    Ok(())
}

/// Apply the dispatch's bench intent to what a lane read. The cap bites on the Lead's decision and nowhere
/// else: that is the one deliverable the Cockpit sets a bench intent to constrain.
///
/// A decision outside the cap is a **rejected deliverable**, not a note — it travels the existing rail, so the
/// lane self-heals once with the intent named in the directive and, if it repeats, the story holds where a
/// human sees it. The rejection is never overwritten if the lane already has one: one refusal per turn keeps a
/// single reason readable.
fn apply_bench_intent(ctx: &ForgeRoleContext<'_>, evidence: &mut ForgeGateEvidence) {
    if evidence.lead_decision.is_none() {
        return;
    }
    let errors = crate::engine::role_slice::bench_intent_errors(
        ctx.bench_intent,
        evidence.lead_decision.as_deref(),
    );
    if errors.is_empty() || evidence.deliverable_rejection.is_some() {
        return;
    }
    evidence.deliverable_rejection = Some(errors.join("; "));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::assay::CommandResult;
    use crate::engine::writer::RecordingWriter;
    use crate::roles::hooks::NoRoleHooks;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A harness that answers every turn identically and counts the turns it was asked for.
    ///
    /// Deliberately not git-backed: these tests are about the lifecycle's shape — where its hook points
    /// are and what the defaults do — and a harness that touched a repository would be testing git.
    struct CountingHarness {
        raw: String,
        candidate_sha: Option<String>,
        turns: AtomicUsize,
    }

    impl CountingHarness {
        fn new(raw: &str, candidate_sha: Option<&str>) -> Self {
            Self {
                raw: raw.into(),
                candidate_sha: candidate_sha.map(str::to_string),
                turns: AtomicUsize::new(0),
            }
        }
        fn turns(&self) -> usize {
            self.turns.load(Ordering::SeqCst)
        }
    }

    impl RoleHarness for CountingHarness {
        fn run_role(
            &self,
            _: &str,
            _: &ActiveForgeRoleTask,
            _: Option<&str>,
        ) -> Result<HarnessOutput> {
            self.turns.fetch_add(1, Ordering::SeqCst);
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
        fn exists_on_base_ref(&self, _: &str, _: &str) -> bool {
            true
        }
        fn assay_cwd(&self) -> &std::path::Path {
            std::path::Path::new(".")
        }
        fn run_command(&self, command: &str) -> CommandResult {
            CommandResult {
                command: command.into(),
                exit_code: 1,
                passed: false,
                excerpt: "not run by this harness".into(),
                unmeasurable: true,
                output: String::new(),
            }
        }
    }

    fn role_task(node_id: &str, story_id: &str) -> ActiveForgeRoleTask {
        ActiveForgeRoleTask {
            task_id: "task-1".into(),
            process_instance_id: "proc-1".into(),
            story_id: story_id.into(),
            token_id: Some("tok-1".into()),
            node_id: Some(node_id.into()),
            status: workflow::TaskStatus::Ready,
            assignee: None,
            candidates: vec![node_id.into()],
        }
    }

    fn context<'a>(
        harness: &'a dyn RoleHarness,
        current: &'a ForgeGateEvidence,
        writer: Option<&'a dyn ForgeStateWriter>,
    ) -> ForgeRoleContext<'a> {
        ForgeRoleContext {
            harness,
            current,
            writer,
            story_run_id: None,
            bench_intent: None,
            test_mode: None,
            contract_assay_commands: &[],
            contract_acceptance_mapped: false,
            require_prod: false,
        }
    }
    /// THE PROPERTY THIS WHOLE EXTRACTION RESTS ON: a lane that overrides no hook gets the shared
    /// lifecycle and nothing lane-specific.
    ///
    /// `smith` is the sharpest node to prove it with, because the runner *does* capture Smith's code for
    /// that lane. Under the default hooks that capture must not happen at all. If this test ever fails,
    /// lane intelligence has leaked back into the lifecycle.
    #[test]
    fn a_lane_that_overrides_nothing_gets_no_lane_behavior() {
        let sha = "a".repeat(40);
        let harness = CountingHarness::new("added the candidate\n", Some(&sha));
        let writer = RecordingWriter::default();
        let current = ForgeGateEvidence::default();
        let task = role_task("smith", "ENG-STORY-1");
        let context = context(&harness, &current, Some(&writer));

        struct NoHooks;
        impl ForgeRoleHooks for NoHooks {}

        let outcome = run_forge_role_turn(&context, "smith", &task, &NoHooks).expect("turn runs");

        assert!(
            harness.turns() >= 1,
            "the default hooks still ask the harness for a turn"
        );
        assert_eq!(outcome.transition_name.as_deref(), Some("complete"));
        assert!(
            writer.artifacts.lock().unwrap().is_empty(),
            "the lifecycle must not capture a candidate: reading Smith's work is Smith's job"
        );
        assert!(
            outcome.evidence.candidate_sha.is_none(),
            "`adopts_candidate_sha` is the lane's answer, so the default must not adopt one"
        );
    }

    /// A lane whose work is not a model turn takes the whole turn and spends no harness turn doing it.
    #[test]
    fn a_lane_that_needs_no_model_turn_never_asks_the_harness_for_one() {
        let harness = CountingHarness::new("unused\n", None);
        let writer = RecordingWriter::default();
        let current = ForgeGateEvidence::default();
        let task = role_task("qa_verify", "ENG-STORY-1");
        let context = context(&harness, &current, Some(&writer));

        struct Measures;
        impl ForgeRoleHooks for Measures {
            fn turn_without_model(
                &self,
                _: &ForgeRoleContext<'_>,
                node_id: &str,
                _: &ActiveForgeRoleTask,
            ) -> Option<Result<ForgeRoleOutcome>> {
                Some(Ok(ForgeRoleOutcome {
                    transition_name: Some("complete".into()),
                    evidence: ForgeGateEvidence {
                        deliverable_rejection: Some(format!("measured {node_id} without a model")),
                        ..ForgeGateEvidence::default()
                    },
                }))
            }
        }

        let outcome =
            run_forge_role_turn(&context, "qa_verify", &task, &Measures).expect("turn runs");

        assert_eq!(
            harness.turns(),
            0,
            "a model-free lane must not spend a model turn"
        );
        assert_eq!(
            outcome.evidence.deliverable_rejection.as_deref(),
            Some("measured qa_verify without a model"),
            "the lane's own outcome is returned as it stands"
        );
    }
    /// The reading sees the turn exactly once, and the refusal it sets is the refusal the deliverable
    /// gate acts on — the whole point of routing refusals through the lane instead of the lifecycle.
    #[test]
    fn the_reading_is_called_once_and_its_refusal_reaches_the_gate() {
        let harness = CountingHarness::new("architecture notes, no handoff\n", None);
        let writer = RecordingWriter::default();
        let current = ForgeGateEvidence::default();
        let task = role_task("architect", "ENG-STORY-1");
        let context = context(&harness, &current, Some(&writer));
        let calls = AtomicUsize::new(0);

        struct Refuses<'a> {
            calls: &'a AtomicUsize,
        }
        impl ForgeRoleHooks for Refuses<'_> {
            fn interpret_turn(
                &self,
                _: &ForgeRoleContext<'_>,
                turn: &ForgeRoleTurn<'_>,
                evidence: &mut ForgeGateEvidence,
            ) -> Result<()> {
                self.calls.fetch_add(1, Ordering::SeqCst);
                assert_eq!(turn.story_id, "ENG-STORY-1");
                assert!(
                    turn.out.raw.contains("architecture notes"),
                    "{}",
                    turn.out.raw
                );
                evidence.deliverable_rejection = Some("Architect said no".into());
                Ok(())
            }
        }

        let outcome = run_forge_role_turn(&context, "architect", &task, &Refuses { calls: &calls })
            .expect("turn runs");

        let turns = harness.turns();
        assert!(
            turns >= 2,
            "this test is only meaningful while the attempt loop retries: it ran {turns} turn(s)"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "one reading for {turns} turns: the lane sees the merged turn, not each attempt"
        );
        assert_eq!(
            outcome.evidence.deliverable_rejection.as_deref(),
            Some("Architect said no")
        );
        let holds = writer.holds.lock().unwrap();
        assert_eq!(holds[0].0, "ENG-STORY-1");
        assert_eq!(holds[0].1, "Architect said no");
    }

    /// Identity is settled before any lane reading runs: a lane with no story fails closed rather than
    /// write identity-bearing records against a process-instance id.
    #[test]
    fn a_task_with_no_story_id_fails_closed_before_any_reading_runs() {
        let harness = CountingHarness::new("work\n", None);
        let writer = RecordingWriter::default();
        let current = ForgeGateEvidence::default();
        let task = role_task("architect", "   ");
        let context = context(&harness, &current, Some(&writer));
        let calls = AtomicUsize::new(0);

        struct Counts<'a> {
            calls: &'a AtomicUsize,
        }
        impl ForgeRoleHooks for Counts<'_> {
            fn interpret_turn(
                &self,
                _: &ForgeRoleContext<'_>,
                _: &ForgeRoleTurn<'_>,
                _: &mut ForgeGateEvidence,
            ) -> Result<()> {
                self.calls.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }

        let result = run_forge_role_turn(&context, "architect", &task, &Counts { calls: &calls });
        assert!(result.is_err(), "a task with no story id must be refused");
        let error = result.err().expect("just asserted to be an error");

        assert!(error.to_string().contains("no story id"), "{error}");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "no reading may run before identity is settled"
        );
        assert_eq!(
            harness.turns(),
            0,
            "identity is settled before any harness turn runs"
        );
        assert!(
            writer.holds.lock().unwrap().is_empty(),
            "nothing may be written for a lane that could not be identified"
        );
    }

    /// The architectural rule of this extraction, encoded and enforced: the shared lifecycle names no
    /// lane. A `match node_id { "smith" => … }` in here is exactly the accretion that made the runner
    /// unmaintainable, and this test is what keeps it from coming back.
    #[test]
    fn the_shared_lifecycle_names_no_lane() {
        let source = include_str!("lifecycle.rs");
        let code = source
            .split("#[cfg(test)]")
            .next()
            .expect("this module has code before its tests");
        for lane in [
            "\"smith\"",
            "\"repair_smith\"",
            "\"architect\"",
            "\"lead_pre\"",
            "\"scout_research\"",
            "\"inspector\"",
            "\"qa_verify\"",
            "\"deploy\"",
            "\"assay\"",
        ] {
            assert!(
                !code.contains(lane),
                "the shared lifecycle must not name a lane ({lane}); that belongs to its service"
            );
        }
    }

    /// A harness that reports what it spent, so the spend meter can be read without a repository.
    struct SpendingHarness;

    impl RoleHarness for SpendingHarness {
        fn run_role(
            &self,
            _: &str,
            _: &ActiveForgeRoleTask,
            _: Option<&str>,
        ) -> Result<HarnessOutput> {
            Ok(HarnessOutput {
                raw: "added the candidate\n".into(),
                candidate_sha: Some("a".repeat(40)),
                assay_commands: vec![],
                acceptance_mapped: false,
                refusal: None,
                execution_base: None,
                usage: Some(HarnessUsage {
                    session_id: "ses_turn".into(),
                    tokens_input: 26_714,
                    tokens_output: 701,
                    cost_usd: 0.005352,
                }),
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
                exit_code: 1,
                passed: false,
                excerpt: "not run by this harness".into(),
                unmeasurable: true,
                output: String::new(),
            }
        }
    }

    /// The bug this test exists for: the Rust port dropped the spend meter, so `tokens_input`/`tokens_output`
    /// stayed NULL and `cost_source='none'` on every run ("cost captured on 0/717"). A turn's measured spend
    /// reaches the run row through the writer, keyed to the run that paid for it.
    ///
    /// Moved here from `engine::runner` when the turn's sequence did: the meter is the lifecycle's, every lane
    /// has it, and it is not a lane reading. The harness is deliberately not git-backed — the capture this
    /// reading never triggers is Smith's, and a lane that overrides no hook must not run it.
    #[test]
    fn a_turns_spend_is_added_to_its_run() {
        let harness = SpendingHarness;
        let writer = RecordingWriter::default();
        let current = ForgeGateEvidence::default();
        let task = role_task("smith", "TST-CAPTURE-001");
        let context = ForgeRoleContext {
            harness: &harness,
            current: &current,
            writer: Some(&writer),
            story_run_id: Some("11111111-2222-3333-4444-555555555555"),
            bench_intent: None,
            test_mode: None,
            contract_assay_commands: &[],
            contract_acceptance_mapped: false,
            require_prod: false,
        };

        let _ = run_forge_role_turn(&context, "smith", &task, &NoRoleHooks);

        let usage = writer.usage.lock().unwrap();
        assert!(!usage.is_empty(), "the turn's spend reached the writer");
        let (run_id, first) = &usage[0];
        assert_eq!(run_id, "11111111-2222-3333-4444-555555555555");
        assert_eq!((first.tokens_input, first.tokens_output), (26_714, 701));
        assert!((first.cost_usd - 0.005352).abs() < 1e-9);
    }

    /// The real lane hooks through the shared turn, counting PAID turns: each of these used to cost two, because the
    /// deliverable the lane reads after the loop was demanded inside it.
    fn one_turn(node: &str, raw: &str, hooks: &dyn ForgeRoleHooks) -> (ForgeRoleOutcome, usize) {
        let harness = CountingHarness::new(raw, None);
        let current = ForgeGateEvidence::default();
        let task = role_task(node, "ENG-STORY-1");
        let context = context(&harness, &current, None);
        let outcome = run_forge_role_turn(&context, node, &task, hooks).expect("turn runs");
        (outcome, harness.turns())
    }

    #[test]
    fn a_valid_architect_handoff_is_delivered_on_its_first_turn() {
        let raw = concat!(
            "plan\n",
            "FORGE_ARCHITECT_HANDOFF: {\"version\":1,\"baseRef\":\"base\",",
            "\"findings\":[{\"id\":\"F1\",\"required\":true,\"summary\":\"do it\",",
            "\"scope\":[\"forge/src/lib.rs\"],\"proofs\":[],\"risks\":[]}]}\n"
        );
        let (outcome, turns) = one_turn("architect", raw, &crate::roles::architect::ArchitectHooks);
        assert_eq!(turns, 1, "a valid handoff is not re-asked for");
        assert_eq!(outcome.evidence.deliverable_rejection, None);
        assert_eq!(outcome.transition_name.as_deref(), Some("complete"));
    }

    #[test]
    fn a_measurement_turn_is_paid_once_and_its_verdict_is_measured_not_claimed() {
        let (outcome, turns) = one_turn(
            "qa_verify",
            "all good\nFORGE_EVIDENCE_JSON: {\"qaPassed\":true}\n",
            &crate::roles::qa::AssayHooks,
        );
        assert_eq!(
            turns, 1,
            "the verdict is measured after the turn; the turn owes nothing"
        );
        assert_eq!(
            outcome.evidence.qa_passed,
            Some(false),
            "nothing was measured, so a model's claim of PASS is not the verdict"
        );
        assert_eq!(
            outcome.transition_name.as_deref(),
            Some("complete"),
            "a QA failure is the workflow's to route, not a hold"
        );
    }

    #[test]
    fn the_lead_decides_on_its_pre_turn_and_a_missing_decision_holds_instead_of_falling_through() {
        let lead = crate::roles::lead::LeadHooks;
        let (decided, turns) = one_turn(
            "lead_pre",
            "FORGE_EVIDENCE_JSON: {\"leadDecision\":\"SMITH\"}\n",
            &lead,
        );
        assert_eq!(turns, 1);
        assert_eq!(decided.evidence.lead_decision.as_deref(), Some("SMITH"));
        assert_eq!(decided.transition_name.as_deref(), Some("complete"));

        let (undecided, _) = one_turn("lead_pre", "I think Smith should do it.\n", &lead);
        assert_eq!(undecided.evidence.lead_decision, None);
        assert_eq!(
            undecided.transition_name.as_deref(),
            Some("hold"),
            "with no decision the gateway would fall to its first branch (SOLO)"
        );
    }
}
