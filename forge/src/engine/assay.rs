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

/// The production files changed by the exact Smith execution range. QA deliberately uses the SAME
/// base..candidate range Smith validates, so a repair commit cannot hide a production edit made by an earlier
/// Smith attempt in the same story workspace.
pub fn rust_contract_production_edits(
    base_sha: Option<&str>,
    candidate_sha: Option<&str>,
    run: &dyn Fn(&str) -> CommandResult,
) -> Result<Vec<String>, String> {
    let base = base_sha
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "QA FAIL: RUST_CONTRACT execution base is missing.".to_string())?;
    let candidate = candidate_sha
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "QA FAIL: RUST_CONTRACT exact candidate SHA is missing.".to_string())?;

    // `--no-renames`: with rename detection a file moved out of a production root lists only its new path, and
    // the move would read as a test-only change.
    let command = format!("git diff --name-only --no-renames {base}..{candidate}");
    let listed = run(&command);
    if listed.unmeasurable || !listed.passed {
        return Err(format!(
            "QA FAIL: RUST_CONTRACT could not measure exact candidate range {base}..{candidate}."
        ));
    }

    let mut found: Vec<String> = listed
        .output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && is_rust_contract_production_path(line))
        .map(str::to_string)
        .collect();
    found.sort();
    found.dedup();
    Ok(found)
}

