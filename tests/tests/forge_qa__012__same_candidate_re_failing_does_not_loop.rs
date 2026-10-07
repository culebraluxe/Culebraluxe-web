//! FORGE.QA — same candidate re-failing does not loop (TST-FORGE-QA-012).
//!
//! CONTRACT. When the same candidate SHA re-fails the same machine classification with no new candidate in between
//! (`no_progress = true`), `route_qa_result` must route to `RepairRouting::Hold`. This is the engine's loop
//! prevention: without this gate, a persistently failing candidate would cycle through repair → fail → repair →
//! fail indefinitely. The test proves that no_progress=true always Holds, even when the disposition is REPAIR and
//! the budget is available — the budget is irrelevant when nothing has changed.
//!
//! Level: L3 Composition — exercises the production `route_qa_result` function from `forge::engine::qa_repair`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_qa__012__same_candidate_re_failing_does_not_loop

use forge::engine::qa_repair::{
    QaDisposition, QaVerdict, RepairAttemptState, RepairBudget, RepairRouting, route_qa_result,
};

#[test]
#[allow(non_snake_case)]
fn forge_qa_012__same_candidate_re_failing_does_not_loop() {
    let budget = RepairBudget::default();

    // ── THE LOOP PREVENTION: no_progress=true with REPAIR and zero attempts must Hold. ────────────────
    // This is the exact scenario: a candidate failed, was repaired, re-failed with the same SHA. The budget
    // says "2 more repairs available" but there is no new candidate — repairing the same code again is a loop.

    let state = RepairAttemptState {
        repair_attempts: 0,
        replan_attempts: 0,
    };

    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Repair),
        state,
        budget,
        true,
        false,
    );

    match routing {
        RepairRouting::Hold { reason } => {
            assert!(
                reason.contains("NO_PROGRESS"),
                "Hold reason must name the no-progress loop prevention: {reason}"
            );
        }
        other => panic!(
            "no_progress with REPAIR and available budget must Hold to prevent looping, got {other:?}"
        ),
    }

    // ── The same candidate re-failing after several repair attempts: still Hold. ────────────────────

    let state_after_repairs = RepairAttemptState {
        repair_attempts: 2,
        replan_attempts: 0,
    };

    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Repair),
        state_after_repairs,
        budget,
        true,
        false,
    );

    assert!(
        matches!(routing, RepairRouting::Hold { .. }),
        "no_progress after 2 repairs must still Hold, got {routing:?}"
    );

    // ── no_progress with REPLAN: must Hold (not Architect). ──────────────────────────────────────────

    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Replan),
        state,
        budget,
        true,
        false,
    );

    assert!(
        matches!(routing, RepairRouting::Hold { .. }),
        "no_progress with REPLAN must Hold to prevent looping, got {routing:?}"
    );

    // ── NEGATIVE CONTROL: without no_progress, the same inputs route to Smith (proving the test exercises
    //    the no_progress flag and not some other condition). ──────────────────────────────────────────

    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Repair),
        state,
        budget,
        false,
        false,
    );

    assert_eq!(
        routing,
        RepairRouting::Smith {
            repair_attempts: 1
        },
        "without no_progress, REPAIR with available budget must route to Smith — this proves the test \
         is exercising the no_progress flag"
    );

    // ── NEGATIVE CONTROL: no_progress=false with REPLAN routes to Architect. ────────────────────────

    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Replan),
        state,
        budget,
        false,
        false,
    );

    assert_eq!(
        routing,
        RepairRouting::Architect {
            replan_attempts: 1
        },
        "without no_progress, REPLAN with available budget must route to Architect"
    );
}
