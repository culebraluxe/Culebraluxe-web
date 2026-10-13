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
use crate::engine::facts::ForgeGateEvidence;
use crate::engine::harness::HarnessUsage;
use crate::engine::hold::{
    deliverable_enforcement_enabled, parse_deliverable_reprompt_budget, OpenHold,
};
use crate::engine::observer::record_forge_observer;
use crate::engine::phase::RoleEffectPorts;
use crate::engine::runner::{ForgeRoleOutcome, ForgeRoleRunner};
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
    pub execution_id: Option<&'a str>,
    pub write_surface: Option<&'a [String]>,
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
    pub model_attempt_control: Option<&'a dyn crate::engine::turn_budget::ModelAttemptControl>,
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
            execution_id: None,
            write_surface: None,
            current: ports.current(),
            writer: ports.writer(),
            story_run_id: ports.story_run_id(),
            bench_intent: ports.bench_intent(),
            test_mode: ports.test_mode(),
            contract_assay_commands: ports.contract_assay_commands(),
            contract_acceptance_mapped: ports.contract_acceptance_mapped(),
            require_prod: ports.require_prod(),
            model_attempt_control: ports.model_attempt_control(),
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
    run_lane_turn_inner(runner, node_id, task, hooks, None)
}

pub fn run_lane_turn_scoped(
    runner: &dyn ForgeRoleRunner,
    node_id: &str,
    task: &ActiveForgeRoleTask,
    hooks: &dyn ForgeRoleHooks,
    execution_id: &str,
) -> Result<ForgeRoleOutcome> {
    run_lane_turn_inner(runner, node_id, task, hooks, Some(execution_id))
}

