//! Port of agents/qa/run.ts adjudicate + assay-collect.ts (no SHA conjunct).

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use crate::engine::facts::ForgeGateEvidence;

#[derive(Debug, Clone)]
pub struct CommandResult {
    /// Set only by the harness after observing that it stopped the process via cancellation.
    /// Command output and exit codes are untrusted evidence and cannot assert cancellation.
    pub cancelled: bool,
    pub command: String,
    pub exit_code: i32,
    pub passed: bool,
    pub excerpt: String,
    pub unmeasurable: bool,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssayVerdict {
    Pass,
    Fail,
    Unproven,
}

/// A measured command failure that is a BUILD failure, not a test failure.
///
/// WHY THIS EXISTS. A lane that cannot compile cannot run its tests, and recording that as
/// `CMD_FAIL` sends a toolchain fault down the product-repair road (FORGE-FIX-003, rescoped
/// 2026-10-08: the batch-45 Failed-without-assays class). The blocker is distinct so the story
/// is never marked Failed for it; the QA verdict reads it as UNPROVEN (escalate), not FAIL.
pub const CMD_BUILD_FAIL: &str = "CMD_BUILD_FAIL";
/// Distinct sentinel for a command stopped by a cancellation signal.
pub const CMD_CANCELLED_EXIT: i32 = -2;

/// Whether the process ended by an external interrupt/termination signal instead of its own exit.
pub fn cancellation_signal(status: &std::process::ExitStatus) -> Option<i32> {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        return status.signal().filter(|signal| matches!(signal, 2 | 15));
    }
    #[cfg(not(unix))]
    {
        let _ = status;
        None
    }
}

pub fn is_cmd_cancelled(result: &CommandResult) -> bool {
    result.cancelled
}

/// Lines of evidence kept in a [`CommandResult`] excerpt by the harness runners.
/// Parity with the pianola executor's `ASSAY_EXCERPT_LINES`: the excerpt is what artifact rows
/// carry, so both runners keep the same window.
pub const COMMAND_EXCERPT_LINES: usize = 500;

/// True when command output shows the Rust toolchain failing to BUILD rather than a test
/// failing: a rustc diagnostic (`error[E0308]`, ...) or cargo's own verdict
/// (`error: could not compile ...`). Checked against the COMBINED stdout+stderr output — cargo
/// prints diagnostics on stderr, so a stream-dropping capture would never see them (the
/// evidence-preservation half of FORGE-FIX-003).
pub fn is_build_failure_output(output: &str) -> bool {
    output.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("error[") || line.contains("could not compile")
    })
}

/// Combine the two process streams into the one evidence string a [`CommandResult`] carries.
/// Stdout first, stderr appended — cargo diagnostics print to stderr while test harnesses print
/// to stdout, and keeping only one silently discards the compiler error (FORGE-FIX-003). Same
/// rule as the pianola executor; the harness runners share it from here.
pub fn combine_command_output(stdout: &str, stderr: &str) -> String {
    let mut combined = stdout.to_string();
    if !stderr.trim().is_empty() {
        if !combined.is_empty() && !combined.ends_with('\n') {
            combined.push('\n');
        }
        combined.push_str(stderr);
    }
    combined
}

#[derive(Debug, Clone)]
pub struct AssayReport {
    pub verdict: AssayVerdict,
    pub blockers: Vec<&'static str>,
}

