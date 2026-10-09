//! Completion unit — engine transition already won (ENG-13).
//!
//! Receipt + evidence merge + repair/replan spend are ONE atomic write (FORGE-B1 slice 2).
//! Receipt id: `forge.completion:{taskId}`. The receipt IS the unit's proof: it exists exactly when the
//! unit's effects are committed, so absence of a receipt is the crash window and a receipt is the answer
//! "already applied".
//!
//! WHY ONE WRITE. The unit used to be applied in four calls (claim → merge → count → finalize) and each
//! boundary was a window: a process that died after the merge and the count but before the finalize left
//! the evidence merged and the budget spent with no receipt, and the next process to look — a resume, a
//! fresh dispatch — re-applied the whole unit and spent the budget AGAIN. The repair budget could be
//! exhausted by crashes alone. Collapsing the unit into one committed effect removes the window rather
//! than compensating for it.

use std::collections::BTreeMap;
use std::sync::Mutex;

use workflow::{Result, WorkflowError};

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

impl CompletionRecord {
    /// The budget this node's completion spends, or `None` for a node that spends none.
    ///
    /// Both repair Smiths spend the one repair budget. `fast_repair_smith` was not counted, so the FAST
    /// lane's budget could never be spent and a failing candidate looped Smith ↔ QA until the money ran out.
    pub fn spend(&self) -> Option<CompletionSpend> {
        match self.node_id.as_deref() {
            Some("repair_smith") | Some("fast_repair_smith") => Some(CompletionSpend::Repair),
            Some("repair_architect") => Some(CompletionSpend::Replan),
            _ => None,
        }
    }

    /// The identity of the unit's receipt key: the one story this unit's effects belong to.
    ///
    /// The key itself is the task (`completion_receipt_id`), and a task id is the engine's, not the
    /// caller's. This pins WHICH story the unit behind that key is, so a receipt left by another story — a
    /// reused or leaked task id — is refused as a conflict instead of being answered "already applied",
    /// which would drop this unit's evidence and budget on the floor without a trace.
    pub fn fingerprint(&self) -> String {
        format!("story:{}", self.story_id)
    }
}

/// Which budget a completion spends. The mapping from a node to a budget is
/// [`CompletionRecord::spend`]; this names the budget that was chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionSpend {
    Repair,
    Replan,
}

/// What applying a unit did — the two answers a caller may act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionApply {
    /// This call wrote the unit: receipt, evidence merge and budget spend are committed together.
    Applied,
    /// A committed receipt already proves this unit. Nothing was written, and nothing was replayed.
    AlreadyApplied,
}

impl CompletionApply {
    /// `true` only for [`CompletionApply::Applied`] — the one answer a resume may count as work done.
    pub fn applied(self) -> bool {
        matches!(self, CompletionApply::Applied)
    }
}

pub trait CompletionLedger: Send + Sync {
    /// Apply the unit — receipt, evidence merge and budget spend — as ONE committed effect.
    ///
    /// `Applied` means this call committed it; `AlreadyApplied` means a committed receipt already proves
    /// the unit and nothing was replayed. Everything else — a conflicting unit, another process mid-unit,
    /// a database that cannot answer — is an `Err`: a lost connection must never read as "someone else
    /// applied it" and quietly drop the evidence merge and the counters.
    fn apply(&self, rec: &CompletionRecord) -> Result<CompletionApply>;
    fn has_final(&self, receipt_id: &str) -> Result<bool>;
    /// Read one bounded page of accepted completions that still lack final effects. Durable ledgers
    /// implement this from their authoritative event/receipt tables; `None` asks the runtime to use
    /// the workflow store's explicit instance/event pagination (the in-memory test ledger).
    fn unfinished(
        &self,
        _story_id: Option<&str>,
        _limit: usize,
    ) -> Result<Option<Vec<CompletionRecord>>> {
        Ok(None)
    }
    /// Return the story's repair and replan budget to full: a fresh instance is a fresh attempt (a board rerun),
    /// and the attempts an earlier instance spent are not this one's. The default (a ledger with no budget of its
    /// own) does nothing.
    fn reset_budget(&self, _story_id: &str) -> Result<()> {
        Ok(())
    }
    /// Newest finalized receipt time for `forge.completion:` — the reconcile watermark, in the same
    /// unit as `ProcessEvent::created_at` (epoch milliseconds).
    fn watermark(&self, _prefix: &str) -> Result<Option<i64>> {
        Ok(None)
    }
}

/// The in-memory ledger: the same unit in one lock and one map, so a reader can never see a receipt without
/// its effects.
///
/// It answers the durable routine's questions the same way on purpose, conflict included — a second
/// implementation that called an unfinished or foreign unit "applied" would be a second adjudicator of the
/// same fact.
#[derive(Default)]
pub struct MemoryLedger {
    state: Mutex<MemoryState>,
}

#[derive(Default)]
struct MemoryState {
    /// Receipt id -> the fingerprint of the unit it proves. A receipt exists ONLY with committed effects,
    /// and it is written last for that reason.
    receipts: BTreeMap<String, String>,
    /// Receipt id -> the clock stamp at which the unit committed (the reconcile watermark).
    committed_at: BTreeMap<String, i64>,
    clock: i64,
    evidence: BTreeMap<String, ForgeGateEvidence>,
    repairs: BTreeMap<String, u32>,
    replans: BTreeMap<String, u32>,
}

