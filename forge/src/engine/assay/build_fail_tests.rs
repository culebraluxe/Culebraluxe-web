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
