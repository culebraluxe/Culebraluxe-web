//! A real non-production MODEL-TURN [`ReplaySandbox`]: one headless agent turn per replay probe.
//!
//! The dogfood experiment can only produce behavioral evidence if something actually executes the
//! candidate harness. This sandbox runs the existing agent harness headless — DSH (`dsh --profile
//! headless`) or the OpenCode V2 CLI — in a temporary workspace with the candidate harness
//! manifest materialized as **content only** (prompt/tool-guidance text; authority fields are not
//! executable and never reach the child), read-only where the backend supports it, a hard timeout
//! and bounded output.
//!
//! § D1 boundary: **no model turn without an explicit [`ModelTurnAuthorization`].** The token has
//! private fields and can only be minted by [`ModelTurnAuthorization::authorize_model_run`] or the
//! script flag [`ModelTurnAuthorization::FLAG`] (`--authorize-model-run`). Production is refused,
//! the backend must be available *before* a token is issued, and a real run also needs the
//! sandbox/real-run authorization from `dogfood`. Tests use a deterministic fake command backend;
//! no live paid model call happens in the test suite.
//!
//! Capture honesty:
//!
//! * cost is `Option<u64>` micro-dollars: `None` means the harness reported no priced usage;
//! * token counts are `Option<u64>` in the receipt; DSH usage is read post-hoc from its own session
//!   log, and an unmeasured turn is labelled `unmeasured` in the evidence refs rather than being
//!   claimed as zero;
//! * stdout/stderr are streamed through a fixed cap (reusing `sandbox_command::read_stream_bounded`)
//!   and the receipt records truncation flags and true totals;
//! * timeouts kill the whole process group and reap the child.
//!
//! The backend selector is `FORGE_HARNESS`: `dsh` (default) or `opencode`. An unknown value fails
//! closed; a selected backend that is not installed fails closed before any spend.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::engine::dsh;
use crate::engine::opencode_client;

use super::model::{BehaviorEvidence, EnvironmentClass, QaVerdict};
use super::probe_replay::{
    assert_non_production_environment, HistoricalTask, ReplayError, ReplayHarness, ReplaySandbox,
    SandboxReading, SandboxRequest,
};
use super::sandbox_command::read_stream_bounded;

/// Environment variable selecting the headless backend (`dsh` or `opencode`).
pub const FORGE_HARNESS_ENV: &str = "FORGE_HARNESS";

/// The default backend name.
pub const DEFAULT_HARNESS_BACKEND: &str = "dsh";

/// Default wall-clock budget for one model turn.
pub const DEFAULT_MODEL_TURN_TIMEOUT: Duration = Duration::from_secs(300);

/// The longest timeout this sandbox accepts for one model turn.
pub const MAX_MODEL_TURN_TIMEOUT: Duration = Duration::from_secs(1800);

/// How much of stdout/stderr is retained per stream in the receipt.
pub const DEFAULT_MODEL_TURN_OUTPUT_CAP: usize = 64 * 1024;

/// The largest materialized prompt this sandbox will send. Oversize prompts are refused, not
/// truncated: silently shortening the candidate harness would change the probe's identity.
pub const DEFAULT_PROMPT_CAP_BYTES: usize = 256 * 1024;

/// Ref prefix for one model-turn execution.
pub const EXEC_REF_PREFIX: &str = "model-turn:exec:";
/// Ref prefix for the JSON receipt.
pub const RECEIPT_REF_PREFIX: &str = "model-turn:receipt:";
/// Ref prefix for the backend that ran.
pub const BACKEND_REF_PREFIX: &str = "model-turn:backend:";
/// Ref label for a measured usage turn.
pub const USAGE_MEASURED_REF: &str = "model-turn:usage:measured";
/// Ref label for an unmeasured usage turn. Never presented as zero usage.
pub const USAGE_UNMEASURED_REF: &str = "model-turn:usage:unmeasured";
/// Ref prefix for the DSH session the turn produced.
pub const SESSION_REF_PREFIX: &str = "model-turn:session:";

/// The marker a fake command backend prints to report usage deterministically.
pub const USAGE_MARKER_PREFIX: &str = "FORGE_MODEL_TURN_USAGE:";

/// Which headless backend executes the turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelTurnBackend {
    /// `dsh --profile headless "<task>"` (the preferred backend, read-only sandbox mode).
    Dsh,
    /// The OpenCode V2 CLI (`run --standalone --format json ...`).
    OpenCode,
    /// Deterministic test backend: run one absolute command, appending the prompt as the final
    /// argument. Only exists in test builds, so production code cannot select it.
    #[cfg(test)]
    Command { program: PathBuf, args: Vec<String> },
}

