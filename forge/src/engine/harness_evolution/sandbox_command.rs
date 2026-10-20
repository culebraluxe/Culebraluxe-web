//! A concrete non-production [`ReplaySandbox`] that runs an allowlisted command as a stand-in for
//! one model turn.
//!
//! The replay executor (`probe_replay`) needs *some* executor to run a harness against a historical
//! task. A real model turn is not wired in this build, so this sandbox runs a bounded, allowlisted
//! local command instead — enough to dogfood the probe/contrast/evidence plumbing end to end
//! without pretending a model was called.
//!
//! Safety properties, all structural rather than conventional:
//!
//! * **No shell.** The program is an absolute path and the arguments are an `OsString` vector:
//!   there is no shell string to inject into. Metacharacters in an argument are literal text.
//! * **Argument allowlist, not just a program allowlist.** An invocation is allowed only when its
//!   *exact* program and argument vector match one configured [`AllowedInvocation`]. Allowing
//!   `/bin/echo` does not allow `/bin/echo` with arbitrary arguments.
//! * **No production.** A production (or otherwise non-probe) environment is refused before the
//!   command is spawned, via the same guard the replay executor uses.
//! * **Isolated workspace.** Each execution gets a fresh directory under the run root, with
//!   `env_clear()` and a minimal environment (`HOME`/`TMPDIR` point into that workspace), stdin from
//!   `/dev/null`, and a hard timeout.
//! * **Bounded streaming capture.** stdout/stderr are read on dedicated threads into fixed-size
//!   buffers while the child runs: memory is capped by the output cap, the child is never blocked by
//!   a full pipe, and read errors are propagated instead of swallowed.
//! * **Process-group kill on timeout AND on normal exit.** The child leads its own process group;
//!   the *group* is killed whenever the direct child is done, so a helper that inherited stdout
//!   cannot keep the pipes (or the probe) alive and no zombie is left behind. The kill is
//!   setsid-safe: it never signals pid 0/1 or this process's own group.
//! * **Capture deadline.** Reader threads are awaited with a deadline, so an escaped descendant
//!   holding a pipe becomes a typed failure instead of an unbounded join.
//! * **Secret-hygienic receipts.** Receipts live under a 0700 run root, are written 0600, hold no
//!   prompt content (only excerpts), and redact URL userinfo, `Bearer` values, secret-key prefixes
//!   and `token=`/`password=`-style assignments before serialization. Receipts older than
//!   [`RECEIPT_RETENTION`] are pruned and the count is capped at [`RECEIPT_MAX_FILES`].
//!
//! The behavioral reading is coarse on purpose: exit status maps to success/tests and the QA proxy
//! verdict, steps = 1, and cost stays `None` (unpriced, never zero). A command is not a model, and
//! the plan/contrast stages treat it as behavioral evidence only.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::model::{BehaviorEvidence, QaVerdict};
use super::probe_replay::{
    assert_non_production_environment, ReplayError, ReplaySandbox, SandboxReading, SandboxRequest,
};

/// Default execution budget for one command.
pub const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(60);

/// The longest timeout this sandbox will accept. A replay probe must stay bounded.
pub const MAX_COMMAND_TIMEOUT: Duration = Duration::from_secs(600);

/// How much of stdout/stderr is retained per stream. Everything beyond it is counted and discarded.
pub const DEFAULT_OUTPUT_CAP_BYTES: usize = 64 * 1024;

/// Ref prefix for the execution id (the receipt's id for one run).
pub const EXEC_REF_PREFIX: &str = "sandbox-command:exec:";
/// Ref prefix for the receipt file.
pub const RECEIPT_REF_PREFIX: &str = "sandbox-command:receipt:";
/// Ref prefix for the program that ran.
pub const PROGRAM_REF_PREFIX: &str = "sandbox-command:program:";

/// How much of one capture buffer is read from a pipe at a time.
const STREAM_CHUNK_BYTES: usize = 8192;

/// How long the capture readers may take after the process group has been killed. A descendant that
/// escaped the group and still holds a pipe can keep EOF away forever; the deadline turns that into
/// a typed failure instead of a hung probe.
pub const CAPTURE_JOIN_GRACE: Duration = Duration::from_secs(5);

/// Replacement text for redacted receipt content.
pub const REDACTED: &str = "[REDACTED]";

/// Receipts older than this are removed on the next run.
pub const RECEIPT_RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Hard cap on receipt files kept per run root (oldest removed first).
pub const RECEIPT_MAX_FILES: usize = 200;

/// One exact invocation the sandbox is allowed to run: an absolute program **and** its complete
/// argument vector. This is the argument allowlist: matching a program is not enough.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowedInvocation {
    pub program: PathBuf,
    pub args: Vec<String>,
}

impl AllowedInvocation {
    pub fn new(program: impl Into<PathBuf>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
        }
    }

    /// An allowed program called with no arguments.
    pub fn exact(program: impl Into<PathBuf>) -> Self {
        Self::new(program, Vec::new())
    }
}

/// The default allowlist: harmless POSIX binaries, each with the exact arguments it may receive.
///
/// The defaults are deliberately narrow. A caller that wants a different command must say so with
/// [`CommandSandbox::with_allowed_invocations`]; production wiring builds its own allowlist around
/// the real sandbox.
pub fn default_allowed_invocations() -> Vec<AllowedInvocation> {
    vec![
        AllowedInvocation::exact("/usr/bin/true"),
        AllowedInvocation::exact("/usr/bin/false"),
        AllowedInvocation::exact("/bin/echo"),
        AllowedInvocation::exact("/usr/bin/echo"),
        AllowedInvocation::new("/bin/sleep", vec!["1".to_string()]),
    ]
}

