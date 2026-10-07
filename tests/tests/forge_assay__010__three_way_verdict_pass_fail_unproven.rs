//! FORGE.ASSAY-010 — three-way verdict: PASS/FAIL/UNPROVEN (TST-FORGE-ASSAY-010).
//!
//! CONTRACT. The assay's verdict is three-valued, and each value has exactly one meaning. `adjudicate_assay`
//! (`forge/src/engine/assay.rs:28`) answers `Pass` only when every measured command passed AND the run maps to its
//! acceptance criteria; `Fail` when anything was measured against the contract and fell short (a failed command, an
//! unmeasurable command, or no commands at all); and `Unproven` when everything measured green but the run proves
//! nothing because no acceptance mapping exists. Collapsing `Unproven` into either neighbour destroys a finding:
//! into `Pass` it certifies what was never mapped, into `Fail` it erases "measured green" from the record.
//!
//! Level: L3 Composition — the production adjudicator, results faked at the adapter boundary.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_assay__010__three_way_verdict_pass_fail_unproven

use forge::engine::assay::{adjudicate_assay, AssayVerdict, CommandResult};

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

fn unmeasurable(command: &str) -> CommandResult {
    CommandResult {
        command: command.into(),
        exit_code: 0,
        passed: true,
        excerpt: String::new(),
        unmeasurable: true,
        output: String::new(),
    }
}

#[test]
fn forge_assay_010__three_way_verdict_pass_fail_unproven() {
    let commands = vec!["cargo test -p probe --test one".to_string()];

    // ── 1. PASS: green AND mapped. ───────────────────────────────────────────
    let pass = adjudicate_assay(&commands, &[result(&commands[0], true)], true);
    assert_eq!(pass.verdict, AssayVerdict::Pass);
    assert!(
        pass.blockers.is_empty(),
        "a pass carries no blockers: {:?}",
        pass.blockers
    );

    // ── 2. FAIL: a failed command fails the assay, mapped or not. ───────────
    let failed = adjudicate_assay(&commands, &[result(&commands[0], false)], true);
    assert_eq!(failed.verdict, AssayVerdict::Fail);
    assert!(
        failed.blockers.contains(&"CMD_FAIL"),
        "the failure must name its cause: {:?}",
        failed.blockers
    );
    let failed_unmapped = adjudicate_assay(&commands, &[result(&commands[0], false)], false);
    assert_eq!(
        failed_unmapped.verdict,
        AssayVerdict::Fail,
        "a failed command is FAIL, never UNPROVEN: an unmapped failure is still a failure"
    );

    // ── 3. FAIL: unmeasurable is not green. ──────────────────────────────────
    let blind = adjudicate_assay(&commands, &[unmeasurable(&commands[0])], true);
    assert_eq!(blind.verdict, AssayVerdict::Fail);
    assert!(
        blind.blockers.contains(&"COMMAND_UNMEASURABLE"),
        "an unmeasured command must name its cause: {:?}",
        blind.blockers
    );

    // ── 4. FAIL: no commands, no pass. ───────────────────────────────────────
    let empty = adjudicate_assay(&[], &[], true);
    assert_eq!(empty.verdict, AssayVerdict::Fail);
    assert!(
        empty.blockers.contains(&"NO_ASSAY_COMMANDS"),
        "measuring nothing must never read as a pass: {:?}",
        empty.blockers
    );

    // ── 5. UNPROVEN: green but unmapped proves nothing. ──────────────────────
    let unproven = adjudicate_assay(&commands, &[result(&commands[0], true)], false);
    assert_eq!(unproven.verdict, AssayVerdict::Unproven);
    assert!(
        unproven.blockers.contains(&"ACCEPTANCE_MAP_MISSING"),
        "the unproven reading must name the missing map: {:?}",
        unproven.blockers
    );

    // ── 6. NEGATIVE: the three values are distinct readings, not aliases. ───
    assert_ne!(
        AssayVerdict::Pass,
        AssayVerdict::Unproven,
        "UNPROVEN must not collapse into PASS, or green-but-unmapped certifies itself"
    );
    assert_ne!(
        AssayVerdict::Fail,
        AssayVerdict::Unproven,
        "UNPROVEN must not collapse into FAIL, or measured-green is erased from the record"
    );
    let multi = vec![
        "cargo test -p probe --test one".to_string(),
        "cargo check --workspace".to_string(),
    ];
    let mixed_results = vec![result(&multi[0], true), result(&multi[1], false)];
    let mixed = adjudicate_assay(&multi, &mixed_results, true);
    assert_eq!(
        mixed.verdict,
        AssayVerdict::Fail,
        "one failed command fails the assay even beside a passing one"
    );
}