impl ModelTurnBackend {
    pub fn id(&self) -> &'static str {
        match self {
            Self::Dsh => "dsh",
            Self::OpenCode => "opencode",
            #[cfg(test)]
            Self::Command { .. } => "command",
        }
    }

    /// Pure selector used by [`ModelTurnBackend::from_env`]; separated so tests do not mutate the
    /// process environment.
    pub fn from_name(name: Option<&str>) -> Result<Self, ReplayError> {
        match name.map(str::trim).filter(|value| !value.is_empty()) {
            None | Some(DEFAULT_HARNESS_BACKEND) => Ok(Self::Dsh),
            Some("opencode") => Ok(Self::OpenCode),
            Some(other) => Err(refused(format!(
                "unknown {FORGE_HARNESS_ENV}={other:?}; expected dsh or opencode"
            ))),
        }
    }

    pub fn from_env() -> Result<Self, ReplayError> {
        Self::from_name(std::env::var(FORGE_HARNESS_ENV).ok().as_deref())
    }

    /// Fail closed before a turn: the selected backend must exist and speak the contract the
    /// adapter is written against. No contract check is deferred to post-spend.
    pub fn ensure_available(&self) -> Result<(), ReplayError> {
        match self {
            Self::Dsh => {
                let cli = dsh::default_cli()
                    .map_err(|error| refused(format!("DSH is unavailable: {error}")))?;
                dsh::preflight::preflight(&cli)
                    .map_err(|error| refused(format!("DSH preflight failed: {error}")))
            }
            Self::OpenCode => {
                let bin = crate::engine::opencode::default_cli_bin();
                opencode_client::verify_vendor_contract(&bin)
                    .map(|_| ())
                    .map_err(|error| refused(format!("OpenCode is unavailable: {error}")))
            }
            #[cfg(test)]
            Self::Command { program, .. } => {
                if program.is_absolute() {
                    Ok(())
                } else {
                    Err(refused(format!(
                        "fake command program {} is not an absolute path",
                        program.display()
                    )))
                }
            }
        }
    }
}

/// Explicit authorization for a real model turn.
///
/// Private fields: outside this module the only constructors are
/// [`ModelTurnAuthorization::authorize_model_run`] (explicit sandbox-class consent, backend
/// availability checked first) and [`ModelTurnAuthorization::from_script_args`] (the
/// `--authorize-model-run` flag). A missing or production authorization is a typed refusal, never a
/// silent dry run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelTurnAuthorization {
    backend: ModelTurnBackend,
    environment_class: EnvironmentClass,
}

impl ModelTurnAuthorization {
    /// The script/CLI flag that expresses explicit model-run authorization.
    pub const FLAG: &'static str = "--authorize-model-run";

    pub fn authorize_model_run(
        backend: ModelTurnBackend,
        environment_class: EnvironmentClass,
    ) -> Result<Self, ReplayError> {
        if environment_class.is_production() || !environment_class.allows_probe() {
            return Err(ReplayError::ProductionRefused { environment_class });
        }
        backend.ensure_available()?;
        Ok(Self {
            backend,
            environment_class,
        })
    }

    /// Parse explicit authorization from script arguments; the flag must be present verbatim.
    pub fn from_script_args(
        args: &[String],
        backend: ModelTurnBackend,
        environment_class: EnvironmentClass,
    ) -> Result<Self, ReplayError> {
        if !args.iter().any(|argument| argument == Self::FLAG) {
            return Err(refused(format!(
                "missing {} flag; a real model turn is refused without explicit authorization",
                Self::FLAG
            )));
        }
        Self::authorize_model_run(backend, environment_class)
    }

    pub fn backend(&self) -> &ModelTurnBackend {
        &self.backend
    }

    pub fn environment_class(&self) -> EnvironmentClass {
        self.environment_class
    }
}

/// Captured usage for one turn. `None` cost means unpriced, not free.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_micros: Option<u64>,
    /// Where the numbers came from (`dsh_session` | `command_marker`); never invented.
    pub source: &'static str,
}

/// Parse the deterministic fake backend's usage marker (test/stand-in backends only).
pub fn parse_usage_marker(stdout: &str) -> Option<CapturedUsage> {
    let line = stdout
        .lines()
        .rev()
        .find(|line| line.trim_start().starts_with(USAGE_MARKER_PREFIX))?;
    let payload = line.trim_start().trim_start_matches(USAGE_MARKER_PREFIX).trim();
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    let input_tokens = value.get("input_tokens")?.as_u64()?;
    let output_tokens = value.get("output_tokens")?.as_u64()?;
    let cost_micros = value
        .get("cost_usd")
        .and_then(serde_json::Value::as_f64)
        .filter(|usd| usd.is_finite() && *usd >= 0.0)
        .map(|usd| (usd * 1_000_000.0).round() as u64);
    Some(CapturedUsage {
        input_tokens,
        output_tokens,
        cost_micros,
        source: "command_marker",
    })
}

/// One recorded model turn: the only durable artifact this sandbox writes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModelTurnReceipt {
    pub execution_id: String,
    pub probe_id: String,
    pub revision: String,
    pub manifest_hash: String,
    pub backend: String,
    pub authorized_via: String,
    pub environment_class: String,
    pub model: String,
    pub model_pin_applied: bool,
    pub timeout_ms: u64,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout_excerpt: String,
    pub stderr_excerpt: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    /// `None` = the harness reported no usage (never zero).
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    /// `None` = unpriced (no rates configured), never zero.
    pub cost_micros: Option<u64>,
    pub usage_source: String,
    pub session_id: Option<String>,
    pub prompt_sha256: String,
    pub prompt_bytes: u64,
    pub workspace: String,
}

/// The real model-turn sandbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelTurnSandbox {
    authorization: ModelTurnAuthorization,
    timeout: Duration,
    output_cap: usize,
    prompt_cap: usize,
    run_root: PathBuf,
}