impl MemoryLedger {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn repairs(&self, story_id: &str) -> u32 {
        *self
            .state
            .lock()
            .unwrap()
            .repairs
            .get(story_id)
            .unwrap_or(&0)
    }
    pub fn replans(&self, story_id: &str) -> u32 {
        *self
            .state
            .lock()
            .unwrap()
            .replans
            .get(story_id)
            .unwrap_or(&0)
    }
    pub fn evidence_for(&self, story_id: &str) -> Option<ForgeGateEvidence> {
        self.state.lock().unwrap().evidence.get(story_id).cloned()
    }
}

impl MemoryState {
    /// The entire unit, under the caller's lock: evidence, spend, then the receipt — so no observer can
    /// read a spent budget or a merged evidence row that no receipt stands behind.
    fn apply(&mut self, rec: &CompletionRecord) -> Result<CompletionApply> {
        let id = completion_receipt_id(&rec.task_id);
        let fingerprint = rec.fingerprint();
        if let Some(stored) = self.receipts.get(&id) {
            if *stored == fingerprint {
                return Ok(CompletionApply::AlreadyApplied);
            }
            return Err(WorkflowError::generic(format!(
                "completion receipt {id} belongs to {stored}, not {fingerprint}: refusing to replay a settled unit"
            )));
        }
        match self.evidence.get(&rec.story_id) {
            Some(prev) => {
                let next = rec.evidence.merge_over(prev);
                self.evidence.insert(rec.story_id.clone(), next);
            }
            None => {
                self.evidence
                    .insert(rec.story_id.clone(), rec.evidence.clone());
            }
        }
        match rec.spend() {
            Some(CompletionSpend::Repair) => {
                *self.repairs.entry(rec.story_id.clone()).or_insert(0) += 1
            }
            Some(CompletionSpend::Replan) => {
                *self.replans.entry(rec.story_id.clone()).or_insert(0) += 1
            }
            None => {}
        }
        self.receipts.insert(id.clone(), fingerprint);
        self.clock += 1;
        self.committed_at.insert(id, self.clock);
        Ok(CompletionApply::Applied)
    }
}

impl CompletionLedger for MemoryLedger {
    fn apply(&self, rec: &CompletionRecord) -> Result<CompletionApply> {
        self.state.lock().unwrap().apply(rec)
    }

    fn has_final(&self, receipt_id: &str) -> Result<bool> {
        Ok(self.state.lock().unwrap().receipts.contains_key(receipt_id))
    }

    fn reset_budget(&self, story_id: &str) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.repairs.remove(story_id);
        state.replans.remove(story_id);
        Ok(())
    }

    fn watermark(&self, prefix: &str) -> Result<Option<i64>> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .committed_at
            .iter()
            .filter(|(id, _)| id.starts_with(prefix))
            .map(|(_, t)| *t)
            .max())
    }
}

/// Apply the post-transition unit, through whichever ledger the runtime holds.
///
/// One call, one effect. The whole unit is handed to the ledger because the receipt and its effects must
/// commit together: the old four-call sequence (claim → merge → count → finalize) could die between its
/// steps and leave the evidence merged and the budget spent with no receipt standing behind them — which is
/// exactly what made a resume spend the repair budget a second time
/// (`legacy/workflow_app/tests/interrupted-sequences.test.ts`: the transition is durable, the evidence is
/// incomplete, no receipt). The durable ledger commits the lot in one transaction; the in-memory one holds
/// one lock across it. Either way a receipt that exists is a unit that happened, and its absence is the
/// crash window the resume reconciles.
pub fn apply_completion_unit(
    ledger: &dyn CompletionLedger,
    rec: CompletionRecord,
) -> Result<CompletionApply> {
    ledger.apply(&rec)
}

#[cfg(test)]
mod spend_tests {
    use super::*;

    fn unit_at(node: Option<&str>) -> CompletionRecord {
        CompletionRecord {
            task_id: "t-1".into(),
            process_instance_id: "p-1".into(),
            story_id: "ENG-1".into(),
            node_id: node.map(str::to_string),
            evidence: ForgeGateEvidence::default(),
        }
    }

    /// Work order §6: both repair Smiths spend the repair budget, `repair_architect` spends the replan one.
    /// The FAST lane is the case that was missed — `fast_repair_smith` was not counted, so its budget could
    /// never be exhausted and a failing candidate looped Smith ↔ QA.
    #[test]
    fn both_repair_smiths_spend_the_repair_budget() {
        assert_eq!(
            unit_at(Some("repair_smith")).spend(),
            Some(CompletionSpend::Repair)
        );
        assert_eq!(
            unit_at(Some("fast_repair_smith")).spend(),
            Some(CompletionSpend::Repair)
        );
        assert_eq!(
            unit_at(Some("repair_architect")).spend(),
            Some(CompletionSpend::Replan)
        );
    }

    /// Every other node spends nothing, and a unit with no node at all spends nothing: an unspent node must
    /// never move a counter (the counter is the budget guard, not a progress log).
    #[test]
    fn only_the_repair_and_replan_nodes_spend() {
        for node in [
            "smith",
            "fast_smith",
            "architect",
            "qa",
            "lead_post",
            "dev_ops",
        ] {
            assert_eq!(unit_at(Some(node)).spend(), None, "{node} spends no budget");
        }
        assert_eq!(
            unit_at(None).spend(),
            None,
            "a node-less unit spends nothing"
        );
    }
}
