//! FORGE-FIX-003 — a build failure is evidence, not a test failure.
//!
//! CONTRACT. `adjudicate_assay` and `adjudicate_qa` keep three outcomes apart on purpose:
//! a measured test failure reads `CMD_FAIL` (verdict `Fail`), a command that could not be
//! measured reads `COMMAND_UNMEASURABLE` / `CMD_UNMEASURABLE` (verdict `Fail`), and a command
//! whose output shows the toolchain never built reads `CMD_BUILD_FAIL` (verdict `Unproven` —
//! escalate, never repair, never Failed). Collapsing a compile error into `CMD_FAIL` would send
//! a toolchain fault down the product-repair road and mark the story Failed for code it never
//! ran; dropping a stream would discard the compiler error itself. Fixture outputs (compile
//! error vs assertion failure vs spawn failure) assert the distinct blockers.
//!
//! Level: L3 Composition — the production adjudicators, commands faked at the result edge.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_assay__013__build_fail_distinct

use forge::engine::assay::{
    adjudicate_assay, combine_command_output, is_build_failure_output, AssayVerdict, CommandResult,
    CMD_BUILD_FAIL,
};
use forge::engine::qa_adjudicate::{adjudicate_qa, AssayPlan};

/// What cargo prints when an UNTOUCHED crate fails to compile: rustc diagnostics on stderr
/// plus cargo's own verdict. The story under assay never ran — nothing about it is a finding
/// about the story's code.
const COMPILE_ERROR_OUTPUT: &str = "   Compiling untouched-crate v0.1.0 (/src/untouched-crate)\n\
     Finished `test` profile [unoptimized + debuginfo] target(s) in 1.02s\n\
     error[E0308]: mismatched types\n\
       --> untouched-crate/src/lib.rs:12:5\n\
        |\n\
     error: could not compile `untouched-crate` (lib) due to 1 previous error";

/// What a GENUINE assertion failure prints: the tests built, ran, and rejected the code.
const ASSERTION_FAILURE_OUTPUT: &str = "running 1 test\n\
     test probe::works ... FAILED\n\
     failures:\n\
         probe::works\n\
     test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured\n\
     thread 'probe::works' panicked at src/lib.rs:9:5:\n\
     assertion failed: `(left == right)`";

fn failed(command: &str, output: &str) -> CommandResult {
    CommandResult {
        command: command.into(),
        exit_code: 101,
        passed: false,
        excerpt: output.into(),
        unmeasurable: false,
        output: output.into(),
    }
}

fn unmeasurable(command: &str) -> CommandResult {
    CommandResult {
        command: command.into(),
        exit_code: -1,
        passed: false,
        excerpt: "could not spawn assay command".into(),
        unmeasurable: true,
        output: String::new(),
    }
}

fn passing(command: &str) -> CommandResult {
    CommandResult {
        command: command.into(),
        exit_code: 0,
        passed: true,
        excerpt: "test result: ok. 1 passed; 0 failed".into(),
        unmeasurable: false,
        output: "test result: ok. 1 passed; 0 failed".into(),
    }
}

fn plan_for(command: &str) -> AssayPlan {
    AssayPlan {
        commands: vec![command.to_string()],
        conditions: vec![],
        negative_control: None,
    }
}

