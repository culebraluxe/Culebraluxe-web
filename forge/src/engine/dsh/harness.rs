//! Harness half of the DSH adapter: one headless DeepSeek Harness turn in the Forge workspace.

use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::*;
use crate::engine::assay::CommandResult;
use crate::engine::config::turn_ceiling;
use crate::engine::packet::{ExecutionWorkspace, StoryPacket};
use crate::engine::runner::{HarnessOutput, RoleHarness};

/// A completed headless process.
pub(super) struct DshRunResult {
    pub(super) exit_code: Option<i32>,
    pub(super) stdout: String,
    pub(super) stderr: String,
}

/// The DSH execution harness: one headless turn per Forge role turn.
///
/// Serial by construction: headless cannot fork a session, so `fork_for_workspace` returns `None` and
/// Forge keeps this lane's work on one slot. The transport is the vendor's `headless` profile; the
/// model is pinned per turn through a `--patch` overlay when one is configured.
pub struct DshHarness {
    /// Resolved CLI command (`dsh` on PATH or `node <lib/bin.js>`).
    pub cli: args::DshCli,
    /// Workspace directory where the turn executes.
    pub workspace: PathBuf,
    /// Resolved model (exact `provider/model`) or `None` for the profile's own default.
    pub model: Option<String>,
    /// What the receipt prints for the model: the pin, or the profile default.
    pub model_name: String,
    /// Sanitized environment for the subprocess.
    pub env: Option<HashMap<String, String>>,
    /// Story packet for context.
    pub packet: StoryPacket,
    /// Execution workspace with branch/base commit context.
    pub execution_workspace: Option<ExecutionWorkspace>,
    /// Story ID for vendor-session evidence.
    pub story_id: Option<String>,
    /// The model-policy intent this harness was constructed for (`cheap` | `judgment`).
    pub model_policy: Option<String>,
    /// Spend cap in USD. DSH's headless path reports no usage, so this is carried for the receipt and
    /// the wall-clock ceiling is the enforced control.
    pub spend_cap_usd: Option<f64>,
    /// Assay commands from the packet or dispatch.
    pub assay_commands: Vec<String>,
    /// Whether acceptance criteria are mapped to assay commands.
    pub acceptance_mapped: bool,
    /// Operator-configured token rates; `None` means cost is unmeasured (never a fabricated zero).
    pub rates: Option<usage::ModelRates>,
    /// The live child of the running turn, so a scoped interrupt can stop it.
    child: Arc<Mutex<Option<std::process::Child>>>,
}

impl DshHarness {
    /// Build the adapter from the run's context: the binary constructs one `HarnessContext` per run
    /// and the adapter consumes it.
    pub fn from_context(context: &crate::engine::harness::HarnessContext) -> Result<Self> {
        if !context.workspace.exists() {
            return Err(WorkflowError::generic(format!(
                "DSH harness workspace does not exist: {}",
                context.workspace.display()
            )));
        }
        let cli = default_cli()?;
        let model = resolve_model(context.model_policy.as_deref())?;
        Ok(Self {
            cli,
            workspace: context.workspace.clone(),
            model_name: model
                .clone()
                .unwrap_or_else(|| "profile-default".to_string()),
            model,
            env: Some(crate::engine::opencode::sanitized_model_env()),
            packet: context.packet.clone(),
            execution_workspace: context.execution_workspace.clone(),
            story_id: Some(context.story_id.clone()),
            model_policy: context.model_policy.clone(),
            spend_cap_usd: context.spend_cap_usd,
            assay_commands: context.assay_commands.clone(),
            acceptance_mapped: context.acceptance_mapped,
            rates: usage::rates_from_env(),
            child: Arc::new(Mutex::new(None)),
        })
    }

