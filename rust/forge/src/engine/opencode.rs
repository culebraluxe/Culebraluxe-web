//! Port of `agent-runtime/opencode/opencode-harness-adapter.ts` as RoleHarness.
//! OpenCode is the inner engine. Forge owns worktree, commit, assay, publish.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::engine::assay::{is_rust_contract_production_path, CommandResult};
use crate::engine::harness_usage::UsageBaseline;
use crate::engine::opencode_agents;
use crate::engine::opencode_client::{
    live_turn_slot, start_opencode_run_streaming, LiveTurnSlot, OpenCodeStartOptions, StreamStop,
    StreamedRunResult, TurnTermination,
};
use crate::engine::opencode_events;
use crate::engine::packet::{build_task_text_with_context, ExecutionWorkspace, StoryPacket};
use crate::engine::runner::{HarnessOutput, RoleHarness};
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::engine::spend_cap::{self, BUDGET_EXHAUSTED_CODE, SPEND_CAP_ENV};
use crate::engine::vendor_session;
use workflow::{Result, WorkflowError};

/// Live pin from ENG-FORGE-V5-01. Override only with OPENCODE_MODEL.
///
/// RENAMED UPSTREAM, 2026-09-29. The pin was `deepseek/deepseek-v4-flash` from the port until today, and that
/// id no longer exists: `opencode models` now lists `deepseek/deepseek-flash` and `deepseek/deepseek-v4-pro`,
/// and an unknown id comes back as `{"name":"UnknownError","message":"Unexpected server error"}` — a
/// provider-shaped error that reads like an outage. Every role died on its first turn because of it (a bare
/// `opencode run --model deepseek/deepseek-v4-flash` fails the same way, which is how this was told apart from
/// a Forge defect). `deepseek-flash` is the renamed same tier, and answers a smoke prompt.
pub const OPENCODE_PINNED_MODEL: &str = "deepseek/deepseek-flash";
pub const OPENCODE_HARNESS_ADAPTER_ID: &str = "opencode-harness";
/// The V2 lane's session marker.
///
/// DELIBERATELY NOT the V1 name (`.forge-session.continue`). A V1 marker sitting in a workspace must never
/// become the authority for a V2 resume — the two generations cannot be confused if they never share a file
/// (§4). The marker holds the exact id V2 minted for this lane's last turn.
pub const SESSION_MARKER_FILENAME: &str = ".forge-opencode-v2-session";
pub const SESSION_CONTINUITY_ENV: &str = "FORGE_SESSION_CONTINUITY";

/// The vendor-session lane key this harness reads and writes.
///
/// V1 wrote under `opencode`; V2 writes under `opencode-v2`. An old V1 session id therefore cannot be picked
/// up as if it were a V2 session, and nothing is deleted: the V1 rows simply stop being read (§4).
pub const VENDOR_SESSION_LANE: &str = "opencode-v2";

/// The attended override that names the vendor CLI explicitly. Honoured exactly as given — and then VERIFIED like
/// any other candidate: an explicit path is not evidence that the binary behind it speaks the contract.
pub const VENDOR_CLI_OVERRIDE_ENV: &str = "OPENCODE_BIN";

/// Where the vendor's own installer keeps its CLI, relative to `$HOME`.
pub const VENDOR_CLI_HOME_RELATIVE: &str = ".opencode/bin/opencode";

/// The name Forge falls back to when nothing else resolves: whatever `PATH` happens to call `opencode`.
pub const VENDOR_CLI_PATH_NAME: &str = "opencode";

/// Which vendor binary Forge will run, as a PURE function of its two inputs, so the precedence is testable without
/// mutating the process environment.
///
/// MEASURED 2026-10-03, and the reason this is not `"opencode"` alone: **two different vendors install a CLI named
/// `opencode` on this machine.** `$HOME/.opencode/bin/opencode` is v2.0.21 — the build this adapter is written
/// against, the one the operator's interactive shell finds, the one running here as `serve --service`. Homebrew's
/// `/opt/homebrew/bin/opencode` is the older npm `opencode-ai` 1.18.26, whose `run` does **not** accept
/// `--standalone` and whose `--format` defaults to `default` rather than `json`.
///
/// That `PATH` export lives in `~/.zshrc`, so only an INTERACTIVE shell sees it. A lane launched with the
/// login/default PATH therefore resolved `opencode` to 1.18.26, and the role turn died on exit 1 with the vendor's
/// own help text as the error — `ENG-FORGE-C1-BUILD-INFO-01`, durable job `b319bf40`, recorded verbatim in
/// `jobs.last_error`. Forge's argument list was correct; the binary behind the name was not. So the vendor's own
/// install root is preferred over a `PATH` name two installers disagree about, and `OPENCODE_BIN` still wins
/// outright for an attended override.
pub fn cli_candidates(explicit: Option<&str>, vendor_home_cli: Option<String>) -> Vec<String> {
    if let Some(explicit) = explicit.map(str::trim).filter(|value| !value.is_empty()) {
        return vec![explicit.to_string()];
    }
    let mut candidates = Vec::new();
    if let Some(home) = vendor_home_cli
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
    {
        candidates.push(home);
    }
    candidates.push(VENDOR_CLI_PATH_NAME.to_string());
    candidates
}

