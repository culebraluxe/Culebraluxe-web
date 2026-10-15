//! Port of `agent-runtime/opencode/opencode-harness-adapter.ts` as RoleHarness.
//! OpenCode is the inner engine. Forge owns worktree, commit, assay, publish.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use crate::engine::assay::CommandResult;
use crate::engine::harness::TurnTermination;
pub use crate::engine::harness::TURN_INTERRUPTED_CODE;
use crate::engine::harness_profile::HarnessProfile;
use crate::engine::harness_usage::UsageBaseline;
use crate::engine::opencode_agents;
use crate::engine::opencode_client::{
    live_turn_slot, start_opencode_run_streaming, LiveTurnSlot, OpenCodeStartOptions, StreamStop,
    StreamedRunResult,
};
use crate::engine::opencode_events;
use crate::engine::packet::{ExecutionWorkspace, StoryPacket, TaskIsolation, TaskText};
use crate::engine::planning::StopReason;
use crate::engine::runaway::{
    ActionRecord, ProgressSignal, RunawayDetector, RunawayPolicy, RUNAWAY_STOP_CODE,
};
use crate::engine::runner::{HarnessOutput, RoleHarness};
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::engine::spend_cap::{self, BUDGET_EXHAUSTED_CODE, SPEND_CAP_ENV};
use crate::engine::vendor_session;
use workflow::{Result, WorkflowError};

mod config;
mod materialize;
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
        Some(StreamStop::new(
            STREAM_UNREADABLE_CODE,
            format!(
                "{consecutive_failures} consecutive stream lines failed to parse \
                 (cap {STREAM_PARSE_FAILURE_CAP}); stopping rather than running uncapped"
            ),
            // Every turn is told to emit canonical events. A stream Forge cannot read broke that rule, which is
            // why this is a violation rather than an engine fault: the turn's own accounting was fiction.
            StopReason::PolicyViolation,
        ))
    } else {
        None
    }
}

/// A stop Forge applied because the attempt's own allowance ran out — its measured spend, or the wall-clock ceiling
/// of one turn (`opencode_client::turn_ceiling_stop` builds the same decision from the client side).
///
/// It is ONE function for one reason because the canonical vocabulary is the contract: if the cap and the ceiling
/// each spelled their own stop, an adjudicator reading two records would have to decide whether two spellings meant
/// one decision. Both are `Termination::Stop(BudgetExhausted)`, and neither can drift.
pub(crate) fn budget_stop(spend: Option<f64>, cap: f64) -> StreamStop {
    StreamStop::new(
        BUDGET_EXHAUSTED_CODE,
        spend_cap::render_spend_cap_line(spend, cap),
        StopReason::BudgetExhausted,
    )
}

/// A stop applied from OUTSIDE Forge's own decision — the turn's interrupt handle — in the same vocabulary.
///
/// Named rather than inlined so the live path and the record agree by construction: an interruption is
/// `Termination::Stop(Interrupted)` whether the driver cancelled a generation or an operator stopped a lane.
pub(crate) fn interrupt_stop(reason: &str) -> StreamStop {
    StreamStop::new(TURN_INTERRUPTED_CODE, reason, StopReason::Interrupted)
}

/// The live C4 guard: the runaway detector the resolved profile implies, plus how far it has read the action stream.
///
/// It is STATEFUL because the verdict is about a sequence: what makes an execution stuck is that the same action
/// happened again with nothing to show for it, not that any single action was wrong. It lives beside the spend cap
/// and the parse trip because it is the third live stop of the same kind — a stop Forge chooses mid-turn, on a
/// reading Forge took, rather than a verdict pronounced after the turn has already spent itself.
#[derive(Debug, Clone)]
pub(crate) struct LiveRunaway {
    policy: RunawayPolicy,
    detector: RunawayDetector,
    /// How many stream actions have already been folded. A live stream only appends, so a high-water mark is all
    /// the bookkeeping folding needs — and re-folding an action would invent a repeat that never happened.
    folded: usize,
    /// The previous shell call's reported exit status, or `None` when none was reported. A read-only lane's shell
    /// calls are its whole progress surface, so this is where that lane's progress is measured.
    last_shell_exit: Option<i64>,
}

