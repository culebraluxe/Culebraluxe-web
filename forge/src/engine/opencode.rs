//! Port of `agent-runtime/opencode/opencode-harness-adapter.ts` as RoleHarness.
//! OpenCode is the inner engine. Forge owns worktree, commit, assay, publish.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use crate::engine::assay::CommandResult;
use crate::engine::harness::TurnTermination;
use crate::engine::harness_usage::UsageBaseline;
use crate::engine::opencode_agents;
use crate::engine::opencode_client::{
    live_turn_slot, start_opencode_run_streaming, LiveTurnSlot, OpenCodeStartOptions, StreamStop,
    StreamedRunResult,
};
use crate::engine::opencode_events;
use crate::engine::packet::{build_task_text_with_context, ExecutionWorkspace, StoryPacket};
use crate::engine::runner::{HarnessOutput, RoleHarness};
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::engine::spend_cap::{self, BUDGET_EXHAUSTED_CODE, SPEND_CAP_ENV};
use crate::engine::vendor_session;
use workflow::{Result, WorkflowError};

mod config;

/// Stable identifier used by the engine when resolving the OpenCode adapter.
pub const OPENCODE_HARNESS_ADAPTER_ID: &str = "opencode-harness";
pub use config::{
    as_model_policy, cli_candidates, default_cli_bin, derive_cargo_target_dir, model_for_policy,
    read_session_id, resolve_model_for_policy, resolve_opencode_model, resume_session,
    sanitize_assay_env, sanitize_model_env, sanitized_model_env, session_continuity_enabled,
    session_marker_path, turn_ceiling, vendor_home_cli_bin, write_session_id,
    DEFAULT_TURN_CEILING_MINUTES, FORGE_MODEL_POLICIES, MODEL_FOR_CHEAP, MODEL_FOR_JUDGMENT,
    OPENCODE_PINNED_MODEL, SESSION_CONTINUITY_ENV, SESSION_MARKER_FILENAME, TURN_CEILING_ENV,
    VENDOR_CLI_HOME_RELATIVE, VENDOR_CLI_OVERRIDE_ENV, VENDOR_CLI_PATH_NAME, VENDOR_SESSION_LANE,
};

/// The stop code an interruption from outside the turn carries: a supervisory stop, distinct from the two budget
/// stops because its cause is not a budget. The reason travels with it (see `LiveTurn.interrupt_reason`).
pub const TURN_INTERRUPTED_CODE: &str = "TURN_INTERRUPTED";

/// The seam a turn's vendor process is started through.
///
/// It is STREAMING because the enforcement is streaming: the second argument is Forge's per-line verdict (the live
/// caps), and the third is the slot the turn publishes itself into so it can be stopped from another thread. A
/// seam that returned a finished transcript could not express either, which is why there is no buffered seam.
pub type StartRunFn = Box<
    dyn Fn(
            OpenCodeStartOptions<'_>,
            &mut dyn FnMut(&str) -> Option<StreamStop>,
            &LiveTurnSlot,
        ) -> StreamedRunResult
        + Send
        + Sync,
>;

/// The environment Forge hands a model subprocess: the sanitized model env plus the Forge-owned V2 config.
///
/// The config travels as `OPENCODE_CONFIG_CONTENT` rather than only as a worktree file because Forge executes
/// inside a LINKED GIT WORKTREE, and a worktree does not discover the repo-root `opencode.json` (measured on the
/// live 2.x build: `debug config` run from a worktree lists only the global config). Delivering it by environment
/// is the only path that applies from any cwd, and the vendor MERGES it with any project file that does exist, so
/// the checked-in `opencode.json` stays the operator's readable copy rather than a second source of truth.
///
/// The content is the posture IN FORCE (`..._from_env`), not the shipped default: an operator who arms
/// `FORGE_SUBAGENTS` must get the armed config on the subprocess, or the switch would be a comment. The
/// default-off case is what `render_v2_agent_config_pretty()` — the checked-in file — records.
///
/// `base` is expected to be `sanitized_model_env()`, i.e. an allow-list rather than the operator's whole
/// environment; this only adds a key to it.
pub fn v2_agent_env(base: Option<&HashMap<String, String>>) -> Result<HashMap<String, String>> {
    let mut env = base.cloned().unwrap_or_default();
    let content = crate::engine::opencode_agents::v2_agent_config_content_from_env()
        .map_err(WorkflowError::generic)?;
    env.insert("OPENCODE_CONFIG_CONTENT".into(), content);
    Ok(env)
}

pub struct OpenCodeHarness {
    pub cli_bin: String,
    pub workspace: PathBuf,
    pub model: String,
    pub env: Option<HashMap<String, String>>,
    pub auto_approve: bool,
    pub start_run: Option<StartRunFn>,
    /// The turn in flight, published so it can be stopped from another thread.
    ///
    /// A budget cap is not the only reason to stop a turn — a stale claim, a shutting-down worker or an operator
    /// all qualify — and every one of them needs the same primitive: signal the real process tree. This is where a
    /// caller finds it while the turn is running.
    pub live_turn: LiveTurnSlot,
    /// Active turns keyed by the durable job lease. A separate slot per execution keeps a
    /// deadline or cancellation from signalling a sibling model process.
    pub execution_turns: std::sync::Arc<std::sync::Mutex<HashMap<String, LiveTurnSlot>>>,
    /// The cap on what ONE turn may spend, in USD. `None` is "no cap configured", which is NOT the same as a cap
    /// of zero: Forge does not invent a dollar ceiling for work it was asked to do.
    ///
    /// Read ONCE, from `FORGE_SPEND_CAP_USD`, when the harness is built — a cap that could change in the middle of
    /// a generation is a cap nobody can reason about. It is a field rather than an environment read at the point of
    /// use so a test can hold a turn to a figure without setting a process-global variable that parallel tests share.
    pub spend_cap_usd: Option<f64>,
    pub assay_commands: Vec<String>,
    pub acceptance_mapped: bool,
    pub packet: StoryPacket,
    pub execution_workspace: Option<ExecutionWorkspace>,
    pub story_id: Option<String>,
}

impl OpenCodeHarness {
    pub fn fork_for_workspace(&self, workspace: ExecutionWorkspace) -> Result<Self> {
        if self.start_run.is_some() {
            return Err(WorkflowError::generic(
                "custom OpenCode start_run hooks cannot be used in concurrent lane worktrees",
            ));
        }
        Ok(Self {
            cli_bin: self.cli_bin.clone(),
            workspace: PathBuf::from(&workspace.worktree_path),
            model: self.model.clone(),
            env: self.env.clone(),
            auto_approve: self.auto_approve,
            start_run: None,
            live_turn: live_turn_slot(),
            execution_turns: Arc::new(std::sync::Mutex::new(HashMap::new())),
            spend_cap_usd: self.spend_cap_usd,
            assay_commands: self.assay_commands.clone(),
            acceptance_mapped: self.acceptance_mapped,
            packet: self.packet.clone(),
            execution_workspace: Some(workspace),
            story_id: self.story_id.clone(),
        })
    }