/// `$HOME/.opencode/bin/opencode` — the vendor's own user-scoped install, and the one its installer upgrades.
pub fn vendor_home_cli_bin() -> Option<String> {
    std::env::var("HOME")
        .ok()
        .map(|home| home.trim().to_string())
        .filter(|home| !home.is_empty())
        .map(|home| {
            PathBuf::from(home)
                .join(VENDOR_CLI_HOME_RELATIVE)
                .to_string_lossy()
                .to_string()
        })
}

/// A candidate only outranks `PATH` when a program is actually there: an absent path must not shadow a working
/// binary, or the repair for one broken machine becomes the cause of the next.
fn is_executable_file(path: &str) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// The vendor CLI this process will run, by precedence. What it deliberately does NOT do is decide whether that
/// binary speaks the V2 contract — that is `engine::opencode_client::verify_vendor_contract`, asked once at lane
/// start, before a token is spent and before a claim is burned on a binary that was never going to answer.
pub fn default_cli_bin() -> String {
    let explicit = std::env::var(VENDOR_CLI_OVERRIDE_ENV).ok();
    let vendor_home = vendor_home_cli_bin().filter(|path| is_executable_file(path));
    cli_candidates(explicit.as_deref(), vendor_home)
        .into_iter()
        .next()
        .unwrap_or_else(|| VENDOR_CLI_PATH_NAME.into())
}

fn smith_writes_code(node_id: &str) -> bool {
    matches!(
        node_id,
        "smith"
            | "smith_split_work"
            | "repair_smith"
            | "fast_smith"
            | "fast_repair_smith"
            | "lead_solo_implement"
    )
}

fn blocked_model_env_key(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    upper == "APP_ENV"
        || upper == "EXECUTION_ENV"
        || upper == "VERCEL_ENV"
        || upper == "DATABASE_URL"
        || upper.starts_with("DATABASE_URL_")
        || upper.starts_with("NEON_")
        || matches!(
            upper.as_str(),
            "PGHOST"
                | "PGPORT"
                | "PGDATABASE"
                | "PGUSER"
                | "PGPASSWORD"
                | "PGSERVICE"
                | "PGSERVICEFILE"
                | "PGPASSFILE"
        )
}

/// Smith runs code with the machine's normal toolchain/provider configuration, but it does not inherit the
/// control plane's production database authority. Git publication is also disabled inside the model subprocess:
/// DEV_OPS/Forge publishes the accepted candidate, never Smith.
pub fn sanitize_model_env(mut env: HashMap<String, String>) -> HashMap<String, String> {
    env.retain(|key, _| !blocked_model_env_key(key));
    env.insert("GIT_CONFIG_COUNT".into(), "1".into());
    env.insert("GIT_CONFIG_KEY_0".into(), "remote.origin.pushurl".into());
    env.insert("GIT_CONFIG_VALUE_0".into(), "/dev/null".into());
    env
}

pub fn sanitized_model_env() -> HashMap<String, String> {
    sanitize_model_env(std::env::vars().collect())
}

/// The two policies an `agent_work_item` may carry (migration 179: `cheap` | `judgment`, NULL reads as `cheap`).
pub const FORGE_MODEL_POLICIES: [&str; 2] = ["cheap", "judgment"];

/// The model each policy names.
///
/// Ported from the legacy `lib/forge-kind.ts` table (`MODEL_FOR_POLICY`), whose assertions survive the deletion of
/// `lib/` in `legacy/workflow_app/tests/forge-kind-routing.test.ts:104-125`: there are exactly two policies and every
/// one names a model the price table can price.
///
/// BOTH name the flash tier, and the `judgment` row is deliberate (captain, 2026-09-16, quoted in that test): the
/// pro/chat tier is interactive-only and cannot be billed per token, so a seat sent there could not answer and every
/// "dear" run was silently a flash run anyway. The point of the table is unchanged — every policy names a priceable
/// model, which is what keeps the cost lens honest and stops a third provider creeping in.
pub const MODEL_FOR_CHEAP: &str = OPENCODE_PINNED_MODEL;
pub const MODEL_FOR_JUDGMENT: &str = OPENCODE_PINNED_MODEL;

