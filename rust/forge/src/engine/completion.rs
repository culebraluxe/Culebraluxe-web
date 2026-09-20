//! Completion unit — engine transition already won (ENG-13).
//! Evidence merge + repair/replan increment are one claim-first receipt.
//! Receipt id: `forge.completion:{taskId}`. Absence of a receipt IS the crash window.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

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
    fn claim(&self, receipt_id: &str) -> bool;
    fn finalize(&self, receipt_id: &str);
    fn has_final(&self, receipt_id: &str) -> bool;
    fn merge_evidence(&self, rec: &CompletionRecord);
    fn increment_repair(&self, story_id: &str);
    fn increment_replan(&self, story_id: &str);
    /// Newest finalized receipt time for `forge.completion:` — reconcile watermark.
    fn watermark(&self, prefix: &str) -> Option<i64> {
        let _ = prefix;
        None
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
    fn claim(&self, receipt_id: &str) -> bool {
        self.claimed.lock().unwrap().insert(receipt_id.to_string())
    }
    fn finalize(&self, receipt_id: &str) {
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
    }
    fn has_final(&self, receipt_id: &str) -> bool {
        self.finalized.lock().unwrap().contains(receipt_id)
    }
    fn watermark(&self, prefix: &str) -> Option<i64> {
        self.finalized_at
            .lock()
            .unwrap()
            .iter()
            .filter(|(id, _)| id.starts_with(prefix))
            .map(|(_, t)| *t)
            .max()
    }
    fn merge_evidence(&self, rec: &CompletionRecord) {
        let mut map = self.evidence.lock().unwrap();
        let next = match map.get(&rec.story_id) {
            Some(prev) => rec.evidence.merge_over(prev),
            None => rec.evidence.clone(),
        };
        map.insert(rec.story_id.clone(), next);
    }
    fn increment_repair(&self, story_id: &str) {
        *self
            .repairs
            .lock()
            .unwrap()
            .entry(story_id.to_string())
            .or_insert(0) += 1;
    }
    fn increment_replan(&self, story_id: &str) {
        *self
            .replans
            .lock()
            .unwrap()
            .entry(story_id.to_string())
            .or_insert(0) += 1;
    }
}

/// Apply the post-transition unit exactly once. Loser/re-run claims nothing.
pub fn apply_completion_unit(ledger: &dyn CompletionLedger, rec: CompletionRecord) -> bool {
    let id = completion_receipt_id(&rec.task_id);
    if !ledger.claim(&id) {
        return false;
    }
    ledger.merge_evidence(&rec);
    match rec.node_id.as_deref() {
        Some("repair_smith") => ledger.increment_repair(&rec.story_id),
        Some("repair_architect") => ledger.increment_replan(&rec.story_id),
        _ => {}
    }
    ledger.finalize(&id);
    true
}
