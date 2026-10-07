//! FORGE.QA — REPAIR disposition routes to Smith when under budget (TST-FORGE-QA-003).
//!
//! CONTRACT. When QA returns a FAIL verdict with a REPAIR disposition and the repair budget is not exhausted,
//! `route_qa_result` must route the story to `RepairRouting::Smith` with the incremented attempt count. When the
//! repair budget IS exhausted, it must route to `RepairRouting::Hold`. The test proves both directions: the happy
//! path (under budget → Smith) and the refusal path (at budget → Hold).
//!
//! Level: L3 Composition — exercises the production `route_qa_result` function from `forge::engine::qa_repair`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_qa__003__repair

use forge::engine::qa_repair::{
    QaDisposition, QaVerdict, RepairAttemptState, RepairBudget, RepairRouting, route_qa_result,
};

#[test]
#[allow(non_snake_case)]
fn forge_qa_003__repair() {
    // ── HAPPY PATH: REPAIR disposition under budget routes to Smith. ────────────────────────────────────

    let state = RepairAttemptState {
        repair_attempts: 0,
        replan_attempts: 0,
    };
    let budget = RepairBudget::default();

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
        "REPAIR under budget must route to Smith with incremented attempt count"
    );

    // ── REFUSAL PATH: REPAIR disposition at budget routes to Hold. ──────────────────────────────────────

    let state_at_budget = RepairAttemptState {
        repair_attempts: 3,
        replan_attempts: 0,
    };

    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Repair),
        state_at_budget,
        budget,
        false,
        false,
    );

    match routing {
        RepairRouting::Hold { reason } => {
            assert!(
                reason.contains("Repair budget exhausted"),
                "Hold reason must name the repair budget exhaustion: {reason}"
            );
        }
        other => panic!("REPAIR at budget must route to Hold, got {other:?}"),
    }

    // ── BOUNDARY: one attempt still under budget (attempts=2, max=3) routes to Smith. ──────────────────

    let state_one_under = RepairAttemptState {
        repair_attempts: 2,
        replan_attempts: 0,
    };

    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Repair),
        state_one_under,
        budget,
        false,
        false,
    );

    assert_eq!(
        routing,
        RepairRouting::Smith {
            repair_attempts: 3
        },
        "REPAIR at attempts=2 (max=3) must still route to Smith"
    );
}