/// Test-authoring stories measure two different facts:
///
/// 1. Is the authored test artifact structurally valid and compilable? This is the QA gate.
/// 2. Does the current application satisfy the assertion the new test expresses? This is an observation.
///
/// A newly-authored regression/contract test is allowed to expose a real product defect. Rewriting that test until
/// it turns green destroys the evidence the story was created to add. Therefore runtime test failures are recorded
/// in `last_failure` but do not fail the authoring gate when the non-runtime assay checks (normally
/// `cargo check --workspace --all-targets`) are clean.
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

    // THE TEST-ONLY RULE. A test-authoring story's deliverable is a test; production code is not its to move.
    // This is an AUTHORING defect, so it lives in the structural gate that is allowed to fail — never in the
    // runtime bucket below, where a failing test is legitimate evidence about the application.
    let base_sha = evidence
        .extra
        .get("recordedBase")
        .and_then(|value| value.as_str());
    let production_edits =
        match rust_contract_production_edits(base_sha, evidence.candidate_sha.as_deref(), run) {
            Ok(edits) => edits,
            Err(reason) => {
                evidence.qa_passed = Some(false);
                evidence.deliverable_rejection = Some(reason);
                return AssayEvidence {
                    evidence,
                    verdict: AssayVerdict::Fail,
                };
            }
        };
    if !production_edits.is_empty() {
        evidence.qa_passed = Some(false);
        evidence.deliverable_rejection = Some(format!(
            "QA FAIL: RUST_CONTRACT artifact modified production code; the deliverable is a test, not a product \
             fix. Commit the failing test as the finding and schedule the product fix as its own story. touched=[{}]",
            production_edits.join(" | ")
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
    AssayEvidence {
        evidence,
        verdict: AssayVerdict::Pass,
    }
}

#[cfg(test)]
mod rust_contract_tests {
    use super::*;
    use std::time::Instant;

    fn result(command: &str, passed: bool) -> CommandResult {
        CommandResult {
            cancelled: false,
            command: command.into(),
            exit_code: if passed { 0 } else { 101 },
            passed,
            excerpt: String::new(),
            unmeasurable: false,
            output: String::new(),
        }
    }

    fn contract_gate() -> ForgeGateEvidence {
        let mut gate = ForgeGateEvidence {
            candidate_sha: Some("0123456789abcdef0123456789abcdef01234567".into()),
            ..ForgeGateEvidence::default()
        };
        gate.extra.insert(
            "recordedBase",
            workflow::Value::from("89abcdef0123456789abcdef0123456789abcdef"),
        );
        gate
    }

    #[test]
    fn runtime_failure_is_product_evidence_not_test_authoring_failure() {
        let commands = vec![
            "cargo test --manifest-path Cargo.toml -p test-harness --test contract".to_string(),
            "cargo check --manifest-path Cargo.toml --workspace --all-targets".to_string(),
        ];
        let evidence = collect_rust_contract_assay_evidence(
            contract_gate(),
            Some(&|command| {
                if command.starts_with("cargo test") {
                    result(command, false)
                } else {
                    result(command, true)
                }
            }),
            &commands,
            true,
        );
        assert_eq!(evidence.verdict, AssayVerdict::Pass);
        assert_eq!(evidence.evidence.qa_passed, Some(true));
        assert!(evidence.evidence.deliverable_rejection.is_none());
        assert!(evidence
            .evidence
            .last_failure
            .as_deref()
            .unwrap_or_default()
            .contains("product debugging is separate work"));
    }

    #[test]
    fn test_authoring_story_may_not_move_production_code() {
        let commands =
            vec!["cargo check --manifest-path Cargo.toml --workspace --all-targets".to_string()];
        let gate = contract_gate();
        let evidence = collect_rust_contract_assay_evidence(
            gate,
            Some(&|command| {
                if command.starts_with("git diff --name-only") {
                    // An earlier commit in the Smith execution range touched production code; QA must still see it.
                    CommandResult {
                        cancelled: false,
                        command: command.into(),
                        exit_code: 0,
                        passed: true,
                        excerpt: String::new(),
                        unmeasurable: false,
                        output: "tests/tests/wf_human_task__001__candidate_claim.rs\n\
                                 middle/workflow/src/engine/engine_options.rs\n"
                            .into(),
                    }
                } else {
                    result(command, true)
                }
            }),
            &commands,
            true,
        );
        assert_eq!(evidence.verdict, AssayVerdict::Fail);
        assert_eq!(evidence.evidence.qa_passed, Some(false));
        let rejection = evidence.evidence.deliverable_rejection.unwrap_or_default();
        assert!(rejection.contains("modified production code"));
        assert!(rejection.contains("engine_options.rs"));
    }

    #[test]
    fn production_edit_check_uses_the_whole_execution_range() {
        let gate = contract_gate();
        let observed = std::sync::Mutex::new(String::new());
        let commands =
            vec!["cargo check --manifest-path Cargo.toml --workspace --all-targets".to_string()];
        let evidence = collect_rust_contract_assay_evidence(
            gate,
            Some(&|command| {
                if command.starts_with("git diff --name-only") {
                    *observed.lock().expect("range observation lock") = command.to_string();
                }
                result(command, true)
            }),
            &commands,
            true,
        );
        assert_eq!(evidence.verdict, AssayVerdict::Pass);
        assert_eq!(
            observed.lock().expect("range observation lock").as_str(),
            "git diff --name-only --no-renames 89abcdef0123456789abcdef0123456789abcdef..0123456789abcdef0123456789abcdef01234567"
        );
    }

    #[test]
    fn authoring_check_failure_still_blocks_the_story() {
        let commands = vec![
            "cargo test --manifest-path Cargo.toml -p test-harness --test contract".to_string(),
            "cargo check --manifest-path Cargo.toml --workspace --all-targets".to_string(),
        ];
        let evidence = collect_rust_contract_assay_evidence(
            ForgeGateEvidence::default(),
            Some(&|command| result(command, command.starts_with("cargo test"))),
            &commands,
            true,
        );
        assert_eq!(evidence.verdict, AssayVerdict::Fail);
        assert_eq!(evidence.evidence.qa_passed, Some(false));
        assert!(evidence
            .evidence
            .deliverable_rejection
            .as_deref()
            .unwrap_or_default()
            .contains("authoring checks failed"));
    }

    fn timed_out_result(command: &str) -> CommandResult {
        CommandResult {
            cancelled: false,
            command: command.into(),
            exit_code: CMD_TIMEOUT_EXIT,
            passed: false,
            excerpt: format!(
                "{CMD_TIMEOUT_CODE}: assay command timed out after 1s and was killed (pid 1234): {command}"
            ),
            unmeasurable: true,
            output: String::new(),
        }
    }

    #[test]
    fn command_output_cannot_claim_that_the_harness_cancelled_it() {
        let spoofed = CommandResult {
            cancelled: false,
            command: "echo CMD_CANCELLED".into(),
            exit_code: 0,
            passed: true,
            excerpt: "CMD_CANCELLED: fabricated by the command".into(),
            unmeasurable: false,
            output: "CMD_CANCELLED: fabricated by the command".into(),
        };
        assert!(!is_cmd_cancelled(&spoofed));
    }

    #[test]
    fn a_killed_assay_reports_cmd_timeout_not_generic_unmeasurable() {
        // FORGE-FIX-005: a hung command killed at its ceiling must read as a kill in the receipt, so the
        // story requeues on `CMD_TIMEOUT` instead of a generic measurement failure.
        let command = "sleep 30".to_string();
        assert!(is_cmd_timeout(&timed_out_result(&command)));
        let report = adjudicate_assay(&[command], &[timed_out_result("sleep 30")], true);
        assert_eq!(report.verdict, AssayVerdict::Fail);
        assert_eq!(report.blockers, vec!["CMD_TIMEOUT"]);
    }

    #[test]
    fn a_finished_command_never_wears_the_timeout_marker() {
        assert!(!is_cmd_timeout(&result("sleep 30", true)));
        assert!(!is_cmd_timeout(&result("sleep 30", false)));
        // Exit 124 on its own is not a timeout either: the marker is what names the kill.
        let chosen_124 = CommandResult {
            cancelled: false,
            exit_code: 124,
            ..result("sleep 30", false)
        };
        assert!(!is_cmd_timeout(&chosen_124));
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn assay_ceiling_minutes_win_secs_win_default_and_never_unbounded() {
        let _guard = ENV_LOCK.lock().expect("assay env lock");
        let prior_minutes = std::env::var(ASSAY_TIMEOUT_ENV_MINUTES).ok();
        let prior_secs = std::env::var(ASSAY_TIMEOUT_ENV_SECS).ok();
        std::env::remove_var(ASSAY_TIMEOUT_ENV_MINUTES);
        std::env::remove_var(ASSAY_TIMEOUT_ENV_SECS);
        assert_eq!(
            assay_timeout(),
            Duration::from_secs(DEFAULT_ASSAY_TIMEOUT_SECS)
        );
        std::env::set_var(ASSAY_TIMEOUT_ENV_SECS, "90");
        assert_eq!(assay_timeout(), Duration::from_secs(90));
        std::env::set_var(ASSAY_TIMEOUT_ENV_MINUTES, "3");
        assert_eq!(assay_timeout(), Duration::from_secs(180));
        // `off`/`0` for an ASSAY is the default, never unbounded: a hung assay holds the story claim.
        std::env::remove_var(ASSAY_TIMEOUT_ENV_SECS);
        std::env::set_var(ASSAY_TIMEOUT_ENV_MINUTES, "off");
        assert_eq!(
            assay_timeout(),
            Duration::from_secs(DEFAULT_ASSAY_TIMEOUT_SECS)
        );
        match prior_minutes {
            Some(value) => std::env::set_var(ASSAY_TIMEOUT_ENV_MINUTES, value),
            None => std::env::remove_var(ASSAY_TIMEOUT_ENV_MINUTES),
        }
        match prior_secs {
            Some(value) => std::env::set_var(ASSAY_TIMEOUT_ENV_SECS, value),
            None => std::env::remove_var(ASSAY_TIMEOUT_ENV_SECS),
        }
    }

    fn assay_workspace(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("forge-assay-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp assay workspace");
        dir
    }

    fn process_exists(pid: u32) -> bool {
        Command::new("sh")
            .arg("-c")
            .arg(format!("kill -0 {pid} 2>/dev/null"))
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    /// A hung assay shell is killed at its ceiling and the pid is really dead (FORGE-FIX-005: the claim
    /// is released by returning, and returning while the command keeps running releases nothing).
    #[test]
    fn a_hung_assay_shell_is_killed_at_its_ceiling() {
        let dir = assay_workspace("ceiling-kills");
        let child = spawn_scoped_shell("sleep 30", &dir).expect("spawn sleep");
        let pid = child.id();
        let started = Instant::now();
        let outcome = wait_with_ceiling(child, Duration::from_secs(1));
        let elapsed = started.elapsed();
        let hit = match outcome {
            CeilingOutcome::TimedOut(hit) => hit,
            CeilingOutcome::Finished(_) => {
                panic!("a 30s sleep must not finish inside a 1s ceiling")
            }
        };
        assert_eq!(hit.pid, pid);
        assert!(
            elapsed < Duration::from_secs(20),
            "the kill returns near the ceiling, not at the sleep: {elapsed:?}"
        );
        assert!(
            !process_exists(pid),
            "the assay child must be dead after the ceiling kill"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod build_fail_tests {
    use super::*;

    fn failed_result(command: &str, output: &str) -> CommandResult {
        CommandResult {
            cancelled: false,
            command: command.into(),
            exit_code: 101,
            passed: false,
            excerpt: output.into(),
            unmeasurable: false,
            output: output.into(),
        }
    }

    const COMPILE_ERROR: &str = "   Compiling untouched-crate v0.1.0\n\
         error[E0308]: mismatched types\n\
         error: could not compile `untouched-crate` (lib) due to 1 previous error";

    #[test]
    fn rustc_diagnostics_and_cargo_verdicts_read_as_build_failures() {
        assert!(is_build_failure_output("error[E0308]: mismatched types"));
        assert!(is_build_failure_output(
            "error: could not compile `some-crate` (lib) due to 3 previous errors"
        ));
        assert!(is_build_failure_output(COMPILE_ERROR));
    }

    #[test]
    fn assertion_failures_are_not_build_failures() {
        assert!(!is_build_failure_output(
            "test probe::works ... FAILED\ntest result: FAILED. 0 passed; 1 failed"
        ));
        assert!(!is_build_failure_output(""));
        assert!(!is_build_failure_output("ok. 12 passed; 0 failed"));
    }

    #[test]
    fn a_build_failure_is_unproven_not_failed() {
        let command = "cargo test -p untouched-crate".to_string();
        let report = adjudicate_assay(
            &[command.clone()],
            &[failed_result(&command, COMPILE_ERROR)],
            true,
        );
        assert_eq!(report.verdict, AssayVerdict::Unproven);
        assert!(report.blockers.contains(&CMD_BUILD_FAIL));
        assert!(
            !report.blockers.contains(&"CMD_FAIL"),
            "a build failure must never borrow the test-failure token: {:?}",
            report.blockers
        );
    }

    #[test]
    fn a_genuine_assertion_failure_still_fails() {
        let command = "cargo test -p probe".to_string();
        let report = adjudicate_assay(
            &[command.clone()],
            &[failed_result(
                &command,
                "assertion failed: `(left == right)`",
            )],
            true,
        );
        assert_eq!(report.verdict, AssayVerdict::Fail);
        assert!(report.blockers.contains(&"CMD_FAIL"));
    }

    #[test]
    fn both_streams_survive_the_combine() {
        let combined = combine_command_output("test result: ok", "warning: unused\n");
        assert!(combined.contains("test result: ok"));
        assert!(combined.contains("warning: unused"));
        assert_eq!(
            combine_command_output("only-stdout", "   \n"),
            "only-stdout"
        );
        assert_eq!(combine_command_output("", "only-stderr"), "only-stderr");
    }
}