/// The command a [`CommandSandbox`] runs to stand in for one model turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxCommand {
    /// Absolute path. A bare name would be a PATH lookup, which is not allowed.
    pub program: PathBuf,
    /// Argument vector. Never joined into a shell string.
    pub args: Vec<String>,
    pub timeout: Duration,
    /// Human label recorded in the receipt (what this command stands in for).
    pub label: String,
}

impl SandboxCommand {
    pub fn new(program: impl Into<PathBuf>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
            timeout: DEFAULT_COMMAND_TIMEOUT,
            label: "harness-evolution-sandbox-command".to_string(),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Configuration validation: absolute program, bounded timeout, non-empty label.
    pub fn validate(&self) -> Result<(), ReplayError> {
        if !self.program.is_absolute() {
            return Err(ReplayError::SandboxRefused {
                sandbox: "command".into(),
                message: format!(
                    "program {:?} is not an absolute path; a PATH lookup is not allowed",
                    self.program
                ),
            });
        }
        if self.timeout.is_zero() || self.timeout > MAX_COMMAND_TIMEOUT {
            return Err(ReplayError::SandboxRefused {
                sandbox: "command".into(),
                message: format!(
                    "timeout {:?} is outside (0, {MAX_COMMAND_TIMEOUT:?}]",
                    self.timeout
                ),
            });
        }
        if self.label.trim().is_empty() {
            return Err(ReplayError::SandboxRefused {
                sandbox: "command".into(),
                message: "command has no label".into(),
            });
        }
        Ok(())
    }
}

/// One recorded execution. This is what the receipt file contains, and it is the only durable
/// artifact the sandbox produces.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SandboxExecutionReceipt {
    pub execution_id: String,
    pub probe_id: String,
    pub label: String,
    pub program: String,
    pub args: Vec<String>,
    /// `None` when the process was killed (for example on timeout).
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: u64,
    /// Up to [`CommandSandbox::output_cap`] bytes of each stream, with a truncation marker.
    pub stdout_excerpt: String,
    pub stderr_excerpt: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    /// True when secret-shaped content was redacted before this receipt was written.
    pub stdout_redacted: bool,
    pub stderr_redacted: bool,
    /// Total bytes the child wrote, including the part that was not retained.
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub workspace: String,
}

/// One stream as captured: bounded text, whether it was truncated, and the true total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamCapture {
    pub text: String,
    pub truncated: bool,
    pub total_bytes: u64,
}

/// The concrete command sandbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSandbox {
    command: SandboxCommand,
    allowed_invocations: Vec<AllowedInvocation>,
    run_root: PathBuf,
    output_cap: usize,
}

impl CommandSandbox {
    /// A sandbox for one command, using the default invocation allowlist and run root.
    pub fn new(command: SandboxCommand) -> Self {
        Self {
            command,
            allowed_invocations: default_allowed_invocations(),
            run_root: default_run_root(),
            output_cap: DEFAULT_OUTPUT_CAP_BYTES,
        }
    }

    /// Replace the invocation allowlist. Tests use this to allow a fake command; production wiring
    /// should keep it as narrow as possible (a real model sandbox replaces this entirely).
    pub fn with_allowed_invocations(mut self, allowed_invocations: Vec<AllowedInvocation>) -> Self {
        self.allowed_invocations = allowed_invocations;
        self
    }

    /// Where workspaces and receipts are written.
    pub fn with_run_root(mut self, run_root: impl Into<PathBuf>) -> Self {
        self.run_root = run_root.into();
        self
    }

    /// Cap for each captured stream. Zero is refused.
    pub fn with_output_cap(mut self, output_cap: usize) -> Self {
        self.output_cap = output_cap;
        self
    }

    pub fn command(&self) -> &SandboxCommand {
        &self.command
    }

    pub fn run_root(&self) -> &Path {
        &self.run_root
    }

    /// Validate the configuration without executing anything: absolute program, bounded timeout,
    /// non-empty label, non-zero output cap, and an **exact** allowlisted invocation.
    pub fn validate(&self) -> Result<(), ReplayError> {
        self.command.validate()?;
        if self.output_cap == 0 {
            return Err(ReplayError::SandboxRefused {
                sandbox: "command".into(),
                message: "output cap must be non-zero".into(),
            });
        }
        self.authorize_invocation()
    }