/// An unrecognised or absent policy reads as `cheap` rather than throwing (legacy `asModelPolicy`, same test file:
/// "an unknown kind or policy reads as the default"). The default is the cheaper one because it bills least.
pub fn as_model_policy(raw: Option<&str>) -> &'static str {
    match raw.map(str::trim) {
        Some("judgment") => "judgment",
        _ => "cheap",
    }
}

/// The model the row's `model_policy` names.
pub fn model_for_policy(raw: Option<&str>) -> &'static str {
    match as_model_policy(raw) {
        "judgment" => MODEL_FOR_JUDGMENT,
        _ => MODEL_FOR_CHEAP,
    }
}

/// The model the lane will bill. Defaults match `model_for_policy`.
/// `OPENCODE_JUDGMENT_MODEL` is the only way the judgment policy names a different model
/// than cheap — the 2026-09-16 pin made both flash because pro could not be billed per token.
/// Setting the env is how that rail becomes observable without changing the default pin.
pub fn resolve_model_for_policy(raw: Option<&str>) -> String {
    match as_model_policy(raw) {
        "judgment" => std::env::var("OPENCODE_JUDGMENT_MODEL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| MODEL_FOR_JUDGMENT.to_string()),
        _ => MODEL_FOR_CHEAP.to_string(),
    }
}

pub fn resolve_opencode_model(model: Option<&str>) -> Result<String> {
    match model {
        None => Ok(std::env::var("OPENCODE_MODEL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| OPENCODE_PINNED_MODEL.into())),
        Some(v) if v.trim().is_empty() => Err(WorkflowError::generic(format!(
            "OpenCode harness has no explicit model configuration: expected '{OPENCODE_PINNED_MODEL}', got empty. Refusing to rely on OpenCode's default model selection."
        ))),
        Some(v) => Ok(v.trim().to_string()),
    }
}

pub fn session_continuity_enabled() -> bool {
    match std::env::var(SESSION_CONTINUITY_ENV) {
        Ok(raw) => {
            let v = raw.trim().to_ascii_lowercase();
            v != "0" && v != "false" && v != "off"
        }
        Err(_) => true,
    }
}

pub fn session_marker_path(workspace: &str) -> PathBuf {
    Path::new(workspace).join(SESSION_MARKER_FILENAME)
}

pub fn read_session_id(workspace: &str) -> Option<String> {
    let raw = fs::read_to_string(session_marker_path(workspace)).ok()?;
    let t = raw.trim();
    if t.is_empty() || t == "1" {
        None
    } else {
        Some(t.to_string())
    }
}

pub fn write_session_id(workspace: &str, session_id: Option<&str>) {
    let path = session_marker_path(workspace);
    if let Some(id) = session_id {
        let _ = fs::write(path, format!("{id}\n"));
    } else {
        let _ = fs::remove_file(path);
    }
}

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
            None => resolve_model_for_policy(model_policy),
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

impl RoleHarness for OpenCodeHarness {
    fn run_role(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
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
        // The config travels with the process (§3): a worktree does not discover the repo-root `opencode.json`,
        // so environment delivery is the path that actually applies.
        let model_env = v2_agent_env(self.env.as_ref())?;
        let lane = VENDOR_SESSION_LANE;
        let session = if session_continuity_enabled() {
            if let Some(story) = self.story_id.as_deref() {
                vendor_session::read_vendor_session_id(story, lane)
                    .ok()
                    .flatten()
                    .or_else(|| read_session_id(&cwd))
            } else {
                read_session_id(&cwd)
            }
        } else {
            None
        };
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
        if let Ok(mut slot) = self.live_turn.lock() {
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
            Some(start) => start(opts, &mut on_line, &self.live_turn),
            None => start_opencode_run_streaming(opts, &mut on_line, &self.live_turn),
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
        let interrupt_reason = self.live_turn.lock().ok().and_then(|mut slot| {
            let reason = slot.interrupt_reason.clone();
            *slot = crate::engine::opencode_client::LiveTurn::default();
            reason
        });
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
        let mut raw = turn.assistant_text;
        let sha = self.run_git(&["rev-parse", "HEAD"]);
        let mut candidate_sha = sha.clone();
        let mut refusal = None;
        let mut measured_base = None;
        if smith_writes_code(node_id) {
            let execution_base = self
                .execution_workspace
                .as_ref()
                .map(|workspace| workspace.base_commit.as_str())
                .or(before_sha.as_deref());
            let rejection = match (execution_base, sha.as_deref()) {
                (None, _) => Some(format!(
                    "Smith candidate refused for {node_id}: execution base is unreadable"
                )),
                (_, None) => Some(format!(
                    "Smith candidate refused for {node_id}: HEAD was unreadable after the role turn"
                )),
                (Some(base), Some(after)) if after == base => Some(format!(
                    "Smith candidate refused for {node_id}: no new commit was created (HEAD stayed {after})"
                )),
                (Some(_), Some(after))
                    if after.len() != 40
                        || !after.bytes().all(|byte| byte.is_ascii_hexdigit()) =>
                {
                    Some(format!(
                        "Smith candidate refused for {node_id}: {after:?} is not a full commit SHA"
                    ))
                }
                (Some(base), Some(after))
                    if self
                        .run_git(&["merge-base", "--is-ancestor", base, after])
                        .is_none() =>
                {
                    Some(format!(
                        "Smith candidate refused for {node_id}: candidate {after} is not a descendant of execution base {base}"
                    ))
                }
                (Some(_), Some(after)) => {
                    match self.run_git(&["status", "--porcelain"]) {
                        None => Some(format!(
                            "Smith candidate refused for {node_id}: git status is unreadable"
                        )),
                        Some(dirty) if !dirty.trim().is_empty() => Some(format!(
                            "Smith candidate refused for {node_id}: uncommitted work remains after candidate {after}: {}",
                            dirty.lines().take(8).collect::<Vec<_>>().join(" | ")
                        )),
                        Some(_) => {
                            let base = execution_base.unwrap_or(after);
                            let range = format!("{base}..{after}");
                            // `--no-renames`: with rename detection a file moved OUT of a production root lists
                            // only its new path, and the move passes the RUST_CONTRACT check below.
                            match self.run_git(&["diff", "--name-only", "--no-renames", &range]) {
                                None => Some(format!(
                                    "Smith candidate refused for {node_id}: changed paths are unreadable for {range}"
                                )),
                                Some(changed) if changed.trim().is_empty() => Some(format!(
                                    "Smith candidate refused for {node_id}: candidate {after} changes no files from execution base {base}"
                                )),
                                Some(changed)
                                    if self.packet.test_mode.as_deref() == Some("RUST_CONTRACT")
                                        && changed
                                            .lines()
                                            .any(is_rust_contract_production_path) =>
                                {
                                    Some(format!(
                                        "Smith candidate refused for {node_id}: RUST_CONTRACT candidate modified production code across {range}"
                                    ))
                                }
                                Some(_) => None,
                            }
                        }
                    }
                }
            };
            if let Some(reason) = rejection.as_deref() {
                candidate_sha = None;
                raw.push_str("\nSMITH_CANDIDATE_REJECTED: ");
                raw.push_str(reason);
            }
            refusal = rejection;
            measured_base = execution_base.map(str::to_string);
        }
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
            refusal,
            execution_base: measured_base,
            usage,
        })
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

    fn execution_base_commit(&self) -> Option<&str> {
        self.execution_workspace
            .as_ref()
            .map(|workspace| workspace.base_commit.as_str())
    }

    fn run_command(&self, command: &str) -> CommandResult {
        let cwd = self.assay_cwd();
        match Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(cwd)
            .output()
        {
            Ok(out) => {
                let excerpt = String::from_utf8_lossy(&out.stdout);
                let err = String::from_utf8_lossy(&out.stderr);
                let text = if excerpt.is_empty() { err } else { excerpt };
                let code = out.status.code().unwrap_or(1);
                CommandResult {
                    command: command.into(),
                    exit_code: code,
                    passed: code == 0,
                    excerpt: text.chars().take(240).collect(),
                    unmeasurable: false,
                    output: text.to_string(),
                }
            }
            Err(e) => CommandResult {
                command: command.into(),
                exit_code: -1,
                passed: false,
                excerpt: e.to_string(),
                unmeasurable: true,
                output: String::new(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let policy_model = resolve_model_for_policy(Some("cheap"));
        assert_eq!(
            policy_model,
            resolve_opencode_model(Some(&policy_model)).unwrap()
        );
        assert_eq!(
            resolve_model_for_policy(Some("cheap")),
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
                // An interrupt can only reach a process that exists, so wait for the turn to publish itself.
                for _ in 0..400 {
                    let published = harness
                        .live_turn
                        .lock()
                        .map(|slot| slot.running.is_some())
                        .unwrap_or(false);
                    if published {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                harness.interrupt_execution("the test supervisor stopped this turn")
            })
        };

        let started = Instant::now();
        let err = match harness.run_role("feature_scout", &role_task(), None) {
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
}
