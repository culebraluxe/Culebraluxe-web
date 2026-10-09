//! FORGE.COMPLETION_RECEIPT — finalized (TST-FORGE-COMPLETION-RECEIPT-003).
//!
//! CONTRACT. An applied unit's receipt is terminal: `has_final` answers `true`, and the unit cannot be
//! entered again — re-applying it answers `AlreadyApplied`, writes nothing and counts nothing. This is
//! DEV-test steps 2 and 3 (`tests/tests/forge_completion_receipt_dev.rs`) replayed without a database
//! against the production `MemoryLedger`.
//!
//! UNDER THE SINGLE-CALL UNIT (FORGE-B1 slice 2) the old half of this contract — "a further CLAIM is
//! refused" — became structural, which is a strictly stronger statement of the same property: the claim
//! step is gone from the ledger, so there is no second door into a settled receipt at all. The state that
//! door used to leave behind (a `pending` receipt inside the 15-minute window) is SQL's, and is covered
//! with a database by DEV step 5b and TST-FORGE-COMPLETION-RECEIPT-004.
//!
//! Level: L4 Adversarial — the negative half: an unknown receipt is not final, and an applied unit cannot
//! be re-entered, so finality can be neither assumed nor re-earned. Deterministic and isolated: no
//! database, no network, never writes to PROD.
//!
//! WHAT IT DOES NOT COVER: the outcome payload a committing unit writes (`outcome = 'success'` with its
//! `result_payload`) lives in SQL (`db/src/forge_engine.rs`) and is covered with a database by the DEV
//! test. The watermark an application advances is TST-FORGE-COMPLETION-RECEIPT-005.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_completion_receipt__003__finalized

use forge::engine::{
    apply_completion_unit, CompletionApply, CompletionLedger, CompletionRecord, ForgeGateEvidence,
};

fn record(task_id: &str) -> CompletionRecord {
    CompletionRecord {
        task_id: task_id.into(),
        process_instance_id: "11111111-1111-1111-1111-111111111111".into(),
        story_id: "story-1".into(),
        // A repair node on purpose: "counts nothing" is then a measured claim, not a comment.
        node_id: Some("repair_smith".into()),
        evidence: ForgeGateEvidence {
            candidate_sha: Some("sha".into()),
            ..Default::default()
        },
    }
}

/// Apply once, then the receipt is terminal: known-final, and re-application is refused.
#[test]
fn forge_completion_receipt_003__finalized() {
    let ledger = forge::engine::MemoryLedger::new();
    let id = forge::engine::completion_receipt_id("task-1");

    // Negative first: nothing is final before anything runs.
    assert!(
        !ledger.has_final(&id).expect("has_final answers"),
        "an unknown receipt is not final: finality must be earned, never assumed"
    );

    // The one call IS the acquire and the finalize: a receipt exists only with its committed effects.
    assert_eq!(
        apply_completion_unit(&ledger, record("task-1")).expect("unit applies"),
        CompletionApply::Applied,
        "the first call applies the unit"
    );
    assert!(
        ledger.has_final(&id).expect("has_final answers"),
        "an applied unit leaves a final receipt"
    );
    assert_eq!(
        ledger.repairs("story-1"),
        1,
        "and the unit spent its budget once"
    );

    // Terminal: re-entry is refused, and refusing writes nothing.
    assert_eq!(
        apply_completion_unit(&ledger, record("task-1")).expect("unit answers"),
        CompletionApply::AlreadyApplied,
        "re-applying a finalized unit merges nothing and counts nothing"
    );
    assert_eq!(
        ledger.repairs("story-1"),
        1,
        "the counter did not move again: a refusal is not a second spend"
    );
    assert!(
        ledger.has_final(&id).expect("has_final answers"),
        "and the refusal left the receipt final: it neither re-opened nor re-closed anything"
    );
}
