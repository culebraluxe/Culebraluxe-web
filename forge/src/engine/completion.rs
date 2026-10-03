//! Completion unit — engine transition already won (ENG-13).
//! Evidence merge + repair/replan increment are one claim-first receipt.
//! Receipt id: `forge.completion:{taskId}`. Absence of a receipt IS the crash window.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use workflow::Result;

use crate::engine::facts::ForgeGateEvidence;
use crate::engine::runtime::completion_receipt_id;

#[derive(Debug, Clone, Default)]
pub struct CompletionRecord {
    pub task_id: String,
    pub process_instance_id: String,
    pub story_id: String,
    pub node_id: Option<String>,
    pub evidence: ForgeGateEvidence,
}

pub trait CompletionLedger: Send + Sync {
    /// `true` = this caller owns the unit and must apply it. `false` = it is already final, or another
    /// process holds the claim. A failure to *ask* is an `Err`, never `false`: a lost database must not
    /// read as "someone else applied it" and quietly drop the evidence merge and the counters.
    fn claim(&self, receipt_id: &str) -> Result<bool>;
    fn finalize(&self, receipt_id: &str) -> Result<()>;
    fn has_final(&self, receipt_id: &str) -> Result<bool>;
    fn merge_evidence(&self, rec: &CompletionRecord) -> Result<()>;
    fn increment_repair(&self, story_id: &str) -> Result<()>;
    fn increment_replan(&self, story_id: &str) -> Result<()>;
    /// Return the story's repair and replan budget to full: a fresh instance is a fresh attempt (a board rerun),
    /// and the attempts an earlier instance spent are not this one's. The default (a ledger with no budget of its
    /// own) does nothing.
    fn reset_budget(&self, _story_id: &str) -> Result<()> {
        Ok(())
    }
    /// Newest finalized receipt time for `forge.completion:` — the reconcile watermark, in the same
    /// unit as `ProcessEvent::created_at` (epoch milliseconds).
    fn watermark(&self, prefix: &str) -> Result<Option<i64>> {
        let _ = prefix;
        Ok(None)
    }
}

#[derive(Default)]
pub struct MemoryLedger {
    claimed: Mutex<BTreeSet<String>>,
    finalized: Mutex<BTreeSet<String>>,
    finalized_at: Mutex<BTreeMap<String, i64>>,
    clock: Mutex<i64>,
    evidence: Mutex<BTreeMap<String, ForgeGateEvidence>>,
    repairs: Mutex<BTreeMap<String, u32>>,
    replans: Mutex<BTreeMap<String, u32>>,
}

impl MemoryLedger {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn repairs(&self, story_id: &str) -> u32 {
        *self.repairs.lock().unwrap().get(story_id).unwrap_or(&0)
    }
    pub fn replans(&self, story_id: &str) -> u32 {
        *self.replans.lock().unwrap().get(story_id).unwrap_or(&0)
    }
    pub fn evidence_for(&self, story_id: &str) -> Option<ForgeGateEvidence> {
        self.evidence.lock().unwrap().get(story_id).cloned()
    }
}

impl CompletionLedger for MemoryLedger {
    fn claim(&self, receipt_id: &str) -> Result<bool> {
        Ok(self.claimed.lock().unwrap().insert(receipt_id.to_string()))
    }
    fn finalize(&self, receipt_id: &str) -> Result<()> {
        self.finalized
            .lock()
            .unwrap()
            .insert(receipt_id.to_string());
        let mut clock = self.clock.lock().unwrap();
        *clock += 1;
        self.finalized_at
            .lock()
            .unwrap()
            .insert(receipt_id.to_string(), *clock);
        Ok(())
    }
    fn has_final(&self, receipt_id: &str) -> Result<bool> {
        Ok(self.finalized.lock().unwrap().contains(receipt_id))
    }
    fn watermark(&self, prefix: &str) -> Result<Option<i64>> {
        Ok(self
            .finalized_at
            .lock()
            .unwrap()
            .iter()
            .filter(|(id, _)| id.starts_with(prefix))
            .map(|(_, t)| *t)
            .max())
    }
    fn merge_evidence(&self, rec: &CompletionRecord) -> Result<()> {
        let mut map = self.evidence.lock().unwrap();
        let next = match map.get(&rec.story_id) {
            Some(prev) => rec.evidence.merge_over(prev),
            None => rec.evidence.clone(),
        };
        map.insert(rec.story_id.clone(), next);
        Ok(())
    }
    fn reset_budget(&self, story_id: &str) -> Result<()> {
        self.repairs.lock().unwrap().remove(story_id);
        self.replans.lock().unwrap().remove(story_id);
        Ok(())
    }
    fn increment_repair(&self, story_id: &str) -> Result<()> {
        *self
            .repairs
            .lock()
            .unwrap()
            .entry(story_id.to_string())
            .or_insert(0) += 1;
        Ok(())
    }
    fn increment_replan(&self, story_id: &str) -> Result<()> {
        *self
            .replans
            .lock()
            .unwrap()
            .entry(story_id.to_string())
            .or_insert(0) += 1;
        Ok(())
    }
}

/// Apply the post-transition unit exactly once. Loser/re-run claims nothing.
///
/// The claim is taken BEFORE the writes and finalized AFTER them, so the durable receipt is the proof of
/// the unit and its absence is the crash window the resume reconciles (`legacy/workflow_app/tests/
/// interrupted-sequences.test.ts`: the transition is durable, the evidence is not written, no receipt).
pub fn apply_completion_unit(ledger: &dyn CompletionLedger, rec: CompletionRecord) -> Result<bool> {
    let id = completion_receipt_id(&rec.task_id);
    if !ledger.claim(&id)? {
        return Ok(false);
    }
    ledger.merge_evidence(&rec)?;
    match rec.node_id.as_deref() {
        // Both repair Smiths spend the one repair budget. `fast_repair_smith` was not counted, so the FAST lane's
        // budget could never be spent and a failing candidate looped Smith ↔ QA until the money ran out.
        Some("repair_smith") | Some("fast_repair_smith") => {
            ledger.increment_repair(&rec.story_id)?
        }
        Some("repair_architect") => ledger.increment_replan(&rec.story_id)?,
        _ => {}
    }
    ledger.finalize(&id)?;
    Ok(true)
}
