//! FORGE.QA — REPLAN disposition routes to Architect when under budget (TST-FORGE-QA-004).
//!
//! CONTRACT. When QA returns a FAIL verdict with a REPLAN disposition and the replan budget is not exhausted,
//! `route_qa_result` must route the story to `RepairRouting::Architect` with the incremented attempt count. When
//! the replan budget IS exhausted, it must route to `RepairRouting::Hold`. The test proves both directions.
//!
//! Level: L3 Composition — exercises the production `route_qa_result` function from `forge::engine::qa_repair`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_qa__004__replan

use forge::engine::qa_repair::{
    QaDisposition, QaVerdict, RepairAttemptState, RepairBudget, RepairRouting, route_qa_result,
};

#[test]
#[allow(non_snake_case)]
fn forge_qa_004__replan() {
    // ── HAPPY PATH: REPLAN disposition under budget routes to Architect. ───────────────────────────────

    let state = RepairAttemptState {
        repair_attempts: 0,
        replan_attempts: 0,
    };
    let budget = RepairBudget::default();

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
        "REPLAN under budget must route to Architect with incremented attempt count"
    );

    // ── REFUSAL PATH: REPLAN disposition at budget routes to Hold. ──────────────────────────────────────

    let state_at_budget = RepairAttemptState {
        repair_attempts: 0,
        replan_attempts: 2,
    };

    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Replan),
        state_at_budget,
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
        other => panic!("REPLAN at budget must route to Hold, got {other:?}"),
    }

    // ── BOUNDARY: one attempt still under budget (attempts=1, max=2) routes to Architect. ─────────────

    let state_one_under = RepairAttemptState {
        repair_attempts: 0,
        replan_attempts: 1,
    };

    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Replan),
        state_one_under,
        budget,
        false,
        false,
    );

    assert_eq!(
        routing,
        RepairRouting::Architect {
            replan_attempts: 2
        },
        "REPLAN at attempts=1 (max=2) must still route to Architect"
    );
}
