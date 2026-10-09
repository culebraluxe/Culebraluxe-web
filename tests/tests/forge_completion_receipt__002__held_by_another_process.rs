//! FORGE.COMPLETION_RECEIPT — held by another process (TST-FORGE-COMPLETION-RECEIPT-002).
//!
//! CONTRACT. One receipt row, two processes, and no unit applied twice: what the first process commits is
//! visible to the second, the second does not re-run it, and the predicate is PER RECEIPT — one committed
//! unit must never read as a global lock, or a single applied task would wedge every other unit on the
//! story. The two handles here are two processes on one durable row: the `Arc`-shared production
//! `MemoryLedger` stands in for the receipt row both processes read, the way `SharedLedger` does in
//! `tests/tests/durable_completion_ledger.rs`.
//!
//! UNDER THE SINGLE-CALL UNIT (FORGE-B1 slice 2) the state this test is named for changed shape, and the
//! change is the point: there is no longer a claim to hold, so "held" is no longer a state a process can
//! leave behind by dying — a unit that did not commit left NO receipt, which is the crash window the resume
//! reconciles on. The literal held state survives for the pre-unit claim row (a `pending` receipt inside
//! the 15-minute window) and is read as `CompletionApply::Busy`; it is SQL's, so it is covered with a
//! database by `tests/tests/forge_completion_receipt_dev.rs` step 5b and
//! `tests/tests/forge_completion_receipt__004__stale_pending_reclamation.rs`. What an in-memory ledger can
//! hold is the durable half: per-receipt idempotence and per-receipt isolation.
//!
//! Level: L4 Adversarial — the negative half is load-bearing: the refusal must be per-receipt, and
//! `has_final` must be false for a unit nobody applied.
//!
//! Level detail: deterministic and isolated — no database, no network, never writes to PROD.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_completion_receipt__002__held_by_another_process

use std::sync::Arc;

use forge::engine::{
    apply_completion_unit, CompletionApply, CompletionLedger, CompletionRecord, ForgeGateEvidence,
    MemoryLedger,
};

fn record(task_id: &str) -> CompletionRecord {
    CompletionRecord {
        task_id: task_id.into(),
        process_instance_id: "11111111-1111-1111-1111-111111111111".into(),
        story_id: "story-1".into(),
        node_id: None,
        evidence: ForgeGateEvidence {
            candidate_sha: Some("sha".into()),
            ..Default::default()
        },
    }
}

/// One committed unit: visible to the other process, refused to it, and not a global lock.
#[test]
fn forge_completion_receipt_002__held_by_another_process() {
    // Two processes, one durable row.
    let process_a = Arc::new(MemoryLedger::new());
    let process_b = process_a.clone();
    let first = forge::engine::completion_receipt_id("task-1");
    let other = forge::engine::completion_receipt_id("task-2");

    assert_eq!(
        apply_completion_unit(process_a.as_ref(), record("task-1")).expect("unit applies"),
        CompletionApply::Applied,
        "process A commits the unit"
    );
    assert!(
        process_b.has_final(&first).expect("has_final answers"),
        "and the receipt A committed is the one B reads: one row, two processes"
    );
    assert_eq!(
        apply_completion_unit(process_b.as_ref(), record("task-1"))
            .expect("the unit answers, it does not fail"),
        CompletionApply::AlreadyApplied,
        "B does not re-run what A committed, and is told which answer it got"
    );

    // The refusal is per-receipt, not a global lock: an unrelated unit is still applyable by B. Under the old
    // claim/finalize shape this was `!claim` on one receipt and `claim` on another; the property is the same
    // and it is the one that keeps one applied task from wedging the whole story.
    assert!(
        !process_b.has_final(&other).expect("has_final answers"),
        "a receipt nobody applied is not final: one unit's commitment is not every unit's"
    );
    assert_eq!(
        apply_completion_unit(process_b.as_ref(), record("task-2")).expect("unit applies"),
        CompletionApply::Applied,
        "an unrelated unit on the same story belongs to whoever applies it first"
    );
    assert!(
        process_b.has_final(&other).expect("has_final answers"),
        "and now it is final — while the first unit's receipt is unchanged"
    );
    assert_eq!(
        apply_completion_unit(process_a.as_ref(), record("task-2"))
            .expect("the unit answers, it does not fail"),
        CompletionApply::AlreadyApplied,
        "A sees B's commitment the same way B saw A's"
    );
}
