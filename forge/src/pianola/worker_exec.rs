//! TST assay execution, evidence recording, and local commit in lane.
//!
//! The worker half of the QA contract: the worker authors a test (see
//! `worker_authoring`), runs the story's prescribed assay commands, records
//! evidence through the existing Forge DAO paths, and commits locally in its
//! lane. It never pushes: the lane is a sandbox, not a release.
//!
//! * Assay runs each `assay_commands` line scoped to `worktree_path` via
//!   `std::process::Command` (`sh -c`), capturing exit code, combined
//!   output, and a 500-line excerpt. `exit_code == 0` maps to `passed`,
//!   non-zero to failed, and a command that cannot be spawned, observed, or
//!   that outlives its timeout to `unmeasurable`.
//! * The [`AssayReport`] comes from `engine::assay::adjudicate_assay`, the
//!   same adjudicator QA uses (`NO_ASSAY_COMMANDS`, `COMMAND_UNMEASURABLE`,
//!   `CMD_FAIL`, `ACCEPTANCE_MAP_MISSING`). A failing test may be the correct
//!   result when the story intentionally exposes an app defect: the failure
//!   is preserved, recorded as evidence, and never mutated green.
//! * Evidence uses the existing DAO paths only: `record_tool_artifact` (the
//!   one sanctioned write of `forge_tool_artifact`) and `append_run_detail`
//!   (the sanctioned evidence line on `storyboard_story_run`). The
//!   `Tests: <summary>` line follows the packet contract
//!   (`packet::extract_tests_summary`). The `merge_workflow_evidence` row
//!   itself stays owned by the completion ledger; workers only append detail.
//! * `commit_in_lane` verifies only test files changed, runs
//!   `git diff --check`, and creates a local commit. It never pushes.
//!
//! Path note: the playbook names this file
//! `rust/forge/src/pianola/worker_exec.rs`. The workspace root is the repo
//! root (`members = ["forge", ...]`), so the canonical path is
//! `forge/src/pianola/worker_exec.rs`.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use db::{DbResult, ForgeEngineDao, NewToolArtifact};

use super::worker::TstStoryView;
use super::worker_authoring::is_test_artifact_path;
use crate::engine::assay::{
    adjudicate_assay, is_rust_contract_runtime_test, AssayReport, CommandResult,
};

/// Whether a `cargo test` / `cargo nextest` invocation actually RAN a test.
///
/// WHY THIS EXISTS. libtest exits **0** when every test in the selected target is `#[ignore]`d:
/// `test result: ok. 0 passed; 0 failed; 1 ignored`. So a test file that is committed, compiles, and is
/// entirely skipped reports SUCCESS to anything that reads the exit code. On 2026-10-05 five
/// `API.ROUTE_CONTRACT` stories were adjudicated `Complete` on exactly that: their prescribed assay
/// command ran zero assertions and still exited 0, which `adjudicate_assay` reads as `Pass`. A green that
/// ran nothing is not a green — it is an unmeasured command wearing one.
///
/// The reading is deliberately conservative and returns `false` unless libtest printed a summary line it can
/// parse, because the cost of a false "ran nothing" (refusing a real pass) is lower than the cost of a false
/// "ran something" (banking an empty run as evidence). Only the `passed`/`failed` counts decide it: a target
/// with failing tests already fails on its exit code, and one with passing tests is unaffected.
///
/// Only applies to rust test runners. `cargo check`, a shell pipeline and a non-zero exit are untouched.
fn ran_no_tests(command: &str, output: &str) -> bool {
    if !is_rust_contract_runtime_test(command) {
        return false;
    }
    // The LAST summary line is the one that counts: a workspace run prints one per target, and the final
    // line is the aggregate verdict.
    let Some(summary) = output
        .lines()
        .rev()
        .find(|line| line.trim_start().starts_with("test result:"))
    else {
        return false; // No summary we can read: assume it ran, do not invent a failure.
    };
    match (
        count_before(summary, "passed"),
        count_before(summary, "failed"),
    ) {
        (Some(passed), Some(failed)) => passed == 0 && failed == 0,
        _ => false,
    }
}

