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
    ForgeGateEvidence::default()
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
fn qa_rust_contract_measurement_uses_only_approved_test_commands() {
    let commands = vec![
        "cargo test --manifest-path Cargo.toml -p test-harness --test contract".to_string(),
        "cargo check --manifest-path Cargo.toml --workspace --all-targets".to_string(),
    ];
    // The element type is `String`, because the assertion below compares against `Vec<String>`; without the
    // annotation `command.into()` is ambiguous (E0282/E0283), which kept this whole test target from
    // compiling since c2c556adc. The harness does not run `cargo test`, so nothing reported it.
    let observed: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    let evidence = collect_rust_contract_assay_evidence(
        contract_gate(),
        Some(&|command| {
            observed
                .lock()
                .expect("command observation lock")
                .push(command.into());
            result(command, true)
        }),
        &commands,
        true,
    );
    assert_eq!(evidence.verdict, AssayVerdict::Pass);
    assert_eq!(evidence.evidence.qa_passed, Some(true));
    assert_eq!(
        observed
            .lock()
            .expect("command observation lock")
            .as_slice(),
        commands.as_slice()
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
