//! FORGE.COMPLETION_RECEIPT — crash after repair count before finalize
//! (TST-FORGE-COMPLETION-RECEIPT-008).
//!
//! CONTRACT. The unit is ONE effect (FORGE-B1 slice 2): an attempt that dies after it has merged the
//! evidence and incremented the repair counter leaves NOTHING behind — no receipt, no merge, no spent
//! budget — and the resume then applies the whole unit exactly once. The old four-call shape let the crash
//! land between the increment and the finalize, and the next process to look re-ran the unit and spent the
//! budget AGAIN.
//!
//! THE COUNT THIS PINS, read before "fixing" it. The repair counter reads **1**, not 2. Under the four-call
//! shape this file pinned 2 ("production counts applications"): the resume re-ran the whole unit because
//! the crashed attempt's increment had committed with no receipt to prove it. Now the increment is inside
//! the unit's transaction, so a crashed attempt's increment is rolled back with it and only the heal's
//! application commits. A future change that makes this read 2 again has reintroduced the defect this file
//! exists to keep closed; a test that asserted `<= 2` could not tell a re-applied unit from a lost one.
//!
//! Level: L4 Adversarial — injected transaction failure. Deterministic and isolated: no database, no
//! network, never PROD.
//!
//! WHAT THE DOUBLE MODELS is in `...__007__crash_after_evidence_before_finalize.rs`, the same double for
//! the other half of the window. WHAT IT DOES NOT COVER: the SQL form of the stale window and the rollback
//! of a real transaction after the merge and the counter are `tests/tests/forge_completion_receipt_dev.rs`
//! (steps 1 and 5); the canonical story-row columns are its step 3.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_completion_receipt__008__crash_after_repair_count_before_finalize

use std::sync::Mutex;

use forge::engine::{
    apply_completion_unit, CompletionApply, CompletionLedger, CompletionRecord, ForgeGateEvidence,
    MemoryLedger,
};
use workflow::{Result, WorkflowError};

/// The production ledger behind one injected transaction failure: `fail_next` makes the next apply answer
/// an error BEFORE it delegates, so nothing is written — the observable shape of a unit whose transaction
/// died. Every other answer is the production `MemoryLedger`'s, so only the crash is modeled.
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
        task_id: "task-8".into(),
        process_instance_id: "11111111-1111-1111-1111-111111111111".into(),
        story_id: "story-1".into(),
        // Both repair Smiths spend the one repair budget (`forge/src/engine/completion.rs:37-43`).
        node_id: Some("repair_smith".into()),
        evidence: ForgeGateEvidence {
            candidate_sha: Some("sha".into()),
            ..Default::default()
        },
    }
}

/// Count already spent when the crash hits: the rollback takes it back, so the heal spends it once.
#[test]
fn forge_completion_receipt_008__crash_after_repair_count_before_finalize() {
    let ledger = CrashLedger::new("the transaction died before finalize");
    let id = forge::engine::completion_receipt_id("task-8");

    // The crash: the unit errors after the merge and the increment, before the finalize.
    let error = apply_completion_unit(&ledger, record()).unwrap_err();
    assert!(
        error.to_string().contains("died before finalize"),
        "the crash surfaces, it is not swallowed: {error}"
    );
    assert_eq!(
        ledger.inner.repairs("story-1"),
        0,
        "the increment went with the transaction: a crash cannot spend the repair budget"
    );
    assert!(
        ledger.inner.evidence_for("story-1").is_none(),
        "and the merge went with it too"
    );
    assert!(
        !ledger.has_final(&id).expect("has_final answers"),
        "a crashed attempt is not final: reading it as done would lose the unit it guards"
    );

    // The heal: the resume applies the unit — the counter's first committed increment.
    assert_eq!(
        apply_completion_unit(&ledger, record()).expect("resume applies"),
        CompletionApply::Applied,
        "the resume applies the unit the crash could not commit"
    );
    assert_eq!(
        ledger.inner.repairs("story-1"),
        1,
        "the repair budget is spent once, by the application that committed"
    );
    assert!(
        ledger.has_final(&id).expect("has_final answers"),
        "the resume converges the unit to final"
    );

    // The rest: a third application spends nothing further.
    assert_eq!(
        apply_completion_unit(&ledger, record()).expect("unit answers"),
        CompletionApply::AlreadyApplied,
        "a third application applies nothing: converged means rested"
    );
    assert_eq!(
        ledger.inner.repairs("story-1"),
        1,
        "the rest spends no further budget"
    );
    assert_eq!(ledger.applied(), 1, "and commits nothing a second time");
}
