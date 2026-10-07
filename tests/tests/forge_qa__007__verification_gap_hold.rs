//! FORGE.QA — verification gap routes to Hold (TST-FORGE-QA-007).
//!
//! CONTRACT. When `verification_gap` is true (QA could not form/run a valid assay command plan), `route_qa_result`
//! must route to `RepairRouting::Hold` regardless of disposition or budget. A verification gap means the packet
//! or config is broken — repair cannot help. The test proves that verification_gap overrides all other inputs.
//!
//! Level: L3 Composition — exercises the production `route_qa_result` function from `forge::engine::qa_repair`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_qa__007__verification_gap_hold

use forge::engine::qa_repair::{
    QaDisposition, QaVerdict, RepairAttemptState, RepairBudget, RepairRouting, route_qa_result,
};

#[test]
#[allow(non_snake_case)]
fn forge_qa_007__verification_gap_hold() {
    let budget = RepairBudget::default();
    let state = RepairAttemptState {
        repair_attempts: 0,
        replan_attempts: 0,
    };

    // ── verification_gap=true with REPAIR disposition: must Hold (not Smith). ────────────────────────

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
                "Hold reason must name the verification gap: {reason}"
            );
        }
        other => panic!("verification_gap must route to Hold, got {other:?}"),
    }

    // ── verification_gap=true with REPLAN disposition: must Hold (not Architect). ────────────────────

    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Replan),
        state,
        budget,
        false,
        true,
    );

    match routing {
        RepairRouting::Hold { reason } => {
            assert!(
                reason.contains("VERIFICATION GAP"),
                "Hold reason must name the verification gap with REPLAN: {reason}"
            );
        }
        other => panic!("verification_gap with REPLAN must route to Hold, got {other:?}"),
    }

    // ── verification_gap=true with ESCALATE disposition: must Hold. ──────────────────────────────────

    let routing = route_qa_result(
        QaVerdict::Fail,
        Some(QaDisposition::Escalate),
        state,
        budget,
        false,
        true,
    );

    assert!(
        matches!(routing, RepairRouting::Hold { .. }),
        "verification_gap with ESCALATE must route to Hold, got {routing:?}"
    );

    // ── verification_gap=true with None disposition: must Hold. ───────────────────────────────────────

    let routing = route_qa_result(QaVerdict::Fail, None, state, budget, false, true);

    assert!(
        matches!(routing, RepairRouting::Hold { .. }),
        "verification_gap with None disposition must route to Hold, got {routing:?}"
    );

    // ── NEGATIVE CONTROL: verification_gap=true must never route to Smith or Architect. ───────────────

    for disposition in [
        Some(QaDisposition::Repair),
        Some(QaDisposition::Replan),
        Some(QaDisposition::Escalate),
        None,
    ] {
        let routing = route_qa_result(QaVerdict::Fail, disposition, state, budget, false, true);
        assert!(
            matches!(routing, RepairRouting::Hold { .. }),
            "verification_gap must never route to Smith or Architect, got {routing:?} with {disposition:?}"
        );
    }
}
