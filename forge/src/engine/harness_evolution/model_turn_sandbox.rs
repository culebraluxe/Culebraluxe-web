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
//! * stdout/stderr are streamed through a fixed cap, the readers are awaited with a deadline, and
//!   the receipt records truncation flags, true totals and whether secret-shaped text was redacted;
//! * the process group is killed on timeout **and on normal exit**, so a helper holding the pipes
//!   cannot hang the probe; the kill is setsid-safe (never pid 0/1 or our own group);
//! * receipts live under a 0700 run root, are written 0600, hold no prompt content, and are pruned
//!   by age/count. A DSH session transcript is **not** persisted: only the opaque session id plus a
//!   [`SESSION_TRANSCRIPT_NOTE_REF`] note are recorded.
//!
//! The backend selector is `FORGE_HARNESS`: `dsh` (default) or `opencode`. An unknown value fails
//! closed; a selected backend that is not installed fails closed before any spend.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::engine::dsh;
use crate::engine::opencode_client;

use super::model::{BehaviorEvidence, EnvironmentClass, QaVerdict};
use super::probe_replay::{
    assert_non_production_environment, HistoricalTask, ReplayError, ReplayHarness, ReplaySandbox,
    SandboxReading, SandboxRequest,
};
use super::sandbox_command::{
    await_stream_capture, cleanup_receipts, create_secure_dir, redact_secrets, spawn_stream_capture,
    terminate_process_group, write_secure_file, CAPTURE_JOIN_GRACE, RECEIPT_MAX_FILES,
    RECEIPT_RETENTION,
};

/// Environment variable selecting the headless backend (`dsh` or `opencode`).
pub const FORGE_HARNESS_ENV: &str = "FORGE_HARNESS";

/// The default backend name.
pub const DEFAULT_HARNESS_BACKEND: &str = "dsh";

/// Default wall-clock budget for one model turn.
pub const DEFAULT_MODEL_TURN_TIMEOUT: Duration = Duration::from_secs(300);

/// The longest timeout this sandbox accepts for one model turn.
pub const MAX_MODEL_TURN_TIMEOUT: Duration = Duration::from_secs(1800);

/// The longest lifetime a model-run authorization token may have.
pub const MAX_MODEL_RUN_TTL: Duration = Duration::from_secs(3600);

/// What a model-run authorization is bound to: the backend, the exact model pin, the historical
/// task, the experiment (probe-id prefix) and an expiry. A token minted for one probe cannot be
/// replayed against another model/task/experiment, and it cannot outlive its expiry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelTurnBinding {
    /// Backend id (`dsh` or `opencode`).
    pub backend: String,
    /// The validated `provider/model` id the turn must run under.
    pub model: String,
    /// The historical task identity (`HistoricalTask.story_id`).
    pub task: String,
    /// The experiment id; probe ids must be `<experiment>-reference|candidate` or similar prefixed.
    pub experiment_id: String,
    /// Wall-clock instant after which the token is invalid.
    pub expires_at: SystemTime,
}

impl ModelTurnBinding {
    /// Bind a real model-run token. `ttl` must be positive and at most [`MAX_MODEL_RUN_TTL`].
    pub fn new(
        backend: &ModelTurnBackend,
        model: &str,
        task: &str,
        experiment_id: &str,
        ttl: Duration,
    ) -> Result<Self, ReplayError> {
        if ttl.is_zero() || ttl > MAX_MODEL_RUN_TTL {
            return Err(refused(format!(
                "model-run token lifetime {ttl:?} is outside (0, {MAX_MODEL_RUN_TTL:?}]"
            )));
        }
        let model = dsh::args::validate_model(model)
            .map_err(|error| refused(format!("model pin refused: {error}")))?;
        let task = task.trim();
        let experiment_id = experiment_id.trim();
        if task.is_empty() || experiment_id.is_empty() {
            return Err(refused(
                "model-run token needs a non-empty task and experiment id",
            ));
        }
        if task.chars().any(char::is_control) || experiment_id.chars().any(char::is_control) {
            return Err(refused(
                "task/experiment id contains control characters; refusing to bind them",
            ));
        }
        Ok(Self {
            backend: backend.id().to_string(),
            model,
            task: task.to_string(),
            experiment_id: experiment_id.to_string(),
            expires_at: SystemTime::now() + ttl,
        })
    }

    pub fn is_expired_at(&self, now: SystemTime) -> bool {
        now >= self.expires_at
    }

    /// Check a request against this binding. Every mismatch is a typed refusal, not a downgrade.
    pub fn check(
        &self,
        backend: &ModelTurnBackend,
        model: &str,
        task: &str,
        probe_id: &str,
        now: SystemTime,
    ) -> Result<(), ReplayError> {
        if self.backend != backend.id() {
            return Err(refused(format!(
                "model-run token is bound to backend {}, but the sandbox is {}",
                self.backend,
                backend.id()
            )));
        }
        if self.model != model.trim() {
            return Err(refused(format!(
                "model-run token is bound to model {:?}, but the probe requests {:?}",
                self.model,
                model.trim()
            )));
        }
        if self.task != task.trim() {
            return Err(refused(format!(
                "model-run token is bound to task {:?}, but the probe runs {:?}",
                self.task,
                task.trim()
            )));
        }
        let experiment_prefix = format!("{}-", self.experiment_id);
        if !probe_id.starts_with(&experiment_prefix) {
            return Err(refused(format!(
                "model-run token is bound to experiment {:?}, but probe {:?} is outside it",
                self.experiment_id, probe_id
            )));
        }
        if self.is_expired_at(now) {
            return Err(refused(
                "model-run token has expired; mint a fresh authorization for this run",
            ));
        }
        Ok(())
    }
}

