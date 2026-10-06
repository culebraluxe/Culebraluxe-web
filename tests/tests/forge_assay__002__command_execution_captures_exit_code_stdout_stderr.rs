//! FORGE.ASSAY-002 — command execution captures exit code/stdout/stderr.
//!
//! CONTRACT. `execute_command_scoped` runs one assay line as `sh -c <command>` scoped to the lane
//! directory and captures the machine facts QA adjudicates on: the exit code, the combined
//! stdout+stderr output, and a bounded excerpt. Exit 0 maps to `passed`; non-zero maps to failed
//! (and measurably so — never `unmeasurable`). Stderr is evidence too, so it is folded into the
//! captured output rather than dropped.
//!
//! Level: L3 Composition — the real production executor against a scratch lane, no DB, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_assay__002__command_execution_captures_exit_code_stdout_stderr

use std::time::Duration;

use forge::pianola::worker_exec::{execute_command_scoped_with_timeout, ASSAY_EXCERPT_LINES};

/// A scratch lane: a real directory the executor scopes into, removed afterwards. The assay
/// commands under test never touch the real checkout.
fn scratch_lane(name: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("forge-assay-002-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch lane");
    dir
}

fn run(lane: &std::path::Path, command: &str) -> forge::engine::assay::CommandResult {
    execute_command_scoped_with_timeout(lane, command, Duration::from_secs(30))
}

#[test]
fn forge_assay_002__command_execution_captures_exit_code_stdout_stderr() {
    let lane = scratch_lane("capture");
    let lane = scope_guard(&lane);

    // ── 1. STDOUT AND EXIT CODE ARE CAPTURED. ────────────────────────────────
    let ok = run(&lane, "echo hello-assay");
    assert!(ok.passed, "exit 0 must read as passed: {ok:?}");
    assert_eq!(ok.exit_code, 0, "the exit code is machine evidence: {ok:?}");
    assert!(!ok.unmeasurable, "a clean run is measured: {ok:?}");
    assert_eq!(ok.command, "echo hello-assay");
    assert!(
        ok.output.contains("hello-assay") && ok.excerpt.contains("hello-assay"),
        "stdout must reach both the full output and the excerpt: {ok:?}"
    );

    // ── 2. STDERR IS EVIDENCE TOO. ──────────────────────────────────────────
    let mixed = run(&lane, "echo on-stdout; echo on-stderr >&2");
    assert!(mixed.passed, "unexpected: {mixed:?}");
    assert!(
        mixed.output.contains("on-stdout"),
        "stdout must be captured: {mixed:?}"
    );
    assert!(
        mixed.output.contains("on-stderr"),
        "stderr must be folded into the captured output, not dropped: {mixed:?}"
    );

    // ── 3. A NON-ZERO EXIT IS CAPTURED EXACTLY. ─────────────────────────────
    let three = run(&lane, "exit 3");
    assert!(!three.passed, "exit 3 must not read as passed: {three:?}");
    assert_eq!(three.exit_code, 3, "the exact code is captured: {three:?}");
    assert!(!three.unmeasurable, "a process that ran is measured: {three:?}");

    // ── 4. NEGATIVE: FAILURE IS FAILED, NOT UNMEASURABLE AND NOT PASSED. ────
    let failed = run(&lane, "false");
    assert!(!failed.passed, "`false` must fail: {failed:?}");
    assert_ne!(failed.exit_code, 0, "`false` exits non-zero: {failed:?}");
    assert!(
        !failed.unmeasurable,
        "a command that ran and failed is a measured failure, not an unmeasurable one: {failed:?}"
    );
    let failing_stderr = run(&lane, "echo boom >&2; exit 7");
    assert!(!failing_stderr.passed, "unexpected: {failing_stderr:?}");
    assert_eq!(failing_stderr.exit_code, 7);
    assert!(
        failing_stderr.output.contains("boom"),
        "a failing command's stderr must still be captured: {failing_stderr:?}"
    );

    // ── 5. THE EXCERPT IS BOUNDED, HEAD-ANCHORED. ───────────────────────────
    let long = run(
        &lane,
        "i=1; while [ $i -le 600 ]; do echo line-$i; i=$((i+1)); done",
    );
    assert!(long.passed, "unexpected: {:?}", long.excerpt.lines().count());
    let lines: Vec<&str> = long.excerpt.lines().collect();
    assert_eq!(
        lines.len(),
        ASSAY_EXCERPT_LINES,
        "the excerpt keeps the first {ASSAY_EXCERPT_LINES} lines, not the whole stream"
    );
    assert_eq!(lines[0], "line-1");
    assert_eq!(lines[ASSAY_EXCERPT_LINES - 1], "line-500");
    assert!(
        long.output.lines().count() > ASSAY_EXCERPT_LINES,
        "the full output stays complete while the excerpt is bounded"
    );
}

/// RAII cleanup for the scratch lane, so a failing assertion cannot litter `/tmp`.
struct LaneGuard {
    path: std::path::PathBuf,
}

impl std::ops::Deref for LaneGuard {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl Drop for LaneGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn scope_guard(path: &std::path::Path) -> LaneGuard {
    LaneGuard {
        path: path.to_path_buf(),
    }
}
