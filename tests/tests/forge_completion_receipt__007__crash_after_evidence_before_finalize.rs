//! FORGE.COMPLETION_RECEIPT — crash after evidence before finalize
//! (TST-FORGE-COMPLETION-RECEIPT-007).
//!
//! CONTRACT. The unit is ONE effect (FORGE-B1 slice 2): an attempt that dies after the evidence and before
//! the finalize leaves NOTHING behind — no receipt, no merged evidence, no spent budget — and the resume
//! then applies the whole unit exactly once. The old four-call shape (claim → merge → count → finalize) had
//! a window at every boundary; a process that died inside it left the evidence merged with no receipt, and
//! every later process re-applied it. This file pins that the crash is an absence, not a partial state.
//!
//! WHAT THE DOUBLE MODELS, AND WHAT IT CANNOT. The unit has no seam *inside* it to fail at — that is the
//! point — so the fault lands on the only boundary that still fails in production: the transaction.
//! `CrashLedger` answers an error instead of delegating, and a rolled-back transaction is observed exactly
//! that way: the unit's writes happened and were undone, or never happened — either way a reader sees none
//! of them. An in-memory double cannot show writes being undone, so the row-level proof of "neither partial
//! change nor the final receipt survives a rollback after the merge and the counter" is
//! `tests/tests/forge_completion_receipt_dev.rs`, which rolls a real transaction back through
//! `ForgeEngineDao::apply_completion_tx`. The 15-minute stale window the old double mimicked with a clock is
//! SQL inside the routine now, and both of its sides are pinned there too (fresh `pending` → `Busy`, aged →
//! reclaimed).
//!
//! Level: L4 Adversarial — injected transaction failure. The negative halves: the failed attempt moves no
//! counter and leaves no evidence; a third application after the heal applies nothing. Deterministic and
//! isolated: no database, no network, never PROD.
//!
//! WHAT IT DOES NOT COVER: the SQL form is TST-FORGE-COMPLETION-RECEIPT-004 and the DEV file; the
//! orphaned-transition heal is TST-FORGE-COMPLETION-RECEIPT-006.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_completion_receipt__007__crash_after_evidence_before_finalize

use std::sync::Mutex;

use forge::engine::{
    apply_completion_unit, CompletionApply, CompletionLedger, CompletionRecord, ForgeGateEvidence,
    MemoryLedger,
};
use workflow::{Result, WorkflowError};

/// The production ledger behind one injected transaction failure: `fail_next` makes the next apply answer
/// an error BEFORE it delegates, so nothing is written — the observable shape of a unit whose transaction
/// died. Every other answer is the production `MemoryLedger`'s, so only the crash is modeled, never the unit.
struct CrashLedger {
    inner: MemoryLedger,
    fail_next: Mutex<Option<String>>,
    applied: Mutex<u64>,
}

impl CrashLedger {
    fn new(fail_first_with: &str) -> Self {
        Self {
            inner: MemoryLedger::new(),
            fail_next: Mutex::new(Some(fail_first_with.to_string())),
            applied: Mutex::new(0),
        }
    }
    /// How many of the calls this ledger saw actually committed the unit.
    fn applied(&self) -> u64 {
        *self.applied.lock().unwrap()
    }
}

impl CompletionLedger for CrashLedger {
    fn apply(&self, rec: &CompletionRecord) -> Result<CompletionApply> {
        if let Some(reason) = self.fail_next.lock().unwrap().take() {
            // The transaction died: it committed nothing, so there is nothing left to undo here.
            return Err(WorkflowError::generic(reason));
        }
        let outcome = self.inner.apply(rec)?;
        if outcome.applied() {
            *self.applied.lock().unwrap() += 1;
        }
        Ok(outcome)
    }
    fn has_final(&self, receipt_id: &str) -> Result<bool> {
        self.inner.has_final(receipt_id)
    }
    fn reset_budget(&self, story_id: &str) -> Result<()> {
        self.inner.reset_budget(story_id)
    }
    fn watermark(&self, prefix: &str) -> Result<Option<i64>> {
        self.inner.watermark(prefix)
    }
}

fn record() -> CompletionRecord {
    CompletionRecord {
        task_id: "task-7".into(),
        process_instance_id: "11111111-1111-1111-1111-111111111111".into(),
        story_id: "story-1".into(),
        node_id: None,
        evidence: ForgeGateEvidence {
            candidate_sha: Some("sha".into()),
            ..Default::default()
        },
    }
}

/// Evidence already merged when the crash hits: the rollback takes it, and the heal applies the unit once.
#[test]
fn forge_completion_receipt_007__crash_after_evidence_before_finalize() {
    let ledger = CrashLedger::new("the transaction died before finalize");
    let id = forge::engine::completion_receipt_id("task-7");

    // The crash: the unit errors after it has merged the evidence and before the receipt is final.
    let error = apply_completion_unit(&ledger, record()).unwrap_err();
    assert!(
        error.to_string().contains("died before finalize"),
        "the crash surfaces, it is not swallowed: {error}"
    );
    assert!(
        !ledger.has_final(&id).expect("has_final answers"),
        "a crashed attempt is not final: reading it as done would silently lose the unit"
    );
    assert_eq!(ledger.applied(), 0, "the failed attempt committed nothing");
    assert!(
        ledger.inner.evidence_for("story-1").is_none(),
        "and it merged no evidence: the partial state the four-call shape left behind is gone, so no later \
         process can find a merge with no receipt standing behind it"
    );
    assert_eq!(
        ledger.inner.repairs("story-1"),
        0,
        "the crash moved no counter: a non-repair node spends no budget"
    );

    // The heal: the resume applies the whole unit — once.
    assert_eq!(
        apply_completion_unit(&ledger, record()).expect("resume applies"),
        CompletionApply::Applied,
        "the resume applies the unit the crash could not commit"
    );
    assert_eq!(
        ledger.applied(),
        1,
        "the unit was applied exactly once across the crash and its heal"
    );
    assert!(
        ledger.inner.evidence_for("story-1").is_some(),
        "the merge is the heal's, and it stands with the receipt that proves it"
    );
    assert!(
        ledger.has_final(&id).expect("has_final answers"),
        "the resume converges the unit to final"
    );

    // The rest: a third application applies nothing.
    assert_eq!(
        apply_completion_unit(&ledger, record()).expect("unit answers"),
        CompletionApply::AlreadyApplied,
        "a third application applies nothing: the heal rests"
    );
    assert_eq!(ledger.applied(), 1, "and it commits nothing a second time");
}