/// The integer libtest prints immediately BEFORE `label` — its summary reads `ok. 3 passed; 0 failed`, so
/// the count leads the word. `None` when the label is absent or carries no count beside it.
fn count_before(line: &str, label: &str) -> Option<u64> {
    let at = line.find(label)?;
    let before = line[..at].trim_end();
    let digits: String = before
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<Vec<char>>()
        .into_iter()
        .rev()
        .collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}
use crate::engine::packet::{extract_tests_summary, TESTS_SUMMARY_MARKER};
use crate::engine::worktree::git_binary;

// ---------------------------------------------------------------------------
// Assay execution
// ---------------------------------------------------------------------------

/// Lines of output kept in a [`CommandResult`] excerpt.
pub const ASSAY_EXCERPT_LINES: usize = 500;

/// Keep the first `max` lines of `output`. The excerpt is what the artifact
/// row carries; the full output stays on the [`CommandResult`] the caller
/// holds for this run.
pub fn excerpt_lines(output: &str, max: usize) -> String {
    output.lines().take(max).collect::<Vec<_>>().join("\n")
}

/// Run one shell command scoped to the lane directory, with a timeout.
///
/// The command runs as `sh -c <command>` with `current_dir` set to
/// `worktree_path`, so relative assay lines (`cargo test -p ...`) resolve
/// against the lane checkout and never against the primary checkout.
/// Stdout and stderr are combined: assay output is evidence either way.
pub fn execute_command_scoped(worktree_path: &Path, command: &str) -> CommandResult {
    execute_command_scoped_with_timeout(
        worktree_path,
        command,
        crate::engine::assay::assay_timeout(),
    )
}

/// Timeout-parameterized half of [`execute_command_scoped`]. The parameter
/// exists so tests can use seconds while workers use the env default.
///
/// Bounded (FORGE-FIX-005): on expiry the command's tree is killed and the result carries `CMD_TIMEOUT`
/// with `unmeasurable: true`, so the story fails closed and requeues instead of holding its claim. The
/// kill is real — the pre-fix code returned while the `sh` child kept running.
pub fn execute_command_scoped_with_timeout(
    worktree_path: &Path,
    command: &str,
    timeout: Duration,
) -> CommandResult {
    use crate::engine::assay::{
        spawn_scoped_shell, wait_with_ceiling, CeilingOutcome, CMD_TIMEOUT_CODE, CMD_TIMEOUT_EXIT,
    };
    let child = match spawn_scoped_shell(command, worktree_path) {
        Err(error) => {
            return CommandResult {
                command: command.to_string(),
                exit_code: -1,
                passed: false,
                excerpt: format!("could not spawn assay command: {error}"),
                unmeasurable: true,
                output: String::new(),
            };
        }
        Ok(child) => child,
    };
    match wait_with_ceiling(child, timeout) {
        CeilingOutcome::TimedOut(hit) => CommandResult {
            command: command.to_string(),
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
            command: command.to_string(),
            exit_code: -1,
            passed: false,
            excerpt: format!("could not observe assay command: {error}"),
            unmeasurable: true,
            output: String::new(),
        },
        CeilingOutcome::Finished(Ok(output)) => {
            let code = output.status.code().unwrap_or(-1);
            let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.trim().is_empty() {
                if !combined.is_empty() && !combined.ends_with('\n') {
                    combined.push('\n');
                }
                combined.push_str(&stderr);
            }
            let ran_nothing = ran_no_tests(command, &combined);
            let mut excerpt = excerpt_lines(&combined, ASSAY_EXCERPT_LINES);
            if ran_nothing {
                // Say why in the receipt, because the exit code says the opposite and the excerpt is what a
                // reader adjudicating this story will actually see.
                excerpt = format!(
                    "ASSAY RAN NO TESTS (exit {code}, every test in the target is #[ignore]d or the target is \
                     empty): this is NOT a pass.\n{excerpt}"
                );
            }
            CommandResult {
                command: command.to_string(),
                exit_code: code,
                // A run that executed nothing has not passed, whatever the process says.
                passed: code == 0 && !ran_nothing,
                excerpt,
                unmeasurable: ran_nothing,
                output: combined,
            }
        }
    }
}

/// Run every assay command from the story packet inside the lane.
///
/// The commands are the already-filtered lines `StoryPacket::load_from_neon`
/// produces (see `engine::packet`): non-empty, trimmed, one shell line each.
/// Order is packet order. Each result carries exit code, combined output,
/// and a 500-line excerpt.
pub fn run_assay_commands(view: &TstStoryView, worktree_path: &Path) -> Vec<CommandResult> {
    view.assay_commands
        .iter()
        .map(|command| execute_command_scoped(worktree_path, command))
        .collect()
}

