//! FORGE.QA — ESCALATE disposition always routes to Hold (TST-FORGE-QA-005).
//!
//! CONTRACT. When QA returns a FAIL verdict with an ESCALATE disposition, `route_qa_result` must ALWAYS route
//! to `RepairRouting::Hold` regardless of budget state. ESCALATE means the operator/Lead must intervene — the
//! engine must never auto-repair or auto-replan an escalation. The test proves this with multiple budget states.
//!
//! Level: L3 Composition — exercises the production `route_qa_result` function from `forge::engine::qa_repair`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_qa__005__escalate

use forge::engine::qa_repair::{
    QaDisposition, QaVerdict, RepairAttemptState, RepairBudget, RepairRouting, route_qa_result,
};

#[test]
#[allow(non_snake_case)]
fn forge_qa_005__escalate() {
    let budget = RepairBudget::default();

    // ── ESCALATE with zero attempts: must still Hold. ───────────────────────────────────────────────────

    let state = RepairAttemptState {
        repair_attempts: 0,
        replan_attempts: 0,
    };

    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Escalate),
        state,
        budget,
        false,
        false,
    );

    match routing {
        RepairRouting::Hold { reason } => {
            assert!(
                reason.contains("ESCALATE"),
                "Hold reason must name the escalation: {reason}"
            );
        }
        other => panic!("ESCALATE must always route to Hold, got {other:?}"),
    }

    // ── ESCALATE with exhausted repair budget: must still Hold (not Smith). ───────────────────────────

    let state_exhausted = RepairAttemptState {
        repair_attempts: 3,
        replan_attempts: 2,
    };

    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Escalate),
        state_exhausted,
        budget,
        false,
        false,
    );

    match routing {
        RepairRouting::Hold { reason } => {
            assert!(
                reason.contains("ESCALATE"),
                "Hold reason must name the escalation even with exhausted budget: {reason}"
            );
        }
        other => panic!("ESCALATE with exhausted budget must still route to Hold, got {other:?}"),
    }

    // ── NEGATIVE CONTROL: ESCALATE must never route to Smith or Architect. ────────────────────────────

    for (repair_attempts, replan_attempts) in [(0u32, 0u32), (1, 0), (0, 1), (3, 2)] {
        let state = RepairAttemptState {
            repair_attempts,
            replan_attempts,
        };
        let routing = route_qa_result(
            QaVerdict::Fail,
            Some(QaDisposition::Escalate),
            state,
            budget,
            false,
            false,
        );
        assert!(
            matches!(routing, RepairRouting::Hold { .. }),
            "ESCALATE must never route to Smith or Architect, got {routing:?} at \
             repair={repair_attempts} replan={replan_attempts}"
        );
    }
}
