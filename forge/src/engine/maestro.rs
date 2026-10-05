//! Maestro execution harness adapter.
//!
//! Minimal V1 implementation: executes a single agent turn using the Maestro CLI
// inside the Forge-supplied worktree. No orchestration, no playbook, no Auto Run.
//!
//! Forge remains the control plane; Maestro is only an execution substrate.

use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::engine::assay::CommandResult;
use crate::engine::harness::HarnessUsage;
use crate::engine::packet::{ExecutionWorkspace, StoryPacket};
use crate::engine::runner::{HarnessOutput, RoleHarness};
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::engine::spend_cap::{self, SPEND_CAP_ENV};
use crate::engine::vendor_session;
use workflow::{Result, WorkflowError};

/// Maestro CLI binary name (can be overridden with MAESTRO_BIN env var).
///
/// Resolved from `PATH` at spawn: unlike OpenCode, Maestro's CLI is not two vendors disagreeing about one
/// name — `maestro-cli` is the Maestro interface, and `maestro` is not it. An explicit `MAESTRO_BIN` still
/// wins outright for an attended override.
pub const MAESTRO_CLI_DEFAULT: &str = "maestro-cli";

/// Environment variable to override the Maestro CLI binary.
const MAESTRO_BIN_ENV: &str = "MAESTRO_BIN";

/// The vendor-session lane key this harness reads and writes.
pub const VENDOR_SESSION_LANE: &str = "maestro-v2";

/// Session marker filename for workspace-level session continuity.
pub const SESSION_MARKER_FILENAME: &str = ".forge-maestro-v2-session";

/// Environment variable to control session continuity.
pub const SESSION_CONTINUITY_ENV: &str = "FORGE_SESSION_CONTINUITY";

/// Environment variable for turn ceiling (wall-clock timeout).
pub const TURN_CEILING_ENV: &str = "FORGE_TURN_TIMEOUT_MINUTES";

/// Default turn ceiling in minutes.
pub const DEFAULT_TURN_CEILING_MINUTES: u64 = 120;

/// Parse turn ceiling from environment (same logic as OpenCode).
fn turn_ceiling(raw: Option<&str>) -> Option<Duration> {
    let minutes = match raw.map(str::trim) {
        None | Some("") => DEFAULT_TURN_CEILING_MINUTES,
        Some(word)
            if word == "0"
                || word.eq_ignore_ascii_case("off")
                || word.eq_ignore_ascii_case("none") =>
        {
            return None;
        }
        Some(word) => word.parse::<u64>().unwrap_or(DEFAULT_TURN_CEILING_MINUTES),
    };
    Some(Duration::from_secs(minutes * 60))
}

/// Whether session continuity is enabled.
fn session_continuity_enabled() -> bool {
    match std::env::var(SESSION_CONTINUITY_ENV) {
        Ok(raw) => {
            let v = raw.trim().to_ascii_lowercase();
            v != "0" && v != "false" && v != "off"
        }
        Err(_) => true,
    }
}

/// Path to the session marker file in the workspace.
fn session_marker_path(workspace: &str) -> PathBuf {
    Path::new(workspace).join(SESSION_MARKER_FILENAME)
}

/// Read session ID from workspace marker file.
fn read_session_id(workspace: &str) -> Option<String> {
    let raw = fs::read_to_string(session_marker_path(workspace)).ok()?;
    let t = raw.trim();
    if t.is_empty() || t == "1" {
        None
    } else {
        Some(t.to_string())
    }
}

/// Write session ID to workspace marker file.
fn write_session_id(workspace: &str, session_id: Option<&str>) {
    let path = session_marker_path(workspace);
    if let Some(id) = session_id {
        let _ = fs::write(path, format!("{id}\n"));
    } else {
        let _ = fs::remove_file(path);
    }
}

/// Resolve which session to resume — pure function for testability.
///
/// Precedence:
/// 1. Workspace marker (same-directory proof)
/// 2. Stored lane session (only if it lives in this directory)
/// 3. None (fresh turn)
fn resume_session(
    workspace_marker: Option<String>,
    lane_session: Option<String>,
    lane_session_lives_here: bool,
) -> Option<String> {
    workspace_marker.or_else(|| lane_session.filter(|_| lane_session_lives_here))
}

/// Maestro harness adapter.
///
/// Minimal V1: runs a single Maestro agent turn in the supplied workspace,
/// waits for completion, captures output, and returns a `HarnessOutput`.
/// Does NOT orchestrate, does not publish, does not access production credentials.
pub struct MaestroHarness {
    /// CLI binary to invoke (`maestro-cli` or explicit path from MAESTRO_BIN).
    pub cli_bin: String,
    /// Workspace directory where the turn executes.
    pub workspace: PathBuf,
    /// The resolved Maestro agent this turn runs on (`MAESTRO_AGENT`, policy agents, or default).
    ///
    /// This is what the turn bills through — not an OpenCode model id. Forge chooses the model *intent*;
    /// this field is the harness's translation of that intent into the vendor's own selection, and it is
    /// what the execution receipt prints.
    pub agent: String,
    /// Model to use for this turn: the agent selection, kept so the shared receipt (`ForgeHarness::model`)
    /// names what the turn runs on without knowing vendors.
    pub model: String,
    /// Environment variables for the subprocess (sanitized).
    pub env: Option<std::collections::HashMap<String, String>>,
    /// Story packet for context (assay commands, acceptance criteria, etc.).
    pub packet: crate::engine::packet::StoryPacket,
    /// Execution workspace with branch/base commit context.
    pub execution_workspace: Option<ExecutionWorkspace>,
    /// Story ID for vendor session persistence.
    pub story_id: Option<String>,
    /// The model-policy intent this harness was constructed for (`cheap` | `judgment`), for the receipt.
    pub model_policy: Option<String>,
    /// Spend cap in USD.
    pub spend_cap_usd: Option<f64>,
    /// Assay commands from the packet or dispatch.
    pub assay_commands: Vec<String>,
    /// Whether acceptance criteria are mapped to assay commands.
    pub acceptance_mapped: bool,
}