fn run_lane_turn_inner(
    runner: &dyn ForgeRoleRunner,
    node_id: &str,
    task: &ActiveForgeRoleTask,
    hooks: &dyn ForgeRoleHooks,
    execution_id: Option<&str>,
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
                execution_id,
                write_surface: task.write_surface.as_deref(),
                ..ForgeRoleContext::from_ports(ports)
            };
            run_forge_role_turn(&ctx, node_id, task, hooks)
        }
        None => match execution_id {
            Some(id) => runner.run_scoped(id, node_id, task),
            None => runner.run(node_id, task),
        },
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
        let permit = reserve_model_attempt(ctx, task, attempt)?;
        finish_model_attempt(ctx, permit.as_ref(), "launched", None)?;
        // The turn's clock is read HERE, at the seam, and handed to the trajectory row with the outcome: that is
        // what makes "time to first edit" a latency relative to a real turn rather than to a run.
        let turn_started_at = chrono::Utc::now();
        let report = match ctx.execution_id {
            Some(execution_id) => ctx.harness.run_role_scoped_report(
                execution_id,
                node_id,
                task,
                self_heal.as_deref(),
            ),
            None => ctx
                .harness
                .run_role_report(node_id, task, self_heal.as_deref()),
        };
        let turn_ended_at = chrono::Utc::now();
        // The generation that paid for this turn. It is the discriminator a REPLAN changes, which is why it is part
        // of the turn's key and not merely a column.
        let generation_id = ctx
            .model_attempt_control
            .map(|control| control.generation_id());
        let usage = report.measured_usage;
        let trajectory = report.trajectory;
        let mut out = match report.outcome {
            Ok(out) => out,
            Err(error) => {
                let detail = error.to_string();
                let status = if detail.contains(crate::engine::harness::TURN_INTERRUPTED_CODE) {
                    "cancelled"
                } else if crate::engine::engine_fault::is_engine_fault_error(&error) {
                    "uncertain"
                } else {
                    "failed"
                };
                let settlement = record_turn_usage(
                    ctx,
                    node_id,
                    task,
                    permit.as_ref(),
                    usage.as_ref(),
                    trajectory.as_ref(),
                    &TurnFacts {
                        generation_id,
                        role_attempt: attempt,
                        started_at: turn_started_at,
                        ended_at: turn_ended_at,
                        outcome: status,
                        candidate_sha: None,
                    },
                );
                let detail = match settlement {
                    Ok(()) => detail,
                    Err(accounting) => format!("{detail}; usage settlement failed: {accounting}"),
                };
                let finish = finish_model_attempt(ctx, permit.as_ref(), status, Some(&detail));
                if let Err(finish_error) = finish {
                    return Err(WorkflowError::generic(format!(
                        "{detail}; model-attempt status settlement failed: {finish_error}"
                    )));
                }
                return if detail == error.to_string() {
                    Err(error)
                } else {
                    Err(WorkflowError::generic(detail))
                };
            }
        };
        out.usage = usage.clone();
        // The harness reported facts; the lane judges them before anything below reads them.
        hooks.judge_output(ctx, node_id, &mut out);
        // The lane judges/captures the candidate before settlement can fail, so accounting cannot make paid code
        // disappear. Settlement is attempt-keyed and idempotent in the DAO.
        if let Err(error) = record_turn_usage(
            ctx,
            node_id,
            task,
            permit.as_ref(),
            usage.as_ref(),
            trajectory.as_ref(),
            &TurnFacts {
                generation_id,
                role_attempt: attempt,
                started_at: turn_started_at,
                ended_at: turn_ended_at,
                outcome: "completed",
                candidate_sha: out.candidate_sha.as_deref(),
            },
        ) {
            let detail = format!("model usage settlement failed after candidate capture: {error}");
            let _ = finish_model_attempt(ctx, permit.as_ref(), "uncertain", Some(&detail));
            return Err(WorkflowError::generic(detail));
        }
        if let Some(usage) = usage.as_ref() {
            match total_usage.as_mut() {
                Some(total) => total.absorb(usage),
                None => total_usage = Some(usage.clone()),
            }
        }
        finish_model_attempt(ctx, permit.as_ref(), "completed", None)?;
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
            if missing.is_empty() && evidence.deliverable_rejection.is_none() {
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
        // Not a lost reading: each attempt's trajectory was settled for itself as that turn ended (see
        // `record_turn_usage` above, keyed by the attempt), and this roll-up is the value the LANE returns, not the
        // value stored. A trajectory is not summable in any case — `first_edit_at_ms` and `last_edit_at_ms` are
        // positions inside ONE turn, so adding attempts together would invent a reading. Absent is honest here.
        trajectory: None,
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

    // WO-120: trace rows are durable writes, so the result is CHECKED and the failure is logged
    // with node and cause — but it does not fail the lane (see `observer_write_warning`: pinned
    // tests run turns on a `RecordingWriter` with no database, and assert the turn still succeeds).
    // A writer-less run (tests, a machine with no PROD URL) still writes nothing, by the gate below.
    if ctx.writer.is_some() {
        if let Err(error) = record_forge_observer(
            &task.process_instance_id,
            story_id,
            &task.task_id,
            node_id,
            "role.completed",
            &format!("node={node_id}"),
        ) {
            tracing::warn!("{}", observer_write_warning(node_id, &error));
        }
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

fn reserve_model_attempt(
    ctx: &ForgeRoleContext<'_>,
    task: &ActiveForgeRoleTask,
    role_attempt: u32,
) -> Result<Option<crate::engine::turn_budget::ModelAttemptPermit>> {
    ctx.model_attempt_control
        .map(|control| control.reserve(&task.task_id, role_attempt))
        .transpose()
        .map_err(|reason| {
            if reason.starts_with(crate::engine::turn_budget::MODEL_TURN_CAP_CODE) {
                WorkflowError::conflict(crate::engine::turn_budget::MODEL_TURN_CAP_CODE, reason)
            } else {
                WorkflowError::generic(reason)
            }
        })
}

fn finish_model_attempt(
    ctx: &ForgeRoleContext<'_>,
    permit: Option<&crate::engine::turn_budget::ModelAttemptPermit>,
    status: &str,
    detail: Option<&str>,
) -> Result<()> {
    match (ctx.model_attempt_control, permit) {
        (Some(control), Some(permit)) => control
            .finish(permit, status, detail)
            .map_err(WorkflowError::generic),
        _ => Ok(()),
    }
}

/// Warning line for an observer-trace write failure (WO-120): trace rows are durable writes, so a
/// failure is CHECKED and LOGGED here — but it does not fail the lane. Two pinned tests
/// (`a_lane_that_overrides_nothing_gets_no_lane_behavior`,
/// `the_reading_is_called_once_and_its_refusal_reaches_the_gate`) run turns against a
/// `RecordingWriter` with no database behind the shared pool, and assert the turn still succeeds:
/// lane-failure on a diagnostic-trace write would break that contract. The old `let _ =` vanished
/// the error entirely; now it reaches the operator log with node and cause attached.
pub(crate) fn observer_write_warning(node_id: &str, error: &str) -> String {
    format!("forge role turn: observer trace write failed for node {node_id}: {error}")
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

/// What the seam observed about the turn it just paid for, carried to the accounting call in one piece.
///
/// It is a struct rather than a handful of arguments because these facts belong together: the clock read around the
/// harness call, the generation that paid, the attempt index, how the turn ended and what it produced.
struct TurnFacts<'a> {
    /// The paid model generation, when a durable attempt controller minted one.
    generation_id: Option<&'a str>,
    /// The attempt index within the role: 0 is the first turn.
    role_attempt: u32,
    started_at: chrono::DateTime<chrono::Utc>,
    ended_at: chrono::DateTime<chrono::Utc>,
    /// `completed`, `failed`, `cancelled` (interrupted) or `uncertain` (engine fault).
    outcome: &'a str,
    candidate_sha: Option<&'a str>,
}

/// The trajectory row for one turn, keyed by the turn's own identity.
///
/// The key mirrors `forge_model_generation_attempt`'s identity — the task, the role attempt and the generation that
/// paid — so a REPLAN keeps the turn before it instead of overwriting it, and a retried settlement lands on its own
/// row. `UNKNOWN` fields stay `None`: the harness does not report the model or provider it ran under back to the
/// lane, so those are absent rather than guessed from a profile id.
fn turn_trajectory(
    node_id: &str,
    task: &ActiveForgeRoleTask,
    permit: Option<&crate::engine::turn_budget::ModelAttemptPermit>,
    metrics: &crate::engine::trajectory::TrajectoryMetrics,
    facts: &TurnFacts<'_>,
) -> crate::engine::trajectory::TurnTrajectory {
    use crate::engine::trajectory::{
        profile_id_for_node, profile_version, time_to_first_edit_ms, trajectory_attempt_key,
        TurnTrajectory,
    };
    let profile_id = profile_id_for_node(node_id).to_string();
    let attempt_key = match permit {
        Some(permit) => trajectory_attempt_key(&permit.attempt_key, facts.generation_id),
        // No controller meant no permit, so the attempt identity is composed from the task and the attempt index the
        // loop is on — the same pair the ledger itself would have keyed on.
        None => trajectory_attempt_key(
            &format!("{}:role-attempt:{}", task.task_id, facts.role_attempt),
            facts.generation_id,
        ),
    };
    TurnTrajectory {
        profile_version: profile_version(&profile_id),
        profile_id,
        attempt_key,
        generation_id: facts.generation_id.map(str::to_owned),
        task_id: Some(task.task_id.clone()),
        role_attempt: i32::try_from(facts.role_attempt).ok(),
        workflow_node: Some(node_id.to_string()),
        role: crate::engine::service_binding::lane_for_node(node_id)
            .ok()
            .map(|lane| lane.service_key().to_string()),
        model: None,
        provider: None,
        turn_started_at: Some(facts.started_at),
        turn_ended_at: Some(facts.ended_at),
        outcome: Some(facts.outcome.to_string()),
        first_edit_latency_ms: time_to_first_edit_ms(facts.started_at.timestamp_millis(), metrics),
        candidate_sha: facts.candidate_sha.map(str::to_owned),
        metrics: metrics.clone(),
    }
}

/// Put one model turn's spend on the record: always on stderr, and on the Story Run row when this lane has one.
///
/// The hand-run lane (`forge --story …`, no `--work-item`) opens no run row, so its spend has nowhere durable to
/// go; the stderr line is then the only reading, and it is printed for every lane so the burn is never silent.
/// A write that fails fails the lane, like every other state write here. The turn's own trajectory row is written
/// here too, keyed by the turn's identity rather than by its run and profile.
fn record_turn_usage(
    ctx: &ForgeRoleContext<'_>,
    node_id: &str,
    task: &ActiveForgeRoleTask,
    permit: Option<&crate::engine::turn_budget::ModelAttemptPermit>,
    usage: Option<&HarnessUsage>,
    trajectory: Option<&crate::engine::trajectory::TrajectoryMetrics>,
    facts: &TurnFacts<'_>,
) -> Result<()> {
    let story_id = task.story_id.as_str();
    if let Some(usage) = usage {
        tracing::info!(
            story = %story_id,
            node = %node_id,
            run = %ctx.story_run_id.unwrap_or("(none)"),
            tokens_in = usage.tokens_input,
            tokens_out = usage.tokens_output,
            cost_usd = %format!("{:.6}", usage.cost_usd),
            session = %usage.session_id,
            "spend"
        );
    } else {
        tracing::info!(
            story = %story_id,
            node = %node_id,
            run = %ctx.story_run_id.unwrap_or("(none)"),
            "spend usage=unknown"
        );
    }
    // What the turn did, keyed by the turn's own identity. Measurement only.
    if let (Some(writer), Some(run_id), Some(metrics)) = (ctx.writer, ctx.story_run_id, trajectory)
    {
        // The row carries what the harness reported for THIS turn. The attempt ledger's receipt is a settlement —
        // what was authorised for a stable attempt — not a second reading of the same turn, and its usage columns
        // (289) are not on every database yet, so the turn's own row is the one home for its spend. A turn with no
        // reading at all is recorded as explicitly unknown, never as a free turn.
        let turn = turn_trajectory(node_id, task, permit, metrics, facts);
        writer
            .record_run_trajectory(run_id, &turn, usage, usage.is_none())
            .map_err(|error| {
                WorkflowError::generic(format!(
                    "record_run_trajectory({story_id}, {run_id}): {error}"
                ))
            })?;
    }
    if let (Some(writer), Some(run_id)) = (ctx.writer, ctx.story_run_id) {
        if let (Some(control), Some(permit)) = (ctx.model_attempt_control, permit) {
            writer
                .settle_model_attempt_usage(
                    run_id,
                    control.generation_id(),
                    &permit.attempt_key,
                    usage,
                )
                .map_err(|error| {
                    WorkflowError::generic(format!(
                        "settle_model_attempt_usage({story_id}, {run_id}, {}): {error}",
                        permit.attempt_key
                    ))
                })?;
        } else if let Some(usage) = usage {
            // Compatibility for explicit direct runs without a durable attempt controller.
            writer.record_run_usage(run_id, usage).map_err(|error| {
                WorkflowError::generic(format!("record_run_usage({story_id}, {run_id}): {error}"))
            })?;
        }
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
mod tests;