impl ModelTurnSandbox {
    /// Build the sandbox from an explicit authorization. The token already proves a non-production
    /// class and an available backend; this constructor only applies defaults.
    pub fn new(authorization: ModelTurnAuthorization) -> Self {
        Self {
            authorization,
            timeout: DEFAULT_MODEL_TURN_TIMEOUT,
            output_cap: DEFAULT_MODEL_TURN_OUTPUT_CAP,
            prompt_cap: DEFAULT_PROMPT_CAP_BYTES,
            run_root: default_run_root(),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_output_cap(mut self, output_cap: usize) -> Self {
        self.output_cap = output_cap;
        self
    }

    pub fn with_prompt_cap(mut self, prompt_cap: usize) -> Self {
        self.prompt_cap = prompt_cap;
        self
    }

    /// Where workspaces and receipts are written.
    pub fn with_run_root(mut self, run_root: impl Into<PathBuf>) -> Self {
        self.run_root = run_root.into();
        self
    }

    pub fn authorization(&self) -> &ModelTurnAuthorization {
        &self.authorization
    }

    pub fn run_root(&self) -> &Path {
        &self.run_root
    }

    /// Validate the configuration without spending anything.
    pub fn validate(&self) -> Result<(), ReplayError> {
        if self.timeout.is_zero() || self.timeout > MAX_MODEL_TURN_TIMEOUT {
            return Err(refused(format!(
                "model-turn timeout {:?} is outside (0, {MAX_MODEL_TURN_TIMEOUT:?}]",
                self.timeout
            )));
        }
        if self.output_cap == 0 {
            return Err(refused("model-turn output cap must be non-zero"));
        }
        if self.prompt_cap == 0 {
            return Err(refused("model-turn prompt cap must be non-zero"));
        }
        if self.authorization.environment_class.is_production()
            || !self.authorization.environment_class.allows_probe()
        {
            return Err(ReplayError::ProductionRefused {
                environment_class: self.authorization.environment_class,
            });
        }
        Ok(())
    }
}

impl ReplaySandbox for ModelTurnSandbox {
    fn run(&self, request: &SandboxRequest<'_>) -> Result<SandboxReading, ReplayError> {
        // Production refusal is always first: no workspace, no receipt, no spawn.
        assert_non_production_environment(request.environment_class)?;
        self.validate()?;
        if request.environment_class != self.authorization.environment_class() {
            return Err(refused(format!(
                "model-turn authorization covers {}, but the probe runs {}",
                self.authorization.environment_class.id(),
                request.environment_class.id()
            )));
        }

        let prompt = materialize_prompt(request.harness, request.task);
        if prompt.len() > self.prompt_cap {
            // Refusing beats silently shortening the candidate harness in the probe prompt.
            return Err(refused(format!(
                "materialized prompt is {} bytes, above the {} byte cap; refusing to alter the probe prompt",
                prompt.len(),
                self.prompt_cap
            )));
        }
        let prompt_sha256 = sha256_hex(prompt.as_bytes());

        let workspace = self.workspace_for(request.probe_id);
        std::fs::create_dir_all(&workspace).map_err(|error| ReplayError::SandboxFailed {
            probe_id: request.probe_id.to_string(),
            message: format!(
                "cannot create model-turn workspace {}: {error}",
                workspace.display()
            ),
        })?;

        let invocation = self.invocation(request, &workspace, &prompt)?;
        let mut command = Command::new(&invocation.program);
        command
            .args(&invocation.prefix)
            .args(&invocation.args)
            .current_dir(&workspace)
            // Never inherit the control plane's environment: clear, then apply the sanitized vendor
            // environment plus the workspace-local values below.
            .env_clear()
            .envs(invocation.env.iter().map(|(key, value)| (key, value)))
            .env("PWD", &workspace)
            .env("HOME", &workspace)
            .env("TMPDIR", &workspace)
            .env("LANG", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }

        let started = Instant::now();
        let mut child = command.spawn().map_err(|error| ReplayError::SandboxFailed {
            probe_id: request.probe_id.to_string(),
            message: format!(
                "cannot spawn {} ({}) for the model turn: {error}",
                invocation.program,
                self.authorization.backend.id()
            ),
        })?;
        let stdout = child.stdout.take().ok_or_else(|| ReplayError::SandboxFailed {
            probe_id: request.probe_id.to_string(),
            message: "model-turn stdout pipe missing".into(),
        })?;
        let stderr = child.stderr.take().ok_or_else(|| ReplayError::SandboxFailed {
            probe_id: request.probe_id.to_string(),
            message: "model-turn stderr pipe missing".into(),
        })?;
        let output_cap = self.output_cap;
        let stdout_thread = thread::spawn(move || read_stream_bounded(stdout, output_cap));
        let stderr_thread = thread::spawn(move || read_stream_bounded(stderr, output_cap));

        let mut timed_out = false;
        let exit_status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => {
                    if started.elapsed() >= self.timeout {
                        terminate_process_group(&mut child);
                        timed_out = true;
                        break None;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    terminate_process_group(&mut child);
                    return Err(ReplayError::SandboxFailed {
                        probe_id: request.probe_id.to_string(),
                        message: format!("cannot observe the model-turn process: {error}"),
                    });
                }
            }
        };
        let duration = started.elapsed();
        let stdout_capture =
            join_capture(stdout_thread, request.probe_id, "model-turn stdout")?;
        let stderr_capture =
            join_capture(stderr_thread, request.probe_id, "model-turn stderr")?;

        // The model patch overlay is only needed while the child runs.
        drop(invocation.patch);

        // Usage is read from the harness's OWN record; absence is UNMEASURED, never zero.
        let dsh_home = dsh_home_path(&workspace);
        let session_root = dsh::args::session_root(&workspace, &dsh_home);
        let session = dsh::args::discover_latest_session(&session_root);
        let usage = self.capture_usage(&stdout_capture.text, session.as_ref(), &session_root);

        let execution_id = format!(
            "{EXEC_REF_PREFIX}{}:{}",
            sanitize_component(request.probe_id),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0)
        );
        let receipt = ModelTurnReceipt {
            execution_id: execution_id.clone(),
            probe_id: request.probe_id.to_string(),
            revision: request.harness.revision.clone(),
            manifest_hash: request.harness.manifest_hash.clone(),
            backend: self.authorization.backend.id().to_string(),
            authorized_via: ModelTurnAuthorization::FLAG.to_string(),
            environment_class: request.environment_class.id().to_string(),
            model: request.model.to_string(),
            model_pin_applied: invocation.model_pin_applied,
            timeout_ms: u64::try_from(self.timeout.as_millis()).unwrap_or(u64::MAX),
            duration_ms: u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
            exit_code: exit_status.and_then(|status| status.code()),
            timed_out,
            stdout_excerpt: stdout_capture.text.clone(),
            stderr_excerpt: stderr_capture.text.clone(),
            stdout_truncated: stdout_capture.truncated,
            stderr_truncated: stderr_capture.truncated,
            stdout_bytes: stdout_capture.total_bytes,
            stderr_bytes: stderr_capture.total_bytes,
            input_tokens: usage.as_ref().map(|usage| usage.input_tokens),
            output_tokens: usage.as_ref().map(|usage| usage.output_tokens),
            cost_micros: usage.as_ref().and_then(|usage| usage.cost_micros),
            usage_source: usage
                .as_ref()
                .map_or_else(|| "unmeasured".to_string(), |usage| usage.source.to_string()),
            session_id: session,
            prompt_sha256,
            prompt_bytes: u64::try_from(prompt.len()).unwrap_or(u64::MAX),
            workspace: workspace.display().to_string(),
        };
        let receipt_path = write_receipt(&self.run_root, &receipt)?;

        if timed_out {
            return Err(ReplayError::SandboxFailed {
                probe_id: request.probe_id.to_string(),
                message: format!(
                    "model turn timed out after {:?} (process group killed); receipt {}",
                    self.timeout,
                    receipt_path.display()
                ),
            });
        }

        let success = exit_status.is_some_and(|status| status.success());
        let mut behavior = BehaviorEvidence::new(execution_id.clone(), "");
        behavior.success = success;
        behavior.tests_passed = u32::from(success);
        behavior.tests_failed = u32::from(!success);
        behavior.qa = if success {
            QaVerdict::Pass
        } else {
            QaVerdict::Fail
        };
        behavior.steps = 1;
        behavior.elapsed_ms = receipt.duration_ms;
        behavior.tokens_in = usage.as_ref().map_or(0, |usage| usage.input_tokens);
        behavior.tokens_out = usage.as_ref().map_or(0, |usage| usage.output_tokens);
        // Unpriced stays `None`; an unmeasured token count is labelled by the usage ref below.
        behavior.cost_micros = usage.as_ref().and_then(|usage| usage.cost_micros);
        behavior.tool_failures = 0;
        let mut evidence_refs = vec![
            execution_id,
            format!("{RECEIPT_REF_PREFIX}{}", receipt_path.display()),
            format!("{BACKEND_REF_PREFIX}{}", self.authorization.backend.id()),
            if usage.is_some() {
                USAGE_MEASURED_REF.to_string()
            } else {
                USAGE_UNMEASURED_REF.to_string()
            },
        ];
        if let Some(session) = &receipt.session_id {
            evidence_refs.push(format!("{SESSION_REF_PREFIX}{session}"));
        }
        behavior.evidence_refs = evidence_refs;
        Ok(SandboxReading::new(behavior))
    }
}

