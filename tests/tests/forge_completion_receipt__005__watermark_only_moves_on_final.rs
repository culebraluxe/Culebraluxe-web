//! FORGE.COMPLETION_RECEIPT — watermark only moves on final (TST-FORGE-COMPLETION-RECEIPT-005).
//!
//! CONTRACT. The reconcile watermark — the newest committed receipt time under the
//! `forge.completion:` prefix, which `reconcile_completions` skips already-seen events on
//! (`forge/src/engine/runtime.rs:405-417`) — advances only on a COMMITTED unit, monotonically.
//! A refusal must not move it, and one prefix's commits must not move another's. This is DEV-test step 3b
//! (`tests/tests/forge_completion_receipt_dev.rs`) without a database: the `MemoryLedger::watermark` under
//! test is the production implementation (`forge/src/engine/completion.rs:207-216`).
//!
//! UNDER THE SINGLE-CALL UNIT (FORGE-B1 slice 2) the old negative half — "a bare CLAIM must not move it" —
//! changed shape and got stricter. The claim step is no longer reachable through the ledger, so the
//! in-memory statement of the same property is "a REFUSED unit moves nothing", and the SQL half (a
//! `pending` row is not the newest final: `db/src/forge_engine.rs:1386-1401`) is covered with a database by
//! DEV step 3b, which claims a receipt through the pre-unit door and reads the watermark before and after.
//!
//! Level: L4 Adversarial — the negative halves: no application leaves no watermark, a refusal leaves it
//! where the newest commit put it, and a commit under one prefix leaves every other prefix untouched, so
//! one story's progress cannot blind the resume of another. Deterministic and isolated: no database, no
//! network, never writes to PROD.
//!
//! WHAT IT DOES NOT COVER: the epoch-milliseconds SQL form of the watermark and its `outcome <> 'pending'`
//! exclusion (`db/src/forge_engine.rs:1386-1401`) are covered with a database by the DEV test (step 3b).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_completion_receipt__005__watermark_only_moves_on_final

use forge::engine::{
    apply_completion_unit, CompletionApply, CompletionLedger, CompletionRecord, ForgeGateEvidence,
};

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

/// Only a commit moves the watermark the resume skips events on — and only under its own prefix.
#[test]
fn forge_completion_receipt_005__watermark_only_moves_on_final() {
    let ledger = forge::engine::MemoryLedger::new();

    // Nothing applied, nothing to skip on: a watermark before the first commit would skip a unit that never
    // ran, which is the whole reason this value exists.
    assert_eq!(
        ledger.watermark(PREFIX).expect("watermark answers"),
        None,
        "a prefix with nothing committed has no watermark: the resume must never skip on a value nobody earned"
    );

    // A commit moves it.
    assert_eq!(
        apply_completion_unit(&ledger, record("task-1")).expect("unit applies"),
        CompletionApply::Applied
    );
    let first = ledger
        .watermark(PREFIX)
        .expect("watermark answers")
        .expect("a committed receipt advances the watermark");
    assert!(first > 0, "a ledger tick, never zero: {first}");

    // Monotonically: the newest final wins, an older commit cannot drag it back.
    assert_eq!(
        apply_completion_unit(&ledger, record("task-2")).expect("unit applies"),
        CompletionApply::Applied
    );
    let second = ledger
        .watermark(PREFIX)
        .expect("watermark answers")
        .expect("a second commit advances the watermark");
    assert!(
        second > first,
        "the watermark is the newest final: {second} > {first}"
    );

    // A REFUSAL IS NOT A COMMIT. Re-applying the first unit writes nothing, so the resume point must stay
    // where the newest commit left it — a refusal that moved the watermark would skip events whose units
    // never ran.
    assert_eq!(
        apply_completion_unit(&ledger, record("task-1")).expect("unit answers"),
        CompletionApply::AlreadyApplied,
        "the first unit is settled, so this call commits nothing"
    );
    assert_eq!(
        ledger.watermark(PREFIX).expect("watermark answers"),
        Some(second),
        "a refused unit must not advance the watermark: the resume would skip its own event"
    );

    // And one story's commits must not advance another story's resume point.
    assert_eq!(
        ledger
            .watermark("forge.completion:other-story:")
            .expect("watermark answers"),
        None,
        "one prefix's finals are not another prefix's progress"
    );
}