    /// Build the adapter from environment variables (attended/dry-run paths).
    pub fn from_env() -> Result<Self> {
        let workspace = std::env::var("FORGE_WORKTREE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        if !workspace.exists() {
            return Err(WorkflowError::generic(format!(
                "DSH harness workspace does not exist: {}",
                workspace.display()
            )));
        }
        let model = resolve_model(None)?;
        Ok(Self {
            cli: default_cli()?,
            workspace,
            model_name: model
                .clone()
                .unwrap_or_else(|| "profile-default".to_string()),
            model,
            env: Some(crate::engine::opencode::sanitized_model_env()),
            spend_cap_usd: crate::engine::spend_cap::parse_forge_spend_cap_usd(
                std::env::var(crate::engine::spend_cap::SPEND_CAP_ENV)
                    .ok()
                    .as_deref(),
            ),
            assay_commands: std::env::var("FORGE_ASSAY_COMMANDS")
                .ok()
                .map(|value| {
                    value
                        .split('\n')
                        .map(|line| line.trim().to_string())
                        .filter(|line| !line.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
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
                    .map(|value| {
                        value
                            .split('\n')
                            .map(|line| line.trim().to_string())
                            .filter(|line| !line.is_empty())
                            .collect()
                    })
                    .unwrap_or_default(),
                branch_name: std::env::var("FORGE_BRANCH").ok(),
                base_ref: std::env::var("FORGE_BASE_REF").ok(),
                base_commit: std::env::var("FORGE_BASE_COMMIT").ok(),
            },
            execution_workspace: None,
            story_id: std::env::var("FORGE_STORY_ID").ok(),
            model_policy: None,
            rates: usage::rates_from_env(),
            child: Arc::new(Mutex::new(None)),
        })
    }

    /// The DSH sandbox mode for one role turn: implement roles write, every other role is
    /// read-only. This reuses the Forge V2 profile authority table (single source), and an unknown
    /// node fails closed to read-only rather than silently gaining write access.
    pub fn permission_mode_for(&self, node_id: &str) -> &'static str {
        use crate::engine::opencode_agents::{v2_agent_for_node, v2_profile, Authority};
        match v2_agent_for_node(node_id)
            .ok()
            .and_then(v2_profile)
            .map(|profile| profile.authority)
        {
            Some(Authority::Implement) => "workspace-write",
            _ => "read-only",
        }
    }

    /// A fork for a concurrent lane: a fresh process slot and the lane's workspace, same CLI,
    /// model and policy. DSH is process-per-turn, so a fork is a clone with a new workspace.
    fn clone_for_workspace(
        &self,
        workspace: &crate::engine::packet::ExecutionWorkspace,
    ) -> DshHarness {
        DshHarness {
            cli: self.cli.clone(),
            workspace: PathBuf::from(&workspace.worktree_path),
            model: self.model.clone(),
            model_name: self.model_name.clone(),
            env: self.env.clone(),
            packet: self.packet.clone(),
            execution_workspace: Some(workspace.clone()),
            story_id: self.story_id.clone(),
            model_policy: self.model_policy.clone(),
            spend_cap_usd: self.spend_cap_usd,
            assay_commands: self.assay_commands.clone(),
            acceptance_mapped: self.acceptance_mapped,
            rates: self.rates,
            child: Arc::new(Mutex::new(None)),
        }
    }

    fn run_git(&self, args: &[&str]) -> Option<String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.workspace)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub(super) fn task_text(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> crate::engine::packet::TaskText {
        use crate::engine::packet::TaskIsolation;
        crate::engine::packet::TaskText::for_node(
            node_id,
            &task.task_id,
            &self.packet,
            TaskIsolation::of(self.execution_workspace.as_ref()),
            self_heal,
        )
    }
}

/// Run one headless DSH process to completion, enforcing the wall-clock turn ceiling.
pub(super) fn run_headless(
    cli: &args::DshCli,
    cwd: &str,
    argv: &[String],
    env: Option<&HashMap<String, String>>,
    max_turn: Option<Duration>,
    child_slot: &Arc<Mutex<Option<std::process::Child>>>,
) -> Result<DshRunResult> {
    let mut cmd = Command::new(&cli.program);
    cmd.args(&cli.prefix);
    cmd.args(argv);
    cmd.current_dir(cwd);
    if let Some(env) = env {
        // The child must never inherit the control plane's environment: clear, then apply the
        // sanitized allowlist (same order as the OpenCode/Maestro adapters).
        cmd.env_clear();
        cmd.envs(env);
    }
    cmd.env("PWD", cwd);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let child = cmd.spawn().map_err(|error| {
        WorkflowError::generic(format!(
            "failed to spawn dsh (`{}`): {error}",
            cli.display()
        ))
    })?;
    let (mut stdout_pipe, mut stderr_pipe) = {
        let mut guard = child_slot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *guard = Some(child);
        let child = guard.as_mut().expect("child stored");
        (
            child.stdout.take().expect("piped stdout"),
            child.stderr.take().expect("piped stderr"),
        )
    };
    let stdout_thread = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout_pipe.read_to_string(&mut text);
        text
    });
    let stderr_thread = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr_pipe.read_to_string(&mut text);
        text
    });

    let deadline = max_turn.map(|ceiling| Instant::now() + ceiling);
    let exit_code = loop {
        let mut guard = child_slot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let outcome = guard
            .as_mut()
            .expect("child stored")
            .try_wait()
            .map_err(|error| {
                WorkflowError::generic(format!("could not observe the dsh process: {error}"))
            });
        match outcome {
            Ok(Some(status)) => {
                let _ = guard.take();
                drop(guard);
                break status.code();
            }
            Ok(None) => {}
            Err(error) => return Err(error),
        }
        drop(guard);
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            let mut guard = child_slot
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
            return Err(WorkflowError::generic(format!(
                "dsh harness turn exceeded the wall-clock ceiling ({}s); process killed",
                max_turn
                    .map(|ceiling| ceiling.as_secs())
                    .unwrap_or_default()
            )));
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let stdout = stdout_thread.join().unwrap_or_default();
    let stderr = stderr_thread.join().unwrap_or_default();
    Ok(DshRunResult {
        exit_code,
        stdout,
        stderr,
    })
}

