//! FORGE.COMPLETION_RECEIPT — crash after evidence before finalize
//! (TST-FORGE-COMPLETION-RECEIPT-007).
//!
//! CONTRACT. Evidence written, receipt never finalized: when a process merges the evidence and dies
//! before finalizing, the unit must NOT read as done (no silent loss), and the resume must converge it
//! to final — re-merging (the patch is last-write-wins, so re-application converges) and then resting.
//! The fault-injecting ledger below models the receipt row's three states exactly as production SQL
//! states them (`db/src/forge_engine.rs:1098-1157`: absent → pending, fresh pending → held, stale
//! pending → reclaimed, final → refused) with an injectable clock standing in for the 15-minute stale
//! window; everything else — evidence merge, counters, budgets — delegates to the production
//! `MemoryLedger`, so only the time rule is modeled, never the unit.
//!
//! Level: L4 Adversarial — injected crash (finalize fails once, after the merge) plus injected time.
//! The negative halves: the crashed attempt moves no counter and leaves no final; a third application
//! after the heal applies nothing. Deterministic and isolated: no database, no network, never PROD.
//!
//! WHAT IT DOES NOT COVER: the SQL form of the stale window is covered with a database by
//! TST-FORGE-COMPLETION-RECEIPT-004; the orphaned-transition heal is TST-FORGE-COMPLETION-RECEIPT-006.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_completion_receipt__007__crash_after_evidence_before_finalize

use std::collections::BTreeMap;
use std::sync::Mutex;

use forge::engine::{
    apply_completion_unit, CompletionLedger, CompletionRecord, ForgeGateEvidence, MemoryLedger,
};
use workflow::{Result, WorkflowError};

struct TickEntry {
    is_final: bool,
    touched: u64,
    finalized_at: u64,
}

/// The receipt row with an injectable clock. State transitions mirror
/// `ForgeEngineDao::claim_workflow_receipt`; the evidence and counters are the production ledger's.
struct TickLedger {
    inner: MemoryLedger,
    entries: Mutex<BTreeMap<String, TickEntry>>,
    now: Mutex<u64>,
    stale_after: u64,
    merges: Mutex<u64>,
    fail_finalize_once: Mutex<bool>,
}

impl TickLedger {
    fn new(stale_after: u64) -> Self {
        Self {
            inner: MemoryLedger::new(),
            entries: Mutex::new(BTreeMap::new()),
            now: Mutex::new(0),
            stale_after,
            merges: Mutex::new(0),
            fail_finalize_once: Mutex::new(true),
        }
    }
    /// Stand in for the time that passed while the crashed process was dead.
    fn advance(&self, ticks: u64) {
        *self.now.lock().unwrap() += ticks;
    }
    fn merges(&self) -> u64 {
        *self.merges.lock().unwrap()
    }
}

impl CompletionLedger for TickLedger {
    fn claim(&self, receipt_id: &str) -> Result<bool> {
        let now = *self.now.lock().unwrap();
        let mut entries = self.entries.lock().unwrap();
        match entries.get_mut(receipt_id) {
            None => {
                entries.insert(
                    receipt_id.to_string(),
                    TickEntry {
                        is_final: false,
                        touched: now,
                        finalized_at: 0,
                    },
                );
                Ok(true)
            }
            Some(entry) if entry.is_final => Ok(false),
            Some(entry) if now - entry.touched >= self.stale_after => {
                entry.touched = now;
                Ok(true)
            }
            Some(_) => Ok(false),
        }
    }
    fn finalize(&self, receipt_id: &str) -> Result<()> {
        if *self.fail_finalize_once.lock().unwrap() {
            // The crash: everything before this point (claim, merge, counters) already happened.
            *self.fail_finalize_once.lock().unwrap() = false;
            return Err(WorkflowError::generic("process died before finalize"));
        }
        let now = *self.now.lock().unwrap();
        let mut entries = self.entries.lock().unwrap();
        let entry = entries
            .get_mut(receipt_id)
            .expect("finalize follows a claim the ledger took");
        entry.is_final = true;
        entry.finalized_at = now;
        Ok(())
    }
    fn has_final(&self, receipt_id: &str) -> Result<bool> {
        Ok(self
            .entries
            .lock()
            .unwrap()
            .get(receipt_id)
            .is_some_and(|entry| entry.is_final))
    }
    fn watermark(&self, prefix: &str) -> Result<Option<i64>> {
        Ok(self
            .entries
            .lock()
            .unwrap()
            .iter()
            .filter(|(id, _)| id.starts_with(prefix))
            .filter(|(_, entry)| entry.is_final)
            .map(|(_, entry)| entry.finalized_at as i64)
            .max())
    }
    fn merge_evidence(&self, rec: &CompletionRecord) -> Result<()> {
        *self.merges.lock().unwrap() += 1;
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

/// Evidence merged, finalize crashed: the resume converges the unit to final, then rests.
#[test]
fn forge_completion_receipt_007__crash_after_evidence_before_finalize() {
    let ledger = TickLedger::new(900);

    // The crash: the unit errors after the merge, before the finalize.
    let error = apply_completion_unit(&ledger, record()).unwrap_err();
    assert!(
        error.to_string().contains("died before finalize"),
        "the crash surfaces, it is not swallowed: {error}"
    );
    assert!(
        !ledger
            .has_final(&forge::engine::completion_receipt_id("task-7"))
            .expect("has_final answers"),
        "a crashed attempt is not final: reading it as done would silently lose the unit"
    );
    assert!(
        ledger.inner.evidence_for("story-1").is_some(),
        "the evidence survived the crash: this is the 'after evidence' half of the window"
    );
    assert_eq!(ledger.merges(), 1);
    assert_eq!(
        ledger.inner.repairs("story-1"),
        0,
        "the crash moved no counter: a non-repair node spends no budget"
    );

    // The resume, after the dead process's claim goes stale: re-applies, converges, rests.
    ledger.advance(901);
    assert!(
        apply_completion_unit(&ledger, record()).expect("resume applies"),
        "the resume reclaims the stale claim and applies the unit"
    );
    assert_eq!(
        ledger.merges(),
        2,
        "re-application re-merges; the patch is last-write-wins, so the evidence converges"
    );
    assert!(
        ledger
            .has_final(&forge::engine::completion_receipt_id("task-7"))
            .expect("has_final answers"),
        "the resume converges the unit to final"
    );
    assert!(
        !apply_completion_unit(&ledger, record()).expect("unit answers"),
        "a third application applies nothing: the heal rests"
    );
}
