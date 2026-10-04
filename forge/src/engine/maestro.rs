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
use crate::engine::harness::{HarnessUsage, TurnTermination};
use crate::engine::opencode_agents;
use crate::engine::packet::{ExecutionWorkspace, StoryPacket};
use crate::engine::runner::{HarnessOutput, RoleHarness};
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::engine::spend_cap::{self, BUDGET_EXHAUSTED_CODE, SPEND_CAP_ENV};
use crate::engine::vendor_session;
use workflow::{Result, WorkflowError};

/// Maestro CLI binary name (can be overridden with MAESTRO_BIN env var).
pub const MAESTRO_CLI_DEFAULT: &str = "maestro";

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
            return None
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
    /// CLI binary to invoke (`maestro` or explicit path from MAESTRO_BIN).
    pub cli_bin: String,
    /// Workspace directory where the turn executes.
    pub workspace: PathBuf,
    /// Model to use for this turn (from Forge's model policy).
    pub model: String,
    /// Environment variables for the subprocess (sanitized).
    pub env: Option<std::collections::HashMap<String, String>>,
    /// Whether to auto-approve (Maestro equivalent of --auto).
    pub auto_approve: bool,
    /// Story packet for context (assay commands, acceptance criteria, etc.).
    pub packet: crate::engine::packet::StoryPacket,
    /// Execution workspace with branch/base commit context.
    pub execution_workspace: Option<ExecutionWorkspace>,
    /// Story ID for vendor session persistence.
    pub story_id: Option<String>,
    /// Spend cap in USD.
    pub spend_cap_usd: Option<f64>,
    /// Assay commands from the packet or dispatch.
    pub assay_commands: Vec<String>,
    /// Whether acceptance criteria are mapped to assay commands.
    pub acceptance_mapped: bool,
}

impl MaestroHarness {
    /// Create a Maestro harness from environment variables.
    pub fn from_env_for_policy(model_policy: Option<&str>) -> Result<Self> {
        let mut harness = Self::from_env()?;
        let override_model = std::env::var("MAESTRO_MODEL").ok();
        harness.model = match override_model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(model) => model.to_string(),
            None => resolve_model_for_policy(model_policy)?,
        };
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

        let model = resolve_opencode_model(None)?;
        let env = Some(sanitized_model_env());

        Ok(Self {
            cli_bin,
            workspace,
            model,
            env,
            auto_approve: true,
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
        })
    }

    /// Resolve the Maestro CLI binary to use.
    pub fn default_cli_bin() -> Result<String> {
        let explicit = std::env::var(MAESTRO_BIN_ENV).ok();
        let vendor_home = std::env::var("HOME")
            .ok()
            .map(|home| home.trim().to_string())
            .filter(|home| !home.is_empty())
            .map(|home| {
                std::path::PathBuf::from(home)
                    .join(".maestro/bin/maestro")
                    .to_string_lossy()
                    .to_string()
            })
            .filter(|path| is_executable_file(path));

        let candidates = if let Some(explicit) = explicit.map(|s| s.trim().to_string()).filter(|v| !v.is_empty()) {
            vec![explicit]
        } else {
            let mut c = Vec::new();
            if let Some(home) = vendor_home {
                c.push(home);
            }
            c.push(MAESTRO_CLI_DEFAULT.to_string());
            c
        };

        for candidate in candidates {
            if is_executable_file(&candidate) {
                return Ok(candidate);
            }
        }
        // If no explicit binary found, return the default name; the run will fail with a clear error.
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

        // Preflight verification: ensure Maestro CLI is available
        maestro_preflight(&self.cli_bin)?;

        let cwd = self.workspace.to_string_lossy().to_string();
        let task_text = self.task_text(node_id, task, self_heal);
        let before_sha = self.run_git(&["rev-parse", "HEAD"]);

        // Resolve agent for this node (FAIL CLOSED)
        let agent = opencode_agents::v2_agent_for_node(node_id).map_err(WorkflowError::generic)?;

        eprintln!(
            "maestro-harness node={node_id} agent={agent}"
        );

        // Build Maestro environment
        let model_env = v2_agent_env(self.env.as_ref())?;

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

        // Build Maestro CLI arguments with session support
        let args = build_maestro_run_args(
            &self.model,
            &task_text,
            self.auto_approve,
            session.as_deref(),
            continue_session,
            Some(agent),
        )?;

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

    fn interrupt_execution(&self, _reason: &str) -> Result<Option<crate::engine::harness::TurnTermination>> {
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

/// Resolve Maestro model from policy (cheap | judgment).
fn resolve_model_for_policy(model_policy: Option<&str>) -> Result<String> {
    // Explicit MAESTRO_MODEL wins, same as OPENCODE_MODEL in OpenCode harness.
    if let Ok(model) = std::env::var("MAESTRO_MODEL") {
        let model = model.trim().to_string();
        if !model.is_empty() {
            return Ok(model);
        }
    }
    let policy = match crate::engine::opencode::as_model_policy(model_policy) {
        "judgment" => "judgment",
        _ => "cheap",
    };
    // Support MAESTRO_JUDGMENT_MODEL for judgment policy (parallel to OPENCODE_JUDGMENT_MODEL)
    if policy == "judgment" {
        if let Ok(model) = std::env::var("MAESTRO_JUDGMENT_MODEL") {
            let model = model.trim().to_string();
            if !model.is_empty() {
                return Ok(model);
            }
        }
    }
    // Fall back to OpenCode's resolution (uses OPENCODE_PINNED_MODEL / OPENCODE_JUDGMENT_MODEL)
    Ok(crate::engine::opencode::resolve_model_for_policy(Some(policy)))
}

/// Resolve Maestro model (placeholder - same as OpenCode for now).
fn resolve_opencode_model(model: Option<&str>) -> Result<String> {
    crate::engine::opencode::resolve_opencode_model(model)
}

/// Sanitize environment for Maestro (same as OpenCode).
fn sanitized_model_env() -> std::collections::HashMap<String, String> {
    crate::engine::opencode::sanitized_model_env()
}

/// Build Maestro agent environment (placeholder - same as OpenCode for now).
fn v2_agent_env(base: Option<&std::collections::HashMap<String, String>>) -> Result<std::collections::HashMap<String, String>> {
    crate::engine::opencode::v2_agent_env(base)
}

/// Build Maestro CLI arguments.
fn build_maestro_run_args(
    model: &str,
    task: &str,
    auto_approve: bool,
    session: Option<&str>,
    continue_session: bool,
    agent: Option<&str>,
) -> Result<Vec<String>> {
    let mut args = vec![
        "run".into(),
        "--model".into(),
        model.into(),
    ];
    if auto_approve {
        args.push("--auto".into());
    }
    if let Some(id) = session {
        args.push("--session".into());
        args.push(id.into());
    }
    if continue_session {
        args.push("--continue".into());
    }
    if let Some(agent) = agent {
        args.push("--agent".into());
        args.push(agent.into());
    }
    args.push(task.into());
    Ok(args)
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

    let mut child = cmd.spawn().map_err(|e| WorkflowError::generic(format!("failed to spawn maestro: {e}")))?;

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

    // Read stdout with spend cap enforcement and timeout
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
                return Err(WorkflowError::generic(format!("failed to read maestro stdout: {e}")));
            }
        }
    }

    // Wait for process to finish
    let status = child.wait().map_err(|e| WorkflowError::generic(e.to_string()))?;
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
                usage_obj.get("input_tokens").or_else(|| usage_obj.get("tokens_input")).and_then(|v| v.as_i64()),
                usage_obj.get("output_tokens").or_else(|| usage_obj.get("tokens_output")).and_then(|v| v.as_i64()),
                usage_obj.get("cost_usd").or_else(|| usage_obj.get("cost")).and_then(|v| v.as_f64()),
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