impl LiveRunaway {
    pub(crate) fn new(policy: RunawayPolicy) -> Self {
        Self {
            detector: RunawayDetector::new(policy.clone()),
            policy,
            folded: 0,
            last_shell_exit: None,
        }
    }

    /// Fold every action reported since the last call and name the first threshold met, if any.
    ///
    /// WHAT IS MEASURED HERE, AND WHAT IS NOT. A turn's own event stream can show two of the four progress signals,
    /// and only those two are used: a mutating call (the vendor reported an edit tool) is a file change, and a shell
    /// call whose reported exit status DIFFERS from the previous shell call's is a diagnostics change, because the
    /// verification state moved. The other two — a changed candidate and new evidence — are not readable from the
    /// stream at all, so they stay false. An unmeasured signal must never be assumed to be progress: assuming it
    /// would forgive exactly the loop this guard exists to catch. `files_changed` is 1 and not a real diff count
    /// because the stream reports that a mutation happened, not how large it was — and any non-zero count is the
    /// same signal.
    pub(crate) fn observe(
        &mut self,
        actions: &[opencode_events::LiveAction],
    ) -> Option<StreamStop> {
        use crate::engine::trajectory::ToolKind;
        while self.folded < actions.len() {
            let action = &actions[self.folded];
            self.folded += 1;
            let mut progress = ProgressSignal::default();
            match action.kind {
                ToolKind::Edit => progress.files_changed = 1,
                ToolKind::Shell => {
                    if let Some(exit) = action.exit {
                        if self
                            .last_shell_exit
                            .is_some_and(|previous| previous != exit)
                        {
                            progress.diagnostics_changed = true;
                        }
                        self.last_shell_exit = Some(exit);
                    }
                }
                ToolKind::Structured => {}
            }
            let record = ActionRecord {
                signature: action.signature.clone(),
                progress,
            };
            if let Some(condition) = self.detector.observe(&record) {
                return Some(StreamStop::new(
                    RUNAWAY_STOP_CODE,
                    condition.detail(&self.policy, &action.signature),
                    // C3/C4, canonical: the guard does not stop the attempt for "a runaway" — a second vocabulary
                    // beside termination. It stops it because the attempt stopped making the progress Forge
                    // measures, which IS `Termination::Stop(NoProgress)`; the threshold it crossed is the detail.
                    StopReason::NoProgress,
                ));
            }
        }
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
    ///
    /// The text is built by [`TaskText`] from the SAME things the turn runs under, so the scaffold, the policy it
    /// was rendered from and the isolation the turn works in are all decided here rather than threaded through
    /// call sites. `profile` is the policy the lifecycle resolved, or `None` for a seam that resolved none — and
    /// that seam still gets the node's OWN lane policy, never nothing.
    fn task_text(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
        profile: Option<&HarnessProfile>,
    ) -> TaskText {
        let isolation = TaskIsolation::of(self.execution_workspace.as_ref());
        match profile {
            Some(profile) => TaskText::for_profile(
                profile,
                node_id,
                &task.task_id,
                &self.packet,
                isolation,
                self_heal,
            ),
            None => TaskText::for_node(node_id, &task.task_id, &self.packet, isolation, self_heal),
        }
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

    /// The slot for a durable execution, refusing one that was already cancelled.
    ///
    /// Cancelled BEFORE launch means no vendor process ran, so there is nothing observed to report — the one case
    /// where an empty report is the honest one, because the turn never spent anything. Every scoped entry point
    /// makes this same refusal, so it is made once here rather than copied into each of them.
    fn launchable_slot(&self, execution_id: &str) -> Result<LiveTurnSlot> {
        let slot = self.execution_slot(execution_id)?;
        if slot
            .lock()
            .map_err(|_| WorkflowError::generic("execution interrupt slot is poisoned"))?
            .interrupt_reason
            .is_some()
        {
            return Err(WorkflowError::generic(format!(
                "{}: execution was cancelled before model launch",
                TURN_INTERRUPTED_CODE
            )));
        }
        Ok(slot)
    }

    fn run_role_with_slot(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
        live_turn: &LiveTurnSlot,
    ) -> Result<HarnessOutput> {
        self.run_role_with_slot_measured(
            node_id, task, self_heal, live_turn, None, &mut None, &mut None,
        )
    }
}

#[cfg(test)]
mod tests;
mod turn;
