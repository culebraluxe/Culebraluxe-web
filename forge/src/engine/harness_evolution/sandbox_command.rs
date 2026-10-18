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
//! * **Allowlisted program.** Only programs this sandbox was configured with may run; the default
//!   allowlist is four harmless POSIX binaries. `/bin/sh` is deliberately *not* allowed by default.
//! * **No production.** A production (or otherwise non-probe) environment is refused before the
//!   command is spawned, via the same guard the replay executor uses.
//! * **Isolated workspace.** Each execution gets a fresh directory under the run root, with
//!   `env_clear()` and a minimal environment (`HOME`/`TMPDIR` point into that workspace), stdin from
//!   `/dev/null`, and a hard timeout. Nothing from the parent environment (tokens, URLs) leaks in.
//! * **Bounded capture.** stdout/stderr are captured to files (no pipe deadlock) and read back
//!   truncated to a fixed cap; the exit status and duration are recorded.
//! * **Durable local receipt.** Each execution writes a JSON receipt under the run root and returns
//!   refs to it. This is a *file* receipt, not a `forge_harness_probe` row: the DB row is written by
//!   the real executor wiring (out of scope here), so the ref is honest about what exists.
//!
//! The behavioral reading is coarse on purpose: exit status maps to success/tests and the QA proxy
//! verdict, steps = 1, and cost stays `None` (unpriced, never zero). A command is not a model, and
//! the plan/contrast stages treat it as behavioral evidence only.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
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

/// How much of stdout/stderr is captured per stream. The rest is discarded with a marker.
pub const DEFAULT_OUTPUT_CAP_BYTES: usize = 64 * 1024;

/// The default allowlist: harmless POSIX binaries (with the two common install locations for
/// `echo`, so the defaults work on both macOS and Linux). No shell, no interpreter, no network
/// tool.
pub const DEFAULT_ALLOWED_PROGRAMS: [&str; 5] = [
    "/bin/echo",
    "/usr/bin/echo",
    "/usr/bin/false",
    "/bin/sleep",
    "/usr/bin/true",
];

/// Ref prefix for the execution id (the receipt's id for one run).
pub const EXEC_REF_PREFIX: &str = "sandbox-command:exec:";
/// Ref prefix for the receipt file.
pub const RECEIPT_REF_PREFIX: &str = "sandbox-command:receipt:";
/// Ref prefix for the program that ran.
pub const PROGRAM_REF_PREFIX: &str = "sandbox-command:program:";

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

    /// Configuration validation: absolute program, sane timeout, non-empty label.
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
    pub workspace: String,
}

/// The concrete command sandbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSandbox {
    command: SandboxCommand,
    allowed_programs: Vec<PathBuf>,
    run_root: PathBuf,
    output_cap: usize,
}

impl CommandSandbox {
    /// A sandbox for one command, using the default allowlist and the default run root.
    pub fn new(command: SandboxCommand) -> Self {
        Self {
            command,
            allowed_programs: DEFAULT_ALLOWED_PROGRAMS.iter().map(PathBuf::from).collect(),
            run_root: default_run_root(),
            output_cap: DEFAULT_OUTPUT_CAP_BYTES,
        }
    }

