//! FORGE.COMPLETION_RECEIPT — finalized (TST-FORGE-COMPLETION-RECEIPT-003).
//!
//! CONTRACT. A finalized receipt is terminal: `has_final` answers `true`, a further claim is refused,
//! and re-applying the unit is a no-op that returns `false`. This is DEV-test step 4
//! (`tests/tests/forge_completion_receipt_dev.rs`: finalize moves the row to `AlreadyFinal` with its
//! outcome) replayed without a database against the production `MemoryLedger`.
//!
//! Level: L4 Adversarial — the negative half: an unknown receipt is not final, and a finalized receipt
//! cannot be re-acquired, so finality can be neither assumed nor re-entered. Deterministic and isolated:
//! no database, no network, never writes to PROD.
//!
//! WHAT IT DOES NOT COVER: the outcome payload the finalize carries (`AlreadyFinal(row)` with
//! `row.outcome`) lives in SQL (`db/src/forge_engine.rs`) and is covered with a database by the DEV
//! test. The watermark a finalize advances is TST-FORGE-COMPLETION-RECEIPT-005.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_completion_receipt__003__finalized

use forge::engine::{apply_completion_unit, CompletionLedger, CompletionRecord, ForgeGateEvidence};

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

/// Finalize, then the receipt is terminal: known-final, unclaimable, and re-application is refused.
#[test]
fn forge_completion_receipt_003__finalized() {
    let ledger = forge::engine::MemoryLedger::new();

    // Negative first: nothing is final before anything runs.
    let id = forge::engine::completion_receipt_id("task-1");
    assert!(
        !ledger.has_final(&id).expect("has_final answers"),
        "an unknown receipt is not final: finality must be earned, never assumed"
    );

    assert!(
        apply_completion_unit(&ledger, record("task-1")).expect("unit applies"),
        "the first application owns the unit"
    );
    assert!(
        ledger.has_final(&id).expect("has_final answers"),
        "an applied unit leaves a final receipt"
    );
    assert!(
        !ledger.claim(&id).expect("claim answers"),
        "a finalized receipt cannot be re-acquired: the unit is closed"
    );
    assert!(
        !apply_completion_unit(&ledger, record("task-1")).expect("unit answers"),
        "re-applying a finalized unit merges nothing and counts nothing"
    );
}