impl MaestroHarness {
    /// Create a Maestro harness from the run's context.
    ///
    /// This is the construction boundary §16 asks for: `forge.rs` builds the `HarnessContext` once per run
    /// and the adapter consumes it, instead of the adapter re-reading the same environment globals the
    /// binary already resolved. What stays environmental is vendor configuration itself (`MAESTRO_BIN`,
    /// `MAESTRO_AGENT` and friends) — resolved here, once, fail closed — because no context field names it.
    pub fn from_context(context: &crate::engine::harness::HarnessContext) -> Result<Self> {
        if !context.workspace.exists() {
            return Err(WorkflowError::generic(format!(
                "Maestro harness workspace does not exist: {}",
                context.workspace.display()
            )));
        }
        let selection = crate::engine::harness::ModelSelection::from_parts(
            context.model_policy.as_deref(),
            None,
        );
        let agent = resolve_maestro_agent(
            &selection,
            std::env::var(MAESTRO_AGENT_ENV).ok().as_deref(),
            std::env::var(MAESTRO_CHEAP_AGENT_ENV).ok().as_deref(),
            std::env::var(MAESTRO_JUDGMENT_AGENT_ENV).ok().as_deref(),
            std::env::var(MAESTRO_DEFAULT_AGENT_ENV).ok().as_deref(),
        )?;
        Ok(Self {
            cli_bin: Self::default_cli_bin()?,
            workspace: context.workspace.clone(),
            model: agent.clone(),
            agent,
            env: Some(sanitized_model_env()),
            model_policy: context.model_policy.clone(),
            spend_cap_usd: context.spend_cap_usd,
            assay_commands: context.assay_commands.clone(),
            acceptance_mapped: context.acceptance_mapped,
            packet: context.packet.clone(),
            execution_workspace: context.execution_workspace.clone(),
            story_id: Some(context.story_id.clone()),
        })
    }

    /// Create a Maestro harness from environment variables.
    ///
    /// The agent comes from Forge's vendor-neutral intent, never from the OpenCode resolver: `MAESTRO_AGENT`
    /// wins, then the turn's policy tier, then the configured default — anything else fails closed here,
    /// before a claim is opened and before a token is spent.
    pub fn from_env_for_policy(model_policy: Option<&str>) -> Result<Self> {
        let mut harness = Self::from_env()?;
        let selection = crate::engine::harness::ModelSelection::from_parts(model_policy, None);
        harness.agent = resolve_maestro_agent(
            &selection,
            std::env::var(MAESTRO_AGENT_ENV).ok().as_deref(),
            std::env::var(MAESTRO_CHEAP_AGENT_ENV).ok().as_deref(),
            std::env::var(MAESTRO_JUDGMENT_AGENT_ENV).ok().as_deref(),
            std::env::var(MAESTRO_DEFAULT_AGENT_ENV).ok().as_deref(),
        )?;
        harness.model = harness.agent.clone();
        harness.model_policy = model_policy.map(str::to_string);
        Ok(harness)
    }

    /// Create a Maestro harness from environment variables (no model policy).
    pub fn from_env() -> Result<Self> {
        let cli_bin = MaestroHarness::default_cli_bin()?;
        let workspace = std::env::var("FORGE_WORKTREE")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        if !workspace.exists() {
            return Err(WorkflowError::generic(format!(
                "Maestro harness workspace does not exist: {}",
                workspace.display()
            )));
        }

        // No agent here: `from_env` builds the transport without a turn's intent, and guessing one would
        // spend the next turn on it. `from_env_for_policy` (and `from_context`) resolve the agent, fail
        // closed when none is configured.
        let agent = String::new();
        let env = Some(sanitized_model_env());

        Ok(Self {
            cli_bin,
            workspace,
            agent: agent.clone(),
            model: agent,
            env,
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
                    Some(crate::engine::packet::ExecutionWorkspace {
                        worktree_path: path,
                        branch_name: branch,
                        base_ref,
                        base_commit,
                    })
                }
                _ => None,
            },
            story_id: std::env::var("FORGE_STORY_ID").ok(),
            model_policy: None,
        })
    }

    /// Resolve the Maestro CLI binary to use.
    pub fn default_cli_bin() -> Result<String> {
        // An explicit, non-empty MAESTRO_BIN wins outright; otherwise maestro-cli from PATH. There is
        // deliberately no ~/.maestro/bin/maestro assumption — that path is not the Maestro interface.
        // Resolution is lenient (a name is not proof); the preflight verifies the contract before any turn.
        if let Some(explicit) = std::env::var(MAESTRO_BIN_ENV)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            return Ok(explicit);
        }
        Ok(MAESTRO_CLI_DEFAULT.to_string())
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

    fn task_text(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> String {
        crate::engine::packet::build_task_text_with_context(
            node_id,
            &task.task_id,
            &self.packet,
            self.execution_workspace.as_ref(),
            self_heal,
        )
    }
}

impl crate::engine::runner::CandidateProbe for MaestroHarness {
    fn git(&self, args: &[&str]) -> Option<String> {
        self.run_git(args)
    }

    fn declared_test_mode(&self) -> Option<&str> {
        self.packet.test_mode.as_deref()
    }
}

impl crate::engine::runner::ProductionProbe for MaestroHarness {
    fn production_url(&self) -> String {
        crate::engine::production_probe::production_base_url()
    }

    fn deployed_sha(&self) -> std::result::Result<String, String> {
        crate::engine::production_probe::fetch_deployed_sha(&self.production_url())
    }
}

