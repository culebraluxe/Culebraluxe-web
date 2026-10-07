//! FORGE.QA — replan budget enforcement (TST-FORGE-QA-010).
//!
//! CONTRACT. The replan budget (default 2 attempts) is a hard ceiling. When `replan_attempts >= max_replan_attempts`
//! and the disposition is REPLAN, `route_qa_result` must route to `RepairRouting::Hold`. Below the ceiling it must
//! route to `RepairRouting::Architect`. The test proves the exact boundary: at max-1 → Architect, at max → Hold.
//!
//! Level: L3 Composition — exercises the production `route_qa_result` function from `forge::engine::qa_repair`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_qa__010__replan_budget

use forge::engine::qa_repair::{
    QaDisposition, QaVerdict, RepairAttemptState, RepairBudget, RepairRouting, route_qa_result,
};

#[test]
#[allow(non_snake_case)]
fn forge_qa_010__replan_budget() {
    let budget = RepairBudget::default();
    assert_eq!(budget.max_replan_attempts, 2, "default replan budget must be 2");

    // ── Below budget: attempts 0, 1 both route to Architect. ─────────────────────────────────────────

    for attempts in 0..2u32 {
        let state = RepairAttemptState {
            repair_attempts: 0,
            replan_attempts: attempts,
        };
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
                replan_attempts: attempts + 1
            },
            "REPLAN at attempts={attempts} (below max=2) must route to Architect"
        );
    }

    // ── At budget: attempts=2 routes to Hold. ─────────────────────────────────────────────────────────

    let state = RepairAttemptState {
        repair_attempts: 0,
        replan_attempts: 2,
    };
    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Replan),
        state,
        budget,
        false,
        false,
    );
    match routing {
        RepairRouting::Hold { reason } => {
            assert!(
                reason.contains("Replan budget exhausted"),
                "Hold reason must name the replan budget exhaustion: {reason}"
            );
        }
        other => panic!("REPLAN at attempts=2 (max=2) must route to Hold, got {other:?}"),
    }

    // ── Over budget: attempts=3 also routes to Hold. ─────────────────────────────────────────────────

    let state = RepairAttemptState {
        repair_attempts: 0,
        replan_attempts: 3,
    };
    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Replan),
        state,
        budget,
        false,
        false,
    );
    assert!(
        matches!(routing, RepairRouting::Hold { .. }),
        "REPLAN at attempts=3 (over max=2) must route to Hold, got {routing:?}"
    );

    // ── Custom budget: max_replan_attempts=1, attempts=1 routes to Hold. ─────────────────────────────

    let custom_budget = RepairBudget {
        max_repair_attempts: 3,
        max_replan_attempts: 1,
    };
    let state = RepairAttemptState {
        repair_attempts: 0,
        replan_attempts: 1,
    };
    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Replan),
        state,
        custom_budget,
        false,
        false,
    );
    assert!(
        matches!(routing, RepairRouting::Hold { .. }),
        "REPLAN at attempts=1 with custom max=1 must route to Hold, got {routing:?}"
    );

    // ── Custom budget: max_replan_attempts=1, attempts=0 routes to Architect. ────────────────────────

    let state = RepairAttemptState {
        repair_attempts: 0,
        replan_attempts: 0,
    };
    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Replan),
        state,
        custom_budget,
        false,
        false,
    );
    assert_eq!(
        routing,
        RepairRouting::Architect {
            replan_attempts: 1
        },
        "REPLAN at attempts=0 with custom max=1 must route to Architect"
    );
}