/// Parse Maestro output into structured fields.
fn parse_maestro_output(stdout: &str) -> Result<ParsedMaestroOutput> {
    let mut result = ParsedMaestroOutput::default();

    // Try to parse as JSON lines (Maestro may emit NDJSON)
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Try to parse as JSON
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            // Extract session ID
            if result.session_id.is_none() {
                if let Some(id) = json.get("session_id").or_else(|| json.get("sessionID")).and_then(|v| v.as_str()) {
                    result.session_id = Some(id.to_string());
                }
            }

            // Extract assistant text from various possible fields
            if result.assistant_text.is_empty() {
                if let Some(text) = json.get("text").and_then(|v| v.as_str()) {
                    result.assistant_text = text.to_string();
                } else if let Some(msg) = json.get("message").and_then(|v| v.as_str()) {
                    result.assistant_text = msg.to_string();
                } else if let Some(content) = json.get("content").and_then(|v| v.as_str()) {
                    result.assistant_text = content.to_string();
                }
            }

            // Extract usage if present
            if result.usage.is_none() {
                if let Some(usage_obj) = json.get("usage").or_else(|| json.get("usage_info")) {
                    if let (Some(input), Some(output), Some(cost)) = (
                        usage_obj.get("input_tokens").or_else(|| usage_obj.get("tokens_input")).and_then(|v| v.as_i64()),
                        usage_obj.get("output_tokens").or_else(|| usage_obj.get("tokens_output")).and_then(|v| v.as_i64()),
                        usage_obj.get("cost_usd").or_else(|| usage_obj.get("cost")).and_then(|v| v.as_f64()),
                    ) {
                        result.usage = Some(HarnessUsage {
                            session_id: result.session_id.clone().unwrap_or_default(),
                            tokens_input: input,
                            tokens_output: output,
                            cost_usd: cost,
                        });
                    }
                }
            }
        } else {
            // Not JSON - treat as plain text assistant response
            if result.assistant_text.is_empty() {
                result.assistant_text = line.to_string();
            } else {
                result.assistant_text.push('\n');
                result.assistant_text.push_str(line);
            }
        }
    }

    // If no structured text found, use the entire stdout as the response
    if result.assistant_text.is_empty() && !stdout.trim().is_empty() {
        result.assistant_text = stdout.trim().to_string();
    }

    Ok(result)
}

/// Verify Maestro CLI is available and functional (preflight check).
fn maestro_preflight(cli_bin: &str) -> Result<()> {
    let mut cmd = Command::new(cli_bin);
    cmd.arg("--version");
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let output = cmd.output().map_err(|e| WorkflowError::generic(format!("maestro preflight failed to spawn: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WorkflowError::generic(format!(
            "maestro preflight failed (exit={}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        )));
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    eprintln!("maestro preflight: version={}", version);
    Ok(())
}

/// Check if a file is executable.
fn is_executable_file(path: &str) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn maestro_harness_construction() {
        // Test that the harness can be constructed from env
        std::env::set_var("FORGE_WORKTREE", ".");
        std::env::set_var("FORGE_STORY_ID", "TEST-STORY");
        let harness = MaestroHarness::from_env().expect("harness construction");
        assert_eq!(harness.cli_bin, "maestro");
        assert!(harness.workspace.exists());
    }

    #[test]
    fn maestro_model_resolution() {
        std::env::set_var("MAESTRO_MODEL", "test-model");
        let model = resolve_model_for_policy(Some("judgment")).unwrap();
        assert_eq!(model, "test-model");
    }
}