impl RoleHarness for MaestroHarness {
    fn run_role(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> Result<HarnessOutput> {
        if !self.workspace.exists() {
            return Err(WorkflowError::generic(format!(
                "Maestro harness workspace does not exist: {}",
                self.workspace.display()
            )));
        }

        // Preflight verification: the Maestro CLI contract, not OpenCode's. A Maestro turn must never
        // depend on the direct OpenCode harness passing.
        maestro_preflight(&self.cli_bin)?;

        let cwd = self.workspace.to_string_lossy().to_string();
        let task_text = self.task_text(node_id, task, self_heal);
        let before_sha = self.run_git(&["rev-parse", "HEAD"]);

        // The transport agent: Forge's role text stays in the task; the configured agent executes it.
        // Resolved once at construction, fail closed — never per-node, never mutated per turn.
        if self.agent.trim().is_empty() {
            return Err(WorkflowError::generic(
                "Maestro harness has no agent: set MAESTRO_AGENT (or MAESTRO_CHEAP_AGENT / \
                 MAESTRO_JUDGMENT_AGENT for the turn's policy, or MAESTRO_DEFAULT_AGENT).",
            ));
        }

        // Execution receipt, before the first model token is spent: what runs, where, as whom.
        eprintln!(
            "harness=maestro agent={} bin={} cwd={} model_policy={}",
            self.agent,
            self.cli_bin,
            cwd,
            self.model_policy.as_deref().unwrap_or("(default cheap)"),
        );

        // Session continuity (V2)
        let cwd = self.workspace.to_string_lossy().to_string();
        let lane = VENDOR_SESSION_LANE;

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
        // The lane session is only resumed when the vendor still lists it: a row keyed (story, lane)
        // knows nothing about directories, so membership is checked against `maestro-cli session list`
        // before an id from a deleted worktree is re-sent. Fail-closed toward fresh: when the vendor
        // cannot be asked, the turn opens a new session and re-reads the packet.
        let lane_session_lives_here = workspace_session.is_none()
            && lane_session
                .as_deref()
                .is_some_and(|id| maestro_session_lists(&self.cli_bin, &self.agent, id));
        let session = resume_session(workspace_session, lane_session, lane_session_lives_here);

        // The send contract, exactly as the vendor documents it (`maestro-cli send --help`): `send`
        // <agent> <message>, plus `--session` when resuming. No --model/--auto/--continue/--agent — those
        // are OpenCode flags the Maestro CLI does not accept, and inventing them fails the turn.
        let args = build_maestro_send_args(&self.agent, &task_text, session.as_deref());

        // Turn ceiling (wall-clock timeout)
        let max_turn = turn_ceiling(std::env::var(TURN_CEILING_ENV).ok().as_deref());

        // Execute Maestro with spend cap enforcement
        let spend_cap = self.spend_cap_usd;
        let result = run_maestro_streaming(
            &self.cli_bin,
            &cwd,
            &args,
            self.env.as_ref(),
            spend_cap,
            max_turn,
        )?;

        if result.exit_code != Some(0) {
            return Err(WorkflowError::generic(format!(
                "maestro-harness failed for {node_id}: exit={:?}, stderr={}",
                result.exit_code, result.stderr
            )));
        }

        // Parse output - Maestro returns structured output
        let parsed = parse_maestro_output(&result.stdout)?;

        // Persist session (V2): write workspace marker and vendor session
        if session_continuity_enabled() {
            if let Some(ref story) = self.story_id {
                let _ = vendor_session::write_vendor_session_id(
                    story,
                    lane,
                    parsed.session_id.as_deref().or(session.as_deref()),
                );
            }
            if let Some(ref id) = parsed.session_id {
                write_session_id(&cwd, Some(id));
            }
        }

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
            raw: parsed.assistant_text,
            candidate_sha,
            assay_commands: assay,
            acceptance_mapped: self.acceptance_mapped,
            refusal: None,
            execution_base,
            usage: parsed.usage,
        })
    }

    fn interrupt_execution(
        &self,
        _reason: &str,
    ) -> Result<Option<crate::engine::harness::TurnTermination>> {
        // V1: no process interruption support
        Ok(None)
    }

    fn exists_on_base_ref(&self, base_ref: &str, path: &str) -> bool {
        self.run_git(&["cat-file", "-e", &format!("{base_ref}:{path}")])
            .is_some()
            || self
                .run_git(&["ls-tree", "--name-only", base_ref, path])
                .map(|s| !s.is_empty())
                .unwrap_or(false)
    }

    fn assay_cwd(&self) -> &std::path::Path {
        if let Some(ws) = &self.execution_workspace {
            return std::path::Path::new(&ws.worktree_path);
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

/// Environment variable naming the Maestro agent this turn runs on. Wins outright: when the operator
/// names an agent, no policy translates it.
pub const MAESTRO_AGENT_ENV: &str = "MAESTRO_AGENT";
/// Policy-specific Maestro agents: the cheap and judgment tiers resolve here when `MAESTRO_AGENT` is unset.
pub const MAESTRO_CHEAP_AGENT_ENV: &str = "MAESTRO_CHEAP_AGENT";
pub const MAESTRO_JUDGMENT_AGENT_ENV: &str = "MAESTRO_JUDGMENT_AGENT";
/// Configured default Maestro agent: the last resort before failing closed.
pub const MAESTRO_DEFAULT_AGENT_ENV: &str = "MAESTRO_DEFAULT_AGENT";

/// Resolve the Maestro agent for one turn — a PURE function of its inputs, so the precedence is testable
/// without mutating the process environment.
///
/// Precedence, and the reason: an explicitly named agent is an attended decision and wins; otherwise the
/// policy names its tier; otherwise the configured default; otherwise FAIL CLOSED before model execution.
/// There is deliberately no silent fallback to an OpenCode model and no arbitrary pick: spending tokens on a
/// vendor nobody chose is the failure this order exists to remove.
///
/// `explicit` is `MAESTRO_AGENT`; `selection` is Forge's vendor-neutral intent. An `Explicit` selection
/// carries a vendor selection directly (a Maestro agent id on this side, an OpenCode model id on the other)
/// and never passes through the OpenCode resolver.
pub fn resolve_maestro_agent(
    selection: &crate::engine::harness::ModelSelection,
    explicit: Option<&str>,
    cheap: Option<&str>,
    judgment: Option<&str>,
    default: Option<&str>,
) -> Result<String> {
    use crate::engine::harness::ModelSelection;
    let clean = |value: Option<&str>| {
        value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    if let Some(agent) = clean(explicit) {
        return Ok(agent);
    }
    if let ModelSelection::Explicit(agent) = selection {
        let agent = agent.trim();
        if !agent.is_empty() {
            return Ok(agent.to_string());
        }
    }
    let tiered = match selection {
        ModelSelection::Judgment => clean(judgment),
        ModelSelection::Cheap => clean(cheap),
        ModelSelection::Explicit(_) => None,
    };
    if let Some(agent) = tiered.or_else(|| clean(default)) {
        return Ok(agent);
    }
    Err(WorkflowError::generic(
        "Maestro harness has no agent: set MAESTRO_AGENT (or MAESTRO_CHEAP_AGENT / MAESTRO_JUDGMENT_AGENT \
         for the turn's policy, or MAESTRO_DEFAULT_AGENT). Refusing to guess a vendor.",
    ))
}

/// Sanitize environment for the Maestro subprocess: same security boundary as OpenCode (no production
/// database authority crosses into a model turn, no git publication out of one), without any OpenCode
/// model configuration — a Maestro agent owns its provider, model and working directory itself.
fn sanitized_model_env() -> std::collections::HashMap<String, String> {
    crate::engine::opencode::sanitized_model_env()
}

/// Build the Maestro `send` invocation — the vendor's headless execution contract.
///
/// `maestro-cli send <agent-id> <message>`, plus exactly one `--session <id>` when resuming. Measured
/// against `maestro-cli send --help`: there is no `--model`, `--auto`, `--continue` or `--agent` on this
/// command, and passing OpenCode flags would fail the turn against the real CLI.
fn build_maestro_send_args(agent: &str, task: &str, session: Option<&str>) -> Vec<String> {
    let mut args = vec!["send".to_string(), agent.to_string(), task.to_string()];
    if let Some(id) = session.map(str::trim).filter(|id| !id.is_empty()) {
        args.push("--session".to_string());
        args.push(id.to_string());
    }
    args
}

/// Whether the vendor still lists `session_id` among its open sessions (`maestro-cli session list --json`).
///
/// Read-only and pre-token: no turn runs, no tab opens. `false` on any failure — an unaskable vendor must
/// yield a fresh turn, never a resumed stranger.
fn maestro_session_lists(cli_bin: &str, agent: &str, session_id: &str) -> bool {
    let output = std::process::Command::new(cli_bin)
        .args(["session", "list", "--json"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();
    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    json.get("sessions")
        .and_then(|sessions| sessions.as_array())
        .is_some_and(|sessions| {
            sessions.iter().any(|entry| {
                entry.get("sessionId").and_then(|id| id.as_str()) == Some(session_id)
                    && entry
                        .get("agentId")
                        .and_then(|id| id.as_str())
                        .is_some_and(|id| {
                            id == agent
                                || entry.get("agentName").and_then(|name| name.as_str())
                                    == Some(agent)
                        })
            })
        })
}

/// Run Maestro streaming with live output capture, spend cap enforcement, and turn timeout.
fn run_maestro_streaming(
    cli_bin: &str,
    cwd: &str,
    args: &[String],
    env: Option<&std::collections::HashMap<String, String>>,
    spend_cap: Option<f64>,
    max_turn: Option<Duration>,
) -> Result<MaestroRunResult> {
    let mut cmd = Command::new(cli_bin);
    cmd.current_dir(cwd);
    cmd.args(args);
    if let Some(env) = env {
        cmd.envs(env);
    }
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.stdin(std::process::Stdio::null());

    let mut child = cmd
        .spawn()
        .map_err(|e| WorkflowError::generic(format!("failed to spawn maestro: {e}")))?;

    let mut stdout = String::new();
    let mut stderr = String::new();

    // Spawn stderr reader in a separate thread
    let stderr_pipe = child.stderr.take().unwrap();
    let stderr_handle = std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        let reader = BufReader::new(stderr_pipe);
        let mut stderr = String::new();
        for line in reader.lines() {
            if let Ok(l) = line {
                stderr.push_str(&l);
                stderr.push('\n');
            }
        }
        stderr
    });

    // Read stdout with spend cap enforcement and timeout.
    //
    // SPEND-CAP LIMITATION, stated not hidden: `maestro-cli send` returns authoritative usage AFTER
    // completion, so a mid-turn streaming kill from vendor numbers is best-effort only — a turn whose cost
    // arrives solely in the final envelope cannot be stopped halfway for going over. The wall-clock ceiling
    // below is the enforcement that always holds; the cap kills early only when a streamed usage line
    // already proves it breached.
    let stdout_pipe = child.stdout.take().unwrap();
    let mut reader = std::io::BufReader::new(stdout_pipe);
    let mut line = String::new();
    let start_time = std::time::Instant::now();

    loop {
        // Check turn timeout
        if let Some(ceiling) = max_turn {
            if start_time.elapsed() >= ceiling {
                let _ = child.kill();
                let _ = child.wait();
                return Err(WorkflowError::generic(format!(
                    "maestro turn exceeded wall-clock ceiling of {:?}",
                    ceiling
                )));
            }
        }

        // Read line with timeout to allow periodic ceiling checks
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {
                stdout.push_str(&line);
                // Check spend cap from parsed usage in line (if Maestro emits usage in streaming output)
                if let Some(cap) = spend_cap {
                    if let Some(usage) = parse_usage_from_line(&line) {
                        if usage.cost_usd > cap {
                            let _ = child.kill();
                            let _ = child.wait();
                            return Err(WorkflowError::generic(format!(
                                "maestro turn exceeded spend cap: ${:.6} > ${:.6}",
                                usage.cost_usd, cap
                            )));
                        }
                    }
                }
            }
            Err(e) => {
                return Err(WorkflowError::generic(format!(
                    "failed to read maestro stdout: {e}"
                )));
            }
        }
    }

    // Wait for process to finish
    let status = child
        .wait()
        .map_err(|e| WorkflowError::generic(e.to_string()))?;
    let stderr = stderr_handle.join().unwrap_or_default();

    Ok(MaestroRunResult {
        exit_code: status.code(),
        stdout,
        stderr,
    })
}

/// Parse usage from a single Maestro streaming output line (if available).
/// Returns None if the line doesn't contain usage info.
fn parse_usage_from_line(line: &str) -> Option<HarnessUsage> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
        if let Some(usage_obj) = json.get("usage").or_else(|| json.get("usage_info")) {
            if let (Some(input), Some(output), Some(cost)) = (
                usage_obj
                    .get("input_tokens")
                    .or_else(|| usage_obj.get("tokens_input"))
                    .and_then(|v| v.as_i64()),
                usage_obj
                    .get("output_tokens")
                    .or_else(|| usage_obj.get("tokens_output"))
                    .and_then(|v| v.as_i64()),
                usage_obj
                    .get("cost_usd")
                    .or_else(|| usage_obj.get("cost"))
                    .and_then(|v| v.as_f64()),
            ) {
                return Some(HarnessUsage {
                    session_id: String::new(), // Will be filled by parse_maestro_output
                    tokens_input: input,
                    tokens_output: output,
                    cost_usd: cost,
                });
            }
        }
    }
    None
}