pub fn adjudicate_assay(
    commands: &[String],
    results: &[CommandResult],
    acceptance_mapped: bool,
) -> AssayReport {
    if commands.is_empty() {
        return AssayReport {
            verdict: AssayVerdict::Fail,
            blockers: vec!["NO_ASSAY_COMMANDS"],
        };
    }
    if results.iter().any(is_cmd_cancelled) {
        return AssayReport {
            verdict: AssayVerdict::Unproven,
            blockers: vec!["CMD_CANCELLED"],
        };
    }
    if results.iter().any(is_cmd_timeout) {
        return AssayReport {
            verdict: AssayVerdict::Fail,
            blockers: vec!["CMD_TIMEOUT"],
        };
    }
    if results.iter().any(|r| r.unmeasurable) {
        return AssayReport {
            verdict: AssayVerdict::Fail,
            blockers: vec!["COMMAND_UNMEASURABLE"],
        };
    }
    if results.iter().any(|r| !r.passed) {
        // A build failure is measured, but it is NOT a test failure: the toolchain never ran the
        // tests. It gets its own blocker and an UNPROVEN reading (escalate, not repair) so the
        // story is never marked Failed for it. A genuine assertion failure still reads CMD_FAIL.
        let mut test_failed = false;
        let mut build_failed = false;
        for result in results.iter().filter(|r| !r.passed) {
            if is_build_failure_output(&result.output) || is_build_failure_output(&result.excerpt) {
                build_failed = true;
            } else {
                test_failed = true;
            }
        }
        let mut blockers = Vec::new();
        if test_failed {
            blockers.push("CMD_FAIL");
        }
        if build_failed {
            blockers.push(CMD_BUILD_FAIL);
        }
        return AssayReport {
            verdict: if test_failed {
                AssayVerdict::Fail
            } else {
                AssayVerdict::Unproven
            },
            blockers,
        };
    }
    if !acceptance_mapped {
        return AssayReport {
            verdict: AssayVerdict::Unproven,
            blockers: vec!["ACCEPTANCE_MAP_MISSING"],
        };
    }
    AssayReport {
        verdict: AssayVerdict::Pass,
        blockers: vec![],
    }
}

/// The blocker for an assay command that outlived its ceiling and was killed. It is deliberately NOT
/// `COMMAND_UNMEASURABLE`: a timeout held the story claim until the ceiling fired, and the receipt must say
/// the command was killed, not merely that nothing could be measured.
pub const CMD_TIMEOUT_CODE: &str = "CMD_TIMEOUT";

/// The exit code a killed assay command reports. Matches `timeout(1)`'s 124, so a reader that has seen a
/// coreutils timeout already knows what this one is.
pub const CMD_TIMEOUT_EXIT: i32 = 124;

/// The operator's per-command assay ceiling, in minutes. New for FORGE-FIX-005: a hung assay held the story
/// claim indefinitely because the `sh -c` shells had no deadline at all.
pub const ASSAY_TIMEOUT_ENV_MINUTES: &str = "FORGE_ASSAY_TIMEOUT_MINUTES";

/// The older per-command assay ceiling, in seconds. Kept as a fallback so existing lanes keep their bound;
/// `FORGE_ASSAY_TIMEOUT_MINUTES` wins when it parses.
pub const ASSAY_TIMEOUT_ENV_SECS: &str = "FORGE_ASSAY_TIMEOUT_SECS";

/// The ceiling when the operator set none: long enough for any suite measured so far, short enough that a
/// hung command cannot hold a claim and a worker slot overnight. Same 20 minutes `pianola::worker_exec`
/// already used via `FORGE_ASSAY_TIMEOUT_SECS`.
pub const DEFAULT_ASSAY_TIMEOUT_SECS: u64 = 1200;

fn default_assay_timeout() -> Duration {
    Duration::from_secs(DEFAULT_ASSAY_TIMEOUT_SECS)
}

/// The ceiling one assay command runs under. Unset or unreadable is the default. `0`/`off`/`none` is ALSO
/// the default, never unbounded: a hung assay holds the story claim, and no claim may be held forever.
/// That is the deliberate difference from the turn ceiling, where `off` is the operator's explicit
/// unbounded choice for a model turn that is still emitting.
pub fn assay_timeout() -> Duration {
    if let Ok(raw) = std::env::var(ASSAY_TIMEOUT_ENV_MINUTES) {
        if let Ok(minutes) = raw.trim().parse::<u64>() {
            if minutes > 0 {
                return Duration::from_secs(minutes * 60);
            }
        }
    }
    if let Ok(raw) = std::env::var(ASSAY_TIMEOUT_ENV_SECS) {
        if let Ok(secs) = raw.trim().parse::<u64>() {
            if secs > 0 {
                return Duration::from_secs(secs);
            }
        }
    }
    default_assay_timeout()
}

