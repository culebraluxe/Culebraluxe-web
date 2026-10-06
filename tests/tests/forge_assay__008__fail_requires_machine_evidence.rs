//! FORGE.ASSAY-008 — FAIL requires machine evidence.
//!
//! CONTRACT. A FAIL verdict is never bare: `collect_assay_evidence` records the machine facts
//! behind it on the durable evidence — `qa_passed = false` plus a `deliverable_rejection`
//! that names the assay reading (`Fail`), the blockers, and the exact failed commands. A
//! reader can weigh the failure without re-running the lane. A passing assay, symmetrically,
//! records `qa_passed = true` with no rejection: evidence is required for failure, and
//! failure alone never manufactures rejection text for a pass.
//!
//! Level: L3 Composition — the production evidence collector, the command runner faked.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_assay__008__fail_requires_machine_evidence

use forge::engine::assay::{collect_assay_evidence, AssayVerdict, CommandResult};
use forge::engine::facts::ForgeGateEvidence;

fn failing(command: &str) -> CommandResult {
    CommandResult {
        command: command.into(),
        exit_code: 101,
        passed: false,
        excerpt: "assertion failed".into(),
        unmeasurable: false,
        output: "assertion failed".into(),
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
    }
}

#[test]
fn forge_assay_008__fail_requires_machine_evidence() {
    let command = "cargo test -p test-harness --test probe".to_string();
    let run = |name: &str| {
        if name == command {
            failing(name)
        } else {
            passing(name)
        }
    };

    // ── 1. A FAIL CARRIES ITS MACHINE EVIDENCE. ──────────────────────────────
    let failed = collect_assay_evidence(
        ForgeGateEvidence::default(),
        Some(&run),
        &[command.clone()],
        true,
    );
    assert_eq!(failed.verdict, AssayVerdict::Fail);
    assert_eq!(
        failed.evidence.qa_passed,
        Some(false),
        "the gate boolean must record the failure"
    );
    let rejection = failed
        .evidence
        .deliverable_rejection
        .expect("a FAIL without a recorded rejection is a bare verdict");
    assert!(
        rejection.contains("CMD_FAIL"),
        "the rejection must name the machine reading: {rejection}"
    );
    assert!(
        rejection.contains(&command),
        "the rejection must name the exact failed command: {rejection}"
    );

    // ── 2. A PASS CARRIES NO REJECTION. ──────────────────────────────────────
    let run_pass = |_: &str| passing(&command);
    let passed = collect_assay_evidence(
        ForgeGateEvidence::default(),
        Some(&run_pass),
        &[command.clone()],
        true,
    );
    assert_eq!(passed.verdict, AssayVerdict::Pass);
    assert_eq!(passed.evidence.qa_passed, Some(true));
    assert!(
        passed.evidence.deliverable_rejection.is_none(),
        "a pass must not manufacture rejection text"
    );

    // ── 3. NEGATIVE: NO RUNNER IS ITSELF RECORDED EVIDENCE. ──────────────────
    let no_runner: Option<&dyn Fn(&str) -> CommandResult> = None;
    let blind =
        collect_assay_evidence(ForgeGateEvidence::default(), no_runner, &[command.clone()], true);
    assert_eq!(blind.verdict, AssayVerdict::Fail);
    assert_eq!(blind.evidence.qa_passed, Some(false));
    assert!(
        blind
            .evidence
            .deliverable_rejection
            .as_deref()
            .unwrap_or_default()
            .contains("no way to run them"),
        "a lane handed commands but no way to run them must say so on the evidence: {:?}",
        blind.evidence.deliverable_rejection
    );
}