    /// The argument allowlist check. A matching program with a different argv is refused, and the
    /// refusal names the allowed invocations so a misconfiguration is obvious.
    fn authorize_invocation(&self) -> Result<(), ReplayError> {
        let program_allowed = self
            .allowed_invocations
            .iter()
            .any(|allowed| allowed.program == self.command.program);
        if !program_allowed {
            return Err(ReplayError::SandboxRefused {
                sandbox: "command".into(),
                message: format!(
                    "program {} is not allowlisted (allowed programs: {})",
                    self.command.program.display(),
                    self.allowed_invocations
                        .iter()
                        .map(|allowed| allowed.program.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            });
        }
        let arguments_allowed = self.allowed_invocations.iter().any(|allowed| {
            allowed.program == self.command.program && allowed.args == self.command.args
        });
        if !arguments_allowed {
            return Err(ReplayError::SandboxRefused {
                sandbox: "command".into(),
                message: format!(
                    "argument vector {:?} is not allowed for program {}; allowed invocations: {}",
                    self.command.args,
                    self.command.program.display(),
                    self.allowed_invocations
                        .iter()
                        .filter(|allowed| allowed.program == self.command.program)
                        .map(|allowed| format!("{}{:?}", allowed.program.display(), allowed.args))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            });
        }
        Ok(())
    }

    fn workspace_for(&self, probe_id: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        self.run_root
            .join(format!("{stamp}-{}", sanitize_component(probe_id)))
    }
}

impl ReplaySandbox for CommandSandbox {
    fn run(&self, request: &SandboxRequest<'_>) -> Result<SandboxReading, ReplayError> {
        // Production refusal comes first: no workspace, no receipt, no spawn.
        assert_non_production_environment(request.environment_class)?;
        self.validate()?;

        let workspace = self.workspace_for(request.probe_id);
        create_secure_dir(&workspace).map_err(|error| ReplayError::SandboxFailed {
            probe_id: request.probe_id.to_string(),
            message: format!(
                "cannot create sandbox workspace {}: {error}",
                workspace.display()
            ),
        })?;

        let mut command = Command::new(&self.command.program);
        command
            .args(&self.command.args)
            .current_dir(&workspace)
            // A cleared environment keeps tokens/URLs out of the sandbox; only the workspace-local
            // values below are visible to the child.
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &workspace)
            .env("TMPDIR", &workspace)
            .env("LANG", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Own process group so a timeout can kill the whole tree, not just the direct child.
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
                message: format!("cannot spawn {}: {error}", self.command.program.display()),
            })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ReplayError::SandboxFailed {
                probe_id: request.probe_id.to_string(),
                message: "stdout pipe missing".into(),
            })?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ReplayError::SandboxFailed {
                probe_id: request.probe_id.to_string(),
                message: "stderr pipe missing".into(),
            })?;
        let output_cap = self.output_cap;
        let stdout_rx = spawn_stream_capture(stdout, output_cap);
        let stderr_rx = spawn_stream_capture(stderr, output_cap);

        let mut timed_out = false;
        let exit_status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => {
                    if started.elapsed() >= self.command.timeout {
                        terminate_process_group(&mut child);
                        timed_out = true;
                        break None;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    terminate_process_group(&mut child);
                    let deadline = Instant::now() + CAPTURE_JOIN_GRACE;
                    let _ = await_stream_capture(stdout_rx, deadline, request.probe_id, "stdout");
                    let _ = await_stream_capture(stderr_rx, deadline, request.probe_id, "stderr");
                    return Err(ReplayError::SandboxFailed {
                        probe_id: request.probe_id.to_string(),
                        message: format!("cannot wait for command: {error}"),
                    });
                }
            }
        };
        let duration = started.elapsed();

        // Kill the group even on a NORMAL exit: a helper that inherited stdout can otherwise hold
        // the pipes open and hang the join forever. The direct child may already be reaped; the
        // negative pid addresses the remaining group members.
        terminate_process_group(&mut child);
        let deadline = Instant::now() + CAPTURE_JOIN_GRACE;
        let stdout_capture = await_stream_capture(stdout_rx, deadline, request.probe_id, "stdout")?;
        let stderr_capture = await_stream_capture(stderr_rx, deadline, request.probe_id, "stderr")?;

        let execution_id = format!(
            "{EXEC_REF_PREFIX}{}:{}",
            sanitize_component(request.probe_id),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or(0)
        );
        let receipt = SandboxExecutionReceipt {
            execution_id: execution_id.clone(),
            probe_id: request.probe_id.to_string(),
            label: self.command.label.clone(),
            program: self.command.program.display().to_string(),
            args: self.command.args.clone(),
            exit_code: exit_status.and_then(|status| status.code()),
            timed_out,
            duration_ms: u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
            stdout_excerpt: redact_secrets(&stdout_capture.text),
            stderr_excerpt: redact_secrets(&stderr_capture.text),
            stdout_redacted: redact_secrets(&stdout_capture.text) != stdout_capture.text,
            stderr_redacted: redact_secrets(&stderr_capture.text) != stderr_capture.text,
            stdout_truncated: stdout_capture.truncated,
            stderr_truncated: stderr_capture.truncated,
            stdout_bytes: stdout_capture.total_bytes,
            stderr_bytes: stderr_capture.total_bytes,
            workspace: workspace.display().to_string(),
        };
        let receipt_path = write_receipt(&self.run_root, &receipt)?;

        if timed_out {
            // A timeout is not behavioral evidence; it is a failed probe.
            return Err(ReplayError::SandboxFailed {
                probe_id: request.probe_id.to_string(),
                message: format!(
                    "command timed out after {:?} (process group killed); receipt {}",
                    self.command.timeout,
                    receipt_path.display()
                ),
            });
        }

        let success = exit_status.is_some_and(|status| status.success());
        let mut behavior = BehaviorEvidence::new(execution_id.clone(), "");
        behavior.success = success;
        // The command IS the check in this stand-in: exit 0 counts as one passing test, a non-zero
        // exit as one failing test. A real model sandbox supplies richer evidence.
        behavior.tests_passed = u32::from(success);
        behavior.tests_failed = u32::from(!success);
        behavior.qa = if success {
            QaVerdict::Pass
        } else {
            QaVerdict::Fail
        };
        behavior.steps = 1;
        behavior.elapsed_ms = receipt.duration_ms;
        // The command is unpriced: `None` is "not priced", never zero.
        behavior.cost_micros = None;
        behavior.tool_failures = 0;
        behavior.evidence_refs = vec![
            execution_id,
            format!("{RECEIPT_REF_PREFIX}{}", receipt_path.display()),
            format!("{PROGRAM_REF_PREFIX}{}", self.command.program.display()),
        ];
        Ok(SandboxReading::new(behavior))
    }
}

fn default_run_root() -> PathBuf {
    std::env::temp_dir().join("forge-harness-evolution-sandbox")
}

/// Keep only characters that are safe in a directory name; everything else becomes `_`.
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
        "sandbox".to_string()
    } else {
        sanitized
    }
}