/// Adjudicate this story's results with the same adjudicator QA uses.
///
/// `acceptance_mapped` is true when the packet carries acceptance criteria:
/// a story with no criteria cannot be proven, which surfaces as
/// `ACCEPTANCE_MAP_MISSING` rather than a pass.
pub fn adjudicate_for_view(view: &TstStoryView, results: &[CommandResult]) -> AssayReport {
    let acceptance_mapped = view
        .acceptance_criteria
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
        == false;
    adjudicate_assay(&view.assay_commands, results, acceptance_mapped)
}

/// True when the story exists to expose an app defect: its goal or criteria
/// name a failing test, a reproduction, or a defect to find. A failing assay
/// for such a story is the deliverable (a finding), never a reason to edit
/// the test green. This is advisory — the failure is still recorded as a
/// failure — it only tells the caller not to "fix" it.
pub fn failing_test_is_valid_outcome(view: &TstStoryView) -> bool {
    const MARKERS: [&str; 6] = [
        "expos",
        "reproduc",
        "defect",
        "failing test",
        "expected to fail",
        "finding",
    ];
    let text = format!(
        "{}\n{}",
        view.goal.as_deref().unwrap_or(""),
        view.acceptance_criteria.as_deref().unwrap_or("")
    )
    .to_lowercase();
    MARKERS.iter().any(|marker| text.contains(marker))
}

// ---------------------------------------------------------------------------
// Evidence recording: existing DAO paths only
// ---------------------------------------------------------------------------

/// The packet-contract summary line: `Tests: <summary>`, truncated to the
/// harness limit by `extract_tests_summary`.
pub fn tests_summary_line(tests_summary: &str) -> String {
    let summary = extract_tests_summary(tests_summary, tests_summary);
    format!("{TESTS_SUMMARY_MARKER} {summary}")
}

/// Build the `forge_tool_artifact` input for one assay run. `tool` is
/// `pianola`, `kind` is `assay`, and the verdict follows the results:
/// `pass` when every command passed, `fail` otherwise (an unmeasurable
/// command is a failure to measure, recorded as `fail` with the per-command
/// flags in `detail`). Neon stays canonical; this is the row a later reader
/// queries instead of re-running the lane.
pub fn assay_artifact(
    story_id: &str,
    run_id: &str,
    assay_results: &[CommandResult],
    tests_summary: &str,
) -> NewToolArtifact {
    let all_passed = !assay_results.is_empty() && assay_results.iter().all(|r| r.passed);
    let commands: Vec<serde_json::Value> = assay_results
        .iter()
        .map(|r| {
            serde_json::json!({
                "command": r.command,
                "exit_code": r.exit_code,
                "passed": r.passed,
                "unmeasurable": r.unmeasurable,
                "excerpt": r.excerpt,
            })
        })
        .collect();
    let detail = serde_json::json!({
        "run_id": run_id,
        "tests_summary": extract_tests_summary(tests_summary, tests_summary),
        "commands": commands,
    });
    NewToolArtifact {
        story_id: story_id.to_string(),
        story_run_id: Some(run_id.to_string()),
        tool: "pianola".to_string(),
        kind: "assay".to_string(),
        verdict: Some(if all_passed { "pass" } else { "fail" }.to_string()),
        summary: Some(tests_summary_line(tests_summary)),
        detail: Some(detail),
        sha: None,
        idempotency_key: None,
    }
}

/// One evidence line for `storyboard_story_run`: the `Tests:` summary plus
/// the per-command pass/fail counts. Appended via `append_run_detail`, the
/// sanctioned evidence-line writer (the same path `escalation` uses).
pub fn evidence_detail_line(assay_results: &[CommandResult], tests_summary: &str) -> String {
    let passed = assay_results.iter().filter(|r| r.passed).count();
    let failed = assay_results.len() - passed;
    format!(
        "{} (assay: {} passed, {} failed of {})",
        tests_summary_line(tests_summary),
        passed,
        failed,
        assay_results.len()
    )
}

