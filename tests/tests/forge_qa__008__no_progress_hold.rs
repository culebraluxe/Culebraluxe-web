//! FORGE.QA — no progress routes to Hold (TST-FORGE-QA-008).
//!
//! CONTRACT. When `no_progress` is true (the same candidate SHA re-failed the same machine classification with no
//! new candidate in between), `route_qa_result` must route to `RepairRouting::Hold` regardless of disposition or
//! budget. The engine must not auto-launch another repair cycle when nothing has changed. The test proves that
//! no_progress overrides all other inputs.
//!
//! Level: L3 Composition — exercises the production `route_qa_result` function from `forge::engine::qa_repair`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_qa__008__no_progress_hold

use forge::engine::qa_repair::{
    QaDisposition, QaVerdict, RepairAttemptState, RepairBudget, RepairRouting, route_qa_result,
};

#[test]
#[allow(non_snake_case)]
fn forge_qa_008__no_progress_hold() {
    let budget = RepairBudget::default();
    let state = RepairAttemptState {
        repair_attempts: 0,
        replan_attempts: 0,
    };

    // ── no_progress=true with REPAIR disposition: must Hold (not Smith). ─────────────────────────────

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
                "Hold reason must name the no-progress condition: {reason}"
            );
        }
        other => panic!("no_progress must route to Hold, got {other:?}"),
    }

    // ── no_progress=true with REPLAN disposition: must Hold (not Architect). ─────────────────────────

    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Replan),
        state,
        budget,
        true,
        false,
    );

    match routing {
        RepairRouting::Hold { reason } => {
            assert!(
                reason.contains("NO_PROGRESS"),
                "Hold reason must name the no-progress condition with REPLAN: {reason}"
            );
        }
        other => panic!("no_progress with REPLAN must route to Hold, got {other:?}"),
    }

    // ── no_progress=true with ESCALATE disposition: must Hold. ────────────────────────────────────────

    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Escalate),
        state,
        budget,
        true,
        false,
    );

    assert!(
        matches!(routing, RepairRouting::Hold { .. }),
        "no_progress with ESCALATE must route to Hold, got {routing:?}"
    );

    // ── no_progress=true with None disposition: must Hold. ────────────────────────────────────────────

    let routing = route_qa_result(QaVerdict::Fail, None, state, budget, true, false);

    assert!(
        matches!(routing, RepairRouting::Hold { .. }),
        "no_progress with None disposition must route to Hold, got {routing:?}"
    );

    // ── NEGATIVE CONTROL: no_progress=true must never route to Smith or Architect. ────────────────────

    for disposition in [
        Some(QaDisposition::Repair),
        Some(QaDisposition::Replan),
        Some(QaDisposition::Escalate),
        None,
    ] {
        let routing = route_qa_result(QaVerdict::Fail, disposition, state, budget, true, false);
        assert!(
            matches!(routing, RepairRouting::Hold { .. }),
            "no_progress must never route to Smith or Architect, got {routing:?} with {disposition:?}"
        );
    }
}
