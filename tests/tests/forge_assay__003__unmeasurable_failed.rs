//! FORGE.ASSAY-003 — unmeasurable != failed.
//!
//! CONTRACT. `CommandResult` carries two independent facts: whether the command could be
//! measured at all (`unmeasurable`) and whether it passed (`passed`). The adjudicators keep
//! them apart on purpose — `adjudicate_assay` refuses an unmeasurable command with
//! `COMMAND_UNMEASURABLE` and a measured failure with `CMD_FAIL`, and `adjudicate_qa`
//! writes `CMD_UNMEASURABLE <command>` versus `CMD_FAIL <command>`. Collapsing "could not
//! measure" into "failed" would send an infrastructure fault down the product-repair road;
//! collapsing it into "passed" would bank nothing as evidence.
//!
//! Level: L3 Composition — the production adjudicators, commands faked at the result edge.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_assay__003__unmeasurable_failed

use forge::engine::assay::{adjudicate_assay, AssayVerdict, CommandResult};
use forge::engine::qa_adjudicate::{adjudicate_qa, AssayPlan};

fn failed(command: &str) -> CommandResult {
    CommandResult {
        command: command.into(),
        exit_code: 101,
        passed: false,
        excerpt: "assertion failed".into(),
        unmeasurable: false,
        output: "assertion failed".into(),

        cancelled: false,
    }
}

fn unmeasurable(command: &str) -> CommandResult {
    CommandResult {
        command: command.into(),
        exit_code: -1,
        passed: false,
        excerpt: "could not observe assay command".into(),
        unmeasurable: true,
        output: String::new(),

        cancelled: false,
    }
}

fn passing(command: &str) -> CommandResult {
    CommandResult {
        command: command.into(),
        exit_code: 0,
        passed: true,
        excerpt: "ok".into(),
        unmeasurable: false,
        output: "ok".into(),

        cancelled: false,
    }
}

#[test]
fn forge_assay_003__unmeasurable_failed() {
    let command = "cargo test -p test-harness --test probe";

    // ── 1. AN UNMEASURABLE COMMAND IS ITS OWN BLOCKER. ───────────────────────
    let unmeasured = adjudicate_assay(&[command.to_string()], &[unmeasurable(command)], true);
    assert_eq!(unmeasured.verdict, AssayVerdict::Fail);
    assert!(
        unmeasured.blockers.contains(&"COMMAND_UNMEASURABLE"),
        "an unmeasured command must surface as unmeasured: {:?}",
        unmeasured.blockers
    );
    assert!(
        !unmeasured.blockers.contains(&"CMD_FAIL"),
        "an unmeasured command must NOT read as a measured failure: {:?}",
        unmeasured.blockers
    );

    // ── 2. A MEASURED FAILURE IS ITS OWN (DIFFERENT) BLOCKER. ───────────────
    let measured_fail = adjudicate_assay(&[command.to_string()], &[failed(command)], true);
    assert_eq!(measured_fail.verdict, AssayVerdict::Fail);
    assert!(
        measured_fail.blockers.contains(&"CMD_FAIL"),
        "a measured failure must surface as a failure: {:?}",
        measured_fail.blockers
    );
    assert!(
        !measured_fail.blockers.contains(&"COMMAND_UNMEASURABLE"),
        "a measured failure must NOT read as unmeasured: {:?}",
        measured_fail.blockers
    );

    // ── 3. THE QA ADJUDICATOR KEEPS THE SAME DISTINCTION, PER COMMAND. ───────
    let plan = AssayPlan {
        commands: vec![command.to_string()],
        conditions: vec![],
        negative_control: None,
    };
    let qa_unmeasured = adjudicate_qa(&plan, &[unmeasurable(command)], None, None, None);
    assert!(
        qa_unmeasured
            .blockers
            .iter()
            .any(|blocker| blocker == &format!("CMD_UNMEASURABLE {command}")),
        "QA must name the unmeasured command as unmeasured: {:?}",
        qa_unmeasured.blockers
    );
    let qa_failed = adjudicate_qa(&plan, &[failed(command)], None, None, None);
    assert!(
        qa_failed
            .blockers
            .iter()
            .any(|blocker| blocker == &format!("CMD_FAIL {command}")),
        "QA must name the failed command as failed: {:?}",
        qa_failed.blockers
    );

    // ── 4. NEGATIVE: NEITHER READING LEAKS INTO THE OTHER, AND GREEN IS GREEN.
    assert!(
        !qa_unmeasured
            .blockers
            .iter()
            .any(|blocker| blocker.starts_with("CMD_FAIL ")),
        "unmeasured must never borrow the failure token: {:?}",
        qa_unmeasured.blockers
    );
    assert!(
        !qa_failed
            .blockers
            .iter()
            .any(|blocker| blocker.starts_with("CMD_UNMEASURABLE ")),
        "failed must never borrow the unmeasured token: {:?}",
        qa_failed.blockers
    );
    let clean = adjudicate_assay(&[command.to_string()], &[passing(command)], true);
    assert_eq!(clean.verdict, AssayVerdict::Pass);
    assert!(
        clean.blockers.is_empty(),
        "a passing command carries neither token: {:?}",
        clean.blockers
    );
}
