//! FORGE.QA — missing disposition routes to Hold (TST-FORGE-QA-006).
//!
//! CONTRACT. When QA returns a FAIL verdict with NO disposition (None), `route_qa_result` must route to
//! `RepairRouting::Hold`. The engine cannot choose a safe repair route without a legal disposition — operator/Lead
//! review is required. The test proves that None disposition always Holds regardless of budget state.
//!
//! Level: L3 Composition — exercises the production `route_qa_result` function from `forge::engine::qa_repair`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_qa__006__missing_disposition_hold

use forge::engine::qa_repair::{
    QaVerdict, RepairAttemptState, RepairBudget, RepairRouting, route_qa_result,
};

#[test]
#[allow(non_snake_case)]
fn forge_qa_006__missing_disposition_hold() {
    let budget = RepairBudget::default();

    // ── None disposition with zero attempts: must Hold. ────────────────────────────────────────────────

    let state = RepairAttemptState {
        repair_attempts: 0,
        replan_attempts: 0,
    };

    let routing = route_qa_result(QaVerdict::Fail, None, state, budget, false, false);

    match routing {
        RepairRouting::Hold { reason } => {
            assert!(
                reason.contains("disposition") || reason.contains("None"),
                "Hold reason must name the missing disposition: {reason}"
            );
        }
        other => panic!("None disposition must route to Hold, got {other:?}"),
    }

    // ── None disposition with exhausted budget: must still Hold (not Smith or Architect). ─────────────

    let state_exhausted = RepairAttemptState {
        repair_attempts: 3,
        replan_attempts: 2,
    };

    let routing = route_qa_result(QaVerdict::Fail, None, state_exhausted, budget, false, false);

    match routing {
        RepairRouting::Hold { reason } => {
            assert!(
                reason.contains("disposition") || reason.contains("None"),
                "Hold reason must name the missing disposition even with exhausted budget: {reason}"
            );
        }
        other => panic!(
            "None disposition with exhausted budget must still route to Hold, got {other:?}"
        ),
    }

    // ── NEGATIVE CONTROL: None disposition must never route to Smith or Architect. ────────────────────

    for (repair_attempts, replan_attempts) in [(0u32, 0u32), (1, 0), (0, 1), (3, 2)] {
        let state = RepairAttemptState {
            repair_attempts,
            replan_attempts,
        };
        let routing = route_qa_result(QaVerdict::Fail, None, state, budget, false, false);
        assert!(
            matches!(routing, RepairRouting::Hold { .. }),
            "None disposition must never route to Smith or Architect, got {routing:?} at \
             repair={repair_attempts} replan={replan_attempts}"
        );
    }
}