/// Maestro run result.
#[derive(Debug, Clone)]
struct MaestroRunResult {
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// Parsed Maestro output with structured fields.
#[derive(Debug, Clone, Default)]
struct ParsedMaestroOutput {
    assistant_text: String,
    session_id: Option<String>,
    usage: Option<HarnessUsage>,
}

/// Parse the `maestro-cli send` JSON response into Forge's turn output.
///
/// The vendor envelope (`maestro-cli send` prints one JSON response):
/// `response` → the turn text, `sessionId` → vendor-session persistence, `usage.*` → spend accounting, and
/// `success: false` → a harness execution failure carrying Maestro's error. A failed response is never
/// reinterpreted as successful plain text, and an unparseable response is a malformed-response failure —
/// never an empty success.
fn parse_maestro_output(stdout: &str) -> Result<ParsedMaestroOutput> {
    // The vendor prints one JSON object; incidental lines around it are ignored by taking the LAST line
    // that parses as an object carrying the envelope. Nothing JSON-shaped at all is malformed output.
    let mut envelope: Option<serde_json::Value> = None;
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(stdout.trim()) {
        if json.is_object() {
            envelope = Some(json);
        }
    }
    if envelope.is_none() {
        for line in stdout.lines().rev() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                if json.is_object()
                    && (json.get("success").is_some()
                        || json.get("response").is_some()
                        || json.get("sessionId").is_some())
                {
                    envelope = Some(json);
                    break;
                }
            }
        }
    }
    let Some(json) = envelope else {
        return Err(WorkflowError::generic(format!(
            "maestro-harness: malformed response (no JSON envelope in {} bytes of stdout)",
            stdout.len()
        )));
    };

    if json.get("success").and_then(|value| value.as_bool()) == Some(false) {
        let error = json
            .get("error")
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| value.to_string())
            })
            .filter(|text| !text.trim().is_empty() && text != "null")
            .unwrap_or_else(|| "unknown Maestro failure".to_string());
        return Err(WorkflowError::generic(format!(
            "maestro-harness: turn failed: {error}"
        )));
    }

    let mut result = ParsedMaestroOutput::default();
    // Maestro's field is `sessionId`. `session_id`/`sessionID` stay as tolerated aliases; nothing else does.
    if let Some(id) = json
        .get("sessionId")
        .or_else(|| json.get("session_id"))
        .or_else(|| json.get("sessionID"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        result.session_id = Some(id.to_string());
    }
    result.assistant_text = json
        .get("response")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .unwrap_or_default();
    if let Some(usage_obj) = json.get("usage") {
        // Cost is authoritative-or-nothing: without `totalCostUsd` the turn's spend is unmeasured and
        // `usage` stays `None` (the field's documented meaning — "unmeasured, never zero"). Recording
        // tokens beside a fake $0 would let a spend cap believe an expensive turn was free.
        if let Some(cost) = usage_obj
            .get("totalCostUsd")
            .and_then(|value| value.as_f64())
        {
            result.usage = Some(HarnessUsage {
                session_id: result.session_id.clone().unwrap_or_default(),
                tokens_input: usage_obj
                    .get("inputTokens")
                    .and_then(|value| value.as_i64())
                    .unwrap_or(0),
                tokens_output: usage_obj
                    .get("outputTokens")
                    .and_then(|value| value.as_i64())
                    .unwrap_or(0),
                cost_usd: cost,
            });
        }
    }
    if result.assistant_text.trim().is_empty() {
        return Err(WorkflowError::generic(
            "maestro-harness: the response envelope carries no response text",
        ));
    }
    Ok(result)
}

