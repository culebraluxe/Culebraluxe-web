//! FORGE.COMPLETION_RECEIPT — watermark only moves on final (TST-FORGE-COMPLETION-RECEIPT-005).
//!
//! CONTRACT. The reconcile watermark — the newest finalized receipt time under the
//! `forge.completion:` prefix, which `reconcile_completions` skips already-seen events on
//! (`forge/src/engine/runtime.rs:405-417`) — advances only on finalize. A bare claim (a unit taken but
//! not applied) must not move it, or the resume would skip events whose units never ran. This is
//! DEV-test steps 3 and 4 (`tests/tests/forge_completion_receipt_dev.rs`) without a database: the
//! `MemoryLedger::watermark` under test is the production implementation
//! (`forge/src/engine/completion.rs:92-101`).
//!
//! Level: L4 Adversarial — the negative halves: a pending receipt leaves no watermark, and a finalize
//! under one prefix leaves every other prefix untouched, so one story's progress cannot blind the resume
//! of another. Deterministic and isolated: no database, no network, never writes to PROD.
//!
//! WHAT IT DOES NOT COVER: the epoch-milliseconds SQL form of the watermark
//! (`db/src/forge_engine.rs:1182-1193`) is covered with a database by the DEV test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_completion_receipt__005__watermark_only_moves_on_final

use forge::engine::{apply_completion_unit, CompletionLedger, CompletionRecord, ForgeGateEvidence};

const PREFIX: &str = "forge.completion:";

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

/// A claim is not an application: only the finalize moves the watermark the resume skips events on.
#[test]
fn forge_completion_receipt_005__watermark_only_moves_on_final() {
    let ledger = forge::engine::MemoryLedger::new();

    // A pending receipt advances nothing.
    let pending = forge::engine::completion_receipt_id("task-pending");
    assert!(ledger.claim(&pending).expect("claim answers"));
    assert_eq!(
        ledger.watermark(PREFIX).expect("watermark answers"),
        None,
        "a pending receipt must not advance the watermark, or the resume skips a unit that never ran"
    );

    // The finalize moves it, monotonically.
    assert!(apply_completion_unit(&ledger, record("task-1")).expect("unit applies"));
    let first = ledger
        .watermark(PREFIX)
        .expect("watermark answers")
        .expect("a finalized receipt advances the watermark");
    assert!(first > 0, "a ledger tick, never zero: {first}");
    assert!(apply_completion_unit(&ledger, record("task-2")).expect("unit applies"));
    let second = ledger
        .watermark(PREFIX)
        .expect("watermark answers")
        .expect("a second finalize advances the watermark");
    assert!(
        second > first,
        "the watermark is the newest final: {second} > {first}"
    );

    // The pending receipt from the opening is still pending: it never contributed.
    assert!(
        !ledger.has_final(&pending).expect("has_final answers"),
        "the unfinalized claim is still open; the watermark above is finals only"
    );
    assert_eq!(
        ledger
            .watermark("forge.completion:other-story:")
            .expect("watermark answers"),
        None,
        "one story's finals must not advance another story's resume point"
    );
}