/// True when the result is a killed-at-ceiling assay command: unmeasurable, the timeout exit, and the
/// `CMD_TIMEOUT` marker in the excerpt. The marker is what distinguishes a kill from any other
/// unmeasurable command; the exit alone could be a command that chose 124 for itself.
pub fn is_cmd_timeout(result: &CommandResult) -> bool {
    result.unmeasurable
        && result.exit_code == CMD_TIMEOUT_EXIT
        && (result.excerpt.contains(CMD_TIMEOUT_CODE) || result.output.contains(CMD_TIMEOUT_CODE))
}

/// Spawn one assay line as `sh -c <command>` scoped to the lane directory, with the pipes open and stdin
/// closed.
///
/// The child is its own process group (unix), so the ceiling kill below reaches the whole command tree.
/// Stdin is null: a command that reads stdin would otherwise block forever on a terminal that never
/// answers, and an assay must never wait on anything but its own ceiling.
pub fn spawn_scoped_shell(command: &str, cwd: &Path) -> io::Result<Child> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn()
}

/// Spawn one assay line as `sh -c <command>` exactly like [`spawn_scoped_shell`], but with a
/// caller-supplied environment instead of the inherited one.
///
/// WHY THIS EXISTS. Three fixes meet in the assay shell: FIX-007 isolates each worker with a
/// sanitized environment (no secrets) plus its own `CARGO_TARGET_DIR`, FIX-005 bounds the wait
/// with a ceiling that kills the whole process tree, and FIX-003 keeps both output streams as
/// evidence. The inherited-environment spawn cannot carry FIX-007's env into the bounded wait,
/// so the harnesses build the assay env and hand it here; the environment is cleared first so
/// nothing the caller filtered out leaks back in through inheritance.
pub fn spawn_scoped_shell_with_env(
    command: &str,
    cwd: &Path,
    env: &HashMap<String, String>,
) -> io::Result<Child> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .envs(env);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn()
}

/// What a ceiling kill leaves behind: which child was stopped, and which ceiling stopped it. The pid is
/// what the assay test asserts is dead — a timeout that returns while the command keeps running has not
/// released anything.
pub struct CeilingHit {
    pub pid: u32,
    pub ceiling: Duration,
}

/// What waiting for an assay child produced: either the wait finished (with whatever the OS said about
/// it), or the ceiling fired and the tree was killed. The two are kept apart on purpose: a wait that
/// failed on its own is an observation failure, while a ceiling kill is a `CMD_TIMEOUT` — collapsing them
/// would let a dead pid read as a hung command.
pub enum CeilingOutcome {
    Finished(io::Result<std::process::Output>),
    TimedOut(CeilingHit),
}

/// Wait for an assay child up to `timeout`, then kill its tree and return.
///
/// The wait runs on its own thread (`wait_with_output` drains both pipes, so a chatty command cannot
/// deadlock a full pipe buffer) while the caller waits on the channel with the ceiling. On expiry the
/// turn's own stop primitive (`RunningTurn::terminate`: TERM the tree, re-enumerate, escalate to KILL)
/// runs, and the timeout returns at once — the reader thread is deliberately NOT joined, the same posture
/// as the streamed turn: a pipe an orphan holds is not part of this command any more, and the claim is
/// released by returning, not by waiting for the orphan.
pub fn wait_with_ceiling(child: Child, timeout: Duration) -> CeilingOutcome {
    let pid = child.id();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let output = child.wait_with_output();
        let _ = tx.send(output);
    });
    match rx.recv_timeout(timeout) {
        Ok(output) => CeilingOutcome::Finished(output),
        Err(_) => {
            let _ = crate::engine::opencode_client::RunningTurn { pid }.terminate();
            CeilingOutcome::TimedOut(CeilingHit {
                pid,
                ceiling: timeout,
            })
        }
    }
}