/// Verify the Maestro CLI contract: the binary spawns and answers `--version`.
///
/// This is the Maestro half of vendor preflight — run when (and only when) the Maestro backend is selected,
/// so a Maestro turn never depends on the OpenCode harness passing. A Maestro turn that cannot prove its
/// transport never starts: the failure names the binary, not the story.
///
/// `Ok` carries the version line the lane logs; `Err` is the diagnostic. Same shape as OpenCode's
/// `verify_vendor_contract`, so the binary can verify either backend through one call shape.
pub fn verify_vendor_contract(cli_bin: &str) -> std::result::Result<String, String> {
    let mut cmd = Command::new(cli_bin);
    cmd.arg("--version");
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let output = cmd.output().map_err(|error| {
        format!(
            "maestro preflight failed to spawn `{cli_bin}`: {error}. Check MAESTRO_BIN and PATH."
        )
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "maestro preflight failed (exit={}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(format!("{version} @ {cli_bin}"))
}

fn maestro_preflight(cli_bin: &str) -> Result<()> {
    verify_vendor_contract(cli_bin)
        .map(|vendor| eprintln!("maestro preflight: version={vendor}"))
        .map_err(WorkflowError::generic)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Environment-mutating tests serialize here: Rust runs tests on threads sharing one process
    /// environment, so two tests setting `FORGE_HARNESS` at once would read each other's values.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    use crate::engine::harness::{HarnessBackend, ModelSelection};

    #[test]
    fn maestro_harness_construction() {
        let _guard = lock_env();
        std::env::set_var("FORGE_WORKTREE", ".");
        std::env::set_var("FORGE_STORY_ID", "TEST-STORY");
        let harness = MaestroHarness::from_env().expect("harness construction");
        assert_eq!(harness.cli_bin, "maestro-cli");
        assert!(harness.workspace.exists());
        assert!(
            harness.agent.is_empty(),
            "from_env builds transport without intent"
        );
    }

    #[test]
    fn backend_selection_opencode_maestro_fail_closed() {
        let _guard = lock_env();
        std::env::remove_var("FORGE_HARNESS");
        assert_eq!(
            HarnessBackend::from_env().expect("unset is opencode"),
            HarnessBackend::OpenCode
        );
        std::env::set_var("FORGE_HARNESS", "opencode");
        assert_eq!(
            HarnessBackend::from_env().expect("opencode is opencode"),
            HarnessBackend::OpenCode
        );
        std::env::set_var("FORGE_HARNESS", "maestro");
        assert_eq!(
            HarnessBackend::from_env().expect("maestro is maestro"),
            HarnessBackend::Maestro
        );
        // A misspelled vendor must fail configuration, never silently run OpenCode.
        std::env::set_var("FORGE_HARNESS", "maetsro");
        assert!(
            HarnessBackend::from_env().is_err(),
            "unknown FORGE_HARNESS must fail closed"
        );
        std::env::remove_var("FORGE_HARNESS");
    }

    #[test]
    fn model_intent_cheap_judgment_explicit() {
        assert_eq!(
            ModelSelection::from_parts(None, None),
            ModelSelection::Cheap
        );
        assert_eq!(
            ModelSelection::from_parts(Some("cheap"), None),
            ModelSelection::Cheap
        );
        assert_eq!(
            ModelSelection::from_parts(Some("judgment"), None),
            ModelSelection::Judgment
        );
        assert_eq!(
            ModelSelection::from_parts(Some("nonsense"), None),
            ModelSelection::Cheap,
            "an unknown policy reads as cheap, the tier that bills least"
        );
        assert_eq!(
            ModelSelection::from_parts(Some("judgment"), Some("agent-x")),
            ModelSelection::Explicit("agent-x".to_string()),
            "an explicit override wins over the policy"
        );
        assert_eq!(
            ModelSelection::from_parts(None, Some("  ")),
            ModelSelection::Cheap,
            "a blank override is no override"
        );
    }

    #[test]
    fn maestro_agent_explicit_wins() {
        let judgment = ModelSelection::Judgment;
        assert_eq!(
            resolve_maestro_agent(
                &judgment,
                Some("forge-muse"),
                Some("cheap-a"),
                Some("judge-a"),
                Some("def-a")
            )
            .expect("explicit wins"),
            "forge-muse"
        );
    }

    #[test]
    fn maestro_agent_tier_routing() {
        assert_eq!(
            resolve_maestro_agent(
                &ModelSelection::Judgment,
                None,
                Some("cheap-a"),
                Some("judge-a"),
                Some("def-a")
            )
            .expect("judgment tier"),
            "judge-a",
            "MAESTRO_JUDGMENT_AGENT serves judgment"
        );
        assert_eq!(
            resolve_maestro_agent(
                &ModelSelection::Cheap,
                None,
                Some("cheap-a"),
                Some("judge-a"),
                Some("def-a")
            )
            .expect("cheap tier"),
            "cheap-a",
            "MAESTRO_CHEAP_AGENT serves cheap"
        );
        assert_eq!(
            resolve_maestro_agent(&ModelSelection::Cheap, None, None, None, Some("def-a"))
                .expect("default"),
            "def-a"
        );
    }

    #[test]
    fn maestro_agent_missing_fails_closed() {
        assert!(
            resolve_maestro_agent(&ModelSelection::Cheap, None, None, None, None).is_err(),
            "no agent anywhere must fail before model execution"
        );
        assert!(
            resolve_maestro_agent(&ModelSelection::Judgment, Some("  "), None, None, None).is_err(),
            "a blank MAESTRO_AGENT is no agent"
        );
    }

    #[test]
    fn maestro_send_args_contract() {
        // No session: exactly `send <agent> <message>`, and none of OpenCode's flags.
        let args = build_maestro_send_args("forge-nemotron", "hello", None);
        assert_eq!(args, vec!["send", "forge-nemotron", "hello"]);
        for forbidden in ["--model", "--auto", "--continue", "--agent", "run"] {
            assert!(
                !args.iter().any(|arg| arg == forbidden),
                "the send contract must not carry {forbidden}"
            );
        }
        // Resuming: exactly one `--session` with the id, appended once.
        let args = build_maestro_send_args("forge-nemotron", "hello", Some("abc123"));
        assert_eq!(
            args,
            vec!["send", "forge-nemotron", "hello", "--session", "abc123"]
        );
        assert_eq!(
            args.iter()
                .filter(|arg| arg.as_str() == "--session")
                .count(),
            1,
            "the session argument appears exactly once"
        );
        // Blank session is no session.
        let args = build_maestro_send_args("forge-nemotron", "hello", Some("  "));
        assert_eq!(args.len(), 3);
    }

    #[test]
    fn maestro_response_envelope_parsing() {
        let stdout = r#"{"agentId":"x","agentName":"forge-nemotron","sessionId":"session-123","response":"done","success":true,"error":null,"usage":{"inputTokens":100,"outputTokens":25,"cacheReadInputTokens":0,"cacheCreationInputTokens":0,"totalCostUsd":0.012,"contextWindow":131072,"contextUsagePercent":1}}"#;
        let parsed = parse_maestro_output(stdout).expect("the documented envelope parses");
        assert_eq!(parsed.assistant_text, "done");
        assert_eq!(parsed.session_id.as_deref(), Some("session-123"));
        let usage = parsed
            .usage
            .expect("usage is recorded when cost is authoritative");
        assert_eq!(usage.tokens_input, 100);
        assert_eq!(usage.tokens_output, 25);
        assert!((usage.cost_usd - 0.012).abs() < 1e-12);
    }

    #[test]
    fn maestro_response_failure_is_failure() {
        let stdout = r#"{"agentId":"x","sessionId":"s-1","response":"","success":false,"error":"agent is busy"}"#;
        let error = parse_maestro_output(stdout).expect_err("success:false must fail");
        assert!(error.to_string().contains("agent is busy"), "{error}");
    }

    #[test]
    fn maestro_response_malformed_is_failure() {
        assert!(
            parse_maestro_output("not json at all\n").is_err(),
            "garbage must never read as a successful turn"
        );
        let no_text = r#"{"success":true,"sessionId":"s-1"}"#;
        assert!(
            parse_maestro_output(no_text).is_err(),
            "an envelope with no response text is not a turn"
        );
    }

    #[test]
    fn maestro_response_usage_without_cost_is_unmeasured() {
        // Tokens without an authoritative cost: usage stays None (unmeasured), never a fake $0.
        let stdout = r#"{"success":true,"sessionId":"s-1","response":"done","usage":{"inputTokens":10,"outputTokens":5}}"#;
        let parsed = parse_maestro_output(stdout).expect("parses");
        assert!(parsed.usage.is_none(), "costless usage must not become $0");
    }

    #[test]
    fn maestro_preflight_pass_and_fail() {
        // `/usr/bin/true` answers --version with exit 0; `/usr/bin/false` refuses; a missing binary
        // never spawns. No network, no tokens, no agent touched.
        assert!(verify_vendor_contract("/usr/bin/true").is_ok());
        assert!(verify_vendor_contract("/usr/bin/false").is_err());
        assert!(verify_vendor_contract("/nonexistent/maestro-cli-xyz").is_err());
    }

    #[test]
    fn maestro_session_resume_precedence() {
        // The pure rule, shared with OpenCode's: marker wins, then a lane session proven local, else fresh.
        assert_eq!(
            resume_session(Some("marker".into()), Some("lane".into()), true),
            Some("marker".to_string())
        );
        assert_eq!(
            resume_session(None, Some("lane".into()), true),
            Some("lane".to_string())
        );
        assert_eq!(resume_session(None, Some("lane".into()), false), None);
        assert_eq!(resume_session(None, None, false), None);
    }

    /// Opt-in integration: the real `maestro-cli` against a cheap test agent. NEVER runs by default —
    /// it spends model tokens. Enable with `FORGE_MAESTRO_INTEGRATION=1` and name the agent explicitly:
    /// `MAESTRO_TEST_AGENT=<cheap test agent>`.
    #[test]
    #[ignore = "spends model tokens: FORGE_MAESTRO_INTEGRATION=1 with MAESTRO_TEST_AGENT set"]
    fn maestro_live_send_integration() {
        if std::env::var("FORGE_MAESTRO_INTEGRATION").ok().as_deref() != Some("1") {
            eprintln!("skipped: FORGE_MAESTRO_INTEGRATION != 1");
            return;
        }
        let agent = std::env::var("MAESTRO_TEST_AGENT").expect(
            "FORGE_MAESTRO_INTEGRATION=1 requires MAESTRO_TEST_AGENT naming a cheap test agent",
        );
        let bin = MaestroHarness::default_cli_bin().expect("maestro-cli resolves");
        let args = build_maestro_send_args(&agent, "Reply with exactly: integration-ok", None);
        let output = std::process::Command::new(&bin)
            .args(&args)
            .stdin(std::process::Stdio::null())
            .output()
            .expect("maestro-cli runs");
        assert!(
            output.status.success(),
            "send failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let parsed = parse_maestro_output(&String::from_utf8_lossy(&output.stdout))
            .expect("the live response parses");
        assert!(
            !parsed.assistant_text.trim().is_empty(),
            "the live turn answered"
        );
        assert!(
            parsed.session_id.is_some(),
            "the live turn minted a session"
        );
    }

    #[test]
    fn maestro_cli_default_is_maestro_cli() {
        let _guard = lock_env();
        std::env::remove_var("MAESTRO_BIN");
        assert_eq!(
            MaestroHarness::default_cli_bin().expect("default resolves"),
            "maestro-cli"
        );
    }

    #[test]
    fn maestro_bin_explicit_override_is_selected() {
        let _guard = lock_env();
        std::env::set_var("MAESTRO_BIN", "/usr/local/bin/maestro-cli");
        assert_eq!(
            MaestroHarness::default_cli_bin().expect("explicit wins"),
            "/usr/local/bin/maestro-cli",
            "a valid explicit MAESTRO_BIN is used as configured"
        );
        std::env::remove_var("MAESTRO_BIN");
    }

    #[test]
    fn maestro_bin_invalid_explicit_fails_closed() {
        let _guard = lock_env();
        std::env::set_var("MAESTRO_BIN", "/nonexistent/maestro-cli-xyz");
        // Resolution stays lenient (a name is not proof), but the bad explicit value is never replaced
        // by a fallback and is refused at preflight — before a claim, before a token.
        let bin = MaestroHarness::default_cli_bin().expect("no silent substitution");
        assert_eq!(bin, "/nonexistent/maestro-cli-xyz");
        assert!(
            verify_vendor_contract(&bin).is_err(),
            "an invalid explicit MAESTRO_BIN must fail closed"
        );
        std::env::remove_var("MAESTRO_BIN");
    }

    #[test]
    fn maestro_backend_never_runs_direct_opencode_preflight() {
        // Structural guard on the binary: backend selection must precede vendor verification, and the
        // OpenCode contract check must sit behind the OpenCode backend arm — selecting Maestro must not
        // require the direct Forge → OpenCode harness preflight to pass.
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/bin/forge.rs")
                .as_path(),
        )
        .expect("forge.rs is readable");
        let selection = source
            .find("HarnessBackend::from_env()")
            .expect("backend is selected");
        let opencode_verify = source
            .find("opencode_client::verify_vendor_contract")
            .expect("opencode verifies its own contract");
        let maestro_verify = source
            .find("maestro::verify_vendor_contract")
            .expect("maestro verifies its own contract");
        assert!(
            selection < opencode_verify && selection < maestro_verify,
            "the vendor must be resolved (and invalid config rejected) before any preflight runs"
        );
        let opencode_arm = source
            .rfind("HarnessBackend::OpenCode =>")
            .expect("opencode arm exists");
        assert!(
            opencode_arm < opencode_verify,
            "the OpenCode preflight lives inside the OpenCode backend arm only"
        );
    }

    #[test]
    fn maestro_target_selection_never_calls_opencode_resolver() {
        // Maestro must translate Forge's vendor-neutral intent on its own: no OpenCode model-resolution
        // helper (or OpenCode agent map) may appear in Maestro's selection path. comments are allowed
        // to NAME the rule, so scan code with `//` line comments stripped.
        let source = include_str!("maestro.rs");
        let code: String = source
            .lines()
            .map(|line| match line.split_once("//") {
                Some((code, _)) => code,
                None => line,
            })
            .collect::<Vec<_>>()
            .join("\n");
        // The guard below names the forbidden symbols; exclude the guard itself from the scan.
        let code = code
            .split("fn maestro_target_selection_never_calls_opencode_resolver")
            .next()
            .unwrap_or(&code);
        for forbidden in [
            "resolve_model_for_policy",
            "resolve_opencode_model",
            "v2_agent_env",
            "opencode_agents::v2_agent_for_node",
        ] {
            assert!(
                !code.contains(forbidden),
                "Maestro target selection must not use OpenCode's {forbidden}"
            );
        }
    }

    #[test]
    fn maestro_turn_ceiling_default_and_overrides() {
        // FORGE_TURN_TIMEOUT_MINUTES preserved: unset/blank/garbage → 120; "0"/"off" disables.
        assert_eq!(turn_ceiling(None), Some(Duration::from_secs(120 * 60)));
        assert_eq!(turn_ceiling(Some("")), Some(Duration::from_secs(120 * 60)));
        assert_eq!(
            turn_ceiling(Some("garbage")),
            Some(Duration::from_secs(120 * 60))
        );
        assert_eq!(turn_ceiling(Some("45")), Some(Duration::from_secs(45 * 60)));
        assert_eq!(turn_ceiling(Some("0")), None);
        assert_eq!(turn_ceiling(Some("off")), None);
    }

    #[test]
    fn maestro_session_never_crosses_worktrees() {
        // A lane session proven to belong to a different workspace is refused; resuming falls through to fresh.
        assert_eq!(
            resume_session(None, Some("other-worktree-session".into()), false),
            None,
            "a session from another worktree must never be resumed here"
        );
    }

    #[test]
    fn maestro_task_text_preserves_forge_context() {
        // The task text Maestro receives is Forge's canonical builder output: role node, goal, brief,
        // acceptance, and execution workspace context survive the adapter unchanged.
        let _guard = lock_env();
        std::env::set_var("FORGE_WORKTREE", ".");
        std::env::set_var("FORGE_STORY_ID", "T-STORY");
        let mut harness = MaestroHarness::from_env().expect("harness");
        harness.packet.goal = Some("GOAL-TEXT".into());
        harness.packet.architect_brief = Some("BRIEF-TEXT".into());
        harness.packet.acceptance_criteria = Some("ACCEPT-TEXT".into());
        let task = crate::engine::runtime::ActiveForgeRoleTask {
            task_id: "task-1".into(),
            process_instance_id: "proc-1".into(),
            story_id: "T-STORY".into(),
            token_id: None,
            node_id: Some("architect".into()),
            status: workflow::TaskStatus::InProgress,
            assignee: None,
            candidates: vec![],
        };
        let text = harness.task_text("architect", &task, None);
        assert!(text.contains("GOAL-TEXT"), "{text}");
        assert!(text.contains("BRIEF-TEXT"), "{text}");
        assert!(text.contains("ACCEPT-TEXT"), "{text}");
        assert!(text.contains("architect"), "{text}");
    }

    #[test]
    fn maestro_agent_selection_ignores_forge_role_ids() {
        // Forge roles (architect/lead/smith/qa/scout/devops) are owned by Forge's lifecycle; the Maestro
        // agent is a transport choice. The resolver's signature must not accept a role/node at all, so no
        // role can silently steer agent selection.
        let source = include_str!("maestro.rs");
        let start = source
            .find("pub fn resolve_maestro_agent")
            .expect("resolver exists");
        let end = source[start..]
            .find(") -> Result<String>")
            .map(|offset| start + offset)
            .expect("signature closes");
        let signature = &source[start..end];
        for forbidden in ["node", "role", "architect", "smith", "scout", "devops"] {
            assert!(
                !signature.contains(forbidden),
                "role id {forbidden} must not steer Maestro agent selection"
            );
        }
    }
}