/// How much of stdout/stderr is retained per stream in the receipt.
pub const DEFAULT_MODEL_TURN_OUTPUT_CAP: usize = 64 * 1024;

/// The largest materialized prompt this sandbox will send. Oversize prompts are refused, not
/// truncated: silently shortening the candidate harness would change the probe's identity.
pub const DEFAULT_PROMPT_CAP_BYTES: usize = 256 * 1024;

/// Ref prefix for one model-turn execution.
pub const EXEC_REF_PREFIX: &str = "model-turn:exec:";

/// Evidence ref that states the DSH session transcript content/paths were not persisted.
///
/// The receipt keeps the opaque vendor session id (for correlation) but never the transcript body
/// or its filesystem path: a transcript can contain prompts, tool output and secrets.
pub const SESSION_TRANSCRIPT_NOTE_REF: &str = "model-turn:session:transcript-not-persisted";
/// Ref prefix for the JSON receipt.
pub const RECEIPT_REF_PREFIX: &str = "model-turn:receipt:";
/// Ref prefix for the backend that ran.
pub const BACKEND_REF_PREFIX: &str = "model-turn:backend:";

/// Ref prefix for the vendor version the turn ran under (FORGE-02 pin evidence).
pub const VENDOR_VERSION_REF_PREFIX: &str = "model-turn:vendor-version:";
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
    /// Fail closed before a turn and return the vendor version the backend reported, when the
    /// backend has one. DSH is pinned (`DSH_EXPECTED_VERSION`) and the actual version travels with
    /// it so the model-turn receipt and probe evidence can name the vendor build.
    pub fn ensure_available(&self) -> Result<Option<String>, ReplayError> {
        match self {
            Self::Dsh => {
                let cli = dsh::default_cli()
                    .map_err(|error| refused(format!("DSH is unavailable: {error}")))?;
                let version = dsh::preflight::preflight_with_version(&cli)
                    .map_err(|error| refused(format!("DSH preflight failed: {error}")))?;
                Ok(Some(version.actual))
            }
            Self::OpenCode => {
                let bin = crate::engine::opencode::default_cli_bin();
                opencode_client::verify_vendor_contract(&bin)
                    .map(|_| None)
                    .map_err(|error| refused(format!("OpenCode is unavailable: {error}")))
            }
            #[cfg(test)]
            Self::Command { program, .. } => {
                if program.is_absolute() {
                    Ok(None)
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
    /// Actual vendor version captured by the fail-closed preflight (`None` for backends without a
    /// versioned contract, e.g. the test command backend).
    vendor_version: Option<String>,
    /// What the token is bound to. A real backend requires `Some`; only the test command backend
    /// may mint an unbound token.
    binding: Option<ModelTurnBinding>,
}

impl ModelTurnAuthorization {
    /// The script/CLI flag that expresses explicit model-run authorization.
    pub const FLAG: &'static str = "--authorize-model-run";

    /// Mint a **bound** token for a real model turn. Callers must supply the exact model pin, task
    /// identity, experiment id and a short lifetime; the sandbox refuses a request that does not
    /// match every one of them.
    pub fn authorize_model_run_bound(
        backend: ModelTurnBackend,
        environment_class: EnvironmentClass,
        binding: ModelTurnBinding,
    ) -> Result<Self, ReplayError> {
        if environment_class.is_production() || !environment_class.allows_probe() {
            return Err(ReplayError::ProductionRefused { environment_class });
        }
        if binding.backend != backend.id() {
            return Err(refused(format!(
                "model-run token is bound to backend {}, but this authorization is for {}",
                binding.backend,
                backend.id()
            )));
        }
        if binding.is_expired_at(SystemTime::now()) {
            return Err(refused(
                "model-run token expired before it was minted; refusing",
            ));
        }
        let vendor_version = backend.ensure_available()?;
        Ok(Self {
            backend,
            environment_class,
            vendor_version,
            binding: Some(binding),
        })
    }

    /// Unbound authorization for the deterministic test command backend only. A real backend is
    /// refused here: an unbound token could be replayed against any model, task or experiment.
    pub fn authorize_model_run(
        backend: ModelTurnBackend,
        environment_class: EnvironmentClass,
    ) -> Result<Self, ReplayError> {
        if environment_class.is_production() || !environment_class.allows_probe() {
            return Err(ReplayError::ProductionRefused { environment_class });
        }
        #[cfg(test)]
        {
            if matches!(backend, ModelTurnBackend::Command { .. }) {
                let vendor_version = backend.ensure_available()?;
                return Ok(Self {
                    backend,
                    environment_class,
                    vendor_version,
                    binding: None,
                });
            }
        }
        Err(refused(format!(
            "backend {} requires a bound model-run token (model/task/experiment/expiry); use ModelTurnAuthorization::authorize_model_run_bound",
            backend.id()
        )))
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

    /// The vendor version the preflight verified, when the backend reports one.
    pub fn vendor_version(&self) -> Option<&str> {
        self.vendor_version.as_deref()
    }

    /// The token binding, when this is a real (bound) model-run authorization.
    pub fn binding(&self) -> Option<&ModelTurnBinding> {
        self.binding.as_ref()
    }

    /// Enforce the token binding against a request. A real backend with no binding is refused; only
    /// the test command backend may run unbound.
    fn check_binding(
        &self,
        request: &SandboxRequest<'_>,
        now: SystemTime,
    ) -> Result<(), ReplayError> {
        match &self.binding {
            Some(binding) => binding.check(
                &self.backend,
                request.model,
                &request.task.story_id,
                request.probe_id,
                now,
            ),
            None => {
                #[cfg(test)]
                {
                    if matches!(self.backend, ModelTurnBackend::Command { .. }) {
                        return Ok(());
                    }
                }
                Err(refused(
                    "unbound model-run token: a real model turn requires model/task/experiment/expiry binding",
                ))
            }
        }
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
    let payload = line
        .trim_start()
        .trim_start_matches(USAGE_MARKER_PREFIX)
        .trim();
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

/// The evidence ref that records a vendor version, when one was reported. Kept as a pure helper so
/// the receipt field and the probe evidence ref cannot disagree.
fn vendor_version_ref(version: Option<&str>) -> Option<String> {
    version.map(|version| format!("{VENDOR_VERSION_REF_PREFIX}{version}"))
}

/// One recorded model turn: the only durable artifact this sandbox writes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModelTurnReceipt {
    pub execution_id: String,
    pub probe_id: String,
    pub revision: String,
    pub manifest_hash: String,
    pub backend: String,
    /// The vendor version the turn ran under, when the backend reports one (FORGE-02).
    #[serde(default)]
    pub vendor_version: Option<String>,
    /// The ProbeSpec identity fields the sandbox saw, recorded honestly even when not enforceable.
    #[serde(default)]
    pub model_policy: String,
    #[serde(default)]
    pub executor: String,
    #[serde(default)]
    pub task_id: String,
    #[serde(default)]
    pub experiment_id: String,
    #[serde(default)]
    pub test_plan: String,
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
    /// True when secret-shaped text was redacted before this receipt was written.
    #[serde(default)]
    pub stdout_redacted: bool,
    #[serde(default)]
    pub stderr_redacted: bool,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    /// `None` = the harness reported no usage (never zero).
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    /// `None` = unpriced (no rates configured), never zero.
    pub cost_micros: Option<u64>,
    pub usage_source: String,
    pub session_id: Option<String>,
    /// Always explains the session evidence policy; the transcript is never persisted.
    #[serde(default)]
    pub session_note: String,
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

    /// Refuse ProbeSpec controls this sandbox does not actually apply. Recording an unenforced pin
    /// as if it were applied would make the probe evidence claim an identity the execution never had.
    fn enforce_identity(&self, request: &SandboxRequest<'_>) -> Result<(), ReplayError> {
        match &self.authorization.backend {
            ModelTurnBackend::Dsh | ModelTurnBackend::OpenCode => validate_identity_fields(
                self.authorization.backend.id(),
                request.executor,
                request.tools,
                request.model_policy,
                request.model,
            ),
            #[cfg(test)]
            ModelTurnBackend::Command { .. } => Ok(()),
        }
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
        // The token is bound to backend+model+task+experiment+expiry, and ProbeSpec fields the
        // model-turn sandbox cannot enforce are refused before a workspace or a spawn.
        self.authorization
            .check_binding(request, SystemTime::now())?;
        self.enforce_identity(request)?;

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
        create_secure_dir(&workspace).map_err(|error| ReplayError::SandboxFailed {
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
        let mut child = command
            .spawn()
            .map_err(|error| ReplayError::SandboxFailed {
                probe_id: request.probe_id.to_string(),
                message: format!(
                    "cannot spawn {} ({}) for the model turn: {error}",
                    invocation.program,
                    self.authorization.backend.id()
                ),
            })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ReplayError::SandboxFailed {
                probe_id: request.probe_id.to_string(),
                message: "model-turn stdout pipe missing".into(),
            })?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ReplayError::SandboxFailed {
                probe_id: request.probe_id.to_string(),
                message: "model-turn stderr pipe missing".into(),
            })?;
        let output_cap = self.output_cap;
        let stdout_rx = spawn_stream_capture(stdout, output_cap);
        let stderr_rx = spawn_stream_capture(stderr, output_cap);

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
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    terminate_process_group(&mut child);
                    let deadline = Instant::now() + CAPTURE_JOIN_GRACE;
                    let _ = await_stream_capture(
                        stdout_rx,
                        deadline,
                        request.probe_id,
                        "model-turn stdout",
                    );
                    let _ = await_stream_capture(
                        stderr_rx,
                        deadline,
                        request.probe_id,
                        "model-turn stderr",
                    );
                    return Err(ReplayError::SandboxFailed {
                        probe_id: request.probe_id.to_string(),
                        message: format!("cannot observe the model-turn process: {error}"),
                    });
                }
            }
        };
        let duration = started.elapsed();

        // Kill the group even on a NORMAL exit: a vendor helper that inherited stdout can hold the
        // pipes open and hang the join forever. The negative pid addresses remaining group members.
        terminate_process_group(&mut child);
        let deadline = Instant::now() + CAPTURE_JOIN_GRACE;
        let stdout_capture =
            await_stream_capture(stdout_rx, deadline, request.probe_id, "model-turn stdout")?;
        let stderr_capture =
            await_stream_capture(stderr_rx, deadline, request.probe_id, "model-turn stderr")?;

        // The model patch overlay is only needed while the child runs.
        drop(invocation.patch);

        // Usage is read from the harness's OWN record; absence is UNMEASURED, never zero. The
        // session may live under the real DSH home or the isolated workspace home depending on how
        // the vendor resolves its home, so discovery scans both.
        let session_roots = dsh_session_roots(&workspace);
        let discovered = discover_session_across(&session_roots);
        let session = discovered.as_ref().map(|(name, _)| name.clone());
        let usage = self.capture_usage(
            &stdout_capture.text,
            discovered.as_ref().map(|(_, path)| path.as_path()),
        );

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
            vendor_version: self.authorization.vendor_version.clone(),
            model_policy: request.model_policy.to_string(),
            executor: request.executor.to_string(),
            task_id: request.task.story_id.clone(),
            experiment_id: self
                .authorization
                .binding
                .as_ref()
                .map(|binding| binding.experiment_id.clone())
                .unwrap_or_default(),
            test_plan: request.test_plan.to_string(),
            authorized_via: ModelTurnAuthorization::FLAG.to_string(),
            environment_class: request.environment_class.id().to_string(),
            model: request.model.to_string(),
            model_pin_applied: invocation.model_pin_applied,
            timeout_ms: u64::try_from(self.timeout.as_millis()).unwrap_or(u64::MAX),
            duration_ms: u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
            exit_code: exit_status.and_then(|status| status.code()),
            timed_out,
            stdout_excerpt: redact_secrets(&stdout_capture.text),
            stderr_excerpt: redact_secrets(&stderr_capture.text),
            stdout_truncated: stdout_capture.truncated,
            stderr_truncated: stderr_capture.truncated,
            stdout_redacted: redact_secrets(&stdout_capture.text) != stdout_capture.text,
            stderr_redacted: redact_secrets(&stderr_capture.text) != stderr_capture.text,
            stdout_bytes: stdout_capture.total_bytes,
            stderr_bytes: stderr_capture.total_bytes,
            input_tokens: usage.as_ref().map(|usage| usage.input_tokens),
            output_tokens: usage.as_ref().map(|usage| usage.output_tokens),
            cost_micros: usage.as_ref().and_then(|usage| usage.cost_micros),
            usage_source: usage.as_ref().map_or_else(
                || "unmeasured".to_string(),
                |usage| usage.source.to_string(),
            ),
            session_id: session,
            session_note: SESSION_TRANSCRIPT_NOTE_REF.to_string(),
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
        // The transcript itself is never persisted, so the note travels with the opaque id.
        evidence_refs.push(SESSION_TRANSCRIPT_NOTE_REF.to_string());
        if let Some(version_ref) = vendor_version_ref(receipt.vendor_version.as_deref()) {
            evidence_refs.push(version_ref);
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
                // The ProbeSpec model is required and must be applicable — a pin the sandbox cannot
                // apply is refused rather than silently falling back to the profile default.
                let model = dsh::args::validate_model(request.model).map_err(|error| {
                    refused(format!(
                        "DSH model pin refused ({}): {error}",
                        request.model
                    ))
                })?;
                let patch = Some(SandboxPatchFile::create(workspace, &model)?);
                let model_pin_applied = true;
                let args = dsh::args::headless_args(
                    dsh::DSH_PROFILE,
                    patch.as_ref().map(SandboxPatchFile::path),
                    prompt,
                );
                // Read-only where the backend supports it: DSH's permission mode.
                env.push(("DSH_PERMISSION_MODE".into(), "read-only".into()));
                // The child needs the vendor's real DSH home (profiles/credentials) to boot; the
                // workspace stays the cwd/HOME for the turn's own files.
                let dsh_home = dsh_home_path(workspace);
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
                let model = dsh::args::validate_model(request.model).map_err(|error| {
                    refused(format!(
                        "OpenCode model pin refused ({}): {error}",
                        request.model
                    ))
                })?;
                let args = opencode_client::build_opencode_run_args(
                    &model, prompt, false, None, false, None,
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
    fn capture_usage(&self, _stdout: &str, session_dir: Option<&Path>) -> Option<CapturedUsage> {
        match &self.authorization.backend {
            ModelTurnBackend::Dsh => {
                let session_dir = session_dir?;
                let totals = dsh::usage::read_token_totals(session_dir)?;
                let cost_micros = dsh::usage::rates_from_env().and_then(|rates| {
                    let usd = totals.cost_usd(&rates);
                    usd.is_finite().then(|| (usd * 1_000_000.0).round() as u64)
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

/// The DSH overlay the sandbox writes for one turn: the model pin only.
///
/// `dsh-base` already mounts the DeepSeek adapter (`@deepseek-ai/dsh-llm-deepseek`, provider route
/// `deepseek-official`); the pin must therefore name THAT provider, not a bare `deepseek/...` (which
/// fails `NO_ADAPTER` before a session log exists). Inserting the adapter here again would be a
/// `DUPLICATE_ADAPTER`. The file deletes itself when the turn ends.
struct SandboxPatchFile {
    path: PathBuf,
}

impl SandboxPatchFile {
    fn create(workspace: &Path, model: &str) -> Result<Self, ReplayError> {
        let path = workspace.join("harness-model-patch.yml");
        let content = dsh_patch_content(model)?;
        std::fs::write(&path, content).map_err(|error| {
            refused(format!(
                "cannot write DSH model overlay {}: {error}",
                path.display()
            ))
        })?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for SandboxPatchFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// The exact overlay content: the model pin, using the vendor's `provider/model` ids. Pure, so the
/// contract is tested without spawning a model turn.
fn dsh_patch_content(model: &str) -> Result<String, ReplayError> {
    dsh::args::model_patch(model)
        .map_err(|error| refused(format!("DSH model overlay refused: {error}")))
}

/// The ProbeSpec controls a real model turn cannot enforce. `tools` would claim a tool surface the
/// sandbox did not grant, `executor` must name the backend that actually runs, and a non-default
/// `model_policy` would claim a tier selection that is not applied; the model id must be the exact
/// validated pin. `test_plan` is a Forge-validation plan, recorded but not executed here.
fn validate_identity_fields(
    expected_executor: &str,
    executor: &str,
    tools: &[String],
    model_policy: &str,
    model: &str,
) -> Result<(), ReplayError> {
    if executor != expected_executor {
        return Err(refused(format!(
            "model-turn sandbox executes as {expected_executor:?}, but the probe spec pins executor {executor:?}"
        )));
    }
    if !tools.is_empty() {
        return Err(refused(format!(
            "model-turn sandbox does not enforce tools, but the probe spec pins {} tool(s); refusing an unenforceable pin",
            tools.len()
        )));
    }
    if !model_policy.is_empty() && model_policy != "default" {
        return Err(refused(format!(
            "model-turn sandbox does not resolve model_policy {model_policy:?}; use \"default\" or refuse the probe"
        )));
    }
    dsh::args::validate_model(model)
        .map_err(|error| refused(format!("model pin is not applicable: {error}")))?;
    Ok(())
}

struct TurnInvocation {
    program: String,
    prefix: Vec<String>,
    args: Vec<String>,
    env: Vec<(String, String)>,
    patch: Option<SandboxPatchFile>,
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
        out.push_str(&format!(
            "\n--- component: {} ---\n{}\n",
            kind.id(),
            content
        ));
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

/// Where this child would write DSH sessions: the explicit `DSH_HOME`, else `$HOME/.dsh` where
/// `HOME` is the throwaway workspace.
/// Resolve the DSH home the CHILD should use. The isolated workspace home alone is not a working
/// DSH home: profiles, bundles and `.credentials.yaml` live in the user's real DSH home, so a
/// sanitized workspace-only home made the headless turn unable to boot (and therefore wrote no
/// session log). `$DSH_HOME` wins, then `$HOME/.dsh`, then the workspace `.dsh`. Pure, so the
/// resolution is testable without spawning anything.
fn resolve_dsh_home(
    workspace: &Path,
    env_dsh_home: Option<&std::ffi::OsStr>,
    env_home: Option<&std::ffi::OsStr>,
) -> PathBuf {
    if let Some(explicit) = env_dsh_home.filter(|value| !value.is_empty()) {
        return PathBuf::from(explicit);
    }
    if let Some(home) = env_home.filter(|value| !value.is_empty()) {
        return Path::new(home).join(".dsh");
    }
    workspace.join(".dsh")
}

fn dsh_home_path(workspace: &Path) -> PathBuf {
    resolve_dsh_home(
        workspace,
        std::env::var_os("DSH_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
}

/// Every session root DSH could have written under for this workspace, most likely first: the
/// resolved DSH home, the isolated workspace home, and the real user home. Discovery scans all of
/// them so a vendor that honours `HOME` rather than `DSH_HOME` (or vice versa) is still captured.
fn dsh_session_roots(workspace: &Path) -> Vec<PathBuf> {
    let mut homes: Vec<PathBuf> = Vec::new();
    for home in [
        std::env::var_os("DSH_HOME").map(PathBuf::from),
        std::env::var_os("HOME").map(|home| Path::new(&home).join(".dsh")),
        Some(workspace.join(".dsh")),
    ]
    .into_iter()
    .flatten()
    {
        if !homes.contains(&home) {
            homes.push(home);
        }
    }
    session_roots_for(workspace, &homes)
}

/// Session roots for one workspace across every candidate DSH home. The workspace is included both
/// as given and as its canonical path: the vendor stores sessions under the realpath project key
/// (on macOS a temp `/var/...` workspace is persisted under `--private-var-...--`), so a discovery
/// that only hashed the uncanonicalised path missed the log and recorded usage as unmeasured.
fn session_roots_for(workspace: &Path, homes: &[PathBuf]) -> Vec<PathBuf> {
    let mut workspaces = vec![workspace.to_path_buf()];
    if let Ok(canonical) = std::fs::canonicalize(workspace) {
        if canonical != workspace {
            workspaces.push(canonical);
        }
    }
    let mut roots: Vec<PathBuf> = Vec::new();
    for home in homes {
        for candidate in &workspaces {
            let root = dsh::args::session_root(candidate, home);
            if !roots.contains(&root) {
                roots.push(root);
            }
        }
    }
    roots
}

/// The newest session directory across all candidate roots, as `(session_name, session_dir)`.
/// Pure apart from the directory reads.
fn discover_session_across(roots: &[PathBuf]) -> Option<(String, PathBuf)> {
    let mut newest: Option<(std::time::SystemTime, String, PathBuf)> = None;
    for root in roots {
        let Some(session) = dsh::args::discover_latest_session(root) else {
            continue;
        };
        let path = root.join(&session);
        let modified = std::fs::metadata(&path)
            .and_then(|meta| meta.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        if newest.as_ref().is_none_or(|(at, _, _)| modified > *at) {
            newest = Some((modified, session, path));
        }
    }
    newest.map(|(_, session, path)| (session, path))
}

fn write_receipt(run_root: &Path, receipt: &ModelTurnReceipt) -> Result<PathBuf, ReplayError> {
    create_secure_dir(run_root).map_err(|error| ReplayError::SandboxFailed {
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
    write_secure_file(&path, &body).map_err(|error| ReplayError::SandboxFailed {
        probe_id: receipt.probe_id.clone(),
        message: format!(
            "cannot write model-turn receipt {}: {error}",
            path.display()
        ),
    })?;
    let _ = cleanup_receipts(run_root, RECEIPT_RETENTION, RECEIPT_MAX_FILES);
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
        assert_eq!(
            authorized.environment_class(),
            EnvironmentClass::Development
        );
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
        assert_eq!(
            reading.behavior.tokens_in, 0,
            "unmeasured is labelled below"
        );
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
        assert!(receipt
            .stdout_excerpt
            .contains("HARNESS EVOLUTION REPLAY PROBE"));
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
        let payload = |prompt: &str| {
            prompt
                .split("--- component:")
                .nth(1)
                .unwrap_or("")
                .to_string()
        };
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
        assert!(receipt
            .stdout_excerpt
            .contains("[truncated at 16 bytes of "));
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

    #[test]
    fn the_dsh_overlay_pins_the_vendor_provider_and_never_reinstalls_the_adapter() {
        let content = dsh_patch_content("deepseek-official/deepseek-v4-flash").expect("overlay");
        assert!(content.contains("provider: deepseek-official"), "{content}");
        assert!(content.contains("model: deepseek-v4-flash"), "{content}");
        assert!(
            !content.contains("llm-deepseek"),
            "the base bundle already mounts the adapter; re-inserting it is DUPLICATE_ADAPTER"
        );
    }

    #[test]
    fn dsh_home_resolution_prefers_explicit_then_home_then_workspace() {
        use std::ffi::OsStr;
        let workspace = std::path::Path::new("/tmp/sandbox-workspace");
        assert_eq!(
            resolve_dsh_home(
                workspace,
                Some(OsStr::new("/explicit/dsh")),
                Some(OsStr::new("/home/u"))
            ),
            std::path::PathBuf::from("/explicit/dsh")
        );
        assert_eq!(
            resolve_dsh_home(workspace, None, Some(OsStr::new("/home/u"))),
            std::path::PathBuf::from("/home/u/.dsh")
        );
        assert_eq!(
            resolve_dsh_home(workspace, Some(OsStr::new("")), Some(OsStr::new(""))),
            std::path::PathBuf::from("/tmp/sandbox-workspace/.dsh")
        );
    }

    #[test]
    fn session_roots_include_the_canonical_workspace_variant() {
        let root =
            std::env::temp_dir().join(format!("forge-dsh-canonical-test-{}", std::process::id()));
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let homes = vec![root.join("dsh-home")];
        let roots = session_roots_for(&workspace, &homes);
        let canonical = std::fs::canonicalize(&workspace).expect("canonical");
        assert!(
            roots.contains(&dsh::args::session_root(&canonical, &homes[0])),
            "the vendor stores sessions under the realpath project key: {roots:?}"
        );
        assert!(roots.contains(&dsh::args::session_root(&workspace, &homes[0])));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn session_discovery_scans_every_candidate_root_and_picks_the_newest() {
        let root =
            std::env::temp_dir().join(format!("forge-dsh-discovery-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let first_root = root.join("real-home/sessions/project");
        let second_root = root.join("workspace/sessions/project");
        std::fs::create_dir_all(&first_root).unwrap();
        std::fs::create_dir_all(&second_root).unwrap();
        std::fs::create_dir_all(first_root.join("session-old")).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::create_dir_all(second_root.join("session-new")).unwrap();
        std::fs::write(second_root.join("session-new").join("log"), "").unwrap();
        let found = discover_session_across(&[first_root, second_root]).expect("newest session");
        assert_eq!(found.0, "session-new");
        assert_eq!(found.1, root.join("workspace/sessions/project/session-new"));
        // No sessions anywhere: UNMEASURED, never a fabricated id.
        assert_eq!(discover_session_across(&[root.join("empty")]), None);
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn the_vendor_version_is_recorded_as_probe_evidence() {
        assert_eq!(
            vendor_version_ref(Some("0.1.1-rc.2")).as_deref(),
            Some("model-turn:vendor-version:0.1.1-rc.2")
        );
        assert_eq!(vendor_version_ref(None), None);
    }
    #[test]
    fn model_run_tokens_are_bound_to_backend_model_task_experiment_and_expiry() {
        let backend = command_backend("/bin/echo", vec!["fake-turn"]);
        let binding = ModelTurnBinding::new(
            &backend,
            "deepseek/deepseek-chat",
            "story-1",
            "exp-1",
            Duration::from_secs(60),
        )
        .expect("binding");
        let now = SystemTime::now();
        binding
            .check(
                &backend,
                "deepseek/deepseek-chat",
                "story-1",
                "exp-1-reference",
                now,
            )
            .expect("matching request");

        assert!(
            binding
                .check(
                    &backend,
                    "deepseek-chat",
                    "story-1",
                    "exp-1-reference",
                    now + Duration::from_secs(61)
                )
                .is_err(),
            "an expired token must be refused"
        );
        assert!(
            binding.is_expired_at(now + Duration::from_secs(61)),
            "expiry is a wall-clock comparison"
        );
        for probe in ["exp-2-reference", "exp-10-candidate", "probe-1"] {
            assert!(
                binding
                    .check(&backend, "deepseek-chat", "story-1", probe, now)
                    .is_err(),
                "{probe} is outside the bound experiment"
            );
        }
        assert!(
            binding
                .check(&backend, "other-model", "story-1", "exp-1-reference", now)
                .is_err(),
            "a different model must be refused"
        );
        assert!(
            binding
                .check(
                    &backend,
                    "deepseek/deepseek-chat",
                    "story-2",
                    "exp-1-reference",
                    now
                )
                .is_err(),
            "a different task must be refused"
        );
        assert!(
            binding
                .check(
                    &ModelTurnBackend::OpenCode,
                    "deepseek-chat",
                    "story-1",
                    "exp-1-reference",
                    now
                )
                .is_err(),
            "a different backend must be refused"
        );
    }

    #[test]
    fn model_run_binding_refuses_bad_ids_and_lifetimes_and_real_backends_need_a_binding() {
        let backend = command_backend("/bin/echo", vec!["fake-turn"]);
        assert!(
            ModelTurnBinding::new(
                &backend,
                "provider/model\n- id: other",
                "story",
                "exp",
                Duration::from_secs(60)
            )
            .is_err(),
            "an injectable model id cannot be bound"
        );
        assert!(
            ModelTurnBinding::new(
                &backend,
                "deepseek/deepseek-chat",
                "story",
                "exp",
                Duration::ZERO
            )
            .is_err(),
            "a zero lifetime is refused"
        );
        assert!(
            ModelTurnBinding::new(
                &backend,
                "deepseek/deepseek-chat",
                "story",
                "exp",
                MAX_MODEL_RUN_TTL + Duration::from_secs(1)
            )
            .is_err(),
            "an over-long lifetime is refused"
        );

        let error = ModelTurnAuthorization::authorize_model_run(
            ModelTurnBackend::Dsh,
            EnvironmentClass::Development,
        )
        .expect_err("a real backend cannot mint an unbound token");
        assert!(error.to_string().contains("bound"), "{error}");
    }

    #[test]
    fn identity_fields_the_model_turn_cannot_enforce_are_refused() {
        validate_identity_fields("dsh", "dsh", &[], "default", "deepseek/deepseek-chat")
            .expect("the enforceable identity");
        assert!(
            validate_identity_fields("dsh", "opencode", &[], "default", "deepseek/deepseek-chat")
                .is_err(),
            "executor must name the actual backend"
        );
        assert!(
            validate_identity_fields(
                "dsh",
                "dsh",
                &["read".to_string()],
                "default",
                "deepseek-chat"
            )
            .is_err(),
            "tools cannot be enforced by the model-turn sandbox"
        );
        assert!(
            validate_identity_fields("dsh", "dsh", &[], "judgment", "deepseek/deepseek-chat")
                .is_err(),
            "a non-default model policy is not applied by this sandbox"
        );
        assert!(
            validate_identity_fields("dsh", "dsh", &[], "default", "").is_err(),
            "an empty model pin is refused"
        );
        assert!(
            validate_identity_fields("dsh", "dsh", &[], "default", "provider/model\n- inject")
                .is_err(),
            "an injectable model id is refused"
        );
    }
    #[test]
    fn a_bound_token_refuses_a_request_outside_its_binding_and_records_identity_when_it_matches() {
        let fixture = Fixture::new();
        let backend = command_backend("/bin/echo", vec!["fake-turn"]);
        let binding = ModelTurnBinding::new(
            &backend,
            "deepseek/deepseek-chat",
            "story-1",
            "exp-1",
            Duration::from_secs(60),
        )
        .expect("binding");
        let authorization = ModelTurnAuthorization::authorize_model_run_bound(
            backend,
            EnvironmentClass::Development,
            binding,
        )
        .expect("bound authorization");
        let sandbox = ModelTurnSandbox::new(authorization).with_run_root(fixture.run_root.path());

        // The fixture request pins model "deepseek-chat"; the token is bound to the exact
        // provider/model id, so the mismatch is refused before any spawn.
        let mismatched = request(
            "exp-1-candidate",
            EnvironmentClass::Development,
            &fixture.task,
            &fixture.harness,
        );
        let error = sandbox
            .run(&mismatched)
            .expect_err("binding mismatch refused");
        assert!(error.to_string().contains("bound to model"), "{error}");

        // A matching request runs and the receipt records the bound identity.
        let mut matching = request(
            "exp-1-reference",
            EnvironmentClass::Development,
            &fixture.task,
            &fixture.harness,
        );
        matching.model = "deepseek/deepseek-chat";
        let reading = sandbox.run(&matching).expect("matching bound request");
        let receipt = receipt_for(&reading);
        assert_eq!(receipt.experiment_id, "exp-1");
        assert_eq!(receipt.task_id, "story-1");
        assert_eq!(receipt.model, "deepseek/deepseek-chat");
    }
    // ---------------------------------------------------------------------------------------
    // P1-C: receipt secret hygiene, retention and capture bounds
    // ---------------------------------------------------------------------------------------

    #[test]
    fn receipts_are_redacted_and_never_persist_the_prompt_or_transcript() {
        let fixture = Fixture::new();
        let script = "printf 'DATABASE_URL=postgres://u:topsecret@h/db token=abcdef123\\n'";
        let args = vec!["-c", script];
        let sandbox = fixture.sandbox(command_backend("/bin/sh", args));
        let reading = run(&sandbox, &fixture, "probe-secret").expect("reading");
        let receipt = receipt_for(&reading);

        assert!(receipt.stdout_redacted, "receipt must record redaction");
        assert!(!receipt.stdout_excerpt.contains("topsecret"));
        assert!(!receipt.stdout_excerpt.contains("abcdef123"));
        assert!(receipt.stdout_excerpt.contains("[REDACTED]"));
        assert_eq!(receipt.session_note, SESSION_TRANSCRIPT_NOTE_REF);
        assert!(reading
            .behavior
            .evidence_refs
            .iter()
            .any(|reference| reference == SESSION_TRANSCRIPT_NOTE_REF));

        // The receipt is a durable artifact: it must not carry the prompt (the candidate harness
        // content or the historical task) or a session transcript path.
        let receipt_path = reading
            .behavior
            .evidence_refs
            .iter()
            .find_map(|reference| reference.strip_prefix(RECEIPT_REF_PREFIX))
            .expect("receipt ref");
        let durable = std::fs::read_to_string(receipt_path).expect("receipt file");
        assert!(!durable.contains("HARNESS EVOLUTION REPLAY PROBE"));
        assert!(!durable.contains("GOAL:"));
        assert!(!durable.contains("base prompt"));
        assert!(durable.contains(SESSION_TRANSCRIPT_NOTE_REF));
    }

    #[cfg(unix)]
    #[test]
    fn receipt_and_run_root_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let fixture = Fixture::new();
        let sandbox = fixture.sandbox(command_backend("/bin/echo", vec!["private"]));
        let reading = run(&sandbox, &fixture, "probe-perms").expect("reading");
        let receipt_path = reading
            .behavior
            .evidence_refs
            .iter()
            .find_map(|reference| reference.strip_prefix(RECEIPT_REF_PREFIX))
            .expect("receipt ref")
            .to_string();
        let file_mode = std::fs::metadata(&receipt_path)
            .expect("receipt metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600, "model-turn receipt must be 0600");
        let dir_mode = std::fs::metadata(fixture.run_root.path())
            .expect("run root metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700, "model-turn run root must be 0700");
    }

    #[test]
    fn model_turn_receipt_retention_is_bounded() {
        let root = tempfile::tempdir().expect("temp dir");
        for name in ["a", "b", "c"] {
            std::fs::write(root.path().join(format!("{name}.json")), b"{}").expect("write");
            std::thread::sleep(Duration::from_millis(10));
        }
        let removed = cleanup_receipts(root.path(), Duration::from_secs(3600), 2).expect("cleanup");
        assert_eq!(removed, 1);
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
    }

    #[test]
    fn a_normal_exit_with_a_lingering_descendant_does_not_hang_the_model_turn() {
        // The fake vendor exits immediately but leaves a 30s helper holding stdout. The group kill
        // on normal exit (plus the join deadline) must keep the probe bounded.
        let fixture = Fixture::new();
        let sandbox = fixture
            .sandbox(command_backend("/bin/sh", vec!["-c", "(sleep 30) & exit 0"]))
            .with_timeout(Duration::from_secs(5));
        let started = Instant::now();
        let reading = run(&sandbox, &fixture, "probe-descendant").expect("reading");
        assert!(
            started.elapsed() < CAPTURE_JOIN_GRACE + Duration::from_secs(2),
            "normal exit with a lingering descendant must not hang the model turn"
        );
        let receipt = receipt_for(&reading);
        assert_eq!(receipt.exit_code, Some(0));
        assert!(!receipt.timed_out);
    }

}
