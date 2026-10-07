//! FORGE.COMPLETION_RECEIPT — crash after repair count before finalize
//! (TST-FORGE-COMPLETION-RECEIPT-008).
//!
//! CONTRACT. Counter spent, receipt never finalized: when a process increments the repair budget and
//! dies before finalizing, the resume must still converge the unit to one final receipt and then rest.
//! The fault-injecting ledger below models the receipt row's three states exactly as production SQL
//! states them (`db/src/forge_engine.rs:1098-1157`) with an injectable clock for the 15-minute stale
//! window; evidence merge and counters delegate to the production `MemoryLedger`, so only the time
//! rule is modeled, never the unit.
//!
//! THE COUNT THIS PINS, read before "fixing" it. The resume reclaims the stale claim and re-runs the
//! whole unit — merge, increment, finalize — so the repair counter reads 2, not 1: production counts
//! *applications*, and what the receipt guarantees is termination (a third application applies nothing),
//! not single counting. A future change that dedupes the increment must update this pin deliberately;
//! a test that asserted `<= 2` could not tell "resume re-applied" from "resume skipped and the unit is
//! lost", which is the liveness failure this file exists to forbid.
//!
//! Level: L4 Adversarial — injected crash (finalize fails once, after merge and increment) plus
//! injected time. Deterministic and isolated: no database, no network, never PROD.
//!
//! WHAT IT DOES NOT COVER: the SQL form of the stale window is TST-FORGE-COMPLETION-RECEIPT-004; the
//! canonical story-row counters (`increment_forge_repair_attempts` refusing a missing story) are
//! `tests/tests/forge_completion_receipt_dev.rs` step 7.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_completion_receipt__008__crash_after_repair_count_before_finalize

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
            // The crash: the repair count below already happened.
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
        task_id: "task-8".into(),
        process_instance_id: "11111111-1111-1111-1111-111111111111".into(),
        story_id: "story-1".into(),
        // Both repair Smiths spend the one repair budget (`forge/src/engine/completion.rs:148-151`).
        node_id: Some("repair_smith".into()),
        evidence: ForgeGateEvidence {
            candidate_sha: Some("sha".into()),
            ..Default::default()
        },
    }
}

/// Count spent, finalize crashed: the resume converges to one final receipt, then rests.
#[test]
fn forge_completion_receipt_008__crash_after_repair_count_before_finalize() {
    let ledger = TickLedger::new(900);
    let id = forge::engine::completion_receipt_id("task-8");

    // The crash: the unit errors after merge and increment, before the finalize.
    let error = apply_completion_unit(&ledger, record()).unwrap_err();
    assert!(
        error.to_string().contains("died before finalize"),
        "the crash surfaces, it is not swallowed: {error}"
    );
    assert_eq!(
        ledger.inner.repairs("story-1"),
        1,
        "the repair budget was spent before the crash: this is the 'after count' half"
    );
    assert_eq!(ledger.merges(), 1);
    assert!(
        !ledger.has_final(&id).expect("has_final answers"),
        "a crashed attempt is not final: reading it as done would lose the unit it guards"
    );

    // The resume, after the dead process's claim goes stale: re-applies, converges, rests.
    ledger.advance(901);
    assert!(
        apply_completion_unit(&ledger, record()).expect("resume applies"),
        "the resume reclaims the stale claim and applies the unit"
    );
    assert_eq!(
        ledger.inner.repairs("story-1"),
        2,
        "reclaim re-runs the unit, and the unit counts applications: the receipt bounds the unit \
         to termination, not to single counting"
    );
    assert_eq!(
        ledger.merges(),
        2,
        "the re-application re-merges; only a re-application converges a never-finalized unit"
    );
    assert!(
        ledger.has_final(&id).expect("has_final answers"),
        "the resume converges the unit to final"
    );
    assert!(
        !apply_completion_unit(&ledger, record()).expect("unit answers"),
        "a third application applies nothing: converged means rested"
    );
    assert_eq!(
        ledger.inner.repairs("story-1"),
        2,
        "the rest spends no further budget"
    );
}