pub(crate) fn is_rust_contract_runtime_test(command: &str) -> bool {
    let command = command.trim_start();
    command == "cargo test"
        || command.starts_with("cargo test ")
        || command == "cargo nextest"
        || command.starts_with("cargo nextest ")
}

/// The roots that hold PRODUCTION code. A test-authoring story may not change anything under these: the
/// deliverable of a RUST_CONTRACT story is a TEST ARTIFACT, never a product fix.
///
/// A test that fails against this code is a FINDING to be scheduled as its own product work. Moving, relaxing
/// or silencing production code so a test turns green destroys the very evidence the story exists to add — the
/// point of the test is to FIND the bug, not to hide it.
const PRODUCTION_ROOTS: [&str; 5] = ["web/", "middle/", "db/", "cli/", "forge/"];

pub fn is_rust_contract_production_path(path: &str) -> bool {
    let path = path.trim();
    PRODUCTION_ROOTS.iter().any(|root| path.starts_with(root))
}

/// Test-authoring stories measure two different facts:
///
/// 1. Is the authored test artifact structurally valid and compilable? This is the QA gate.
/// 2. Does the current application satisfy the assertion the new test expresses? This is an observation.
///
/// A newly-authored regression/contract test is allowed to expose a real product defect. Rewriting that test until
/// it turns green destroys the evidence the story was created to add. Therefore runtime test failures are recorded
/// in `last_failure` but do not fail the authoring gate when the non-runtime assay checks (normally
/// `cargo check --workspace --all-targets`) are clean. Source ownership stays with Smith; QA reads no Git diff.
pub fn collect_rust_contract_assay_evidence(
    mut evidence: ForgeGateEvidence,
    run_command: Option<&dyn Fn(&str) -> CommandResult>,
    assay_commands: &[String],
    acceptance_mapped: bool,
) -> AssayEvidence {
    let Some(run) = run_command else {
        evidence.qa_passed = Some(false);
        evidence.deliverable_rejection =
            Some("QA FAIL: the RUST_CONTRACT lane had no command runner.".into());
        return AssayEvidence {
            evidence,
            verdict: AssayVerdict::Fail,
        };
    };

    if assay_commands.is_empty() {
        evidence.qa_passed = Some(false);
        evidence.deliverable_rejection =
            Some("QA FAIL: RUST_CONTRACT has no assay commands.".into());
        return AssayEvidence {
            evidence,
            verdict: AssayVerdict::Fail,
        };
    }

    let results: Vec<CommandResult> = assay_commands.iter().map(|command| run(command)).collect();

    if results.iter().any(|result| result.unmeasurable) {
        evidence.qa_passed = Some(false);
        evidence.deliverable_rejection =
            Some("QA FAIL: RUST_CONTRACT assay command was unmeasurable.".into());
        return AssayEvidence {
            evidence,
            verdict: AssayVerdict::Fail,
        };
    }

    if !acceptance_mapped {
        evidence.qa_passed = Some(false);
        evidence.deliverable_rejection =
            Some("QA UNPROVEN: RUST_CONTRACT acceptance mapping is missing.".into());
        return AssayEvidence {
            evidence,
            verdict: AssayVerdict::Unproven,
        };
    }

    let structural: Vec<&CommandResult> = results
        .iter()
        .filter(|result| !is_rust_contract_runtime_test(&result.command))
        .collect();
    if structural.is_empty() {
        evidence.qa_passed = Some(false);
        evidence.deliverable_rejection = Some(
            "QA FAIL: RUST_CONTRACT needs a non-runtime authoring check (for example cargo check --all-targets)."
                .into(),
        );
        return AssayEvidence {
            evidence,
            verdict: AssayVerdict::Fail,
        };
    }

    let structural_failures: Vec<String> = structural
        .iter()
        .filter(|result| !result.passed)
        .map(|result| result.command.clone())
        .collect();
    if !structural_failures.is_empty() {
        evidence.qa_passed = Some(false);
        evidence.deliverable_rejection = Some(format!(
            "QA FAIL: RUST_CONTRACT authoring checks failed=[{}]",
            structural_failures.join(" | ")
        ));
        return AssayEvidence {
            evidence,
            verdict: AssayVerdict::Fail,
        };
    }

    let runtime_failures: Vec<String> = results
        .iter()
        .filter(|result| is_rust_contract_runtime_test(&result.command) && !result.passed)
        .map(|result| result.command.clone())
        .collect();

    evidence.qa_passed = Some(true);
    evidence.deliverable_rejection = None;
    // A CLEAN PASS RESOLVES THE FAILURE THAT SENT THE STORY TO REPAIR (TST-FORGE-QA-013). `deliverable_rejection`
    // above is the CURRENT rejection; `last_failure` is the previous stage's durable reason
    // (`forge_last_failure_reason`, what the work queue reads). A story that failed a stage, was repaired, and then
    // measured clean is no longer a failure, so the reason that opened the repair is cleared with the pass. The
    // release executor's publish-success path is the durable merge's other resolution
    // (`db::ForgeEngineDao::merge_workflow_evidence` nulls `last_failure` when `release_failure_resolved` is set);
    // this is the QA half of the same rule.
    if runtime_failures.is_empty() {
        evidence.last_failure = None;
    }
    if !runtime_failures.is_empty() {
        evidence.last_failure = Some(format!(
            "RUST_CONTRACT observation: authored test currently fails against existing application code; \
             test artifact accepted and product debugging is separate work. failed=[{}]",
            runtime_failures.join(" | ")
        ));
    }

    AssayEvidence {
        evidence,
        verdict: AssayVerdict::Pass,
    }
}