/// Record assay evidence through the existing engine paths and nothing else:
///
/// 1. `ForgeEngineDao::record_tool_artifact` — the one sanctioned write of
///    `forge_tool_artifact`.
/// 2. `ForgeEngineDao::append_run_detail` — the sanctioned evidence line on
///    `storyboard_story_run`.
///
/// No custom table; Neon remains canonical. A failing test that exposes an
/// app defect is recorded as-is (see [`failing_test_is_valid_outcome`]);
/// nothing here rewrites the test to make the lane green.
pub async fn record_evidence(
    engine: &ForgeEngineDao,
    story_id: &str,
    run_id: &str,
    assay_results: &[CommandResult],
    tests_summary: &str,
) -> DbResult<()> {
    let input = assay_artifact(story_id, run_id, assay_results, tests_summary);
    engine.record_tool_artifact(&input).await?;
    engine
        .append_run_detail(run_id, &evidence_detail_line(assay_results, tests_summary))
        .await?;
    tracing::info!(
        target: "pianola::worker",
        story = %story_id,
        run = %run_id,
        commands = assay_results.len(),
        "pianola worker assay evidence recorded via existing Forge DAO path"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Local commit in lane
// ---------------------------------------------------------------------------

/// The lane commit message. The story id, title, and lane run id are all in
/// the subject so `git log` in any lane reads without a database lookup.
pub fn commit_message(story_id: &str, title: &str, run_id: &str) -> String {
    let title = title.trim();
    let title = if title.is_empty() {
        "(untitled)"
    } else {
        title
    };
    format!("TST {story_id}: {title} - test authored in lane {run_id}")
}

fn git_in(worktree_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new(git_binary())
        .args(args)
        .current_dir(worktree_path)
        .output()
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git {} failed: {}", args.join(" "), err.trim()));
    }
    // Trailing-newline trim only: `git status --porcelain` aligns its XY
    // columns with a LEADING space (`" M <path>"`), and a full trim would
    // eat that column and shift every path parse by one.
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string())
}