    /// The lane's harness, with the model the **row** names (migration 179 `model_policy`).
    ///
    /// `OPENCODE_MODEL` wins when it is set: that is an explicit, attended configuration, and an empty one is
    /// refused by `resolve_opencode_model` rather than silently replaced by the policy's model. Everything else is
    /// decided by the policy the dispatch carried — until 2026-09-29 the column existed and the model was always the
    /// pin, so the policy the Cockpit showed had no effect on what billed.
    pub fn from_env_for_policy(model_policy: Option<&str>) -> Result<Self> {
        let mut harness = Self::from_env()?;
        let override_model = std::env::var("OPENCODE_MODEL").ok();
        harness.model = match override_model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(model) => resolve_opencode_model(Some(model))?,
            None => resolve_model_for_policy(model_policy)?,
        };
        Ok(harness)
    }

    pub fn from_env() -> Result<Self> {
        Ok(Self {
            cli_bin: default_cli_bin(),
            workspace: std::env::var("FORGE_WORKTREE")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
            model: resolve_opencode_model(None)?,
            env: Some(sanitized_model_env()),
            auto_approve: true,
            start_run: None,
            live_turn: live_turn_slot(),
            execution_turns: Arc::new(std::sync::Mutex::new(HashMap::new())),
            spend_cap_usd: spend_cap::parse_forge_spend_cap_usd(
                std::env::var(SPEND_CAP_ENV).ok().as_deref(),
            ),
            assay_commands: vec![],
            acceptance_mapped: false,
            packet: StoryPacket {
                id: std::env::var("FORGE_STORY_ID").unwrap_or_default(),
                title: std::env::var("FORGE_STORY_TITLE").unwrap_or_default(),
                goal: std::env::var("FORGE_STORY_GOAL").ok(),
                special_instructions: std::env::var("FORGE_STORY_INSTRUCTIONS").ok(),
                architect_brief: std::env::var("FORGE_ARCHITECT_BRIEF").ok(),
                acceptance_criteria: std::env::var("FORGE_ACCEPTANCE").ok(),
                test_mode: std::env::var("FORGE_TEST_MODE").ok(),
                assay_commands: std::env::var("FORGE_ASSAY_COMMANDS")
                    .ok()
                    .map(|s| {
                        s.split('\n')
                            .map(|l| l.trim().to_string())
                            .filter(|l| !l.is_empty())
                            .collect()
                    })
                    .unwrap_or_default(),
                branch_name: std::env::var("FORGE_BRANCH").ok(),
                base_ref: std::env::var("FORGE_BASE_REF").ok(),
                base_commit: std::env::var("FORGE_BASE_COMMIT").ok(),
            },
            execution_workspace: match (
                std::env::var("FORGE_WORKTREE").ok(),
                std::env::var("FORGE_BRANCH").ok(),
                std::env::var("FORGE_BASE_REF").ok(),
                std::env::var("FORGE_BASE_COMMIT").ok(),
            ) {
                (Some(path), Some(branch), Some(base_ref), Some(base_commit)) => {
                    Some(ExecutionWorkspace {
                        worktree_path: path,
                        branch_name: branch,
                        base_ref,
                        base_commit,
                    })
                }
                _ => None,
            },
            story_id: std::env::var("FORGE_STORY_ID").ok(),
        })
    }

    fn run_git(&self, args: &[&str]) -> Option<String> {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.workspace)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// `self_heal` is the runner's corrective directive for this attempt (see `RoleHarness::run_role`). It is
    /// appended to the task text as extra context — the legacy harness carried it the same way.
    fn task_text(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> String {
        build_task_text_with_context(
            node_id,
            &task.task_id,
            &self.packet,
            self.execution_workspace.as_ref(),
            self_heal,
        )
    }
}

impl crate::engine::runner::CandidateProbe for OpenCodeHarness {
    fn git(&self, args: &[&str]) -> Option<String> {
        self.run_git(args)
    }

    fn declared_test_mode(&self) -> Option<&str> {
        self.packet.test_mode.as_deref()
    }
}

impl crate::engine::runner::ProductionProbe for OpenCodeHarness {
    fn production_url(&self) -> String {
        crate::engine::production_probe::production_base_url()
    }

    fn deployed_sha(&self) -> std::result::Result<String, String> {
        crate::engine::production_probe::fetch_deployed_sha(&self.production_url())
    }
}

impl OpenCodeHarness {
    fn execution_slot(&self, execution_id: &str) -> Result<LiveTurnSlot> {
        let mut slots = self
            .execution_turns
            .lock()
            .map_err(|_| WorkflowError::generic("execution interrupt registry is poisoned"))?;
        Ok(Arc::clone(
            slots
                .entry(execution_id.to_string())
                .or_insert_with(live_turn_slot),
        ))
    }

