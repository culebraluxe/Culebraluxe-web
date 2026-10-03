//! The durable completion ledger: exactly-once across processes, and a failure that is not silence.
//!
//! WHY THIS EXISTS. The engine runs as one child process per dispatch, so the "exactly once" the completion
//! unit promises held only inside a single run: the next process began with an empty ledger, every
//! `task.completed` in the instance history looked unapplied, and the unit was re-applied — the evidence
//! merge replayed, `forge_repair_attempts` / `forge_replan_attempts` incremented once per process that ever
//! looked at the story. The contract the rows must hold is stated by `legacy/workflow_app/tests/
//! interrupted-sequences.test.ts`: the transition is durable, the evidence is not yet written, the receipt is
//! absent — and the resume applies the unit exactly once.
//!
//! TWO PROPERTIES, PINNED HERE. (1) The ledger is fallible: a database that cannot answer must stop the
//! caller, because `false` from `claim` means "someone already applied this unit". (2) The unit applies once
//! regardless of which process looks. The row-level half of the same contract — that the SQL really behaves
//! this way — is `tests/tests/forge_completion_receipt_dev.rs`, because a unit test cannot see a
//! column typo.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use forge::engine::*;
use workflow::{json, MemoryStore, Result, WorkflowError};

/// A ledger shared by two runtimes, standing in for the receipt row both processes read.
#[derive(Default)]
struct SharedLedger {
    receipts: Mutex<BTreeMap<String, String>>,
    evidence_merges: Mutex<Vec<String>>,
    repairs: Mutex<BTreeMap<String, u32>>,
    replans: Mutex<BTreeMap<String, u32>>,
}

impl SharedLedger {
    fn state(&self, receipt_id: &str) -> Option<String> {
        self.receipts.lock().unwrap().get(receipt_id).cloned()
    }
    fn merges(&self) -> usize {
        self.evidence_merges.lock().unwrap().len()
    }
    fn repairs(&self, story_id: &str) -> u32 {
        *self.repairs.lock().unwrap().get(story_id).unwrap_or(&0)
    }
    fn replans(&self, story_id: &str) -> u32 {
        *self.replans.lock().unwrap().get(story_id).unwrap_or(&0)
    }
}

