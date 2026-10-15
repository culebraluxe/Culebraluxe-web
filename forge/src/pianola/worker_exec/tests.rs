// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;
use crate::engine::assay::adjudicate_assay;

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
    let failed = "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out";
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
fn pianola_summary_serializes_long_unicode_without_truncating_a_character() {
    let summary = format!("Tests: {}🙂 trailing", "a".repeat(298));
    let line = tests_summary_line(&summary);
    let serialized = serde_json::to_string(&line).expect("receipt summary serializes");
    assert!(line.ends_with('…'));
    assert!(line.len() <= crate::engine::packet::TESTS_SUMMARY_MAX_LENGTH + 7);
    assert!(serialized.contains('…'));
    assert!(serialized.contains("Tests:"));
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
        cancelled: false,
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
        cancelled: false,
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

#[cfg(unix)]
#[test]
fn a_signalled_assay_command_is_recorded_as_cancelled() {
    let lane = tmp_lane("cancelled");
    let res = execute_command_scoped_with_timeout(&lane, "kill -TERM $$", Duration::from_secs(30));
    assert!(!res.passed);
    assert!(res.unmeasurable);
    assert!(crate::engine::assay::is_cmd_cancelled(&res));
    assert!(res.excerpt.contains("signal 15"));
    let report = adjudicate_assay(&[res.command.clone()], &[res], true);
    assert_eq!(report.verdict, crate::engine::assay::AssayVerdict::Unproven);
    assert!(report.blockers.contains(&"CMD_CANCELLED"));
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
    assert_eq!(report.verdict, crate::engine::assay::AssayVerdict::Unproven);
    assert!(report.blockers.contains(&"ASSAY_PLAN_REQUIRED"));
    let failing = adjudicate_for_view(&view, &[result("cargo test -p probe", false)]);
    assert!(failing.blockers.contains(&"ASSAY_PLAN_REQUIRED"));
    let empty = view_with(&[], "author a test", "test exists");
    let no_cmds = adjudicate_for_view(&empty, &[]);
    assert!(no_cmds.blockers.contains(&"NO_ASSAY_COMMANDS"));
    let no_criteria = view_with(&["cargo test -p probe"], "author a test", "   ");
    let unproven = adjudicate_for_view(&no_criteria, &[result("cargo test -p probe", true)]);
    assert!(unproven.blockers.contains(&"ASSAY_PLAN_REQUIRED"));
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
    assert_eq!(artifact.verdict.as_deref(), Some("UNPROVEN"));
    let summary = artifact.summary.unwrap_or_default();
    assert!(summary.starts_with("Tests:"), "unexpected: {summary}");
    let detail = artifact.detail.expect("detail");
    assert_eq!(detail["run_id"], serde_json::json!("run-1"));
    assert_eq!(detail["commands"].as_array().unwrap().len(), 2);
    assert_eq!(detail["test_artifact_judgment"], "UNPROVEN");
    assert_eq!(detail["product_judgment"], "UNPROVEN");
    let passing = assay_artifact("TST-7", "run-1", &[result("cargo test", true)], "all green");
    assert_eq!(passing.verdict.as_deref(), Some("UNPROVEN"));
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
    assert!(check_lane_overlap(&changed_b, &[vec!["forge/tests/other.rs".to_string()]]).is_none());
    let _ = std::fs::remove_dir_all(&lane_a);
    let _ = std::fs::remove_dir_all(&lane_b);
}
