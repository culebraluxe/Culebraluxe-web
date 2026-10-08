//! FORGE.COMPLETION_RECEIPT — held by another process (TST-FORGE-COMPLETION-RECEIPT-002).
//!
//! CONTRACT. A receipt claimed by one process reads as held — not free and not final — to every other
//! process: a second claim on the same id is refused (`false`) while `has_final` stays `false`. The two
//! handles here are two processes on one durable row: the `Arc`-shared production `MemoryLedger` stands
//! in for the receipt row both processes read, the way `SharedLedger` does in
//! `tests/tests/durable_completion_ledger.rs`. This is DEV-test step 2
//! (`tests/tests/forge_completion_receipt_dev.rs`) without a database.
//!
//! Level: L4 Adversarial — the negative half is load-bearing: the refusal must be per-receipt (a second,
//! unrelated receipt is still acquirable), or one in-flight unit would wedge the whole story. A bare
//! `false` with no `has_final` distinction is the defect the DAO's `HeldByAnother` variant exists to
//! close; the ledger-level equivalent pinned here is `!claim && !has_final`.
//!
//! WHAT IT DOES NOT COVER: the `HeldByAnother` enum variant itself and the 15-minute stale window live in
//! SQL (`db/src/forge_engine.rs:918-925`, `1098-1157`) and are covered with a database by
//! `tests/tests/forge_completion_receipt_dev.rs` and
//! `tests/tests/forge_completion_receipt__004__stale_pending_reclamation.rs`.
//!
//! Level detail: deterministic and isolated — no database, no network, never writes to PROD.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_completion_receipt__002__held_by_another_process

use std::sync::Arc;

use forge::engine::{CompletionLedger, MemoryLedger};

/// One claim in flight: the second process is refused, and is told apart from "already applied".
#[test]
fn forge_completion_receipt_002__held_by_another_process() {
    // Two processes, one durable row.
    let process_a = Arc::new(MemoryLedger::new());
    let process_b = process_a.clone();
    let id = forge::engine::completion_receipt_id("task-1");

    assert!(
        process_a.claim(&id).expect("claim answers"),
        "process A acquires the unit"
    );
    assert!(
        !process_b.claim(&id).expect("claim answers"),
        "process B is refused while A is mid-flight: refusing here is what stops duplicate work"
    );
    assert!(
        !process_b.has_final(&id).expect("has_final answers"),
        "held is not final: a refusal that read as 'already applied' would silently drop the unit"
    );

    // The refusal is per-receipt, not a global lock: an unrelated unit is still acquirable.
    let other = forge::engine::completion_receipt_id("task-2");
    assert!(
        process_b.claim(&other).expect("claim answers"),
        "one in-flight unit must not wedge every other receipt on the story"
    );
}
