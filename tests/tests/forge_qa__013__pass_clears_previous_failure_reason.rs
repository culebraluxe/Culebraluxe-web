//! FORGE.QA — a PASS verdict clears the previous failure reason (TST-FORGE-QA-013).
//!
//! Contract: when the Assay lane measures a PASS, the previous failure reason the run carries
//! (`ForgeGateEvidence::last_failure`, the durable `forge_last_failure_reason` the work queue
//! reads) must be cleared. A story that failed a stage, was repaired, and then PASSES its
//! re-verification is no longer a failure — the reason that sent it to repair is resolved by
//! the pass that closed the repair loop.
//!
//! This is the QA lane's own reading, and the seam is the same one production uses:
//!
//!   1. **the RUST_CONTRACT measurement** — `collect_rust_contract_assay_evidence`
//!      (`forge/src/engine/assay.rs:132`), reached in production through
//!      `AssayHooks::turn_without_model` → `run_rust_contract_qa` (`forge/src/roles/qa.rs:127`).
//!      The lane MEASURES instead of talking: the verdict is a function of what the commands did,
//!      and the evidence it produces is the node's evidence.
//!   2. **the general measurement** — `collect_assay_evidence` (`forge/src/engine/assay.rs:277`),
//!      reached through `AssayHooks::interpret_turn` → `read_assay_measurement`
//!      (`forge/src/roles/qa.rs:253`).
//!
//! The pass path already clears the CURRENT rejection (`deliverable_rejection = None`) — the
//! same pass must clear the PREVIOUS reason. The durable merge agrees a resolved failure is
//! cleared (`db::ForgeEngineDao::merge_workflow_evidence` nulls `last_failure` when
//! `release_failure_resolved` is set, and the release executor's publish-success path is the one
//! production caller that sets it) — the QA pass is the other resolution, and it has no clearing
//! at all.
//!
//! Negative and fault cases are load-bearing: a FAIL must NOT clear the reason (the failure
//! stands until a pass resolves it), and a pass with failing runtime tests records the NEW
//! observation as the reason — the rule is about a CLEAN pass, not about erasing every failure.
//!
//! Deterministic and isolated: the command runner is a scripted closure at the adapter boundary,
//! never a shell. No database, no network, no PROD mutation.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_qa__013__pass_clears_previous_failure_reason

use forge::engine::assay::{
    collect_assay_evidence, collect_rust_contract_assay_evidence, AssayVerdict, CommandResult,
};
use forge::engine::facts::ForgeGateEvidence;
use workflow::Value;

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-QA-013";
/// The candidate the run is verifying, and the base it was measured against.
const CANDIDATE: &str = "0123456789abcdef0123456789abcdef01234567";
const BASE: &str = "89abcdef0123456789abcdef0123456789abcdef";
/// The reason a previous, failed stage left on the run — what a PASS must clear.
const PREVIOUS_FAILURE: &str = "PROD_MIGRATION: migration 244 failed to apply on prod";

/// The command runner, scripted at the adapter boundary. Every command the QA lane runs passes,
/// and the production-edit range diff is empty, so the measurement is a clean PASS.
fn passing_runner(command: &str) -> CommandResult {
    CommandResult {
        command: command.to_string(),
        exit_code: 0,
        passed: true,
        excerpt: String::new(),
        unmeasurable: false,
        output: String::new(),
    }
}

/// The command runner whose runtime test fails while the structural check passes — the
/// RUST_CONTRACT authoring case where the authored test currently fails against the application.
fn runtime_failing_runner(command: &str) -> CommandResult {
    let runtime = command.starts_with("cargo test") || command.starts_with("cargo nextest");
    CommandResult {
        command: command.to_string(),
        exit_code: if runtime { 101 } else { 0 },
        passed: !runtime,
        excerpt: String::new(),
        unmeasurable: false,
        output: String::new(),
    }
}

/// The gate as a run that already failed a stage carries it: a valid candidate, a recorded base,
/// and the previous stage's failure reason still on the evidence.
fn gate_with_previous_failure() -> ForgeGateEvidence {
    let mut gate = ForgeGateEvidence {
        candidate_sha: Some(CANDIDATE.into()),
        last_failure: Some(PREVIOUS_FAILURE.into()),
        ..ForgeGateEvidence::default()
    };
    gate.extra
        .insert("recordedBase", Value::from(BASE));
    gate
}