/// Read a stream into a buffer capped at `cap` bytes while counting the true total.
///
/// Bounded: memory never exceeds `cap` plus one fixed-size read chunk. Streaming: the pipe keeps
/// being drained after the cap so the child is never blocked by a full pipe. Errors are returned,
/// never swallowed.
pub fn read_stream_bounded<R: Read>(mut reader: R, cap: usize) -> io::Result<StreamCapture> {
    let mut retained: Vec<u8> = Vec::new();
    let mut total: u64 = 0;
    let mut buffer = [0u8; STREAM_CHUNK_BYTES];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        let remaining = cap.saturating_sub(retained.len());
        if remaining > 0 {
            let take = remaining.min(read);
            retained.extend_from_slice(&buffer[..take]);
        }
    }
    let truncated = total > cap as u64;
    let mut text = String::from_utf8_lossy(&retained).to_string();
    if truncated {
        text.push_str(&format!("\n[truncated at {cap} bytes of {total}]"));
    }
    Ok(StreamCapture {
        text,
        truncated,
        total_bytes: total,
    })
}

/// Spawn a bounded reader for one pipe and hand its result back over a channel, so the waiter can
/// apply a join deadline (`JoinHandle::join` has no timeout).
pub fn spawn_stream_capture<R: Read + Send + 'static>(
    reader: R,
    cap: usize,
) -> Receiver<io::Result<StreamCapture>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(read_stream_bounded(reader, cap));
    });
    receiver
}

/// Await one capture with a deadline. Timeout and disconnect are typed failures, never a hang and
/// never an empty "successful" capture.
pub fn await_stream_capture(
    receiver: Receiver<io::Result<StreamCapture>>,
    deadline: Instant,
    probe_id: &str,
    stream: &str,
) -> Result<StreamCapture, ReplayError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    match receiver.recv_timeout(remaining) {
        Ok(Ok(capture)) => Ok(capture),
        Ok(Err(error)) => Err(ReplayError::SandboxFailed {
            probe_id: probe_id.to_string(),
            message: format!("cannot read {stream}: {error}"),
        }),
        Err(RecvTimeoutError::Timeout) => Err(ReplayError::SandboxFailed {
            probe_id: probe_id.to_string(),
            message: format!(
                "{stream} capture did not finish before the join deadline (process group killed)"
            ),
        }),
        Err(RecvTimeoutError::Disconnected) => Err(ReplayError::SandboxFailed {
            probe_id: probe_id.to_string(),
            message: format!("{stream} reader ended without reporting a capture"),
        }),
    }
}

/// Kill the child's whole process group (setsid-safe) and reap the child if it is still running.
///
/// The child leads its own group (`process_group(0)`), so a negative pid reaches helpers it spawned.
/// The guards mean this can never signal pid 0/1 or this process's own group.
pub fn terminate_process_group(child: &mut Child) {
    #[cfg(unix)]
    {
        use std::os::raw::c_int;
        extern "C" {
            fn kill(pid: c_int, sig: c_int) -> c_int;
        }
        const SIGKILL: c_int = 9;
        let pid = child.id() as c_int;
        let self_pid = std::process::id() as c_int;
        if pid > 1 && pid != self_pid {
            unsafe {
                kill(-pid, SIGKILL);
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Create a directory with owner-only permissions (0700 on unix).
pub fn create_secure_dir(path: &Path) -> io::Result<()> {
    std::fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Write a file with owner-only permissions (0600 on unix).
pub fn write_secure_file(path: &Path, body: &[u8]) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(body)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, body)
    }
}

/// Remove `[REDACTED]`-shaped secrets from captured output before it is persisted.
///
/// This is defense-in-depth for receipts, not a content firewall: it catches the common shapes a
/// harness or tool prints on failure (URL userinfo, bearer tokens, key prefixes, env assignments).
/// Anything else can still be in the excerpt, so receipts stay 0600 and are pruned.
pub fn redact_secrets(text: &str) -> String {
    let redacted = redact_url_credentials(text);
    let redacted = redact_marker_value(&redacted, "bearer ");
    let redacted = redact_assignments(&redacted);
    redact_prefixed_tokens(&redacted)
}

fn redact_url_credentials(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"://") {
            out.push_str("://");
            index += 3;
            let start = index;
            let mut end = index;
            let mut at = None;
            while end < bytes.len() && !bytes[end].is_ascii_whitespace() {
                if bytes[end] == b'@' {
                    at = Some(end);
                    break;
                }
                end += 1;
            }
            if let Some(at) = at {
                if at > start {
                    out.push_str(REDACTED);
                }
                out.push_str(&text[at..end]);
                index = end;
                continue;
            }
            out.push_str(&text[start..end]);
            index = end;
            continue;
        }
        let character = text[index..].chars().next().expect("char boundary");
        out.push(character);
        index += character.len_utf8();
    }
    out
}

fn redact_marker_value(text: &str, marker: &str) -> String {
    let lowered = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    while let Some(found) = lowered[cursor..].find(marker) {
        let marker_start = cursor + found;
        let value_start = marker_start + marker.len();
        out.push_str(&text[cursor..value_start]);
        let mut value_end = value_start;
        while value_end < text.len() {
            let character = text[value_end..].chars().next().expect("char boundary");
            if character.is_whitespace() || matches!(character, '"' | '\'' | '&' | ',' | ')') {
                break;
            }
            value_end += character.len_utf8();
        }
        if value_end > value_start {
            out.push_str(REDACTED);
        }
        cursor = value_end;
    }
    out.push_str(&text[cursor..]);
    out
}

