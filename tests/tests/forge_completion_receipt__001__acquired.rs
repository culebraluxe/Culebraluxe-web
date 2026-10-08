//! FORGE.COMPLETION_RECEIPT — acquired (TST-FORGE-COMPLETION-RECEIPT-001).
//!
//! CONTRACT. The first claim on a fresh completion receipt acquires the unit: `claim` answers `true`,
//! the unit applies, and the receipt reads final afterwards. This is DEV-test step 1
//! (`tests/tests/forge_completion_receipt_dev.rs`) replayed without a database, against the ledger the
//! production runtime installs by default (`ForgeRuntime`'s default constructor builds
//! `Arc::new(MemoryLedger::new())`, `forge/src/engine/runtime.rs:123`).
//!
//! Level: L4 Adversarial — the refusal half is the adversarial case: the same receipt cannot be acquired
//! twice, so a second winner can never exist. Deterministic and isolated: no database, no network, no
//! process spawned, never writes to PROD.
//!
//! WHAT IT DOES NOT COVER: the row-level half (the SQL `INSERT ... ON CONFLICT DO NOTHING` really
//! acquiring) is `tests/tests/forge_completion_receipt_dev.rs`, because an in-memory ledger cannot see a
//! column typo. Cross-process durability is `tests/tests/durable_completion_ledger.rs`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_completion_receipt__001__acquired

use forge::engine::{
    apply_completion_unit, CompletionLedger, CompletionRecord, ForgeGateEvidence, MemoryLedger,
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

/// The first claim acquires: the unit applies, and the receipt is final afterwards.
#[test]
fn forge_completion_receipt_001__acquired() {
    let ledger = MemoryLedger::new();
    let id = forge::engine::completion_receipt_id("task-1");

    assert!(
        ledger.claim(&id).expect("claim answers"),
        "a fresh receipt is acquirable: this is the claim-first half of the unit"
    );
    assert!(
        !ledger.claim(&id).expect("claim answers"),
        "the same receipt cannot be acquired twice: exclusivity is what makes the winner single"
    );
    assert!(
        !ledger.has_final(&id).expect("has_final answers"),
        "a bare claim is not a final: claiming without finalizing must not read as applied"
    );

    // The full unit on a second receipt: acquired, applied, final.
    let rec = record("task-2");
    assert!(
        apply_completion_unit(&ledger, rec).expect("unit applies"),
        "the first application of a fresh receipt owns the unit"
    );
    assert!(
        ledger
            .has_final(&forge::engine::completion_receipt_id("task-2"))
            .expect("has_final answers"),
        "an applied unit leaves a final receipt: absence of a receipt IS the crash window"
    );
    assert!(
        !apply_completion_unit(&ledger, record("task-2")).expect("unit answers"),
        "the same unit, seen again, is not applied twice"
    );
}