/// The RUST_CONTRACT authoring checks: one runtime test and one structural check, both named in
/// the packet's acceptance, so the acceptance is mapped.
fn contract_commands() -> Vec<String> {
    vec![
        "cargo test --manifest-path Cargo.toml -p test-harness --test forge_qa__013__pass_clears_previous_failure_reason".into(),
        "cargo check --manifest-path Cargo.toml --workspace --all-targets".into(),
    ]
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-QA-013); the file and the assay use it.
fn forge_qa_013__pass_clears_previous_failure_reason() {
    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CONTRACT — A CLEAN RUST_CONTRACT PASS CLEARS THE PREVIOUS FAILURE REASON. The lane
    //    measures a PASS (every command passed, the acceptance is mapped, no production code was
    //    touched), and the evidence it produces must carry no failure reason: the pass resolved
    //    the failure that sent the story to repair.
    // -----------------------------------------------------------------------------------------------------------
    let measured = collect_rust_contract_assay_evidence(
        gate_with_previous_failure(),
        Some(&passing_runner),
        &contract_commands(),
        true,
    );
    assert_eq!(
        measured.verdict,
        AssayVerdict::Pass,
        "{HARNESS} ({STORY_ID}): every command passed and the acceptance is mapped, so the measurement is a PASS"
    );
    assert_eq!(
        measured.evidence.qa_passed,
        Some(true),
        "{HARNESS}: the gate records the measured PASS"
    );
    assert!(
        measured.evidence.deliverable_rejection.is_none(),
        "{HARNESS}: a PASS clears the current rejection, got: {:?}",
        measured.evidence.deliverable_rejection
    );
    assert!(
        measured.evidence.last_failure.is_none(),
        "{HARNESS}: a PASS must clear the previous failure reason {PREVIOUS_FAILURE:?}, but it survived the pass: {:?}",
        measured.evidence.last_failure
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE SAME CONTRACT ON THE GENERAL MEASUREMENT PATH. `collect_assay_evidence` is the
    //    reading every non-contract verification turn goes through; the rule is the lane's, not
    //    one path's.
    // -----------------------------------------------------------------------------------------------------------
    let general = collect_assay_evidence(
        gate_with_previous_failure(),
        Some(&passing_runner),
        &contract_commands(),
        true,
    );
    assert_eq!(general.verdict, AssayVerdict::Pass);
    assert_eq!(general.evidence.qa_passed, Some(true));
    assert!(
        general.evidence.last_failure.is_none(),
        "{HARNESS}: a PASS on the general measurement path must also clear the previous failure reason, got: {:?}",
        general.evidence.last_failure
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE — A FAIL DOES NOT CLEAR THE REASON. The failure stands until a pass resolves
    //    it; a rule that cleared on any verdict would erase the evidence of the failure itself.
    //    A failing structural check fails the gate and keeps the reason.
    // -----------------------------------------------------------------------------------------------------------
    let failing = |command: &str| -> CommandResult {
        CommandResult {
            command: command.to_string(),
            exit_code: 1,
            passed: false,
            excerpt: String::new(),
            unmeasurable: false,
            output: String::new(),
        }
    };
    let failed = collect_rust_contract_assay_evidence(
        gate_with_previous_failure(),
        Some(&failing),
        &contract_commands(),
        true,
    );
    assert_eq!(failed.verdict, AssayVerdict::Fail);
    assert_eq!(failed.evidence.qa_passed, Some(false));
    assert!(
        failed.evidence.last_failure.is_some(),
        "{HARNESS}: a FAIL must not clear the previous failure reason"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. FAULT — A PASS WITH A FAILING RUNTIME TEST RECORDS THE NEW OBSERVATION. The RUST_CONTRACT
    //    authoring rule lets a failing runtime test be product evidence (the deliverable is the
    //    test); that observation REPLACES the previous reason — it is the new failure the story
    //    carries, not the stale one.
    // -----------------------------------------------------------------------------------------------------------
    let observed = collect_rust_contract_assay_evidence(
        gate_with_previous_failure(),
        Some(&runtime_failing_runner),
        &contract_commands(),
        true,
    );
    assert_eq!(observed.verdict, AssayVerdict::Pass);
    assert_eq!(observed.evidence.qa_passed, Some(true));
    let reason = observed.evidence.last_failure.as_deref().unwrap_or_default();
    assert!(
        reason.contains("product debugging is separate work"),
        "{HARNESS}: a pass with a failing runtime test records the observation as the new reason, got: {reason:?}"
    );
    assert_ne!(
        reason, PREVIOUS_FAILURE,
        "{HARNESS}: the new observation replaces the previous reason, not the other way round"
    );
}