    fn run_role_with_slot(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
        live_turn: &LiveTurnSlot,
    ) -> Result<HarnessOutput> {
        let cwd = self.workspace.to_string_lossy().to_string();
        if !self.workspace.exists() {
            return Err(WorkflowError::generic(format!(
                "OpenCode harness workspace does not exist: {cwd}"
            )));
        }
        let task_text = self.task_text(node_id, task, self_heal);
        let before_sha = self.run_git(&["rev-parse", "HEAD"]);
        // The agent for this node, resolved FAIL CLOSED: a node Forge cannot map is refused here rather than run
        // as the vendor's default agent, whose permissions Forge did not author (see `engine::opencode_agents`).
        let agent = opencode_agents::v2_agent_for_node(node_id).map_err(WorkflowError::generic)?;
        // The subagent gate is stated in the log for EVERY turn, both postures. The disarmed line says the
        // recorded spend is the whole turn; the armed line says it is a lower bound, because that is the
        // measured behaviour (`harness_usage`). A silence here would be the one place an undercount could hide.
        eprintln!(
            "opencode-harness node={node_id} agent={agent} {}",
            opencode_agents::render_subagent_declaration(
                opencode_agents::subagents_enabled_from_env(),
                std::env::var(opencode_agents::SUBAGENTS_ENV)
                    .ok()
                    .as_deref(),
            )
        );
        // Execution receipt, before the first model token is spent: what runs, where, as whom.
        eprintln!(
            "harness=opencode model={} bin={} cwd={}",
            self.model, self.cli_bin, cwd
        );
        // The config travels with the process (§3): a worktree does not discover the repo-root `opencode.json`,
        // so environment delivery is the path that actually applies.
        let model_env = v2_agent_env(self.env.as_ref())?;
        let lane = VENDOR_SESSION_LANE;
        // WHICH SESSION THIS TURN RESUMES, AND WHY THE WORKTREE DECIDES (2026-10-03, ENG-FORGE-C1-BUILD-INFO-01).
        //
        // Session continuity exists to pay a story's startup cost once. It was switched on when the engine was
        // mostly stable; it has been a liability since, because the stored session was chosen WITHOUT asking
        // whether it belongs to the directory this turn is about to run in. A vendor session is scoped to the
        // project directory that created it, and `worktree::derive_worktree_path` puts the RUN id in the worktree
        // path — so a FLIP (a fresh run, a fresh worktree) left `forge_vendor_session` naming a session in a
        // directory the engine had deleted, and every turn of every run after the flip re-sent that id. The
        // vendor refuses it before the first token: `UnexpectedStatus: 500`, tokens_in=0, cost_usd=0.000000 —
        // three attempts, $0.00, a Hold. Reproduced on demand: the same command without `--session` answers `ok`.
        //
        // THE DISCRIMINATOR IS THE DIRECTORY, NOT A JOB KIND. Retry or flip, the turn knows which worktree it is
        // in, and that is the same boundary the vendor enforces — so no mode field, no schema change, and nothing
        // a story would have to be told about itself:
        //
        //   * a RETRY inside a run keeps its worktree, so it keeps its directory, so it keeps its session and the
        //     startup cost stays paid once (`--session` = the exact id, as before);
        //   * a FLIP mints a new worktree, so the stored id is refused and the turn opens a fresh session in the
        //     directory it is actually in (`resume_session` above).
        //
        // Both facts are read here, and the probe is skipped when the marker already answered — which is the
        // common case inside a live run.
        let workspace_session = if session_continuity_enabled() {
            read_session_id(&cwd)
        } else {
            None
        };
        let lane_session = if session_continuity_enabled() {
            self.story_id.as_deref().and_then(|story| {
                vendor_session::read_vendor_session_id(story, lane)
                    .ok()
                    .flatten()
            })
        } else {
            None
        };
        let lane_session_lives_here = workspace_session.is_none()
            && lane_session.as_deref().is_some_and(|id| {
                crate::engine::harness_usage::session_lives_in(
                    &self.cli_bin,
                    &cwd,
                    Some(&model_env),
                    id,
                )
            });
        let session = resume_session(workspace_session, lane_session, lane_session_lives_here);
        let continue_session =
            session_continuity_enabled() && session.is_none() && session_marker_path(&cwd).exists();
        let opts = OpenCodeStartOptions {
            cli_bin: &self.cli_bin,
            cwd: &cwd,
            model: &self.model,
            task: &task_text,
            env: Some(&model_env),
            auto_approve: self.auto_approve,
            session: session.as_deref(),
            continue_session,
            agent: Some(agent),
            max_turn: turn_ceiling(std::env::var(TURN_CEILING_ENV).ok().as_deref()),
        };
        // Read BEFORE the turn: a resumed session's totals are cumulative, so its spend is a difference.
        let baseline = UsageBaseline::before_turn(
            &cwd,
            session.as_deref(),
            continue_session,
            &self.cli_bin,
            Some(&model_env),
        );
        // A FRESH turn owns the slot: a reason left over from the previous turn would be reported against work it
        // never touched.
        if let Ok(mut slot) = live_turn.lock() {
            *slot = crate::engine::opencode_client::LiveTurn::default();
        }
        // The live spend cap, read per turn the way the runner reads its own knobs: a cap is an operator decision,
        // not a constant. Unset stays unset — Forge does not invent a dollar ceiling for work it was asked to do,
        // and the turn-count ceiling below is the cap that always applies.
        //
        // WHAT THIS SEAM CAN AND CANNOT ENFORCE. The GENERATION turn cap is the DRIVER's to enforce
        // (`executor::drive_forge_story` counts dispatches per generation, which is the unit V1 measured: "a healthy
        // FEATURE generation costs five turns: architect, lead_pre, smith, post, qa"). This harness sees exactly one
        // turn and cannot count a generation's, so it does not pretend to. What it can do is stop the turn it is
        // holding, mid-flight, the moment the turn's own measured spend passes the cap — which is the difference
        // between a cap and an obituary.
        let spend_cap = self.spend_cap_usd;
        let mut transcript = String::new();
        let mut scanner = opencode_events::RunEventScanner::default();
        let mut on_line = |line: &str| -> Option<StreamStop> {
            // Kept verbatim: the transcript is what the turn is READ from below, so the live view and the recorded
            // view cannot disagree.
            transcript.push_str(line);
            transcript.push('\n');
            let _ = scanner.feed(line);
            // A FLOOR, never a total: the live build does not reliably emit a `step_finish` for a turn's terminal
            // step (see `engine::opencode_events`). A floor is the safe side to stop on — it can only delay a stop,
            // never cause one for a turn that is still inside its cap.
            let spend = scanner.turn().usage.as_ref().map(|usage| usage.cost_usd);
            match spend_cap {
                Some(cap) if spend_cap::forge_spend_should_hold(spend, Some(cap)) => {
                    Some(StreamStop {
                        code: BUDGET_EXHAUSTED_CODE.to_string(),
                        detail: spend_cap::render_spend_cap_line(spend, cap),
                    })
                }
                _ => None,
            }
        };
        let result = match &self.start_run {
            Some(start) => start(opts, &mut on_line, &live_turn),
            None => start_opencode_run_streaming(opts, &mut on_line, &live_turn),
        };
        drop(on_line);
        // The heartbeat Forge actually observed. Recorded, never acted on: the loop counts the intervals in which
        // the vendor said nothing, and silence is not a verdict — a long tool call is silent and healthy.
        if result.silent_ticks > 0 || result.stop.is_some() {
            eprintln!(
                "opencode-harness turn node={node_id} lines={} last_line_ms={} silent_ticks={} stopped_by={}",
                result.lines,
                result.last_line_ms,
                result.silent_ticks,
                result
                    .stop
                    .as_ref()
                    .map(|stop| stop.code.as_str())
                    .unwrap_or("(none)")
            );
        }
        // A stop from OUTSIDE the turn leaves no `stop` in the result: the process simply died, and without this it
        // would be reported as a crash with no stderr. The reason was stored with the turn precisely so it survives
        // to here.
        let interrupt_reason = live_turn.lock().ok().and_then(|mut slot| {
            let reason = slot.interrupt_reason.clone();
            *slot = crate::engine::opencode_client::LiveTurn::default();
            reason
        });
        if let Some(reason) = interrupt_reason.as_deref() {
            eprintln!(
                "opencode-harness interrupt_observed=true process_reaped=true exit_code={:?} reason={reason}",
                result.exit_code
            );
        }
        let stop = result.stop.clone().or_else(|| {
            interrupt_reason.map(|reason| StreamStop {
                code: TURN_INTERRUPTED_CODE.to_string(),
                detail: reason,
            })
        });
        // Read the V2 structured contract BEFORE judging the turn. A successful exit whose stream Forge cannot
        // read is NOT a successful turn (§3); failing closed here is the difference between "the harness is
        // broken" and "the role said nothing", which Forge would otherwise charge to the model.
        let turn = opencode_events::parse_run_events(&transcript);
        let reported_session = turn
            .as_ref()
            .ok()
            .and_then(|events| events.session_id.clone());
        // What the turn spent.
        //
        // PREFERRED: the vendor's `session export`, whose totals are authoritative. FALLBACK: the sum of this
        // run's own `step_finish` events, used only when the export cannot be read — the live 2.0.21 build does
        // not reliably emit a `step_finish` for a turn's TERMINAL step, so that sum is a LOWER BOUND, never the
        // figure Forge prefers. It is taken rather than dropped because a FAILED turn often has no other
        // reading, and §7 requires that spend not to vanish with the turn.
        let usage = baseline
            .after_turn(reported_session.as_deref())
            .map(|u| crate::engine::harness::HarnessUsage {
                session_id: u.session_id,
                tokens_input: u.tokens_input,
                tokens_output: u.tokens_output,
                cost_usd: u.cost_usd,
            })
            .or_else(|| turn.as_ref().ok().and_then(|events| events.usage.clone()));
        let spent = usage
            .as_ref()
            .map(|u| {
                format!(
                    " (spent tokens_in={} tokens_out={} cost_usd={:.6} session={})",
                    u.tokens_input, u.tokens_output, u.cost_usd, u.session_id
                )
            })
            .unwrap_or_default();
        if let Some(stop) = stop.as_ref() {
            // A TURN FORGE STOPPED IS NOT A TURN THAT FINISHED. However much text arrived before the stop, it is a
            // truncated answer, and reading it as one is how a capped generation gets recorded as productive. The
            // code and the reason are both in the record, and the spend measured up to the stop is with them.
            return Err(WorkflowError::generic(format!(
                "opencode-harness stopped for {node_id}: {} — {}{spent}",
                stop.code, stop.detail
            )));
        }
        if result.status != crate::engine::opencode_client::OpenCodeRunStatus::Success {
            // A failed turn still cost money. The error carries the reading so the spend is at least on the
            // record of the failure instead of vanishing with it.
            //
            // The vendor's own error event is preferred when it reported one: V2's stdout is a machine
            // transcript, so pasting it into an exception is noise, and its stderr is often EMPTY on a provider
            // refusal (an unknown model id arrives on stdout as {"name":"UnknownError","message":…}).
            let detail = match &turn {
                Ok(events) => events.error.clone().unwrap_or_else(|| {
                    if result.stderr.trim().is_empty() {
                        format!("no stderr; {} v2 events read", events.events.len())
                    } else {
                        result.stderr.trim().to_string()
                    }
                }),
                Err(error) => format!("unreadable v2 stream: {error}"),
            };
            return Err(WorkflowError::generic(format!(
                "opencode-harness failed for {node_id} exit={:?}{spent}: {detail}",
                result.exit_code
            )));
        }
        let turn = turn.map_err(|error| {
            WorkflowError::generic(format!(
                "opencode-harness produced a v2 stream Forge cannot read for {node_id}{spent}: {error}"
            ))
        })?;
        // An exit 0 that reported a vendor error and produced no output is a failure, not an empty success.
        if turn.assistant_text.trim().is_empty() {
            if let Some(error) = turn.error.as_deref() {
                return Err(WorkflowError::generic(format!(
                    "opencode-harness reported an error with no output for {node_id}{spent}: {error}"
                )));
            }
        }
        // THE V2 SESSION FIX (§4): the id the vendor actually used comes from the turn itself, so a FRESH run
        // persists a real session and the next turn resumes it explicitly. V1 had no such reading and wrote
        // back the id it had *asked* for — `None` on a fresh turn — so a lane's first session was never kept.
        let actual_session = reported_session;
        if let Some(story) = self.story_id.as_deref() {
            let _ = vendor_session::write_vendor_session_id(
                story,
                VENDOR_SESSION_LANE,
                actual_session.as_deref().or(session.as_deref()),
            );
        }
        if let Some(id) = actual_session.as_deref() {
            write_session_id(&cwd, Some(id));
        }
        // The role output is the assistant's text (§3), never the NDJSON transcript it arrived in.
        let raw = turn.assistant_text;
        // FACTS ONLY. What the turn left in the repository is reported here; whether it is an acceptable
        // candidate is the delivering lane's judgement (`roles::smith::judge_delivered_candidate`), applied by
        // the lane's hooks the moment this output reaches the lifecycle.
        let candidate_sha = self.run_git(&["rev-parse", "HEAD"]);
        let execution_base = self
            .execution_workspace
            .as_ref()
            .map(|workspace| workspace.base_commit.clone())
            .or(before_sha);
        let assay = if self.assay_commands.is_empty() {
            self.packet.assay_commands.clone()
        } else {
            self.assay_commands.clone()
        };
        Ok(HarnessOutput {
            raw,
            candidate_sha,
            assay_commands: assay,
            acceptance_mapped: self.acceptance_mapped,
            refusal: None,
            execution_base,
            usage,
        })
    }
}

