//! FORGE.COMPLETION_RECEIPT — multiple new Forge child processes
//! (TST-FORGE-COMPLETION-RECEIPT-009).
//!
//! CONTRACT. The engine runs as one child process per dispatch, so several newborn processes can look
//! at the same unapplied unit at once: exactly one of them must apply it. Eight threads race the
//! production `apply_completion_unit` over the production `MemoryLedger` — the mutual exclusion under
//! test is the ledger's own, not a test lock — and exactly one call wins. The counting decorator only
//! observes: every claim decision and every write is the production ledger's.
//!
//! Level: L4 Adversarial — thread-race convergence to one legal durable state. The losing calls must
//! answer `Ok(false)` (lost to a real winner), never `Err` (an error is not a loss, and unwrapping
//! keeps a failure loud). Deterministic outcome — the winner varies, the count does not — isolated: no
//! database, no network, never PROD.
//!
//! WHAT IT DOES NOT COVER: cross-process (not cross-thread) exclusion rests on the receipt row, whose
//! SQL form is covered with a database by `tests/tests/forge_completion_receipt_dev.rs` and
//! TST-FORGE-COMPLETION-RECEIPT-004. The single-process resume door is TST-FORGE-COMPLETION-RECEIPT-010.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_completion_receipt__009__multiple_new_forge_child_processes

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use forge::engine::{
    apply_completion_unit, CompletionLedger, CompletionRecord, ForgeGateEvidence, MemoryLedger,
};
use workflow::Result;

/// Observes the production ledger without deciding anything: claims and writes are the inner
/// ledger's; the counters only record what it decided.
struct CountingLedger {
    inner: MemoryLedger,
    wins: AtomicU64,
    merges: AtomicU64,
}

impl CountingLedger {
    fn new() -> Self {
        Self {
            inner: MemoryLedger::new(),
            wins: AtomicU64::new(0),
            merges: AtomicU64::new(0),
        }
    }
}

impl CompletionLedger for CountingLedger {
    fn claim(&self, receipt_id: &str) -> Result<bool> {
        let won = self.inner.claim(receipt_id)?;
        if won {
            self.wins.fetch_add(1, Ordering::SeqCst);
        }
        Ok(won)
    }
    fn finalize(&self, receipt_id: &str) -> Result<()> {
        self.inner.finalize(receipt_id)
    }
    fn has_final(&self, receipt_id: &str) -> Result<bool> {
        self.inner.has_final(receipt_id)
    }
    fn watermark(&self, prefix: &str) -> Result<Option<i64>> {
        self.inner.watermark(prefix)
    }
    fn merge_evidence(&self, rec: &CompletionRecord) -> Result<()> {
        self.merges.fetch_add(1, Ordering::SeqCst);
        self.inner.merge_evidence(rec)
    }
    fn reset_budget(&self, story_id: &str) -> Result<()> {
        self.inner.reset_budget(story_id)
    }
    fn increment_repair(&self, story_id: &str) -> Result<()> {
        self.inner.increment_repair(story_id)
    }
    fn increment_replan(&self, story_id: &str) -> Result<()> {
        self.inner.increment_replan(story_id)
    }
}

fn record() -> CompletionRecord {
    CompletionRecord {
        task_id: "task-9".into(),
        process_instance_id: "11111111-1111-1111-1111-111111111111".into(),
        story_id: "story-1".into(),
        node_id: None,
        evidence: ForgeGateEvidence {
            candidate_sha: Some("sha".into()),
            ..Default::default()
        },
    }
}

/// Eight newborn processes race one unapplied unit: exactly one applies it.
#[test]
fn forge_completion_receipt_009__multiple_new_forge_child_processes() {
    const PROCESSES: u64 = 8;
    let ledger = Arc::new(CountingLedger::new());
    let id = forge::engine::completion_receipt_id("task-9");

    let mut applied = 0u64;
    let mut refused = 0u64;
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..PROCESSES)
            .map(|_| {
                scope.spawn(|| {
                    // An `Err` here is not a loss, so it stays loud: only `Ok(false)` counts as refused.
                    apply_completion_unit(ledger.as_ref(), record()).expect("unit answers")
                })
            })
            .collect();
        for handle in handles {
            if handle.join().expect("process panics fail the test") {
                applied += 1;
            } else {
                refused += 1;
            }
        }
    });

    assert_eq!(
        applied, 1,
        "exactly one newborn process applies the unit: {applied} winners is a fork"
    );
    assert_eq!(
        refused,
        PROCESSES - 1,
        "every loser is refused, none is errored and none slips through"
    );
    assert_eq!(
        ledger.merges.load(Ordering::SeqCst),
        1,
        "the evidence merges exactly once across all processes"
    );
    assert!(
        ledger.has_final(&id).expect("has_final answers"),
        "the race converges to one final receipt"
    );
    assert!(
        ledger.inner.evidence_for("story-1").is_some(),
        "the winner's evidence is the ledger's evidence"
    );
}