/// Relative changed paths in the lane (`git status --porcelain`, both
/// staged and unstaged, untracked files listed individually via `-uall` so
/// a new `forge/tests/` file reports as its file path and never as a
/// collapsed `forge/` directory). Empty when the lane is clean.
pub fn changed_files_in_lane(worktree_path: &Path) -> Result<Vec<String>, String> {
    let status = git_in(worktree_path, &["status", "--porcelain=v1", "-uall"])?;
    let mut out = Vec::new();
    for line in status.lines() {
        // `XY <path>` or `XY <orig> -> <new>` for renames.
        let path = line.get(3..).unwrap_or("").trim();
        if path.is_empty() {
            continue;
        }
        let resolved = path.split(" -> ").last().unwrap_or(path).trim();
        let resolved = resolved.trim_matches('"');
        if !resolved.is_empty() {
            out.push(resolved.to_string());
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// True when the lane diff for `relative` only appends test code: no
/// non-blank line is removed, and an added line carries `#[cfg(test)]`.
/// This is the CrateLocalMod shape (`worker_authoring` appends an inline
/// test module to the target file): the file is a production path, but the
/// change is test-only.
pub fn diff_confined_to_test_module(worktree_path: &Path, relative: &str) -> Result<bool, String> {
    let diff = git_in(worktree_path, &["diff", "--", relative])?;
    if diff.trim().is_empty() {
        return Ok(true);
    }
    let mut adds_cfg_test = false;
    for line in diff.lines() {
        if line.starts_with("+++") || line.starts_with("---") {
            continue;
        }
        if let Some(removed) = line.strip_prefix('-') {
            if !removed.trim().is_empty() {
                return Ok(false);
            }
        } else if let Some(added) = line.strip_prefix('+') {
            if added.contains("#[cfg(test)]") {
                adds_cfg_test = true;
            }
        }
    }
    Ok(adds_cfg_test)
}

/// Split lane changes into `(prod_edits, test_edits)` using the same test
/// taxonomy the authoring guards use, extended with the appended-test-module
/// shape. Pure over an already-computed file list so tests need no git.
pub fn partition_changed(worktree_path: &Path, changed: &[String]) -> (Vec<String>, Vec<String>) {
    let mut prod = Vec::new();
    let mut test = Vec::new();
    for path in changed {
        if is_test_artifact_path(path) {
            test.push(path.clone());
            continue;
        }
        match diff_confined_to_test_module(worktree_path, path) {
            Ok(true) => test.push(path.clone()),
            _ => prod.push(path.clone()),
        }
    }
    (prod, test)
}

/// Detect two lanes claiming the same target file. `changed_here` is this
/// lane's changed-file list; `other_lanes` holds one changed-file list per
/// sibling lane. Returns the first conflicting path, which the caller
/// escalates as `OverlappingTarget`. Pure: no git, no I/O.
pub fn check_lane_overlap(changed_here: &[String], other_lanes: &[Vec<String>]) -> Option<String> {
    for path in changed_here {
        for other in other_lanes {
            if other.iter().any(|p| p == path) {
                return Some(path.clone());
            }
        }
    }
    None
}

/// Commit the lane's test work locally. Fails (without committing) when:
///
/// * the lane is clean (nothing to commit),
/// * any changed file is a production edit rather than a test artifact or
///   an appended `#[cfg(test)]` module (the prod-rewrite shape: refuse, so
///   the caller escalates instead of landing it),
/// * `git diff --check` reports whitespace errors.
///
/// Creates the commit with [`commit_message`] and returns its SHA. Never
/// pushes: publishing is an escalation (`PublishRequested`), handled by the
/// conflict module, not by the worker.
///
/// NOTE: the playbook sketches this as `(worktree_path, story_id)`. That
/// arity cannot produce the required message (`TST <story_id>: <title> -
/// test authored in lane <run_id>`), so `title` and `run_id` are explicit
/// parameters.
pub fn commit_in_lane(
    worktree_path: &Path,
    story_id: &str,
    title: &str,
    run_id: &str,
) -> Result<String, String> {
    let changed = changed_files_in_lane(worktree_path)?;
    if changed.is_empty() {
        return Err(format!(
            "lane is clean: nothing to commit for story {story_id}"
        ));
    }
    let (prod_edits, _test_edits) = partition_changed(worktree_path, &changed);
    if !prod_edits.is_empty() {
        return Err(format!(
            "lane touches production files outside test taxonomy ({}): refusing to commit; escalate instead",
            prod_edits.join(", ")
        ));
    }
    git_in(worktree_path, &["diff", "--check"])?;
    let message = commit_message(story_id, title, run_id);
    git_in(worktree_path, &["add", "--", "."])?;
    git_in(
        worktree_path,
        &[
            "-c",
            "user.name=pianola-worker",
            "-c",
            "user.email=pianola-worker@localhost",
            "commit",
            "-m",
            &message,
        ],
    )?;
    let sha = git_in(worktree_path, &["rev-parse", "HEAD"])?;
    tracing::info!(
        target: "pianola::worker",
        story = %story_id,
        sha = %sha,
        "pianola worker committed test in lane (unpushed)"
    );
    Ok(sha)
}

/// Lane worktree path for tests that must not touch the real checkout.
#[cfg(test)]
fn tmp_lane(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pianola-exec-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("fixture lane");
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- the empty-green detector -------------------------------------------------
    //
    // These are the self-checks for `ran_no_tests`. A guard that cannot fail guards nothing, and the
    // defect being guarded against is precisely a guard that could not fail.

    const ALL_IGNORED: &str = "running 1 test\ntest api_route_contract_008__x ... ignored\n\
        test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s";

    #[test]
    fn a_fully_ignored_target_is_not_a_pass() {
        assert!(
            ran_no_tests("cargo test -p test-harness --test some_target", ALL_IGNORED),
            "a target whose only test is #[ignore]d ran nothing and must not read as a pass"
        );
    }

    #[test]
    fn a_real_pass_is_still_a_pass() {
        let ran = "test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.06s";
        assert!(!ran_no_tests(
            "cargo test -p test-harness --test some_target",
            ran
        ));
    }

    #[test]
    fn a_failing_run_is_not_reported_as_ran_nothing() {
        // Failures already fail on exit code; calling this "ran nothing" would blame the wrong thing.
        let failed =
            "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out";
        assert!(!ran_no_tests(
            "cargo test -p test-harness --test some_target",
            failed
        ));
    }

    #[test]
    fn only_the_last_summary_line_decides() {
        // A workspace run prints one summary per target. An early empty target must not condemn a run whose
        // aggregate actually executed tests, nor excuse one that did not.
        let mixed = format!("test result: ok. 0 passed; 0 failed; 0 ignored\n{ALL_IGNORED}");
        assert!(ran_no_tests("cargo test --workspace", &mixed));
        let mixed_ok = format!("{ALL_IGNORED}\ntest result: ok. 7 passed; 0 failed; 0 ignored");
        assert!(!ran_no_tests("cargo test --workspace", &mixed_ok));
    }

    #[test]
    fn an_unreadable_summary_is_not_guessed_at() {
        assert!(!ran_no_tests(
            "cargo test -p test-harness --test x",
            "error: could not compile"
        ));
        assert!(!ran_no_tests("cargo test -p test-harness --test x", ""));
        assert!(!ran_no_tests(
            "cargo test -p test-harness --test x",
            "test result: ok. passed; failed"
        ));
    }

    #[test]
    fn non_test_commands_are_untouched() {
        // `cargo check` prints no summary, and a build command must never be reclassified by this rule.
        assert!(!ran_no_tests(
            "cargo check --workspace --all-targets",
            ALL_IGNORED
        ));
        assert!(!ran_no_tests("sh -c true", ALL_IGNORED));
    }

    #[test]
    fn count_before_reads_only_digits() {
        assert_eq!(count_before("ok. 12 passed; 0 failed", "passed"), Some(12));
        assert_eq!(count_before("ok. 0 passed; 3 failed", "failed"), Some(3));
        assert_eq!(count_before("nothing here", "passed"), None);
        assert_eq!(count_before("passed; failed", "passed"), None);
    }

    #[test]
    fn an_empty_run_is_refused_by_adjudication_not_banked_as_a_pass() {
        // The end-to-end outcome, which is the whole point: the CommandResult the executor now builds for an
        // all-ignored target must come out `Fail`/`COMMAND_UNMEASURABLE`. Before this change the same run was
        // `passed: true` on exit 0 and adjudicated `Pass`, which is how five stories were closed on nothing.
        let result = CommandResult {
            command: "cargo test --manifest-path Cargo.toml -p test-harness --test some_target"
                .to_string(),
            exit_code: 0,
            passed: false,
            excerpt: "ASSAY RAN NO TESTS".to_string(),
            unmeasurable: ran_no_tests(
                "cargo test --manifest-path Cargo.toml -p test-harness --test some_target",
                ALL_IGNORED,
            ),
            output: ALL_IGNORED.to_string(),
        };
        assert!(
            result.unmeasurable,
            "the detector must classify it unmeasurable"
        );
        let report = adjudicate_assay(&[result.command.clone()], &[result], true);
        assert_eq!(report.verdict, crate::engine::assay::AssayVerdict::Fail);
        assert!(report.blockers.contains(&"COMMAND_UNMEASURABLE"));
    }

    fn view_with(commands: &[&str], goal: &str, criteria: &str) -> TstStoryView {
        TstStoryView {
            id: "TST-1".to_string(),
            title: "Probe story".to_string(),
            goal: Some(goal.to_string()),
            architect_brief: None,
            acceptance_criteria: Some(criteria.to_string()),
            assay_commands: commands.iter().map(|c| c.to_string()).collect(),
            test_mode: None,
            scope: None,
            operating_surface: None,
        }
    }

    fn result(command: &str, passed: bool) -> CommandResult {
        CommandResult {
            command: command.into(),
            exit_code: if passed { 0 } else { 101 },
            passed,
            excerpt: String::new(),
            unmeasurable: false,
            output: String::new(),
        }
    }

    #[test]
    fn passing_command_maps_to_passed_with_excerpt() {
        let lane = tmp_lane("pass");
        let res =
            execute_command_scoped_with_timeout(&lane, "echo hello-assay", Duration::from_secs(30));
        assert!(res.passed, "unexpected: {res:?}");
        assert_eq!(res.exit_code, 0);
        assert!(!res.unmeasurable);
        assert!(res.output.contains("hello-assay"));
        assert!(res.excerpt.contains("hello-assay"));
        let _ = std::fs::remove_dir_all(&lane);
    }

    #[test]
    fn failing_command_maps_to_failed_not_unmeasurable() {
        let lane = tmp_lane("fail");
        let res = execute_command_scoped_with_timeout(&lane, "false", Duration::from_secs(30));
        assert!(!res.passed);
        assert!(!res.unmeasurable);
        assert_ne!(res.exit_code, 0);
        let _ = std::fs::remove_dir_all(&lane);
    }

    #[test]
    fn a_hung_assay_command_is_killed_at_its_ceiling_with_cmd_timeout() {
        // FORGE-FIX-005: the sleep-style assay with a tiny ceiling. The command must die at the ceiling
        // (not hold the claim for the sleep's 30s), report `CMD_TIMEOUT` with `unmeasurable: true`, and
        // adjudicate to the `CMD_TIMEOUT` blocker so the story fails closed and requeues.
        use crate::engine::assay::{CMD_TIMEOUT_CODE, CMD_TIMEOUT_EXIT};
        let lane = tmp_lane("timeout");
        let started = std::time::Instant::now();
        let res = execute_command_scoped_with_timeout(&lane, "sleep 30", Duration::from_secs(1));
        let elapsed = started.elapsed();
        assert!(!res.passed);
        assert!(
            res.unmeasurable,
            "a killed command measured nothing: {res:?}"
        );
        assert_eq!(res.exit_code, CMD_TIMEOUT_EXIT);
        assert!(
            res.excerpt.contains(CMD_TIMEOUT_CODE),
            "the receipt names the kill: {}",
            res.excerpt
        );
        assert!(
            elapsed < Duration::from_secs(20),
            "returned near the ceiling, not at the sleep: {elapsed:?}"
        );
        let report = adjudicate_assay(&[res.command.clone()], &[res], true);
        assert_eq!(report.verdict, crate::engine::assay::AssayVerdict::Fail);
        assert!(report.blockers.contains(&"CMD_TIMEOUT"));
        let _ = std::fs::remove_dir_all(&lane);
    }

    #[test]
    fn excerpt_keeps_first_500_lines() {
        let lane = tmp_lane("excerpt");
        let res = execute_command_scoped_with_timeout(
            &lane,
            "i=1; while [ $i -le 600 ]; do echo line-$i; i=$((i+1)); done",
            Duration::from_secs(30),
        );
        assert!(res.passed, "unexpected: {res:?}");
        let lines: Vec<&str> = res.excerpt.lines().collect();
        assert_eq!(lines.len(), ASSAY_EXCERPT_LINES);
        assert_eq!(lines[0], "line-1");
        assert_eq!(lines[ASSAY_EXCERPT_LINES - 1], "line-500");
        let _ = std::fs::remove_dir_all(&lane);
    }

    #[test]
    fn adjudication_uses_shared_blockers() {
        let view = view_with(&["cargo test -p probe"], "author a test", "test exists");
        let report = adjudicate_for_view(&view, &[result("cargo test -p probe", true)]);
        assert!(report.blockers.is_empty());
        let failing = adjudicate_for_view(&view, &[result("cargo test -p probe", false)]);
        assert!(failing.blockers.contains(&"CMD_FAIL"));
        let empty = view_with(&[], "author a test", "test exists");
        let no_cmds = adjudicate_for_view(&empty, &[]);
        assert!(no_cmds.blockers.contains(&"NO_ASSAY_COMMANDS"));
        let no_criteria = view_with(&["cargo test -p probe"], "author a test", "   ");
        let unproven = adjudicate_for_view(&no_criteria, &[result("cargo test -p probe", true)]);
        assert!(unproven.blockers.contains(&"ACCEPTANCE_MAP_MISSING"));
    }

    #[test]
    fn expose_defect_story_marks_failure_as_valid_outcome() {
        let exposing = view_with(
            &["cargo test -p probe"],
            "author a regression test that exposes the ranking defect",
            "the test fails against current code and that failure is the finding",
        );
        assert!(failing_test_is_valid_outcome(&exposing));
        let ordinary = view_with(
            &["cargo test -p probe"],
            "author a contract test for the assay gate",
            "tests compile and the gate stays green",
        );
        assert!(!failing_test_is_valid_outcome(&ordinary));
    }

    #[test]
    fn artifact_carries_tests_marker_and_command_verdicts() {
        let results = vec![
            result("cargo test -p probe --lib a", true),
            result("cargo test -p probe --lib b", false),
        ];
        let artifact = assay_artifact("TST-7", "run-1", &results, "2 passed, 1 failed");
        assert_eq!(artifact.tool, "pianola");
        assert_eq!(artifact.kind, "assay");
        assert_eq!(artifact.story_id, "TST-7");
        assert_eq!(artifact.verdict.as_deref(), Some("fail"));
        let summary = artifact.summary.unwrap_or_default();
        assert!(summary.starts_with("Tests:"), "unexpected: {summary}");
        let detail = artifact.detail.expect("detail");
        assert_eq!(detail["run_id"], serde_json::json!("run-1"));
        assert_eq!(detail["commands"].as_array().unwrap().len(), 2);
        let passing = assay_artifact("TST-7", "run-1", &[result("cargo test", true)], "all green");
        assert_eq!(passing.verdict.as_deref(), Some("pass"));
    }

    #[test]
    fn commit_message_names_story_title_and_lane() {
        assert_eq!(
            commit_message("TST-9", "Probe title", "run-abc"),
            "TST TST-9: Probe title - test authored in lane run-abc"
        );
    }

    fn init_repo(path: &Path) {
        git_in(path, &["init"]).expect("git init");
        git_in(path, &["config", "user.name", "pianola-test"]).expect("config name");
        git_in(path, &["config", "user.email", "test@localhost"]).expect("config email");
        std::fs::write(path.join("README.md"), "# lane fixture\n").expect("seed file");
        git_in(path, &["add", "--", "."]).expect("seed add");
        git_in(path, &["commit", "-m", "seed"]).expect("seed commit");
    }

    #[test]
    fn commit_path_accepts_test_file_and_returns_sha() {
        let lane = tmp_lane("commit-ok");
        init_repo(&lane);
        std::fs::create_dir_all(lane.join("forge/tests")).expect("tests dir");
        std::fs::write(
            lane.join("forge/tests/contract_probe.rs"),
            "#[test]\nfn tst_probe() {}\n",
        )
        .expect("test file");
        let sha = commit_in_lane(&lane, "TST-11", "Probe story", "run-1").expect("commit");
        assert_eq!(sha.len(), 40, "unexpected sha: {sha}");
        assert!(sha.bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(changed_files_in_lane(&lane).expect("status").is_empty());
        let _ = std::fs::remove_dir_all(&lane);
    }

    #[test]
    fn commit_path_refuses_production_edit() {
        let lane = tmp_lane("commit-prod");
        init_repo(&lane);
        std::fs::write(lane.join("README.md"), "# lane fixture\nmore prod\n").expect("prod edit");
        let err = commit_in_lane(&lane, "TST-12", "Probe story", "run-2").unwrap_err();
        assert!(err.contains("production"), "unexpected refusal text: {err}");
        let _ = std::fs::remove_dir_all(&lane);
    }

    #[test]
    fn two_lanes_touching_same_file_escalate_overlap() {
        // Two sibling lanes author against the same target: the second lane's
        // changed set intersects the first, so the worker escalates
        // OverlappingTarget instead of committing over it.
        let lane_a = tmp_lane("overlap-a");
        let lane_b = tmp_lane("overlap-b");
        init_repo(&lane_a);
        init_repo(&lane_b);
        for lane in [&lane_a, &lane_b] {
            std::fs::create_dir_all(lane.join("forge/tests")).expect("tests dir");
        }
        std::fs::write(
            lane_a.join("forge/tests/contract_probe.rs"),
            "#[test]\nfn tst_a() {}\n",
        )
        .expect("lane a file");
        std::fs::write(
            lane_b.join("forge/tests/contract_probe.rs"),
            "#[test]\nfn tst_b() {}\n",
        )
        .expect("lane b file");
        let changed_a = changed_files_in_lane(&lane_a).expect("status a");
        let changed_b = changed_files_in_lane(&lane_b).expect("status b");
        let conflict = check_lane_overlap(&changed_b, &[changed_a]);
        assert_eq!(
            conflict.as_deref(),
            Some("forge/tests/contract_probe.rs"),
            "same target file in two lanes must escalate"
        );
        assert!(
            check_lane_overlap(&changed_b, &[vec!["forge/tests/other.rs".to_string()]]).is_none()
        );
        let _ = std::fs::remove_dir_all(&lane_a);
        let _ = std::fs::remove_dir_all(&lane_b);
    }
}