impl RoleHarness for OpenCodeHarness {
    fn run_role(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> Result<HarnessOutput> {
        self.run_role_with_slot(node_id, task, self_heal, &self.live_turn)
    }

    fn run_role_scoped(
        &self,
        execution_id: &str,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> Result<HarnessOutput> {
        let slot = self.execution_slot(execution_id)?;
        if slot
            .lock()
            .map_err(|_| WorkflowError::generic("execution interrupt slot is poisoned"))?
            .interrupt_reason
            .is_some()
        {
            return Err(WorkflowError::generic(format!(
                "{}: execution was cancelled before model launch",
                crate::engine::opencode::TURN_INTERRUPTED_CODE
            )));
        }
        self.run_role_with_slot(node_id, task, self_heal, &slot)
    }

    fn fork_for_workspace(
        &self,
        workspace: ExecutionWorkspace,
    ) -> Result<Option<Arc<dyn RoleHarness>>> {
        Ok(Some(Arc::new(OpenCodeHarness::fork_for_workspace(
            self, workspace,
        )?)))
    }

    fn begin_execution(&self, execution_id: &str) -> Result<()> {
        self.execution_slot(execution_id).map(|_| ())
    }

    /// Stop the turn this harness is running, if it is running one.
    ///
    /// The vendor's own `POST /session/{id}/abort` is NOT reachable from here: a `--standalone` run speaks stdio to a
    /// private child server and exposes no addressable HTTP endpoint (measured — see `engine::opencode_client`). The
    /// primitive is therefore a signal to the process group Forge spawned, and the reason is stored on the turn so
    /// the read that follows reports an interruption rather than a crash with no stderr.
    fn interrupt_execution(&self, reason: &str) -> Result<Option<TurnTermination>> {
        let running = {
            let mut slot = self.live_turn.lock().map_err(|_| {
                WorkflowError::generic("the live-turn slot is poisoned; refusing to claim a stop")
            })?;
            let Some(running) = slot.running else {
                // Nothing is running: an answer, not a failure.
                return Ok(None);
            };
            slot.interrupt_reason = Some(reason.to_string());
            running
        };
        let termination = running.terminate();
        eprintln!(
            "opencode-harness interrupted pid={} reason={reason} existed={} killed={} signalled={:?}",
            running.pid, termination.existed, termination.killed, termination.signalled
        );
        Ok(Some(termination))
    }

    fn interrupt_execution_scoped(
        &self,
        execution_id: &str,
        reason: &str,
    ) -> Result<Option<TurnTermination>> {
        let slot = self.execution_slot(execution_id)?;
        let running = {
            let mut turn = slot
                .lock()
                .map_err(|_| WorkflowError::generic("execution interrupt slot is poisoned"))?;
            turn.interrupt_reason = Some(reason.to_string());
            turn.running
        };
        let Some(running) = running else {
            eprintln!("forge-interrupt execution={execution_id} requested=true delivered=pending reason={reason}");
            return Ok(None);
        };
        let termination = running.terminate();
        eprintln!(
            "forge-interrupt execution={execution_id} requested=true delivered=true pid={} existed={} killed={} signalled={:?} reason={reason}",
            running.pid, termination.existed, termination.killed, termination.signalled
        );
        Ok(Some(termination))
    }

    fn finish_execution(&self, execution_id: &str) {
        if let Ok(mut slots) = self.execution_turns.lock() {
            slots.remove(execution_id);
        }
    }

    fn supports_interrupt(&self) -> bool {
        true
    }

    fn exists_on_base_ref(&self, base_ref: &str, path: &str) -> bool {
        self.run_git(&["cat-file", "-e", &format!("{base_ref}:{path}")])
            .is_some()
            || self
                .run_git(&["ls-tree", "--name-only", base_ref, path])
                .map(|s| !s.is_empty())
                .unwrap_or(false)
    }

    fn assay_cwd(&self) -> &Path {
        if let Some(ws) = &self.execution_workspace {
            return Path::new(&ws.worktree_path);
        }
        self.workspace.as_path()
    }

    fn candidate_probe(&self) -> Option<&dyn crate::engine::runner::CandidateProbe> {
        Some(self)
    }

    fn production_probe(&self) -> Option<&dyn crate::engine::runner::ProductionProbe> {
        Some(self)
    }

    fn execution_base_commit(&self) -> Option<&str> {
        self.execution_workspace
            .as_ref()
            .map(|workspace| workspace.base_commit.as_str())
    }

    fn execution_workspace(&self) -> Option<&ExecutionWorkspace> {
        self.execution_workspace.as_ref()
    }

    fn run_command(&self, command: &str) -> CommandResult {
        use crate::engine::assay::{
            assay_timeout, cancellation_signal, is_cmd_timeout, spawn_scoped_shell_with_env,
            CeilingOutcome, CMD_CANCELLED_EXIT, CMD_TIMEOUT_CODE, CMD_TIMEOUT_EXIT,
        };
        let cwd = self.assay_cwd();
        // FIX-007: per-worktree CARGO_TARGET_DIR for build isolation, sanitized env (no secrets).
        let cargo_target_dir = derive_cargo_target_dir(cwd);
        let mut assay_env = sanitize_assay_env(std::env::vars().collect());
        assay_env.insert(
            "CARGO_TARGET_DIR".into(),
            cargo_target_dir.to_string_lossy().to_string(),
        );
        // FIX-005: bounded — a hung assay held the story claim indefinitely because this shell had
        // no deadline. The ceiling kills the command's tree; the claim is released by returning, and the
        // story requeues on the `CMD_TIMEOUT` blocker the adjudicator reads from the excerpt.
        let child = match spawn_scoped_shell_with_env(command, cwd, &assay_env) {
            Ok(child) => child,
            Err(e) => {
                return CommandResult {
                    cancelled: false,
                    command: command.into(),
                    exit_code: -1,
                    passed: false,
                    excerpt: e.to_string(),
                    unmeasurable: true,
                    output: String::new(),
                };
            }
        };
        match crate::engine::assay::wait_with_ceiling(child, assay_timeout()) {
            CeilingOutcome::TimedOut(hit) => CommandResult {
                cancelled: false,
                command: command.into(),
                exit_code: CMD_TIMEOUT_EXIT,
                passed: false,
                excerpt: format!(
                    "{CMD_TIMEOUT_CODE}: assay command timed out after {}s and was killed (pid {}): {command}",
                    hit.ceiling.as_secs(),
                    hit.pid
                ),
                unmeasurable: true,
                output: String::new(),
            },
            CeilingOutcome::Finished(Err(e)) => CommandResult {
                cancelled: false,
                command: command.into(),
                exit_code: -1,
                passed: false,
                excerpt: format!("could not observe assay command: {e}"),
                unmeasurable: true,
                output: String::new(),
            },
            CeilingOutcome::Finished(Ok(out)) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                // FIX-003: BOTH streams are evidence — cargo diagnostics print to stderr while test
                // harnesses print to stdout, and keeping only one silently discards the compiler error.
                let text = crate::engine::assay::combine_command_output(&stdout, &stderr);
                let signal = cancellation_signal(&out.status);
                let code = if signal.is_some() {
                    CMD_CANCELLED_EXIT
                } else {
                    out.status.code().unwrap_or(1)
                };
                let text = if let Some(signal) = signal {
                    format!("CMD_CANCELLED: assay command received signal {signal}\n{text}")
                } else {
                    text
                };
                let result = CommandResult {
                    cancelled: signal.is_some(),
                    command: command.into(),
                    exit_code: code,
                    passed: code == 0,
                    excerpt: text
                        .lines()
                        .take(crate::engine::assay::COMMAND_EXCERPT_LINES)
                        .collect::<Vec<_>>()
                        .join("\n"),
                    unmeasurable: signal.is_some(),
                    output: text,
                };
                debug_assert!(
                    !is_cmd_timeout(&result),
                    "a finished command must not wear the timeout marker"
                );
                result
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn a_policy_names_a_priceable_model_and_an_unknown_policy_reads_as_cheap() {
        // Exactly two policies, and every one names a model the price table can price — the assertions that survive
        // `lib/forge-kind.ts` in `legacy/workflow_app/tests/forge-kind-routing.test.ts:104-125`.
        assert_eq!(FORGE_MODEL_POLICIES, ["cheap", "judgment"]);
        assert_eq!(model_for_policy(Some("cheap")), "deepseek/deepseek-flash");
        assert_eq!(
            model_for_policy(Some("judgment")),
            "deepseek/deepseek-flash"
        );
        // The live pin IS the flash tier after the upstream rename, so both policies name what bills today.
        assert_eq!(MODEL_FOR_CHEAP, OPENCODE_PINNED_MODEL);
        assert_eq!(MODEL_FOR_JUDGMENT, OPENCODE_PINNED_MODEL);
        // An unknown or absent policy reads as the default rather than throwing (`asModelPolicy`).
        assert_eq!(as_model_policy(Some("premium")), "cheap");
        assert_eq!(as_model_policy(Some(" judgment ")), "judgment");
        assert_eq!(as_model_policy(None), "cheap");
    }

    #[test]
    fn smith_subprocess_has_no_prod_database_authority_and_cannot_push() {
        let mut env = HashMap::new();
        env.insert("PATH".into(), "/usr/bin".into());
        env.insert("DATABASE_URL_PROD".into(), "postgres://prod".into());
        env.insert("NEON_API_KEY".into(), "secret".into());
        env.insert("APP_ENV".into(), "production".into());
        env.insert("EXECUTION_ENV".into(), "PROD".into());
        env.insert("OPENCODE_TOKEN".into(), "keep-me".into());

        let clean = sanitize_model_env(env);
        assert_eq!(clean.get("PATH").map(String::as_str), Some("/usr/bin"));
        assert_eq!(
            clean.get("OPENCODE_TOKEN").map(String::as_str),
            Some("keep-me")
        );
        assert!(!clean.contains_key("DATABASE_URL_PROD"));
        assert!(!clean.contains_key("NEON_API_KEY"));
        assert!(!clean.contains_key("APP_ENV"));
        assert!(!clean.contains_key("EXECUTION_ENV"));
        assert_eq!(
            clean.get("GIT_CONFIG_KEY_0").map(String::as_str),
            Some("remote.origin.pushurl")
        );
        assert_eq!(
            clean.get("GIT_CONFIG_VALUE_0").map(String::as_str),
            Some("/dev/null")
        );
    }

    /// The real V2 stream shape (opencode v2.0.21), neutralised: one step, carrying usage and a text answer.
    const V2_STREAM: &str = concat!(
        r#"{"type":"step_start","timestamp":1,"sessionID":"ses_fixture_turn_0001","part":{"type":"step-start"}}"#,
        "\n",
        r#"{"type":"step_finish","timestamp":2,"sessionID":"ses_fixture_turn_0001","part":{"type":"step-finish","reason":"stop","cost":0.000363204,"tokens":{"input":650,"output":57,"reasoning":0,"cache":{"read":26368,"write":0}}}}"#,
        "\n",
        r#"{"type":"text","timestamp":3,"sessionID":"ses_fixture_turn_0001","part":{"type":"text","text":"DONE"}}"#,
    );

    fn role_task() -> ActiveForgeRoleTask {
        ActiveForgeRoleTask {
            task_id: "task-v2".into(),
            process_instance_id: "proc-v2".into(),
            story_id: "STORY-V2".into(),
            token_id: None,
            // A real dispatchable Scout node (`SCOUT_NODES` in `engine::phase`), not the bare position
            // "scout": the harness now resolves its agent from this id and refuses an id the engine itself
            // could not have dispatched (`forge_role_node_plan` is what `ForgePhaseAgent::new` applies first,
            // for this same id, two lines into the runner's loop).
            node_id: Some("feature_scout".into()),
            status: workflow::TaskStatus::Ready,
            assignee: None,
            candidates: vec![],
            write_surface: None,
        }
    }

    /// A harness whose model turn is a scripted V2 stream. A Scout node writes no code, so no git repository
    /// is needed; the temp workspace only has to exist.
    fn harness_with_stream(workspace: &Path, stream: &str) -> OpenCodeHarness {
        let stdout = stream.to_string();
        OpenCodeHarness {
            // A binary that cannot exist: this test is about session capture, so the usage baseline must fail
            // fast and read as UNMEASURED rather than shelling out to a real vendor installation.
            cli_bin: "/nonexistent/opencode".into(),
            workspace: workspace.to_path_buf(),
            model: OPENCODE_PINNED_MODEL.into(),
            env: None,
            auto_approve: true,
            // The seam replays the scripted transcript LINE BY LINE, exactly as the streaming client does, so the
            // live guard and the post-turn read see the same input. A seam that returned a finished transcript could
            // not exercise either.
            start_run: Some(Box::new(move |_opts, on_line, _live| {
                let mut stop = None;
                let mut lines = 0usize;
                for line in stdout.lines() {
                    lines += 1;
                    if let Some(verdict) = on_line(line) {
                        stop = Some(verdict);
                        break;
                    }
                }
                StreamedRunResult {
                    status: crate::engine::opencode_client::OpenCodeRunStatus::Success,
                    exit_code: Some(0),
                    stderr: String::new(),
                    stop,
                    lines,
                    last_line_ms: 1,
                    silent_ticks: 0,
                    termination: None,
                }
            })),
            live_turn: live_turn_slot(),
            execution_turns: Arc::new(std::sync::Mutex::new(HashMap::new())),
            // No dollar cap: the cap tests set the field directly, and a streamed Scout turn has no spend to cap.
            spend_cap_usd: None,
            assay_commands: vec![],
            acceptance_mapped: false,
            packet: StoryPacket {
                id: "STORY-V2".into(),
                title: "an OpenCode v2 turn".into(),
                ..Default::default()
            },
            execution_workspace: None,
            // No story id: the durable vendor-session lane needs the control plane's database and this test must
            // not depend on one. What a FRESH turn does not need is proved through the workspace marker here;
            // the Neon lane is covered by the live contract smoke.
            story_id: None,
        }
    }

    #[test]
    fn a_fresh_v2_turn_persists_the_session_id_the_vendor_reported() {
        let workspace = std::env::temp_dir().join(format!("forge-v2-turn-{}", std::process::id()));
        let _ = fs::remove_dir_all(&workspace);
        fs::create_dir_all(&workspace).expect("temp workspace");
        let harness = harness_with_stream(&workspace, V2_STREAM);

        let out = match harness.run_role("feature_scout", &role_task(), None) {
            Ok(out) => out,
            Err(error) => panic!("a readable v2 turn succeeds: {error}"),
        };

        assert_eq!(
            out.raw, "DONE",
            "the role output is the assistant text, never the NDJSON transcript"
        );
        // Usage: the vendor export cannot be read here (the harness's binary does not exist), so the fallback
        // applies — the sum of the turn's own `step_finish` events. It is a LOWER BOUND, not the preferred
        // figure: the vendor's `session export` is preferred whenever it can be read.
        assert_eq!(
            out.usage.as_ref().map(|usage| usage.tokens_input),
            Some(650),
            "with no readable export the stream's own step_finish sum is the recorded reading"
        );

        let cwd = workspace.to_string_lossy().to_string();
        let stored = read_session_id(&cwd);
        assert_eq!(
            stored.as_deref(),
            Some("ses_fixture_turn_0001"),
            "a FRESH turn must keep the id the VENDOR minted, or the next turn cannot resume it. \
             V1 wrote back the id it had asked for — None here — so the first session was never kept."
        );

        // And that stored id is exactly what the next turn asks for.
        let next = crate::engine::opencode_client::build_opencode_run_args(
            &harness.model,
            "turn 2",
            true,
            stored.as_deref(),
            false,
            None,
        );
        assert!(
            next.windows(2)
                .any(|w| w == ["--session", "ses_fixture_turn_0001"]),
            "the second turn resumes the captured id explicitly: {next:?}"
        );
        assert!(!next.iter().any(|arg| arg == "--continue"));
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn a_successful_exit_with_an_unreadable_v2_stream_is_a_failed_turn() {
        let workspace = std::env::temp_dir().join(format!("forge-v2-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&workspace);
        fs::create_dir_all(&workspace).expect("temp workspace");
        let harness = harness_with_stream(&workspace, "this is not json at all\n");

        let err = match harness.run_role("feature_scout", &role_task(), None) {
            Ok(_) => panic!("exit 0 with a broken structured contract must not read as a success"),
            Err(error) => error.to_string(),
        };
        assert!(
            err.contains("cannot read"),
            "the failure names the broken contract: {err}"
        );
        let _ = fs::remove_dir_all(&workspace);
    }

    /// The refusal for an unmappable node is PRE-FLIGHT: no vendor process is started, so no turn can be spent
    /// and no default agent — whose permissions Forge did not author — can be reached by an id Forge has never
    /// heard of. A post-flight check would be a receipt for work already done under an unknown authority.
    #[test]
    fn a_node_with_no_agent_is_refused_before_any_model_turn_is_started() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let workspace =
            std::env::temp_dir().join(format!("forge-v2-unmapped-{}", std::process::id()));
        let _ = fs::remove_dir_all(&workspace);
        fs::create_dir_all(&workspace).expect("temp workspace");

        let starts = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&starts);
        let mut harness = harness_with_stream(&workspace, V2_STREAM);
        harness.start_run = Some(Box::new(move |_opts, _on_line, _live| {
            counter.fetch_add(1, Ordering::SeqCst);
            StreamedRunResult {
                status: crate::engine::opencode_client::OpenCodeRunStatus::Success,
                exit_code: Some(0),
                stderr: String::new(),
                stop: None,
                lines: 0,
                last_line_ms: 0,
                silent_ticks: 0,
                termination: None,
            }
        }));

        // "qa" is a POSITION, not a dispatchable node: `forge_role_node_plan` (which the runner applies through
        // `ForgePhaseAgent::new` before it ever reaches a harness) does not name it.
        let err = match harness.run_role("qa", &role_task(), None) {
            Ok(_) => panic!("an unmappable node must not run a turn"),
            Err(error) => error.to_string(),
        };
        assert!(
            err.contains("No Forge agent-runtime mapping for engine node 'qa'"),
            "the refusal must name the node and the missing mapping: {err}"
        );
        assert_eq!(
            starts.load(Ordering::SeqCst),
            0,
            "the model must not be started at all: a turn under an unowned agent is unauthorized work"
        );
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn the_row_decides_the_model_and_an_explicit_override_still_wins() {
        // The row's policy is what the harness would run on when no explicit model is set.
        let policy_model = resolve_model_for_policy(Some("cheap")).expect("cheap resolves");
        assert_eq!(
            policy_model,
            resolve_opencode_model(Some(&policy_model)).unwrap()
        );
        assert_eq!(
            resolve_model_for_policy(Some("cheap")).expect("cheap resolves"),
            model_for_policy(Some("cheap"))
        );
        // An explicit empty override is refused rather than silently replaced (the harness's own rule).
        assert!(resolve_opencode_model(Some("")).is_err());
    }

    /// A transcript whose first step already costs real money: the shape a live spend cap is there to stop.
    const SPEND_STREAM: &str = concat!(
        r#"{"type":"step_finish","sessionID":"ses_spend","part":{"type":"step-finish","reason":"tool-calls","cost":0.0005,"tokens":{"input":10,"output":2}}}"#,
        "\n",
        r#"{"type":"step_finish","sessionID":"ses_spend","part":{"type":"step-finish","reason":"stop","cost":0.0005,"tokens":{"input":10,"output":2}}}"#,
        "\n",
        r#"{"type":"text","sessionID":"ses_spend","part":{"type":"text","text":"DONE"}}"#,
        "\n",
    );

    /// `BUDGET_EXHAUSTED`, enforced DURING the turn rather than reported after it: the second step never runs, and
    /// the text that would have followed it is never read as if the turn had finished nicely.
    #[test]
    fn a_turn_that_passes_the_spend_cap_is_stopped_and_is_not_read_as_an_answer() {
        let workspace = std::env::temp_dir().join(format!("forge-v2-cap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&workspace);
        fs::create_dir_all(&workspace).expect("temp workspace");
        let mut harness = harness_with_stream(&workspace, SPEND_STREAM);
        // A cap the FIRST step already exceeds: this is the smallest honest test of a live stop.
        harness.spend_cap_usd = Some(0.000001);

        let err = match harness.run_role("feature_scout", &role_task(), None) {
            Ok(_) => {
                panic!("a stopped turn must not be read as an answer, however much text arrived")
            }
            Err(error) => error.to_string(),
        };
        assert!(
            err.contains("BUDGET_EXHAUSTED"),
            "the stop must name its code: {err}"
        );
        assert!(
            err.contains("FORGE_SPEND_CAP_USD"),
            "and must say what would lift the cap: {err}"
        );
        assert!(
            err.contains("stopped for feature_scout"),
            "a stopped turn is not a failure of the harness: {err}"
        );
        let _ = fs::remove_dir_all(&workspace);
    }

    /// The control: the SAME transcript under a cap it never reaches runs to completion and reports its spend. A
    /// stop that fires whatever the cap would not be a control, it would be a broken turn.
    #[test]
    fn the_same_turn_under_a_cap_it_respects_is_allowed_to_finish() {
        let workspace = std::env::temp_dir().join(format!("forge-v2-nocap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&workspace);
        fs::create_dir_all(&workspace).expect("temp workspace");
        let mut harness = harness_with_stream(&workspace, SPEND_STREAM);
        harness.spend_cap_usd = Some(0.01);

        let out = match harness.run_role("feature_scout", &role_task(), None) {
            Ok(out) => out,
            Err(error) => panic!("a turn inside its cap must be allowed to finish: {error}"),
        };
        assert_eq!(out.raw, "DONE");
        let usage = out
            .usage
            .expect("the stream's own step_finish sum measures it");
        assert!(
            (usage.cost_usd - 0.001).abs() < 1e-9,
            "the spend is the sum the transcript reported: {usage:?}"
        );
        let _ = fs::remove_dir_all(&workspace);
    }

    /// A harness whose vendor is a script Forge really spawns. The seam cannot test an interruption — the whole
    /// question is whether a signal reaches a process — so this one uses the real streaming path.
    fn harness_with_real_cli(workspace: &Path, script: &str) -> OpenCodeHarness {
        let path = workspace.join("fake-opencode.sh");
        fs::write(&path, format!("#!/bin/sh\n{script}\n")).expect("script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
        }
        let mut harness = harness_with_stream(workspace, "");
        harness.cli_bin = path.to_string_lossy().to_string();
        harness.start_run = None;
        harness
    }

    /// `interrupt_execution` stops a REAL process, from ANOTHER thread, and the turn reports the stop by name.
    ///
    /// This is the test the whole interruption story rests on: a supervisor stopping a lane has no result to read
    /// and no `stop` to inspect — it only has the harness, a reason, and a running process. If the signal does not
    /// reach the process, the turn sits in its silence until the script decides to end, which is exactly the spend
    /// a hard stop exists to prevent.
    #[test]
    fn an_interrupt_from_another_thread_stops_the_real_process_and_is_reported_by_name() {
        use std::sync::Arc;
        use std::time::{Duration, Instant};

        let workspace =
            std::env::temp_dir().join(format!("forge-v2-interrupt-{}", std::process::id()));
        let _ = fs::remove_dir_all(&workspace);
        fs::create_dir_all(&workspace).expect("temp workspace");
        // One readable line, then 30 seconds of silence. The `session` case matters: the harness reads a turn's
        // spend through `session export`/`session list` AFTER the turn, so a stand-in that slept for those too would
        // make a fast stop look slow — the delay would be the measurement, not the interruption.
        let harness = Arc::new(harness_with_real_cli(
            &workspace,
            r#"case "$1" in
  session) printf '{}'; exit 0 ;;
esac
printf '{"type":"step_start","sessionID":"ses_interrupt"}\n'
sleep 30"#,
        ));

        let interrupter = {
            let harness = Arc::clone(&harness);
            std::thread::spawn(move || {
                harness
                    .begin_execution("job-lease-1")
                    .expect("register execution before launch");
                // An interrupt can only reach a process that exists, so wait for the turn to publish itself.
                for _ in 0..400 {
                    let published = harness
                        .execution_slot("job-lease-1")
                        .expect("execution slot")
                        .lock()
                        .map(|slot| slot.running.is_some())
                        .unwrap_or(false);
                    if published {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                harness.interrupt_execution_scoped(
                    "job-lease-1",
                    "the test supervisor stopped this turn",
                )
            })
        };

        let started = Instant::now();
        let err = match harness.run_role_scoped("job-lease-1", "feature_scout", &role_task(), None)
        {
            Ok(_) => panic!("a stopped turn must not be read as an answer"),
            Err(error) => error.to_string(),
        };
        let elapsed = started.elapsed();
        let receipt = interrupter
            .join()
            .expect("the interrupter thread")
            .expect("interrupt_execution")
            .expect("a receipt: something was running");

        assert!(
            err.contains("TURN_INTERRUPTED"),
            "an interruption from outside must be named, not reported as a crash: {err}"
        );
        assert!(
            err.contains("the test supervisor stopped this turn"),
            "the reason must survive into the record: {err}"
        );
        assert!(
            receipt.existed,
            "the interrupt reached a live process: {receipt:?}"
        );
        assert!(
            !receipt.killed,
            "TERM was enough, so no KILL was needed: {receipt:?}"
        );
        assert!(
            elapsed < Duration::from_secs(20),
            "the turn must not have waited out the script's 30 seconds: {elapsed:?}"
        );
        harness.finish_execution("job-lease-1");
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn scoped_interrupt_stops_only_the_named_execution() {
        use std::process::Command;

        let workspace =
            std::env::temp_dir().join(format!("forge-v2-scoped-{}", std::process::id()));
        let _ = fs::remove_dir_all(&workspace);
        fs::create_dir_all(&workspace).expect("temp workspace");
        let harness = harness_with_stream(&workspace, V2_STREAM);
        harness.begin_execution("job-a").expect("register A");
        harness.begin_execution("job-b").expect("register B");
        let mut b = Command::new("sleep").arg("30").spawn().expect("sleep B");
        harness
            .execution_slot("job-a")
            .expect("slot A")
            .lock()
            .unwrap()
            .running = Some(crate::engine::opencode_client::RunningTurn { pid: u32::MAX });
        harness
            .execution_slot("job-b")
            .expect("slot B")
            .lock()
            .unwrap()
            .running = Some(crate::engine::opencode_client::RunningTurn { pid: b.id() });

        let receipt = harness
            .interrupt_execution_scoped("job-a", "cancel lane A")
            .expect("interrupt A")
            .expect("A was running");
        assert!(
            !receipt.existed,
            "the test PID is absent, but the exact slot was addressed"
        );
        assert!(b.try_wait().expect("poll B").is_none(), "B remains running");
        let _ = b.kill();
        let _ = b.wait();
        harness.finish_execution("job-a");
        harness.finish_execution("job-b");
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn cancellation_latched_before_launch_prevents_model_start() {
        let workspace =
            std::env::temp_dir().join(format!("forge-v2-prelaunch-{}", std::process::id()));
        let _ = fs::remove_dir_all(&workspace);
        fs::create_dir_all(&workspace).expect("temp workspace");
        let harness = harness_with_stream(&workspace, V2_STREAM);
        harness
            .begin_execution("job-before-launch")
            .expect("register execution");
        assert!(harness
            .interrupt_execution_scoped("job-before-launch", "lease lost before launch")
            .expect("interrupt before launch")
            .is_none());
        let error =
            match harness.run_role_scoped("job-before-launch", "feature_scout", &role_task(), None)
            {
                Ok(_) => panic!("a prelaunch cancellation must refuse to start a model"),
                Err(error) => error,
            };
        assert!(error.to_string().contains(TURN_INTERRUPTED_CODE));
        harness.finish_execution("job-before-launch");
        let _ = fs::remove_dir_all(&workspace);
    }

    /// Nothing running is an ANSWER, not a failure: a supervisor that stops an idle harness must not be told a
    /// process was killed, and a harness with no subprocess must not claim a stop it cannot perform.
    #[test]
    fn interrupting_an_idle_harness_reports_that_nothing_was_stopped() {
        let workspace = std::env::temp_dir().join(format!("forge-v2-idle-{}", std::process::id()));
        let _ = fs::remove_dir_all(&workspace);
        fs::create_dir_all(&workspace).expect("temp workspace");
        let harness = harness_with_stream(&workspace, V2_STREAM);

        assert!(
            harness
                .interrupt_execution("nothing is running")
                .expect("no error")
                .is_none(),
            "an idle harness has nothing to stop and must say so"
        );
        let _ = fs::remove_dir_all(&workspace);
    }

    #[test]
    fn the_turn_ceiling_defaults_and_can_be_switched_off_only_by_name() {
        let minutes = |m: u64| Some(std::time::Duration::from_secs(m * 60));
        assert_eq!(turn_ceiling(None), minutes(DEFAULT_TURN_CEILING_MINUTES));
        assert_eq!(
            turn_ceiling(Some("  ")),
            minutes(DEFAULT_TURN_CEILING_MINUTES)
        );
        assert_eq!(turn_ceiling(Some("45")), minutes(45));
        assert_eq!(
            turn_ceiling(Some("forty")),
            minutes(DEFAULT_TURN_CEILING_MINUTES),
            "an unreadable value is the default, never unbounded"
        );
        for off in ["0", "off", "OFF", "none"] {
            assert_eq!(turn_ceiling(Some(off)), None, "{off}");
        }
    }
}
