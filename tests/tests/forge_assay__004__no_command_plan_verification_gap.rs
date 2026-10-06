//! FORGE.ASSAY-004 — no command plan → verification gap.
//!
//! CONTRACT. QA cannot verify what it was never given a way to run. When the assay fails and
//! the lane could not form or run a valid assay command plan (missing `## Assay commands` in
//! the packet, or no frozen plan), `route_qa_result` holds with a `VERIFICATION GAP` reason:
//! repair cannot help, so the engine must not burn repair budget on it — the packet/config
//! is fixed, then the story re-dispatches. The ready gate says the same thing up front: a
//! `FEATURE` story with no assay plan is not ready (`ready-gate:missing-assay-plan`).
//!
//! Level: L3 Composition — the production repair router and ready gate, no I/O.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_assay__004__no_command_plan_verification_gap

use forge::engine::qa_repair::{
    route_qa_result, RepairAttemptState, RepairBudget, RepairRouting, QaDisposition, QaVerdict,
};
use forge::engine::ready_gate::{parse_assay_commands, story_ready_to_run_reasons};

fn fresh() -> (RepairAttemptState, RepairBudget) {
    (
        RepairAttemptState {
            repair_attempts: 0,
            replan_attempts: 0,
        },
        RepairBudget::default(),
    )
}

#[test]
fn forge_assay_004__no_command_plan_verification_gap() {
    // ── 1. A FAILED ASSAY WITH NO COMMAND PLAN HOLDS AS A VERIFICATION GAP. ──
    let (state, budget) = fresh();
    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Repair),
        state,
        budget,
        false,
        true,
    );
    match routing {
        RepairRouting::Hold { reason } => {
            assert!(
                reason.contains("VERIFICATION GAP"),
                "the hold must name the gap, not a generic failure: {reason}"
            );
            assert!(
                reason.contains("assay command plan"),
                "the hold must say what is missing so the packet can be fixed: {reason}"
            );
        }
        other => panic!("no command plan must hold, never repair or pass: {other:?}"),
    }

    // ── 2. THE READY GATE REFUSES A PLAN-LESS STORY UP FRONT. ────────────────
    assert!(
        parse_assay_commands(None).is_empty(),
        "no assay section parses to no commands"
    );
    assert!(
        parse_assay_commands(Some("   \n # a comment, not a command\n")).is_empty(),
        "blank lines and comments are not a command plan"
    );
    let reasons = story_ready_to_run_reasons("FEATURE", Some("it works"), None, None);
    assert!(
        reasons.contains(&"ready-gate:missing-assay-plan"),
        "a feature story with no assay plan is not ready: {reasons:?}"
    );
    assert_eq!(
        parse_assay_commands(Some("cargo test -p probe\ncargo check --workspace")),
        vec![
            "cargo test -p probe".to_string(),
            "cargo check --workspace".to_string()
        ],
        "a real plan parses to its lines, trimmed, in order"
    );

    // ── 3. NEGATIVE: THE GAP IS ABOUT THE PLAN, NOT THE FAILURE. ────────────
    // Same failure, valid plan: the repair road stays open — the gap must not swallow it.
    let (state, budget) = fresh();
    let repaired = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Repair),
        state,
        budget,
        false,
        false,
    );
    assert!(
        matches!(repaired, RepairRouting::Smith { .. }),
        "a failed assay WITH a command plan routes to repair, not to a gap hold: {repaired:?}"
    );
    // A pass is a pass even when the gap flag is set: the gap explains failures, never pass.
    let (state, budget) = fresh();
    let passed = route_qa_result(QaVerdict::Pass, None, state, budget, false, true);
    assert_eq!(
        passed,
        RepairRouting::Pass,
        "a passing assay never holds for a verification gap: {passed:?}"
    );
}