fn session_continuity_enabled() -> bool {
    !matches!(
        std::env::var("FORGE_SESSION_CONTINUITY").ok().as_deref(),
        Some("0") | Some("false") | Some("off")
    )
}

fn write_session_marker(workspace: &std::path::Path, id: &str) -> std::result::Result<(), String> {
    std::fs::write(workspace.join(SESSION_MARKER_FILENAME), id)
        .map_err(|error| format!("could not write {SESSION_MARKER_FILENAME}: {error}"))
}

impl crate::engine::runner::CandidateProbe for DshHarness {
    fn git(&self, args: &[&str]) -> Option<String> {
        self.run_git(args)
    }

    fn declared_test_mode(&self) -> Option<&str> {
        self.packet.test_mode.as_deref()
    }
}

impl crate::engine::runner::ProductionProbe for DshHarness {
    fn production_url(&self) -> String {
        crate::engine::production_probe::production_base_url()
    }

    fn deployed_sha(&self) -> std::result::Result<String, String> {
        crate::engine::production_probe::fetch_deployed_sha(&self.production_url())
    }
}

impl RoleHarness for DshHarness {
    fn run_role(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> Result<HarnessOutput> {
        if !self.workspace.exists() {
            return Err(WorkflowError::generic(format!(
                "DSH harness workspace does not exist: {}",
                self.workspace.display()
            )));
        }
        preflight::preflight(&self.cli)?;

        let cwd = self.workspace.to_string_lossy().to_string();
        let task_text = self.task_text(node_id, task, self_heal);
        let before_sha = self.run_git(&["rev-parse", "HEAD"]);

        eprintln!(
            "harness={} bin={} cwd={} model={} model_policy={}",
            DSH_HARNESS_ADAPTER_ID,
            self.cli.display(),
            cwd,
            self.model_name,
            self.model_policy.as_deref().unwrap_or("(default cheap)")
        );

        let patch = match self.model.as_deref() {
            Some(model) => Some(args::ModelPatchFile::create(model)?),
            None => None,
        };
        let argv = args::headless_args(
            DSH_PROFILE,
            patch.as_ref().map(args::ModelPatchFile::path),
            task_text.as_str(),
        );
        let max_turn = turn_ceiling(std::env::var(TURN_CEILING_ENV).ok().as_deref());
        let permission_mode = self.permission_mode_for(node_id);
        eprintln!(
            "harness={} permission={permission_mode}",
            DSH_HARNESS_ADAPTER_ID
        );
        let mut env = self.env.clone().unwrap_or_default();
        env.insert(
            "DSH_PERMISSION_MODE".to_string(),
            permission_mode.to_string(),
        );
        let result = run_headless(&self.cli, &cwd, &argv, Some(&env), max_turn, &self.child)?;
        drop(patch);

        if result.exit_code != Some(0) {
            return Err(WorkflowError::generic(format!(
                "dsh-harness failed for {node_id}: exit={:?}, stderr={}",
                result.exit_code,
                result.stderr.trim()
            )));
        }

        let dsh_home = args::dsh_home(
            std::env::var_os("DSH_HOME").as_deref(),
            std::env::var_os("HOME").as_deref(),
        );
        let session_root = args::session_root(&self.workspace, &dsh_home);
        let parsed = output::parse(&result.stdout, &session_root);

        // Session evidence, not resumption: headless always opens a fresh session. Both writes are
        // durable, so a failure propagates rather than letting the turn read as if its id was kept.
        if session_continuity_enabled() {
            if let (Some(story), Some(session)) = (&self.story_id, parsed.session_id.as_deref()) {
                crate::engine::vendor_session::write_vendor_session_id(
                    story,
                    VENDOR_SESSION_LANE,
                    Some(session),
                )
                .map_err(|error| {
                    crate::engine::opencode::vendor_session_persist_error(
                        "dsh-harness",
                        node_id,
                        story,
                        &error,
                    )
                })?;
            }
            if let Some(session) = parsed.session_id.as_deref() {
                write_session_marker(&self.workspace, session).map_err(|error| {
                    WorkflowError::generic(format!(
                        "dsh-harness could not persist the session marker for {node_id}: {error}"
                    ))
                })?;
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

        let usage = parsed
            .session_id
            .as_deref()
            .zip(parsed.session_dir.as_deref())
            .and_then(|(session_id, dir)| {
                usage::harness_usage(dir, session_id, self.rates.as_ref())
            });

        Ok(HarnessOutput {
            raw: parsed.assistant_text,
            candidate_sha,
            assay_commands: assay,
            acceptance_mapped: self.acceptance_mapped,
            refusal: None,
            execution_base,
            // Measured from the session log; UNMEASURED (None) when no token rates are configured
            // or the session log carries no usage.
            usage,
            trajectory: None,
        })
    }

    fn interrupt_execution(
        &self,
        _reason: &str,
    ) -> Result<Option<crate::engine::harness::TurnTermination>> {
        let mut guard = self
            .child
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(child) = guard.as_mut() else {
            return Ok(None);
        };
        let pid = child.id();
        let killed = child.kill().is_ok();
        Ok(Some(crate::engine::harness::TurnTermination {
            pid,
            existed: true,
            killed,
            signalled: Vec::new(),
        }))
    }

    fn supports_interrupt(&self) -> bool {
        true
    }

    fn fork_for_workspace(
        &self,
        workspace: crate::engine::packet::ExecutionWorkspace,
    ) -> Result<Option<Arc<dyn RoleHarness>>> {
        Ok(Some(Arc::new(self.clone_for_workspace(&workspace))))
    }

    fn exists_on_base_ref(&self, base_ref: &str, path: &str) -> bool {
        self.run_git(&["cat-file", "-e", &format!("{base_ref}:{path}")])
            .is_some()
            || self
                .run_git(&["ls-tree", "--name-only", base_ref, path])
                .map(|text| !text.is_empty())
                .unwrap_or(false)
    }

    fn assay_cwd(&self) -> &std::path::Path {
        if let Some(workspace) = &self.execution_workspace {
            return std::path::Path::new(&workspace.worktree_path);
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
            assay_timeout, cancellation_signal, spawn_scoped_shell_with_env, CeilingOutcome,
            CMD_CANCELLED_EXIT, CMD_TIMEOUT_CODE, CMD_TIMEOUT_EXIT,
        };
        let cwd = self.assay_cwd();
        let mut assay_env = crate::engine::opencode::sanitize_assay_env(std::env::vars().collect());
        assay_env.insert(
            "CARGO_TARGET_DIR".into(),
            crate::engine::opencode::derive_cargo_target_dir(cwd)
                .to_string_lossy()
                .to_string(),
        );
        let child = match spawn_scoped_shell_with_env(command, cwd, &assay_env) {
            Ok(child) => child,
            Err(error) => {
                return CommandResult {
                    cancelled: false,
                    command: command.into(),
                    exit_code: -1,
                    passed: false,
                    excerpt: error.to_string(),
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
            CeilingOutcome::Finished(Err(error)) => CommandResult {
                cancelled: false,
                command: command.into(),
                exit_code: -1,
                passed: false,
                excerpt: format!("could not observe assay command: {error}"),
                unmeasurable: true,
                output: String::new(),
            },
            CeilingOutcome::Finished(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let text = crate::engine::assay::combine_command_output(&stdout, &stderr);
                let signal = cancellation_signal(&output.status);
                let code = if signal.is_some() {
                    CMD_CANCELLED_EXIT
                } else {
                    output.status.code().unwrap_or(1)
                };
                let text = if let Some(signal) = signal {
                    format!("CMD_CANCELLED: assay command received signal {signal}\n{text}")
                } else {
                    text
                };
                CommandResult {
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
                }
            }
        }
    }
}