const SECRET_ASSIGNMENTS: [&str; 8] = [
    "token=",
    "access_token=",
    "api_key=",
    "apikey=",
    "secret=",
    "password=",
    "database_url=",
    "authorization=",
];

fn redact_assignments(text: &str) -> String {
    let mut out = text.to_string();
    for assignment in SECRET_ASSIGNMENTS {
        out = redact_marker_value(&out, assignment);
        let upper = assignment.to_ascii_uppercase();
        out = redact_marker_value(&out, &upper);
    }
    out
}

const SECRET_PREFIXES: [&str; 7] = ["sk-", "ghp_", "gho_", "xoxb-", "AKIA", "ASIA", "eyJ"];

fn redact_prefixed_tokens(text: &str) -> String {
    text.split_inclusive(char::is_whitespace)
        .map(|chunk| {
            let trimmed = chunk.trim_end_matches(char::is_whitespace);
            let suffix = &chunk[trimmed.len()..];
            if SECRET_PREFIXES
                .iter()
                .any(|prefix| trimmed.starts_with(prefix))
            {
                format!("{REDACTED}{suffix}")
            } else {
                chunk.to_string()
            }
        })
        .collect()
}

/// Remove old receipts and cap the count. Best-effort: a failure to prune never fails a probe, but
/// `Ok(removed)` is returned so a caller can log it.
pub fn cleanup_receipts(
    run_root: &Path,
    retention: Duration,
    max_entries: usize,
) -> io::Result<usize> {
    let entries = match std::fs::read_dir(run_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    let now = SystemTime::now();
    let cutoff = now.checked_sub(retention).unwrap_or(UNIX_EPOCH);
    let mut receipts: Vec<(SystemTime, PathBuf)> = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let metadata = entry.metadata()?;
        if !metadata.is_file() {
            continue;
        }
        receipts.push((metadata.modified().unwrap_or(UNIX_EPOCH), path));
    }
    receipts.sort_by_key(|(modified, _)| *modified);
    let mut removed = 0usize;
    let mut kept: Vec<PathBuf> = Vec::new();
    for (modified, path) in receipts {
        if modified < cutoff {
            std::fs::remove_file(&path)?;
            removed += 1;
        } else {
            kept.push(path);
        }
    }
    if kept.len() > max_entries {
        for path in kept.drain(..kept.len() - max_entries) {
            std::fs::remove_file(path)?;
            removed += 1;
        }
    }
    Ok(removed)
}

