//! Port of `agent-runtime/opencode/opencode-harness-adapter.ts` as RoleHarness.
//! OpenCode is the inner engine. Forge owns worktree, commit, assay, publish.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use crate::engine::assay::CommandResult;
use crate::engine::harness::TurnTermination;
pub use crate::engine::harness::TURN_INTERRUPTED_CODE;
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
mod role_harness;

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

/// A turn whose stream has failed to parse this many CONSECUTIVE lines is not being read at all, so
/// Forge stops the turn instead of running the spend guard on fiction: with no readable `step_finish`
/// events the cap cannot see spend, so an unreadable stream is an uncapped turn. Ten is the trip: a
/// line or two of vendor noise is weather, ten in a row is a broken pipe (WO-120).
pub const STREAM_PARSE_FAILURE_CAP: usize = 10;
/// Stop code for a turn stopped because its stream stayed unreadable (see [`STREAM_PARSE_FAILURE_CAP`]).
pub const STREAM_UNREADABLE_CODE: &str = "STREAM_UNREADABLE";

/// The parse-failure trip: `None` while the stream is still readable, `Some(stop)` once repeated
/// consecutive failures mean the turn's own spend accounting is fiction.
pub(crate) fn stream_parse_trip(consecutive_failures: usize) -> Option<StreamStop> {
    if consecutive_failures >= STREAM_PARSE_FAILURE_CAP {
        Some(StreamStop {
            code: STREAM_UNREADABLE_CODE.to_string(),
            detail: format!(
                "{consecutive_failures} consecutive stream lines failed to parse \
                 (cap {STREAM_PARSE_FAILURE_CAP}); stopping rather than running uncapped"
            ),
        })
    } else {
        None
    }
}

/// Error for a vendor-session persist failure: the turn SUCCEEDED, but without the persisted id the
/// lane's next turn cannot resume its session, so this propagates instead of silently stranding the
/// lane on fresh turns. Shared with the Maestro harness, which names its own source (WO-120).
pub(crate) fn vendor_session_persist_error(
    harness: &str,
    node_id: &str,
    story: &str,
    error: &str,
) -> WorkflowError {
    WorkflowError::generic(format!(
        "{harness} could not persist the vendor session for {node_id} (story {story}): {error}"
    ))
}

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
        self.run_role_with_slot_measured(node_id, task, self_heal, live_turn, &mut None, &mut None)
    }

    /// One turn, with the two measurements it produces published even when the turn does not succeed.
    ///
    /// `measured_usage` and `measured_trajectory` are OUT-parameters rather than parts of the return value because the
    /// failure paths below return `Err` — and a reading that only exists on the success path is exactly how a failed
    /// turn gets recorded as a turn that nothing happened in.
    fn run_role_with_slot_measured(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
        live_turn: &LiveTurnSlot,
        measured_usage: &mut Option<crate::engine::harness::HarnessUsage>,
        measured_trajectory: &mut Option<crate::engine::trajectory::TrajectoryMetrics>,
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
        tracing::info!(
            node = %node_id,
            agent = %agent,
            "opencode-harness {}",
            opencode_agents::render_subagent_declaration(
                opencode_agents::subagents_enabled_from_env(),
                std::env::var(opencode_agents::SUBAGENTS_ENV).ok().as_deref(),
            )
        );
        // Execution receipt, before the first model token is spent: what runs, where, as whom.
        tracing::info!(
            model = %self.model,
            bin = %self.cli_bin,
            cwd = %cwd,
            "harness=opencode"
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
        // WO-120: consecutive lines the scanner cannot read. Reset on every readable line — a line
        // or two of vendor noise is weather — and the turn stops when the trip below fires, because a
        // stream Forge cannot read is a turn whose spend the cap cannot see.
        let mut parse_failures: usize = 0;
        let mut on_line = |line: &str| -> Option<StreamStop> {
            // Kept verbatim: the transcript is what the turn is READ from below, so the live view and the recorded
            // view cannot disagree.
            transcript.push_str(line);
            transcript.push('\n');
            match scanner.feed(line) {
                Ok(()) => parse_failures = 0,
                Err(error) => {
                    parse_failures += 1;
                    tracing::warn!(
                        failures = parse_failures,
                        cap = STREAM_PARSE_FAILURE_CAP,
                        error = %error,
                        "opencode-harness: stream line failed to parse"
                    );
                    if let Some(stop) = stream_parse_trip(parse_failures) {
                        return Some(stop);
                    }
                }
            }
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
            let stopped_by = result
                .stop
                .as_ref()
                .map(|stop| stop.code.as_str())
                .unwrap_or("(none)");
            tracing::info!(
                node = %node_id,
                lines = result.lines,
                last_line_ms = result.last_line_ms,
                silent_ticks = result.silent_ticks,
                stopped_by = %stopped_by,
                "opencode-harness turn"
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
            tracing::warn!(
                exit_code = ?result.exit_code,
                reason = %reason,
                "opencode-harness interrupt_observed=true process_reaped=true"
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
        *measured_usage = usage.clone();
        // WHAT THE TURN DID — captured HERE, before any verdict below can return.
        //
        // Every exit from this point on (a Forge stop, a vendor failure, a stream Forge cannot read) still emitted
        // tool calls, and those calls ARE the measurement. Summarizing only on the success path recorded a failed
        // turn as a turn in which nothing happened, which is how a lane that thrashed for twenty edits and then
        // died looked identical to one that never started. `None` stays `None` when the stream could not be parsed:
        // unmeasured is not the same as measured zero, and a zero here would be a fabricated reading.
        if let Ok(events) = &turn {
            *measured_trajectory = Some(crate::engine::trajectory::summarize(&events.tool_calls));
        }
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
            if let Err(error) = vendor_session::write_vendor_session_id(
                story,
                VENDOR_SESSION_LANE,
                actual_session.as_deref().or(session.as_deref()),
            ) {
                tracing::warn!(
                    story = %story,
                    error = %error,
                    "opencode-harness: vendor session persist failed"
                );
                return Err(vendor_session_persist_error(
                    "opencode-harness",
                    node_id,
                    story,
                    &error,
                ));
            }
        }
        if let Some(id) = actual_session.as_deref() {
            write_session_id(&cwd, Some(id)).map_err(|error| {
                tracing::warn!(
                    workspace = %cwd,
                    error = %error,
                    "opencode-harness: session marker write failed"
                );
                WorkflowError::generic(format!(
                    "opencode-harness could not persist the session marker for {node_id} in {cwd}: {error}"
                ))
            })?;
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
            // The same summary captured above, not a second reading: the success path and the failure paths must
            // report the turn's work from one source or they can disagree about it.
            trajectory: (*measured_trajectory).clone(),
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

#[cfg(test)]
mod tests;
