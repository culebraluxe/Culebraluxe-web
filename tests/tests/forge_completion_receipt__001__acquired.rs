//! FORGE.COMPLETION_RECEIPT — acquired (TST-FORGE-COMPLETION-RECEIPT-001).
//!
//! CONTRACT. The first application of a unit acquires it: `apply` answers `Applied`, the receipt reads
//! final afterwards, and a second look at the same unit answers `AlreadyApplied`, merging nothing and
//! spending nothing. This is DEV-test step 1 (`tests/tests/forge_completion_receipt_dev.rs`) replayed
//! without a database, against the ledger the production runtime installs by default (`ForgeRuntime`'s
//! default constructor builds `Arc::new(MemoryLedger::new())`, `forge/src/engine/runtime.rs:123`).
//!
//! Level: L4 Adversarial — the refusal half is the adversarial case: a unit cannot be applied twice, so a
//! second winner can never exist, and there is no state in which a receipt exists without its effects.
//! Deterministic and isolated: no database, no network, no process spawned, never writes to PROD.
//!
//! WHAT IT DOES NOT COVER: the row-level half (the unit's single transaction really committing the receipt
//! together with the evidence and the counters) is `tests/tests/forge_completion_receipt_dev.rs`, because an
//! in-memory ledger cannot see a column typo. Cross-process durability is
//! `tests/tests/durable_completion_ledger.rs`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_completion_receipt__001__acquired

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

/// The first application acquires: the unit commits once, and the receipt is final afterwards.
#[test]
fn forge_completion_receipt_001__acquired() {
    let ledger = MemoryLedger::new();
    let id = forge::engine::completion_receipt_id("task-1");

    assert!(
        !ledger.has_final(&id).expect("has_final answers"),
        "nothing is applied until the unit is: absence of a receipt IS the crash window"
    );

    // The full unit on the receipt: applied, final, and never applied a second time.
    assert_eq!(
        apply_completion_unit(&ledger, record("task-1")).expect("unit applies"),
        CompletionApply::Applied,
        "the first application of a fresh receipt owns the unit"
    );
    assert!(
        ledger.has_final(&id).expect("has_final answers"),
        "an applied unit leaves a final receipt: absence of a receipt IS the crash window"
    );
    assert_eq!(
        apply_completion_unit(&ledger, record("task-1")).expect("unit answers"),
        CompletionApply::AlreadyApplied,
        "the same unit, seen again, is not applied twice"
    );
}