#[test]
fn forge_assay_013__build_fail_distinct() {
    let command = "cargo test --manifest-path Cargo.toml -p probe --test probe";

    // ── 1. A COMPILE ERROR IN AN UNTOUCHED CRATE IS CMD_BUILD_FAIL, NOT A FAILURE. ──
    assert!(
        is_build_failure_output(COMPILE_ERROR_OUTPUT),
        "the detector must read rustc/cargo output as a build failure"
    );
    let build = adjudicate_assay(
        &[command.to_string()],
        &[failed(command, COMPILE_ERROR_OUTPUT)],
        true,
    );
    assert!(
        build.blockers.contains(&CMD_BUILD_FAIL),
        "a compile error must surface as CMD_BUILD_FAIL: {:?}",
        build.blockers
    );
    assert!(
        !build.blockers.contains(&"CMD_FAIL"),
        "a compile error must NOT read as a test failure: {:?}",
        build.blockers
    );
    assert_ne!(
        build.verdict,
        AssayVerdict::Fail,
        "a build failure must NOT mark the story Failed: {:?}",
        build.verdict
    );

    // ── 2. A GENUINE ASSERTION FAILURE IS STILL CMD_FAIL + FAILED. ───────────────
    assert!(
        !is_build_failure_output(ASSERTION_FAILURE_OUTPUT),
        "the detector must not mistake a test verdict for a build failure"
    );
    let assertion = adjudicate_assay(
        &[command.to_string()],
        &[failed(command, ASSERTION_FAILURE_OUTPUT)],
        true,
    );
    assert_eq!(
        assertion.verdict,
        AssayVerdict::Fail,
        "a genuine assertion failure still fails: {:?}",
        assertion.blockers
    );
    assert!(
        assertion.blockers.contains(&"CMD_FAIL"),
        "a genuine failure keeps the failure token: {:?}",
        assertion.blockers
    );

    // ── 3. A SPAWN FAILURE STAYS UNMEASURABLE, DISTINCT FROM BOTH. ───────────────
    let spawn = adjudicate_assay(&[command.to_string()], &[unmeasurable(command)], true);
    assert_eq!(spawn.verdict, AssayVerdict::Fail);
    assert!(spawn.blockers.contains(&"COMMAND_UNMEASURABLE"));
    assert!(!spawn.blockers.contains(&CMD_BUILD_FAIL));

    // ── 4. THE QA ADJUDICATOR KEEPS THE SAME THREE-WAY SPLIT, PER COMMAND. ───────
    let plan = plan_for(command);
    let qa_build = adjudicate_qa(
        &plan,
        &[failed(command, COMPILE_ERROR_OUTPUT)],
        None,
        None,
        None,
    );
    assert!(
        qa_build
            .blockers
            .iter()
            .any(|blocker| blocker == &format!("{CMD_BUILD_FAIL} {command}")),
        "QA must name the unbuilt command as a build failure: {:?}",
        qa_build.blockers
    );
    assert!(
        !qa_build
            .blockers
            .iter()
            .any(|blocker| blocker.starts_with("CMD_FAIL ")),
        "QA must not record a build failure as a test failure: {:?}",
        qa_build.blockers
    );
    assert_ne!(
        qa_build.verdict,
        AssayVerdict::Fail,
        "QA must not fail the story for a build failure: {:?}",
        qa_build.verdict
    );
    let qa_assertion = adjudicate_qa(
        &plan,
        &[failed(command, ASSERTION_FAILURE_OUTPUT)],
        None,
        None,
        None,
    );
    assert_eq!(
        qa_assertion.verdict,
        AssayVerdict::Fail,
        "QA still fails a genuine assertion failure: {:?}",
        qa_assertion.blockers
    );
    assert!(
        qa_assertion
            .blockers
            .iter()
            .any(|blocker| blocker == &format!("CMD_FAIL {command}")),
        "QA keeps the failure token for a genuine failure: {:?}",
        qa_assertion.blockers
    );

    // ── 5. BOTH STREAMS ARE EVIDENCE: STDOUT AND STDERR SURVIVE TOGETHER. ────────
    let combined = combine_command_output("test harness on stdout", COMPILE_ERROR_OUTPUT);
    assert!(
        combined.contains("test harness on stdout"),
        "stdout must survive the combine: {combined:?}"
    );
    assert!(
        combined.contains("error[E0308]"),
        "stderr diagnostics must survive the combine: {combined:?}"
    );
    assert!(
        is_build_failure_output(&combined),
        "the combined evidence must still read as a build failure"
    );

    // ── 6. NEGATIVE: GREEN IS STILL GREEN. ───────────────────────────────────────
    let clean = adjudicate_assay(&[command.to_string()], &[passing(command)], true);
    assert_eq!(clean.verdict, AssayVerdict::Pass);
    assert!(
        clean.blockers.is_empty(),
        "a passing command carries no token: {:?}",
        clean.blockers
    );
}
