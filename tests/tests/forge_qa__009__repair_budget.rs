//! FORGE.QA — repair budget enforcement (TST-FORGE-QA-009).
//!
//! CONTRACT. The repair budget (default 3 attempts) is a hard ceiling. When `repair_attempts >= max_repair_attempts`
//! and the disposition is REPAIR, `route_qa_result` must route to `RepairRouting::Hold`. Below the ceiling it must
//! route to `RepairRouting::Smith`. The test proves the exact boundary: at max-1 → Smith, at max → Hold.
//!
//! Level: L3 Composition — exercises the production `route_qa_result` function from `forge::engine::qa_repair`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_qa__009__repair_budget

use forge::engine::qa_repair::{
    QaDisposition, QaVerdict, RepairAttemptState, RepairBudget, RepairRouting, route_qa_result,
};

#[test]
#[allow(non_snake_case)]
fn forge_qa_009__repair_budget() {
    let budget = RepairBudget::default();
    assert_eq!(budget.max_repair_attempts, 3, "default repair budget must be 3");

    // ── Below budget: attempts 0, 1, 2 all route to Smith. ────────────────────────────────────────────

    for attempts in 0..3u32 {
        let state = RepairAttemptState {
            repair_attempts: attempts,
            replan_attempts: 0,
        };
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
                repair_attempts: attempts + 1
            },
            "REPAIR at attempts={attempts} (below max=3) must route to Smith"
        );
    }

    // ── At budget: attempts=3 routes to Hold. ─────────────────────────────────────────────────────────

    let state = RepairAttemptState {
        repair_attempts: 3,
        replan_attempts: 0,
    };
    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Repair),
        state,
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
        other => panic!("REPAIR at attempts=3 (max=3) must route to Hold, got {other:?}"),
    }

    // ── Over budget: attempts=4 also routes to Hold. ─────────────────────────────────────────────────

    let state = RepairAttemptState {
        repair_attempts: 4,
        replan_attempts: 0,
    };
    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Repair),
        state,
        budget,
        false,
        false,
    );
    assert!(
        matches!(routing, RepairRouting::Hold { .. }),
        "REPAIR at attempts=4 (over max=3) must route to Hold, got {routing:?}"
    );

    // ── Custom budget: max_repair_attempts=1, attempts=1 routes to Hold. ─────────────────────────────

    let custom_budget = RepairBudget {
        max_repair_attempts: 1,
        max_replan_attempts: 2,
    };
    let state = RepairAttemptState {
        repair_attempts: 1,
        replan_attempts: 0,
    };
    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Repair),
        state,
        custom_budget,
        false,
        false,
    );
    assert!(
        matches!(routing, RepairRouting::Hold { .. }),
        "REPAIR at attempts=1 with custom max=1 must route to Hold, got {routing:?}"
    );

    // ── Custom budget: max_repair_attempts=1, attempts=0 routes to Smith. ────────────────────────────

    let state = RepairAttemptState {
        repair_attempts: 0,
        replan_attempts: 0,
    };
    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Repair),
        state,
        custom_budget,
        false,
        false,
    );
    assert_eq!(
        routing,
        RepairRouting::Smith {
            repair_attempts: 1
        },
        "REPAIR at attempts=0 with custom max=1 must route to Smith"
    );
}