    /// Replace the allowlist. Tests use this to allow `/bin/sh`; production wiring should keep it
    /// as narrow as possible (a real model sandbox replaces this entirely).
    pub fn with_allowed_programs(mut self, programs: Vec<PathBuf>) -> Self {
        self.allowed_programs = programs;
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

    /// Validate the sandbox configuration without executing anything: absolute allowlisted program,
    /// bounded timeout, non-empty label, non-zero output cap.
    pub fn validate(&self) -> Result<(), ReplayError> {
        self.command.validate()?;
        if self.output_cap == 0 {
            return Err(ReplayError::SandboxRefused {
                sandbox: "command".into(),
                message: "output cap must be non-zero".into(),
            });
        }
        if !self.is_allowlisted(&self.command.program) {
            return Err(ReplayError::SandboxRefused {
                sandbox: "command".into(),
                message: format!(
                    "program {} is not allowlisted (allowed: {})",
                    self.command.program.display(),
                    self.allowed_programs
                        .iter()
                        .map(|program| program.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            });
        }
        Ok(())
    }

    fn is_allowlisted(&self, program: &Path) -> bool {
        self.allowed_programs
            .iter()
            .any(|allowed| allowed == program)
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
        fs::create_dir_all(&workspace).map_err(|error| ReplayError::SandboxFailed {
            probe_id: request.probe_id.to_string(),
            message: format!(
                "cannot create sandbox workspace {}: {error}",
                workspace.display()
            ),
        })?;
        let stdout_path = workspace.join("stdout.log");
        let stderr_path = workspace.join("stderr.log");
        let stdout_file =
            fs::File::create(&stdout_path).map_err(|error| ReplayError::SandboxFailed {
                probe_id: request.probe_id.to_string(),
                message: format!("cannot create {}: {error}", stdout_path.display()),
            })?;
        let stderr_file =
            fs::File::create(&stderr_path).map_err(|error| ReplayError::SandboxFailed {
                probe_id: request.probe_id.to_string(),
                message: format!("cannot create {}: {error}", stderr_path.display()),
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
            .stdout(Stdio::from(stdout_file))
            .stderr(Stdio::from(stderr_file));

        let started = Instant::now();
        let mut child = command
            .spawn()
            .map_err(|error| ReplayError::SandboxFailed {
                probe_id: request.probe_id.to_string(),
                message: format!("cannot spawn {}: {error}", self.command.program.display()),
            })?;

        let mut timed_out = false;
        let exit_status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => {
                    if started.elapsed() >= self.command.timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        timed_out = true;
                        break None;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    let _ = child.kill();
                    return Err(ReplayError::SandboxFailed {
                        probe_id: request.probe_id.to_string(),
                        message: format!("cannot wait for command: {error}"),
                    });
                }
            }
        };
        let duration = started.elapsed();

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
            stdout_excerpt: read_capped(&stdout_path, self.output_cap),
            stderr_excerpt: read_capped(&stderr_path, self.output_cap),
            workspace: workspace.display().to_string(),
        };
        let receipt_path = write_receipt(&self.run_root, &receipt)?;

        if timed_out {
            // A timeout is not behavioral evidence; it is a failed probe.
            return Err(ReplayError::SandboxFailed {
                probe_id: request.probe_id.to_string(),
                message: format!(
                    "command timed out after {:?}; receipt {}",
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

fn read_capped(path: &Path, cap: usize) -> String {
    let bytes = fs::read(path).unwrap_or_default();
    if bytes.len() <= cap {
        return String::from_utf8_lossy(&bytes).to_string();
    }
    let boundary = cap.min(bytes.len());
    format!(
        "{}\n[truncated at {boundary} bytes]",
        String::from_utf8_lossy(&bytes[..boundary])
    )
}

fn write_receipt(
    run_root: &Path,
    receipt: &SandboxExecutionReceipt,
) -> Result<PathBuf, ReplayError> {
    fs::create_dir_all(run_root).map_err(|error| ReplayError::SandboxFailed {
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
    fs::write(&path, body).map_err(|error| ReplayError::SandboxFailed {
        probe_id: receipt.probe_id.clone(),
        message: format!("cannot write sandbox receipt {}: {error}", path.display()),
    })?;
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

        fn sandbox(&self, command: SandboxCommand) -> CommandSandbox {
            CommandSandbox::new(command).with_run_root(self.run_root.path())
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
        let body = fs::read(reference).expect("receipt file");
        serde_json::from_slice(&body).expect("receipt json")
    }

    #[test]
    fn a_successful_command_returns_a_reading_with_a_durable_receipt_ref() {
        let fixture = Fixture::new();
        let sandbox = fixture.sandbox(SandboxCommand::new(
            "/bin/echo",
            vec!["sandbox-hello".into()],
        ));
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
        let sandbox = fixture.sandbox(SandboxCommand::new("/usr/bin/false", Vec::new()));
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
            SandboxCommand::new("/bin/sleep", vec!["5".into()])
                .with_timeout(Duration::from_millis(100)),
        );
        let error = run(&sandbox, &fixture, "probe-ref").unwrap_err();
        assert_eq!(error.code(), "replay_sandbox_failed");
        assert!(error.to_string().contains("timed out"));
        // The receipt for the killed execution still exists and is named in the error.
        assert!(error.to_string().contains("receipt"));
    }

    #[test]
    fn production_is_refused_before_any_workspace_or_receipt() {
        let fixture = Fixture::new();
        let sandbox = fixture.sandbox(SandboxCommand::new("/bin/echo", vec!["x".into()]));
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
            fs::read_dir(fixture.run_root.path())
                .map(|entries| entries.count())
                .unwrap_or(0),
            0,
            "production refusal creates no workspace and no receipt"
        );
    }

    #[test]
    fn a_program_outside_the_allowlist_is_refused() {
        let fixture = Fixture::new();
        let sandbox = fixture.sandbox(SandboxCommand::new("/usr/bin/env", Vec::new()));
        let error = run(&sandbox, &fixture, "probe-ref").unwrap_err();
        assert_eq!(error.code(), "replay_sandbox_refused");
        assert!(error.to_string().contains("not allowlisted"));
        assert_eq!(
            fs::read_dir(fixture.run_root.path())
                .map(|entries| entries.count())
                .unwrap_or(0),
            0,
            "an unallowlisted program is refused before any workspace exists"
        );
    }

    #[test]
    fn a_relative_program_is_refused_no_path_lookup() {
        let fixture = Fixture::new();
        let sandbox = fixture.sandbox(SandboxCommand::new("echo", vec!["x".into()]));
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
        let sandbox = fixture.sandbox(SandboxCommand::new("/bin/echo", metacharacters.clone()));
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
        // Nothing was injected into the workspace.
        assert!(!receipt.workspace.is_empty());
        let workspace = Path::new(&receipt.workspace);
        assert!(!workspace.join("pwned").exists());
        assert!(!workspace.join("pwned2").exists());
    }

    #[test]
    fn the_child_environment_is_cleared_and_home_points_into_the_workspace() {
        let fixture = Fixture::new();
        let sandbox = fixture
            .sandbox(SandboxCommand::new(
                "/bin/sh",
                vec!["-c".into(), "printf %s \"$HOME\"".into()],
            ))
            .with_allowed_programs(vec![PathBuf::from("/bin/sh")]);
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
    fn captured_output_is_bounded() {
        let fixture = Fixture::new();
        let sandbox = fixture
            .sandbox(SandboxCommand::new(
                "/bin/echo",
                vec!["0123456789abcdef".into()],
            ))
            .with_output_cap(8);
        let reading = run(&sandbox, &fixture, "probe-cap").expect("reading");
        let receipt = receipt_for(&reading);
        assert!(receipt.stdout_excerpt.contains("[truncated at 8 bytes]"));
        assert!(!receipt.stdout_excerpt.contains("0123456789abcdef"));
    }

    #[test]
    fn a_timeout_outside_the_bounds_is_refused_at_configuration_time() {
        let fixture = Fixture::new();
        let too_long = fixture.sandbox(
            SandboxCommand::new("/bin/echo", vec!["x".into()])
                .with_timeout(MAX_COMMAND_TIMEOUT + Duration::from_secs(1)),
        );
        assert_eq!(
            too_long.validate().unwrap_err().code(),
            "replay_sandbox_refused"
        );
        let zero = fixture.sandbox(
            SandboxCommand::new("/bin/echo", vec!["x".into()]).with_timeout(Duration::ZERO),
        );
        assert_eq!(
            zero.validate().unwrap_err().code(),
            "replay_sandbox_refused"
        );
    }

    #[test]
    fn two_executions_get_distinct_durable_refs() {
        let fixture = Fixture::new();
        let sandbox = fixture.sandbox(SandboxCommand::new("/bin/echo", vec!["x".into()]));
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
        let sandbox = fixture.sandbox(SandboxCommand::new("/usr/bin/true", Vec::new()));
        let reading = run(&sandbox, &fixture, "probe-roundtrip").expect("reading");
        let receipt = receipt_for(&reading);
        let encoded = serde_json::to_string(&receipt).expect("serialize");
        let decoded: SandboxExecutionReceipt = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(receipt, decoded);
        assert_eq!(receipt.exit_code, Some(0));
    }
}