impl CompletionLedger for SharedLedger {
    fn claim(&self, receipt_id: &str) -> Result<bool> {
        let mut receipts = self.receipts.lock().unwrap();
        if receipts.contains_key(receipt_id) {
            return Ok(false);
        }
        receipts.insert(receipt_id.to_string(), "pending".into());
        Ok(true)
    }
    fn finalize(&self, receipt_id: &str) -> Result<()> {
        self.receipts
            .lock()
            .unwrap()
            .insert(receipt_id.to_string(), "final".into());
        Ok(())
    }
    fn has_final(&self, receipt_id: &str) -> Result<bool> {
        Ok(self.state(receipt_id).as_deref() == Some("final"))
    }
    fn merge_evidence(&self, rec: &CompletionRecord) -> Result<()> {
        self.evidence_merges
            .lock()
            .unwrap()
            .push(rec.story_id.clone());
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

/// A ledger whose every question fails. It stands for a database that is not there, which is the case the
/// old `bool` trait could not express: it had to answer `false`, and `false` is what "already applied" says.
#[derive(Default)]
struct UnreachableLedger {
    finalized: Mutex<Vec<String>>,
    merged: Mutex<Vec<String>>,
}

impl CompletionLedger for UnreachableLedger {
    fn claim(&self, _receipt_id: &str) -> Result<bool> {
        Err(WorkflowError::generic("database unreachable"))
    }
    fn finalize(&self, receipt_id: &str) -> Result<()> {
        self.finalized.lock().unwrap().push(receipt_id.to_string());
        Ok(())
    }
    fn has_final(&self, _receipt_id: &str) -> Result<bool> {
        Err(WorkflowError::generic("database unreachable"))
    }
    fn merge_evidence(&self, rec: &CompletionRecord) -> Result<()> {
        self.merged.lock().unwrap().push(rec.story_id.clone());
        Ok(())
    }
    fn increment_repair(&self, _story_id: &str) -> Result<()> {
        Err(WorkflowError::generic("database unreachable"))
    }
    fn increment_replan(&self, _story_id: &str) -> Result<()> {
        Err(WorkflowError::generic("database unreachable"))
    }
}

/// A ledger that claims fine and then fails to write the evidence: the unit is claimed and NOT applied,
/// which is the crash window the resume is for. It must not be finalized as if it had worked.
#[derive(Default)]
struct HalfWrittenLedger {
    finalized: Mutex<Vec<String>>,
}

impl CompletionLedger for HalfWrittenLedger {
    fn claim(&self, _receipt_id: &str) -> Result<bool> {
        Ok(true)
    }
    fn finalize(&self, receipt_id: &str) -> Result<()> {
        self.finalized.lock().unwrap().push(receipt_id.to_string());
        Ok(())
    }
    fn has_final(&self, _receipt_id: &str) -> Result<bool> {
        Ok(false)
    }
    fn merge_evidence(&self, _rec: &CompletionRecord) -> Result<()> {
        Err(WorkflowError::generic("evidence write failed"))
    }
    fn increment_repair(&self, _story_id: &str) -> Result<()> {
        Ok(())
    }
    fn increment_replan(&self, _story_id: &str) -> Result<()> {
        Ok(())
    }
}

fn record(task_id: &str, node_id: Option<&str>) -> CompletionRecord {
    CompletionRecord {
        task_id: task_id.into(),
        process_instance_id: "11111111-1111-1111-1111-111111111111".into(),
        story_id: "story-1".into(),
        node_id: node_id.map(|n| n.to_string()),
        evidence: ForgeGateEvidence {
            candidate_sha: Some("sha".into()),
            ..Default::default()
        },
    }
}

fn compact_runtime(
    store: MemoryStore,
    ledger: Arc<dyn CompletionLedger>,
) -> ForgeRuntime<MemoryStore> {
    let writer = Arc::new(RecordingWriter::default());
    ForgeRuntime::from_store(
        store,
        writer,
        None,
        None,
        ledger,
        forge_sdlc_compact_definition(),
    )
    .expect("compact fixture")
}

fn feature_story() -> ForgeGateEvidence {
    ForgeGateEvidence {
        work_type: Some("FEATURE".into()),
        ..Default::default()
    }
}

/// The defect this rail removes, stated as a test: a second process must not re-apply the unit.
///
/// The runtimes share a `MemoryStore` (its clones share state, so this is one durable engine) and a ledger,
/// and the first is dropped the way a crashed process is. Under a process-local ledger the third look
/// applies the unit again — measured before this change — which is why production now names the receipt row.
#[test]
fn a_ledger_shared_by_two_processes_applies_the_orphaned_unit_once() {
    let store = MemoryStore::new();
    let ledger = Arc::new(SharedLedger::default());

    // Process 1 — the transition is won, then the process dies before the unit runs.
    let task_id = {
        let writer = Arc::new(RecordingWriter::default());
        let mut rt = ForgeRuntime::from_store(
            store.clone(),
            writer,
            None,
            None,
            ledger.clone(),
            forge_sdlc_compact_definition(),
        )
        .expect("compact fixture");
        rt.start_story("story-1", "FEATURE", feature_story())
            .expect("start");
        let task = rt.list_role_tasks("story-1").expect("tasks")[0].clone();
        rt.claim_role_task(&task.task_id, "lead").expect("claim");
        rt.engine()
            .complete_task(workflow::CompleteTaskParams {
                task_id: task.task_id.clone(),
                user_id: "lead".into(),
                form_data: json!({ "candidateSha": "sha" }),
                transition_name: Some("smith".into()),
            })
            .expect("the transition is durable even though the unit is not");
        assert_eq!(
            ledger.state(&completion_receipt_id(&task.task_id)),
            None,
            "no receipt: this is the crash window the resume reconciles on"
        );
        task.task_id
    };

    // Process 2 — the resume heals it, once.
    let mut rt2 = compact_runtime(store.clone(), ledger.clone());
    assert_eq!(
        rt2.reconcile_completions("story-1").expect("reconcile"),
        1,
        "the orphaned unit is applied by the next process"
    );
    assert!(ledger
        .has_final(&completion_receipt_id(&task_id))
        .expect("has_final"));
    assert_eq!(ledger.merges(), 1);

    // Process 3 — a second resume merges nothing.
    let mut rt3 = compact_runtime(store.clone(), ledger.clone());
    assert_eq!(
        rt3.reconcile_completions("story-1").expect("reconcile"),
        0,
        "the receipt already exists; a third process must apply nothing"
    );
    assert_eq!(
        ledger.merges(),
        1,
        "the evidence merge happened exactly once across three processes"
    );
}

/// The counters are observers on the canonical story row, and one unit moves each one once.
#[test]
fn repair_and_replan_counters_move_once_per_unit() {
    let ledger = Arc::new(SharedLedger::default());
    let repair = record("task-repair", Some("repair_smith"));
    assert!(apply_completion_unit(ledger.as_ref(), repair.clone()).expect("ledger"));
    // The same unit, seen again: not a second repair attempt.
    assert!(!apply_completion_unit(ledger.as_ref(), repair.clone()).expect("ledger"));
    let replan = record("task-replan", Some("repair_architect"));
    assert!(apply_completion_unit(ledger.as_ref(), replan.clone()).expect("ledger"));
    assert!(!apply_completion_unit(ledger.as_ref(), replan.clone()).expect("ledger"));
    assert_eq!(ledger.repairs("story-1"), 1);
    assert_eq!(ledger.replans("story-1"), 1);
    assert_eq!(ledger.merges(), 2, "one merge per unit, not per look");
}

/// A ledger that cannot answer must stop the run. Under the old `bool` trait this case could not be
/// written: there was no way to say "I could not ask", and the answer was `false` — "someone else applied it".
#[test]
fn a_ledger_that_cannot_answer_is_not_already_applied() {
    let ledger = Arc::new(UnreachableLedger::default());
    let error = apply_completion_unit(ledger.as_ref(), record("task-1", None)).unwrap_err();
    assert!(
        error.to_string().contains("database unreachable"),
        "the failure is reported, not swallowed: {error}"
    );
    assert!(
        ledger.merged.lock().unwrap().is_empty(),
        "nothing is written against a ledger that never took the claim"
    );
}

/// A unit whose evidence write fails stays claimed and unfinalized: that is the crash window, and the
/// resume — not a finalize the unit never earned — is what closes it.
#[test]
fn a_failed_evidence_write_does_not_finalize_the_receipt() {
    let ledger = Arc::new(HalfWrittenLedger::default());
    let error = apply_completion_unit(ledger.as_ref(), record("task-1", None)).unwrap_err();
    assert!(error.to_string().contains("evidence write failed"));
    assert!(
        ledger.finalized.lock().unwrap().is_empty(),
        "an unfinalized receipt is the durable signal the resume reconciles on"
    );
}

/// The runtime door must not turn a ledger failure into a completed task: `complete_role_task` propagates.
#[test]
fn a_ledger_failure_reaches_the_caller_of_complete_role_task() {
    let ledger = Arc::new(UnreachableLedger::default());
    let mut rt = compact_runtime(MemoryStore::new(), ledger);
    rt.start_story("story-1", "FEATURE", feature_story())
        .expect("start");
    let task = rt.list_role_tasks("story-1").expect("tasks")[0].clone();
    rt.claim_role_task(&task.task_id, "lead").expect("claim");
    let error = rt
        .complete_role_task(
            &task.task_id,
            "lead",
            Some("smith"),
            ForgeGateEvidence::default(),
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("database unreachable"),
        "the transition is durable and the unit's failure is still reported: {error}"
    );
}

/// The fence on the wiring, because nothing drives the engine binary in a test: the one production runtime
/// call site must name the durable ledger, never the fixture one.
#[test]
fn the_engine_binary_installs_the_durable_ledger() {
    // The path is the forge crate's, spelled from this file rather than from the crate it used to live in.
    let source = include_str!("../../forge/src/bin/forge.rs");
    assert!(
        source.contains("durable_completion_ledger()"),
        "forge/src/bin/forge.rs must build its runtime with the receipt row; a process-local ledger there re-applies \
         every completion in the instance history"
    );
}