fn write_receipt(
    run_root: &Path,
    receipt: &SandboxExecutionReceipt,
) -> Result<PathBuf, ReplayError> {
    create_secure_dir(run_root).map_err(|error| ReplayError::SandboxFailed {
        probe_id: receipt.probe_id.clone(),
        message: format!(
            "cannot create sandbox run root {}: {error}",
            run_root.display()
        ),
    })?;
    let path = run_root.join(format!(
        "{}.json",
        sanitize_component(&receipt.execution_id)
    ));
    let body = serde_json::to_vec_pretty(receipt).map_err(|error| ReplayError::SandboxFailed {
        probe_id: receipt.probe_id.clone(),
        message: format!("cannot encode sandbox receipt: {error}"),
    })?;
    write_secure_file(&path, &body).map_err(|error| ReplayError::SandboxFailed {
        probe_id: receipt.probe_id.clone(),
        message: format!("cannot write sandbox receipt {}: {error}", path.display()),
    })?;
    // Bounded retention: a probe never leaves an unbounded pile of /tmp receipts behind.
    let _ = cleanup_receipts(run_root, RECEIPT_RETENTION, RECEIPT_MAX_FILES);
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::harness_evolution::model::{
        Component, ComponentKind, EnvironmentClass, HarnessDomain, HarnessManifest, HarnessRevision,
    };
    use crate::engine::harness_evolution::probe::ProbeSide;
    use crate::engine::harness_evolution::probe_replay::{
        HistoricalTask, ReplayHarness, SandboxRequest,
    };

    fn revision() -> HarnessRevision {
        HarnessRevision::draft(
            "base-1",
            HarnessDomain::RustBackend,
            HarnessManifest::from_components([Component::green(
                ComponentKind::SystemPrompt,
                "base prompt",
                "prompt",
            )]),
            "owner",
        )
    }

    fn task() -> HistoricalTask {
        HistoricalTask::new("story-1", "run-1", "Story", "GOAL:\nfix it")
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
                harness: ReplayHarness::materialize(&revision()).expect("harness"),
            }
        }

        fn sandbox(
            &self,
            command: SandboxCommand,
            allowed: Vec<AllowedInvocation>,
        ) -> CommandSandbox {
            CommandSandbox::new(command)
                .with_run_root(self.run_root.path())
                .with_allowed_invocations(allowed)
        }
    }

    fn run(
        sandbox: &CommandSandbox,
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

    fn receipt_for(reading: &SandboxReading) -> SandboxExecutionReceipt {
        let reference = reading
            .behavior
            .evidence_refs
            .iter()
            .find_map(|reference| reference.strip_prefix(RECEIPT_REF_PREFIX))
            .expect("receipt ref");
        let body = std::fs::read(reference).expect("receipt file");
        serde_json::from_slice(&body).expect("receipt json")
    }

    fn receipt_of_error(error: &ReplayError) -> SandboxExecutionReceipt {
        let message = error.to_string();
        let path = message
            .split("receipt ")
            .nth(1)
            .expect("timeout error names its receipt")
            .trim();
        let body = std::fs::read(path).expect("timeout receipt file");
        serde_json::from_slice(&body).expect("timeout receipt json")
    }

    fn echo(argument: &str) -> (SandboxCommand, Vec<AllowedInvocation>) {
        let args = vec![argument.to_string()];
        (
            SandboxCommand::new("/bin/echo", args.clone()),
            vec![AllowedInvocation::new("/bin/echo", args)],
        )
    }

    #[test]
    fn a_successful_command_returns_a_reading_with_a_durable_receipt_ref() {
        let fixture = Fixture::new();
        let (command, allowed) = echo("sandbox-hello");
        let sandbox = fixture.sandbox(command, allowed);
        let reading = run(&sandbox, &fixture, "probe-ref").expect("reading");

        assert!(reading.behavior.success);
        assert_eq!(reading.behavior.qa, QaVerdict::Pass);
        assert_eq!(reading.behavior.tests_passed, 1);
        assert_eq!(reading.behavior.tests_failed, 0);
        assert_eq!(reading.behavior.steps, 1);
        assert_eq!(reading.behavior.cost_micros, None, "unpriced is not zero");
        assert!(!reading.behavior.evidence_id.trim().is_empty());
        assert!(reading.behavior.evidence_id.starts_with(EXEC_REF_PREFIX));
        assert!(reading
            .behavior
            .evidence_refs
            .iter()
            .any(|reference| reference.starts_with(RECEIPT_REF_PREFIX)));

        let receipt = receipt_for(&reading);
        assert_eq!(receipt.probe_id, "probe-ref");
        assert_eq!(receipt.exit_code, Some(0));
        assert!(!receipt.timed_out);
        assert!(receipt.stdout_excerpt.contains("sandbox-hello"));
        assert_eq!(receipt.program, "/bin/echo");
        assert_eq!(receipt.args, vec!["sandbox-hello".to_string()]);
    }

    #[test]
    fn a_nonzero_exit_is_a_failed_behavior_not_a_sandbox_error() {
        let fixture = Fixture::new();
        let sandbox = fixture.sandbox(
            SandboxCommand::new("/usr/bin/false", Vec::new()),
            vec![AllowedInvocation::exact("/usr/bin/false")],
        );
        let reading = run(&sandbox, &fixture, "probe-ref").expect("reading");

        assert!(!reading.behavior.success);
        assert_eq!(reading.behavior.qa, QaVerdict::Fail);
        assert_eq!(reading.behavior.tests_passed, 0);
        assert_eq!(reading.behavior.tests_failed, 1);
        let receipt = receipt_for(&reading);
        assert_eq!(receipt.exit_code, Some(1));
    }

    #[test]
    fn a_command_that_exceeds_the_timeout_is_a_failed_probe() {
        let fixture = Fixture::new();
        let sandbox = fixture.sandbox(
            SandboxCommand::new("/bin/sleep", vec!["5".to_string()])
                .with_timeout(Duration::from_millis(100)),
            vec![AllowedInvocation::new("/bin/sleep", vec!["5".to_string()])],
        );
        let error = run(&sandbox, &fixture, "probe-ref").unwrap_err();
        assert_eq!(error.code(), "replay_sandbox_failed");
        assert!(error.to_string().contains("timed out"));
        let receipt = receipt_of_error(&error);
        assert!(receipt.timed_out);
        assert_eq!(receipt.exit_code, None);
    }

    #[cfg(unix)]
    fn process_exists(pid: i32) -> bool {
        use std::os::raw::c_int;
        extern "C" {
            fn kill(pid: c_int, sig: c_int) -> c_int;
        }
        unsafe { kill(pid, 0) == 0 }
    }

    #[cfg(unix)]
    #[test]
    fn a_timeout_kills_the_whole_process_group_and_reaps_the_child() {
        let fixture = Fixture::new();
        // The shell spawns a helper, records its pid, and waits. A timeout must kill both.
        let args = vec![
            "-c".to_string(),
            "sleep 30 & echo $! > grandchild.pid; wait".to_string(),
        ];
        let sandbox = fixture.sandbox(
            SandboxCommand::new("/bin/sh", args.clone()).with_timeout(Duration::from_millis(300)),
            vec![AllowedInvocation::new("/bin/sh", args)],
        );
        let started = std::time::Instant::now();
        let error = run(&sandbox, &fixture, "probe-group").unwrap_err();
        // The timeout bounds the wait; killing the group must not add a full timeout of latency.
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(error.to_string().contains("process group killed"));

        let receipt = receipt_of_error(&error);
        assert!(receipt.timed_out);
        let workspace = Path::new(&receipt.workspace).to_path_buf();
        let pid_path = workspace.join("grandchild.pid");
        // The helper may take a moment to be reaped after SIGKILL.
        let mut helper_pid = None;
        for _ in 0..40 {
            if let Ok(text) = std::fs::read_to_string(&pid_path) {
                if let Ok(pid) = text.trim().parse::<i32>() {
                    helper_pid = Some(pid);
                    if !process_exists(pid) {
                        return;
                    }
                }
            }
            thread::sleep(Duration::from_millis(25));
        }
        panic!(
            "helper process {:?} survived the process-group kill",
            helper_pid
        );
    }

    #[test]
    fn production_is_refused_before_any_workspace_or_receipt() {
        let fixture = Fixture::new();
        let (command, allowed) = echo("x");
        let sandbox = fixture.sandbox(command, allowed);
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
            0,
            "production refusal creates no workspace and no receipt"
        );
    }

    #[test]
    fn an_argument_outside_the_program_allowlist_is_refused() {
        let fixture = Fixture::new();
        let sandbox = fixture.sandbox(
            SandboxCommand::new("/bin/echo", vec!["allowed".to_string()]),
            vec![AllowedInvocation::new(
                "/bin/echo",
                vec!["different".to_string()],
            )],
        );
        let error = run(&sandbox, &fixture, "probe-ref").unwrap_err();
        assert_eq!(error.code(), "replay_sandbox_refused");
        assert!(error.to_string().contains("argument vector"));
        assert_eq!(
            std::fs::read_dir(fixture.run_root.path())
                .map(|entries| entries.count())
                .unwrap_or(0),
            0,
            "argument refusal happens before any workspace exists"
        );
    }

    #[test]
    fn a_program_outside_the_allowlist_is_refused() {
        let fixture = Fixture::new();
        let sandbox = fixture.sandbox(
            SandboxCommand::new("/usr/bin/env", Vec::new()),
            vec![AllowedInvocation::exact("/bin/echo")],
        );
        let error = run(&sandbox, &fixture, "probe-ref").unwrap_err();
        assert_eq!(error.code(), "replay_sandbox_refused");
        assert!(error.to_string().contains("not allowlisted"));
    }

    #[test]
    fn a_relative_program_is_refused_no_path_lookup() {
        let fixture = Fixture::new();
        let sandbox = fixture.sandbox(
            SandboxCommand::new("echo", Vec::new()),
            vec![AllowedInvocation::exact("echo")],
        );
        let error = run(&sandbox, &fixture, "probe-ref").unwrap_err();
        assert_eq!(error.code(), "replay_sandbox_refused");
        assert!(error.to_string().contains("absolute"));
    }

    #[test]
    fn arguments_are_a_vector_never_a_shell_string() {
        let fixture = Fixture::new();
        let metacharacters = vec![
            "; rm -rf .".to_string(),
            "$(touch pwned)".to_string(),
            "`touch pwned2`".to_string(),
            "&& echo injected".to_string(),
        ];
        let sandbox = fixture.sandbox(
            SandboxCommand::new("/bin/echo", metacharacters.clone()),
            vec![AllowedInvocation::new("/bin/echo", metacharacters.clone())],
        );
        let reading = run(&sandbox, &fixture, "probe-shell").expect("reading");
        let receipt = receipt_for(&reading);
        // Each argument is echoed literally: no shell parsed it.
        for argument in &metacharacters {
            assert!(
                receipt.stdout_excerpt.contains(argument.as_str()),
                "argument {argument:?} was not passed literally"
            );
        }
        assert_eq!(receipt.args, metacharacters);
        let workspace = Path::new(&receipt.workspace);
        assert!(!workspace.join("pwned").exists());
        assert!(!workspace.join("pwned2").exists());
    }

    #[test]
    fn the_child_environment_is_cleared_and_home_points_into_the_workspace() {
        let fixture = Fixture::new();
        let args = vec!["-c".to_string(), "printf %s \"$HOME\"".to_string()];
        let sandbox = fixture.sandbox(
            SandboxCommand::new("/bin/sh", args.clone()),
            vec![AllowedInvocation::new("/bin/sh", args)],
        );
        let reading = run(&sandbox, &fixture, "probe-env").expect("reading");
        let receipt = receipt_for(&reading);
        assert_eq!(
            receipt.stdout_excerpt.trim(),
            receipt.workspace,
            "HOME must point at the per-run sandbox workspace, not the host's"
        );
        assert!(
            !receipt.stdout_excerpt.contains("/Users/"),
            "the host home directory must not be visible to the child"
        );
    }

    #[test]
    fn captured_output_is_streamed_and_bounded_with_an_honest_marker() {
        let fixture = Fixture::new();
        let long = "0123456789abcdef";
        let (command, allowed) = echo(long);
        let sandbox = fixture.sandbox(command, allowed).with_output_cap(8);
        let reading = run(&sandbox, &fixture, "probe-cap").expect("reading");
        let receipt = receipt_for(&reading);
        assert!(receipt.stdout_truncated);
        assert!(
            receipt.stdout_excerpt.contains("[truncated at 8 bytes of "),
            "marker must state cap and true total, got {:?}",
            receipt.stdout_excerpt
        );
        assert!(!receipt.stdout_excerpt.contains(long));
        assert!(receipt.stdout_bytes > 8);
    }

    #[test]
    fn the_bounded_reader_propagates_read_errors_instead_of_swallowing_them() {
        struct FailingReader;
        impl Read for FailingReader {
            fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("simulated read failure"))
            }
        }
        let error = read_stream_bounded(FailingReader, 16).expect_err("read error must surface");
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert!(error.to_string().contains("simulated read failure"));
    }

    #[test]
    fn the_bounded_reader_keeps_draining_after_the_cap() {
        // A reader over more data than the cap still consumes everything and reports the total,
        // so a child writing past the cap is never blocked by a full pipe.
        let payload = vec![b'x'; 100_000];
        let capture = read_stream_bounded(io::Cursor::new(payload), 16).expect("capture");
        assert!(capture.truncated);
        assert_eq!(capture.total_bytes, 100_000);
        assert_eq!(capture.text.lines().next().unwrap().len(), 16);
    }

    #[test]
    fn a_timeout_outside_the_bounds_is_refused_at_configuration_time() {
        let fixture = Fixture::new();
        let (command, allowed) = echo("x");
        let too_long = fixture.sandbox(
            command
                .clone()
                .with_timeout(MAX_COMMAND_TIMEOUT + Duration::from_secs(1)),
            allowed.clone(),
        );
        assert_eq!(
            too_long.validate().unwrap_err().code(),
            "replay_sandbox_refused"
        );
        let zero = fixture.sandbox(command.with_timeout(Duration::ZERO), allowed);
        assert_eq!(
            zero.validate().unwrap_err().code(),
            "replay_sandbox_refused"
        );
        let (command, allowed) = echo("x");
        let no_cap = fixture.sandbox(command, allowed).with_output_cap(0);
        assert_eq!(
            no_cap.validate().unwrap_err().code(),
            "replay_sandbox_refused"
        );
    }

    #[test]
    fn two_executions_get_distinct_durable_refs() {
        let fixture = Fixture::new();
        let (command, allowed) = echo("x");
        let sandbox = fixture.sandbox(command, allowed);
        let first = run(&sandbox, &fixture, "probe-ref").expect("first");
        let second = run(&sandbox, &fixture, "probe-ref").expect("second");
        assert_ne!(first.behavior.evidence_id, second.behavior.evidence_id);
        assert_ne!(
            receipt_for(&first).workspace,
            receipt_for(&second).workspace
        );
    }

    #[test]
    fn the_receipt_round_trips_through_json() {
        let fixture = Fixture::new();
        let sandbox = fixture.sandbox(
            SandboxCommand::new("/usr/bin/true", Vec::new()),
            vec![AllowedInvocation::exact("/usr/bin/true")],
        );
        let reading = run(&sandbox, &fixture, "probe-roundtrip").expect("reading");
        let receipt = receipt_for(&reading);
        let encoded = serde_json::to_string(&receipt).expect("serialize");
        let decoded: SandboxExecutionReceipt = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(receipt, decoded);
        assert_eq!(receipt.exit_code, Some(0));
    }
    // ---------------------------------------------------------------------------------------
    // P1-C: capture deadline, normal-exit group kill, receipt secret hygiene
    // ---------------------------------------------------------------------------------------

    #[test]
    fn redaction_removes_common_secret_shapes_and_keeps_benign_text() {
        let text = concat!(
            "DATABASE_URL=postgres://user:supersecret@db.example/x\n",
            "Authorization: Bearer abc.def.ghi\n",
            "token=deadbeef access_token=feedface\n",
            "key sk-live-123 ghp_456 AKIAABCDEF eyJhbGciOi\n",
            "harmless: the test passed and files are unchanged\n"
        );
        let redacted = redact_secrets(text);
        for secret in [
            "supersecret",
            "abc.def.ghi",
            "deadbeef",
            "feedface",
            "sk-live-123",
            "ghp_456",
            "AKIAABCDEF",
            "eyJhbGciOi",
        ] {
            assert!(
                !redacted.contains(secret),
                "secret {secret:?} survived: {redacted}"
            );
        }
        assert!(redacted.contains(REDACTED));
        assert!(redacted.contains("harmless: the test passed"));
    }

    #[test]
    fn receipts_are_redacted_before_they_are_written() {
        let fixture = Fixture::new();
        let script = "printf 'DATABASE_URL=postgres://u:topsecret@h/db token=abcdef123\\n'";
        let args = vec!["-c".to_string(), script.to_string()];
        let sandbox = fixture.sandbox(
            SandboxCommand::new("/bin/sh", args.clone()),
            vec![AllowedInvocation::new("/bin/sh", args)],
        );
        let reading = run(&sandbox, &fixture, "probe-secret").expect("reading");
        let receipt = receipt_for(&reading);
        assert!(receipt.stdout_redacted, "receipt must record redaction");
        assert!(!receipt.stdout_excerpt.contains("topsecret"));
        assert!(!receipt.stdout_excerpt.contains("abcdef123"));
        assert!(receipt.stdout_excerpt.contains(REDACTED));
    }

    #[cfg(unix)]
    #[test]
    fn receipts_and_run_root_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let fixture = Fixture::new();
        let (command, allowed) = echo("private");
        let sandbox = fixture.sandbox(command, allowed);
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
        assert_eq!(file_mode, 0o600, "receipt must be 0600");
        let dir_mode = std::fs::metadata(fixture.run_root.path())
            .expect("run root metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700, "run root must be 0700");
        let workspace = receipt_for(&reading).workspace;
        let workspace_mode = std::fs::metadata(workspace)
            .expect("workspace metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(workspace_mode, 0o700, "workspace must be 0700");
    }

    #[test]
    fn receipt_retention_prunes_by_age_and_count() {
        let root = tempfile::tempdir().expect("temp dir");
        for name in ["a", "b", "c"] {
            std::fs::write(root.path().join(format!("{name}.json")), b"{}").expect("write");
            thread::sleep(Duration::from_millis(10));
        }
        let removed = cleanup_receipts(root.path(), Duration::from_secs(3600), 2).expect("cleanup");
        assert_eq!(removed, 1, "count cap removes the oldest receipt");
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);

        thread::sleep(Duration::from_millis(10));
        let removed_old = cleanup_receipts(root.path(), Duration::ZERO, 10).expect("cleanup age");
        assert_eq!(
            removed_old, 2,
            "zero retention removes every existing receipt"
        );
    }

    #[test]
    fn a_normal_exit_with_a_lingering_descendant_does_not_hang() {
        // The shell exits immediately but leaves a 30s helper holding stdout. The group kill on
        // normal exit must close the pipes; the join deadline bounds it if it cannot.
        let fixture = Fixture::new();
        let args = vec!["-c".to_string(), "(sleep 30) & exit 0".to_string()];
        let sandbox = fixture.sandbox(
            SandboxCommand::new("/bin/sh", args.clone()).with_timeout(Duration::from_secs(5)),
            vec![AllowedInvocation::new("/bin/sh", args)],
        );
        let started = Instant::now();
        let reading = run(&sandbox, &fixture, "probe-descendant").expect("reading");
        assert!(
            started.elapsed() < CAPTURE_JOIN_GRACE + Duration::from_secs(2),
            "normal exit with a lingering descendant must not hang the probe"
        );
        let receipt = receipt_for(&reading);
        assert_eq!(receipt.exit_code, Some(0));
        assert!(!receipt.timed_out);
    }
}