impl ModelTurnSandbox {
    fn workspace_for(&self, probe_id: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        self.run_root
            .join(format!("{stamp}-{}", sanitize_component(probe_id)))
    }

    /// Build the child's argv/env for the selected backend. The prompt is the last argument, on
    /// stdin-free argv (never a shell string).
    fn invocation(
        &self,
        request: &SandboxRequest<'_>,
        workspace: &Path,
        prompt: &str,
    ) -> Result<TurnInvocation, ReplayError> {
        let mut env: Vec<(String, String)> = crate::engine::opencode::sanitized_model_env()
            .into_iter()
            .collect();
        match &self.authorization.backend {
            ModelTurnBackend::Dsh => {
                let cli = dsh::default_cli()
                    .map_err(|error| refused(format!("DSH is unavailable: {error}")))?;
                // The DSH headless path has no `--model` flag; the only pin seam is a patch overlay.
                // A pin we cannot express is left to the profile and recorded as not applied.
                let (patch, model_pin_applied) =
                    match dsh::args::validate_model(request.model) {
                        Ok(model) => {
                            let patch = dsh::args::ModelPatchFile::create(&model)
                                .map_err(|error| refused(format!("DSH model patch: {error}")))?;
                            (Some(patch), true)
                        }
                        Err(_) => (None, false),
                    };
                let args = dsh::args::headless_args(
                    dsh::DSH_PROFILE,
                    patch.as_ref().map(dsh::args::ModelPatchFile::path),
                    prompt,
                );
                // Read-only where the backend supports it: DSH's permission mode.
                env.push(("DSH_PERMISSION_MODE".into(), "read-only".into()));
                // Session discovery must look where this child writes.
                let dsh_home = std::env::var_os("DSH_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| workspace.join(".dsh"));
                env.push(("DSH_HOME".into(), dsh_home.to_string_lossy().to_string()));
                Ok(TurnInvocation {
                    program: cli.program,
                    prefix: cli.prefix,
                    args,
                    env,
                    patch,
                    model_pin_applied,
                })
            }
            ModelTurnBackend::OpenCode => {
                let bin = crate::engine::opencode::default_cli_bin();
                // No `--auto`: the turn runs without auto-approval in a throwaway workspace. The
                // vendor's own agent/permission config remains the read-only boundary here.
                let args = opencode_client::build_opencode_run_args(
                    request.model,
                    prompt,
                    false,
                    None,
                    false,
                    None,
                );
                Ok(TurnInvocation {
                    program: bin,
                    prefix: Vec::new(),
                    args,
                    env,
                    patch: None,
                    model_pin_applied: true,
                })
            }
            #[cfg(test)]
            ModelTurnBackend::Command { program, args } => {
                let mut argv = args.clone();
                argv.push(prompt.to_string());
                Ok(TurnInvocation {
                    program: program.to_string_lossy().to_string(),
                    prefix: Vec::new(),
                    args: argv,
                    env,
                    patch: None,
                    model_pin_applied: false,
                })
            }
        }
    }

    /// Usage from the harness's own record. DSH's session log is metered; OpenCode's export is not
    /// wired here; the fake command backend uses the deterministic marker.
    fn capture_usage(
        &self,
        _stdout: &str,
        session: Option<&String>,
        session_root: &Path,
    ) -> Option<CapturedUsage> {
        match &self.authorization.backend {
            ModelTurnBackend::Dsh => {
                let session = session?;
                let totals = dsh::usage::read_token_totals(&session_root.join(session))?;
                let cost_micros = dsh::usage::rates_from_env().and_then(|rates| {
                    let usd = totals.cost_usd(&rates);
                    usd.is_finite()
                        .then(|| (usd * 1_000_000.0).round() as u64)
                });
                Some(CapturedUsage {
                    input_tokens: totals.input_tokens,
                    output_tokens: totals.output_tokens,
                    cost_micros,
                    source: "dsh_session",
                })
            }
            ModelTurnBackend::OpenCode => None,
            #[cfg(test)]
            ModelTurnBackend::Command { .. } => parse_usage_marker(_stdout),
        }
    }
}

struct TurnInvocation {
    program: String,
    prefix: Vec<String>,
    args: Vec<String>,
    env: Vec<(String, String)>,
    patch: Option<dsh::args::ModelPatchFile>,
    model_pin_applied: bool,
}

/// Materialize the candidate harness as **content only** plus the historical task packet.
///
/// `ReplayHarness` carries component text and nothing executable: no authority class, no lifecycle
/// field, no tool grant. That is the point — a sandbox cannot act on a permission it was never
/// given, and the string it receives here cannot contain one.
pub fn materialize_prompt(harness: &ReplayHarness, task: &HistoricalTask) -> String {
    let mut out = String::new();
    out.push_str("HARNESS EVOLUTION REPLAY PROBE\n");
    out.push_str(
        "You are executing one historical task under a pinned harness revision in a sandbox workspace. \
You are read-only: do not modify files, do not publish, do not run destructive commands. \
Report what you did and whether the task's acceptance checks passed.\n",
    );
    out.push_str(&format!("revision: {}\n", harness.revision));
    out.push_str(&format!("manifest: {}\n", harness.manifest_hash));
    for (kind, content) in &harness.components {
        out.push_str(&format!("\n--- component: {} ---\n{}\n", kind.id(), content));
    }
    out.push_str("\n--- task packet ---\n");
    out.push_str(&task.packet);
    out.push_str("\n--- end task packet ---\n");
    out
}

fn default_run_root() -> PathBuf {
    std::env::temp_dir().join("forge-harness-evolution-model-turn")
}

fn refused(message: impl Into<String>) -> ReplayError {
    ReplayError::SandboxRefused {
        sandbox: "model-turn".into(),
        message: message.into(),
    }
}

fn sanitize_component(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "model-turn".to_string()
    } else {
        sanitized
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn join_capture(
    handle: thread::JoinHandle<io::Result<super::sandbox_command::StreamCapture>>,
    probe_id: &str,
    stream: &str,
) -> Result<super::sandbox_command::StreamCapture, ReplayError> {
    match handle.join() {
        Ok(Ok(capture)) => Ok(capture),
        Ok(Err(error)) => Err(ReplayError::SandboxFailed {
            probe_id: probe_id.to_string(),
            message: format!("cannot read {stream}: {error}"),
        }),
        Err(_) => Err(ReplayError::SandboxFailed {
            probe_id: probe_id.to_string(),
            message: format!("{stream} reader thread panicked"),
        }),
    }
}

/// Kill the child's whole process group, then reap it. (Same contract as the command sandbox.)
#[cfg(unix)]
fn terminate_process_group(child: &mut Child) {
    use std::os::raw::c_int;
    extern "C" {
        fn kill(pid: c_int, sig: c_int) -> c_int;
    }
    const SIGKILL: c_int = 9;
    let pid = child.id() as c_int;
    if pid > 0 {
        unsafe {
            kill(-pid, SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(not(unix))]
fn terminate_process_group(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Where this child would write DSH sessions: the explicit `DSH_HOME`, else `$HOME/.dsh` where
/// `HOME` is the throwaway workspace.
fn dsh_home_path(workspace: &Path) -> PathBuf {
    std::env::var_os("DSH_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join(".dsh"))
}

fn write_receipt(run_root: &Path, receipt: &ModelTurnReceipt) -> Result<PathBuf, ReplayError> {
    std::fs::create_dir_all(run_root).map_err(|error| ReplayError::SandboxFailed {
        probe_id: receipt.probe_id.clone(),
        message: format!(
            "cannot create model-turn run root {}: {error}",
            run_root.display()
        ),
    })?;
    let path = run_root.join(format!(
        "{}.json",
        sanitize_component(&receipt.execution_id)
    ));
    let body = serde_json::to_vec_pretty(receipt).map_err(|error| ReplayError::SandboxFailed {
        probe_id: receipt.probe_id.clone(),
        message: format!("cannot encode model-turn receipt: {error}"),
    })?;
    std::fs::write(&path, body).map_err(|error| ReplayError::SandboxFailed {
        probe_id: receipt.probe_id.clone(),
        message: format!("cannot write model-turn receipt {}: {error}", path.display()),
    })?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::harness_evolution::model::{
        Component, ComponentKind, HarnessDomain, HarnessManifest, HarnessRevision,
    };
    use crate::engine::harness_evolution::probe::ProbeSide;

    fn revision(content: &str) -> HarnessRevision {
        HarnessRevision::draft(
            "base-1",
            HarnessDomain::RustBackend,
            HarnessManifest::from_components([
                Component::green(ComponentKind::SystemPrompt, content, "prompt"),
                Component::yellow(ComponentKind::ToolGuidance, "tool guidance", "tools"),
            ]),
            "owner",
        )
    }

    fn task() -> HistoricalTask {
        HistoricalTask::new("story-1", "run-1", "Story", "GOAL:\nfix the bug")
    }

    fn request<'a>(
        probe_id: &'a str,
        environment_class: EnvironmentClass,
        task: &'a HistoricalTask,
        harness: &'a ReplayHarness,
    ) -> SandboxRequest<'a> {
        SandboxRequest {
            probe_id,
            side: ProbeSide::Reference,
            environment_class,
            task,
            harness,
            model: "deepseek-chat",
            model_policy: "default",
            executor: "opencode",
            tools: &[],
            test_plan: "cargo test",
        }
    }

    fn command_backend(program: &str, args: Vec<&str>) -> ModelTurnBackend {
        ModelTurnBackend::Command {
            program: PathBuf::from(program),
            args: args.into_iter().map(str::to_string).collect(),
        }
    }

    struct Fixture {
        run_root: tempfile::TempDir,
        task: HistoricalTask,
        harness: ReplayHarness,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                run_root: tempfile::tempdir().expect("temp dir"),
                task: task(),
                harness: ReplayHarness::materialize(&revision("base prompt")).expect("harness"),
            }
        }

        fn sandbox(&self, backend: ModelTurnBackend) -> ModelTurnSandbox {
            let authorization =
                ModelTurnAuthorization::authorize_model_run(backend, EnvironmentClass::Development)
                    .expect("authorized");
            ModelTurnSandbox::new(authorization).with_run_root(self.run_root.path())
        }
    }

    fn run(
        sandbox: &ModelTurnSandbox,
        fixture: &Fixture,
        probe_id: &str,
    ) -> Result<SandboxReading, ReplayError> {
        sandbox.run(&request(
            probe_id,
            EnvironmentClass::Development,
            &fixture.task,
            &fixture.harness,
        ))
    }

    fn receipt_for(reading: &SandboxReading) -> ModelTurnReceipt {
        let path = reading
            .behavior
            .evidence_refs
            .iter()
            .find_map(|reference| reference.strip_prefix(RECEIPT_REF_PREFIX))
            .expect("receipt ref");
        let body = std::fs::read(path).expect("receipt file");
        serde_json::from_slice(&body).expect("receipt json")
    }

    #[test]
    fn backend_selection_is_fail_closed() {
        assert_eq!(
            ModelTurnBackend::from_name(None).unwrap(),
            ModelTurnBackend::Dsh
        );
        assert_eq!(
            ModelTurnBackend::from_name(Some("dsh")).unwrap(),
            ModelTurnBackend::Dsh
        );
        assert_eq!(
            ModelTurnBackend::from_name(Some("opencode")).unwrap(),
            ModelTurnBackend::OpenCode
        );
        let unknown = ModelTurnBackend::from_name(Some("maestro")).unwrap_err();
        assert_eq!(unknown.code(), "replay_sandbox_refused");
        assert!(unknown.to_string().contains("FORGE_HARNESS"));
    }

    #[test]
    fn a_model_run_requires_the_explicit_authorization_flag() {
        let missing = ModelTurnAuthorization::from_script_args(
            &[],
            command_backend("/bin/echo", vec!["x"]),
            EnvironmentClass::Development,
        )
        .unwrap_err();
        assert_eq!(missing.code(), "replay_sandbox_refused");
        assert!(missing.to_string().contains("--authorize-model-run"));

        let authorized = ModelTurnAuthorization::from_script_args(
            &["--authorize-model-run".to_string()],
            command_backend("/bin/echo", vec!["x"]),
            EnvironmentClass::Development,
        )
        .expect("authorized");
        assert_eq!(authorized.environment_class(), EnvironmentClass::Development);
    }

    #[test]
    fn production_and_unavailable_backends_are_refused_before_a_turn() {
        let production = ModelTurnAuthorization::authorize_model_run(
            command_backend("/bin/echo", vec!["x"]),
            EnvironmentClass::Production,
        )
        .unwrap_err();
        assert_eq!(production.code(), "replay_production_refused");

        let unavailable = ModelTurnAuthorization::authorize_model_run(
            ModelTurnBackend::Command {
                program: PathBuf::from("echo"),
                args: vec![],
            },
            EnvironmentClass::Development,
        )
        .unwrap_err();
        assert_eq!(unavailable.code(), "replay_sandbox_refused");
        assert!(unavailable.to_string().contains("absolute"));
    }

    #[test]
    fn a_fake_command_turn_returns_bounded_evidence_without_live_model_usage() {
        let fixture = Fixture::new();
        let sandbox = fixture.sandbox(command_backend("/bin/echo", vec!["fake-turn"]));
        let reading = run(&sandbox, &fixture, "probe-model").expect("reading");

        assert!(reading.behavior.success);
        assert_eq!(reading.behavior.qa, QaVerdict::Pass);
        assert_eq!(reading.behavior.steps, 1);
        assert_eq!(reading.behavior.tokens_in, 0, "unmeasured is labelled below");
        assert_eq!(reading.behavior.tokens_out, 0);
        assert_eq!(reading.behavior.cost_micros, None, "unpriced is not zero");
        assert!(reading
            .behavior
            .evidence_refs
            .iter()
            .any(|reference| reference == USAGE_UNMEASURED_REF));
        assert!(reading
            .behavior
            .evidence_refs
            .iter()
            .any(|reference| reference.starts_with(BACKEND_REF_PREFIX)));

        let receipt = receipt_for(&reading);
        assert_eq!(receipt.backend, "command");
        assert_eq!(receipt.authorized_via, "--authorize-model-run");
        assert_eq!(receipt.environment_class, "development");
        assert_eq!(receipt.exit_code, Some(0));
        assert!(!receipt.timed_out);
        assert_eq!(receipt.input_tokens, None);
        assert_eq!(receipt.cost_micros, None);
        assert_eq!(receipt.usage_source, "unmeasured");
        assert_eq!(receipt.prompt_sha256.len(), 64);
        assert!(receipt.prompt_bytes > 0);
        // The materialized prompt reached the fake harness as argv, content only.
        assert!(receipt.stdout_excerpt.contains("HARNESS EVOLUTION REPLAY PROBE"));
        assert!(receipt.stdout_excerpt.contains("GOAL:"));
        assert!(!receipt.stdout_excerpt.contains("authority"));
    }

    #[test]
    fn a_backend_that_reports_usage_has_it_captured_and_priced() {
        let fixture = Fixture::new();
        let marker = format!(
            "{} {{\"input_tokens\":11,\"output_tokens\":7,\"cost_usd\":0.000123}}",
            USAGE_MARKER_PREFIX
        );
        let script = format!("printf '%s\\n' '{marker}'");
        let sandbox = fixture.sandbox(command_backend("/bin/sh", vec!["-c", script.as_str()]));
        let reading = run(&sandbox, &fixture, "probe-usage").expect("reading");
        assert_eq!(reading.behavior.tokens_in, 11);
        assert_eq!(reading.behavior.tokens_out, 7);
        assert_eq!(reading.behavior.cost_micros, Some(123));
        assert!(reading
            .behavior
            .evidence_refs
            .iter()
            .any(|reference| reference == USAGE_MEASURED_REF));
        let receipt = receipt_for(&reading);
        assert_eq!(receipt.usage_source, "command_marker");
        assert_eq!(receipt.input_tokens, Some(11));
        assert_eq!(receipt.cost_micros, Some(123));
    }

    #[test]
    fn a_nonzero_exit_is_failed_behavior_and_a_timeout_is_a_failed_probe() {
        let fixture = Fixture::new();
        let failing = fixture.sandbox(command_backend("/usr/bin/false", vec![]));
        let reading = run(&failing, &fixture, "probe-fail").expect("reading");
        assert!(!reading.behavior.success);
        assert_eq!(reading.behavior.qa, QaVerdict::Fail);

        // `sh -c 'sleep 5' <prompt>` treats the appended prompt as $0, so it still sleeps.
        let timing_out = fixture
            .sandbox(command_backend("/bin/sh", vec!["-c", "sleep 5"]))
            .with_timeout(Duration::from_millis(100));
        let error = run(&timing_out, &fixture, "probe-timeout").unwrap_err();
        assert_eq!(error.code(), "replay_sandbox_failed");
        assert!(error.to_string().contains("timed out"));
        assert!(error.to_string().contains("process group killed"));
        let message = error.to_string();
        let path = message.split("receipt ").nth(1).unwrap().trim();
        let body = std::fs::read(path).expect("timeout receipt");
        let receipt: ModelTurnReceipt = serde_json::from_slice(&body).expect("json");
        assert!(receipt.timed_out);
        assert_eq!(receipt.exit_code, None);
    }

    #[test]
    fn production_is_refused_before_any_workspace_or_receipt() {
        let fixture = Fixture::new();
        let sandbox = fixture.sandbox(command_backend("/bin/echo", vec!["x"]));
        let error = sandbox
            .run(&request(
                "probe-prod",
                EnvironmentClass::Production,
                &fixture.task,
                &fixture.harness,
            ))
            .unwrap_err();
        assert_eq!(error.code(), "replay_production_refused");
        assert_eq!(
            std::fs::read_dir(fixture.run_root.path())
                .map(|entries| entries.count())
                .unwrap_or(0),
            0
        );
    }

    #[test]
    fn an_authorization_for_another_environment_is_refused() {
        let fixture = Fixture::new();
        let sandbox = fixture.sandbox(command_backend("/bin/echo", vec!["x"]));
        let error = sandbox
            .run(&request(
                "probe-ci",
                EnvironmentClass::Ci,
                &fixture.task,
                &fixture.harness,
            ))
            .unwrap_err();
        assert_eq!(error.code(), "replay_sandbox_refused");
        assert!(error.to_string().contains("authorization covers"));
    }

    #[test]
    fn the_prompt_is_materialized_from_content_only() {
        let content = "reasoning guidance text";
        let green = HarnessRevision::draft(
            "rev-green",
            HarnessDomain::RustBackend,
            HarnessManifest::from_components([Component::green(
                ComponentKind::SystemPrompt,
                content,
                "prompt",
            )]),
            "owner",
        );
        let red = HarnessRevision::draft(
            "rev-red",
            HarnessDomain::RustBackend,
            HarnessManifest::from_components([Component::red(
                ComponentKind::SystemPrompt,
                content,
                "prompt",
            )]),
            "owner",
        );
        let task = task();
        let green_harness = ReplayHarness::materialize(&green).unwrap();
        let red_harness = ReplayHarness::materialize(&red).unwrap();
        let green_prompt = materialize_prompt(&green_harness, &task);
        let red_prompt = materialize_prompt(&red_harness, &task);
        // The header names the (different) immutable revisions, so compare the payload: same
        // content, different declared authority means the sandbox receives the same content.
        let payload = |prompt: &str| prompt.split("--- component:").nth(1).unwrap_or("").to_string();
        assert_eq!(payload(&green_prompt), payload(&red_prompt));
        assert!(green_prompt.contains(content));
        assert!(green_prompt.contains("GOAL:"));
        assert!(!payload(&green_prompt).contains("authority"));
        assert!(!payload(&green_prompt).contains("rev-green"));
        assert!(!payload(&green_prompt).contains("rev-red"));
    }

    #[test]
    fn an_oversize_prompt_is_refused_not_truncated() {
        let fixture = Fixture::new();
        let sandbox = fixture
            .sandbox(command_backend("/bin/echo", vec!["x"]))
            .with_prompt_cap(8);
        let error = run(&sandbox, &fixture, "probe-prompt").unwrap_err();
        assert_eq!(error.code(), "replay_sandbox_refused");
        assert!(error.to_string().contains("above the 8 byte cap"));
    }

    #[test]
    fn output_is_streamed_and_bounded_with_a_marker() {
        let fixture = Fixture::new();
        let sandbox = fixture
            .sandbox(command_backend("/bin/echo", vec!["0123456789abcdef"]))
            .with_output_cap(16);
        let reading = run(&sandbox, &fixture, "probe-cap").expect("reading");
        let receipt = receipt_for(&reading);
        assert!(receipt.stdout_truncated);
        assert!(receipt.stdout_excerpt.contains("[truncated at 16 bytes of "));
        assert!(receipt.stdout_bytes > 16);
    }

    #[test]
    fn the_receipt_round_trips_and_refs_are_distinct() {
        let fixture = Fixture::new();
        let sandbox = fixture.sandbox(command_backend("/bin/echo", vec!["x"]));
        let first = run(&sandbox, &fixture, "probe-ref").expect("first");
        let second = run(&sandbox, &fixture, "probe-ref").expect("second");
        assert_ne!(first.behavior.evidence_id, second.behavior.evidence_id);
        let receipt = receipt_for(&first);
        let encoded = serde_json::to_string(&receipt).unwrap();
        let decoded: ModelTurnReceipt = serde_json::from_str(&encoded).unwrap();
        assert_eq!(receipt, decoded);
        assert!(receipt.execution_id.starts_with(EXEC_REF_PREFIX));
    }
}