/// The QA lane's reading, with the durable evidence it produced.
///
/// The two are kept apart on purpose: `verdict` is the assay's **own three-way reading** (`PASS`/`FAIL`/`UNPROVEN`,
/// the token the `forge_tool_artifact` row carries), while `evidence.qa_passed` is the gate's boolean. Collapsing
/// "not proven" into "failed" inside the evidence is the gate's business; it must not erase what the lane measured.
pub struct AssayEvidence {
    pub evidence: ForgeGateEvidence,
    pub verdict: AssayVerdict,
}

pub fn collect_assay_evidence(
    mut evidence: ForgeGateEvidence,
    run_command: Option<&dyn Fn(&str) -> CommandResult>,
    assay_commands: &[String],
    acceptance_mapped: bool,
) -> AssayEvidence {
    let Some(run) = run_command else {
        evidence.qa_passed = Some(false);
        evidence.deliverable_rejection =
            Some("QA FAIL: the lane was handed assay commands but no way to run them.".into());
        return AssayEvidence {
            evidence,
            verdict: AssayVerdict::Fail,
        };
    };
    let results: Vec<CommandResult> = assay_commands.iter().map(|c| run(c)).collect();
    let report = adjudicate_assay(assay_commands, &results, acceptance_mapped);
    if report.verdict != AssayVerdict::Pass {
        let failed: Vec<String> = results
            .iter()
            .filter(|r| !r.passed && !r.unmeasurable)
            .map(|r| r.command.clone())
            .collect();
        evidence.qa_passed = Some(false);
        evidence.deliverable_rejection = Some(format!(
            "QA {:?}: blockers=[{}] failed=[{}]",
            report.verdict,
            report.blockers.join(", "),
            failed.join(" | ")
        ));
        return AssayEvidence {
            evidence,
            verdict: report.verdict,
        };
    }
    evidence.qa_passed = Some(true);
    // The general measurement path carries the same rule as the RUST_CONTRACT path above: a pass resolves the
    // reason the story was sent to repair, so it must not survive the pass (TST-FORGE-QA-013). A FAIL returns
    // above and leaves the reason standing.
    evidence.deliverable_rejection = None;
    evidence.last_failure = None;
    AssayEvidence {
        evidence,
        verdict: AssayVerdict::Pass,
    }
}

#[cfg(test)]
mod rust_contract_tests;

#[cfg(test)]
mod build_fail_tests;
