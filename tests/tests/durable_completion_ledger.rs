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
//! caller, because "someone already applied this unit" is an answer only a committed receipt may give. (2)
//! The unit applies once regardless of which process looks. Both are now expressed on ONE method — `apply`
//! returns `Result<CompletionApply>` (FORGE-B1 slice 2) — because the unit is one committed effect: there is
//! no longer a claim to lose, a merge to replay or a finalize to forget. The row-level half of the same
//! contract — that the SQL behaves this way, one transaction, receipt and effects together — is
//! `tests/tests/forge_completion_receipt_dev.rs`, because a unit test cannot see a column typo.

use std::sync::{Arc, Mutex};

use forge::engine::*;
use workflow::{json, MemoryStore, Result, WorkflowError};

/// A ledger shared by two runtimes, standing in for the receipt row both processes read.
///
/// It IS the production in-memory ledger — `MemoryLedger` — rather than a second implementation of the same
/// semantics, because two adjudicators of one fact is how a wrong verdict gets a second author. What this
/// double adds is the book of *committed applications*, which is what "exactly once across processes" is
/// measured on.
#[derive(Default)]
struct SharedLedger {
    inner: MemoryLedger,
    applied: Mutex<Vec<String>>,
}

impl SharedLedger {
    fn applied_count(&self) -> usize {
        self.applied.lock().unwrap().len()
    }
    fn repairs(&self, story_id: &str) -> u32 {
        self.inner.repairs(story_id)
    }
    fn replans(&self, story_id: &str) -> u32 {
        self.inner.replans(story_id)
    }
}

impl CompletionLedger for SharedLedger {
    fn apply(&self, rec: &CompletionRecord) -> Result<CompletionApply> {
        let outcome = self.inner.apply(rec)?;
        if outcome.applied() {
            self.applied
                .lock()
                .unwrap()
                .push(completion_receipt_id(&rec.task_id));
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

/// A ledger whose every question fails. It stands for a database that is not there ("database unreachable")
/// or a unit whose transaction died ("evidence write failed"), which is the case the old `bool` trait could
/// not express: it had to answer `false`, and `false` is what "already applied" says.
#[derive(Default)]
struct FailingLedger {
    reason: String,
    calls: Mutex<u32>,
}

impl FailingLedger {
    fn new(reason: &str) -> Self {
        Self {
            reason: reason.to_string(),
            calls: Mutex::new(0),
        }
    }
    fn calls(&self) -> u32 {
        *self.calls.lock().unwrap()
    }
    fn failure(&self) -> WorkflowError {
        WorkflowError::generic(self.reason.clone())
    }
}

impl CompletionLedger for FailingLedger {
    fn apply(&self, _rec: &CompletionRecord) -> Result<CompletionApply> {
        *self.calls.lock().unwrap() += 1;
        Err(self.failure())
    }
    fn has_final(&self, _receipt_id: &str) -> Result<bool> {
        Err(self.failure())
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
        let rt = ForgeRuntime::from_store(
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
        assert!(
            !ledger
                .has_final(&completion_receipt_id(&task.task_id))
                .expect("has_final"),
            "no receipt: this is the crash window the resume reconciles on"
        );
        assert_eq!(
            ledger.applied_count(),
            0,
            "and the unit is not applied either"
        );
        task.task_id
    };

    // Process 2 — the resume heals it, once.
    let rt2 = compact_runtime(store.clone(), ledger.clone());
    assert_eq!(
        rt2.reconcile_completions("story-1").expect("reconcile"),
        1,
        "the orphaned unit is applied by the next process"
    );
    assert!(ledger
        .has_final(&completion_receipt_id(&task_id))
        .expect("has_final"));
    assert_eq!(ledger.applied_count(), 1);

    // Process 3 — a second resume applies nothing.
    let rt3 = compact_runtime(store.clone(), ledger.clone());
    assert_eq!(
        rt3.reconcile_completions("story-1").expect("reconcile"),
        0,
        "the receipt already exists; a third process must apply nothing"
    );
    assert_eq!(
        ledger.applied_count(),
        1,
        "the unit was applied exactly once across three processes"
    );
}

/// The counters are observers on the canonical story row, and one unit moves each one once.
#[test]
fn repair_and_replan_counters_move_once_per_unit() {
    let ledger = Arc::new(SharedLedger::default());
    let repair = record("task-repair", Some("repair_smith"));
    assert_eq!(
        apply_completion_unit(ledger.as_ref(), repair.clone()).expect("ledger"),
        CompletionApply::Applied
    );
    // The same unit, seen again: not a second repair attempt.
    assert_eq!(
        apply_completion_unit(ledger.as_ref(), repair.clone()).expect("ledger"),
        CompletionApply::AlreadyApplied
    );
    let replan = record("task-replan", Some("repair_architect"));
    assert_eq!(
        apply_completion_unit(ledger.as_ref(), replan.clone()).expect("ledger"),
        CompletionApply::Applied
    );
    assert_eq!(
        apply_completion_unit(ledger.as_ref(), replan.clone()).expect("ledger"),
        CompletionApply::AlreadyApplied
    );
    assert_eq!(ledger.repairs("story-1"), 1);
    assert_eq!(ledger.replans("story-1"), 1);
    assert_eq!(
        ledger.applied_count(),
        2,
        "one application per unit, not one per look"
    );
}

/// A ledger that cannot answer must stop the run. Under the old `bool` trait this case could not be
/// written: there was no way to say "I could not ask", and the answer was `false` — "someone else applied it".
#[test]
fn a_ledger_that_cannot_answer_is_not_already_applied() {
    let ledger = Arc::new(FailingLedger::new("database unreachable"));
    let error = apply_completion_unit(ledger.as_ref(), record("task-1", None)).unwrap_err();
    assert!(
        error.to_string().contains("database unreachable"),
        "the failure is reported, not swallowed: {error}"
    );
    assert_eq!(
        ledger.calls(),
        1,
        "and the unit is not attempted again behind the caller's back"
    );
}

/// A unit whose write fails leaves the transition durable and the receipt absent — the crash window — and
/// the next process heals it exactly once. Under the four-call shape the retry could not be told from a
/// first attempt either: the evidence was already merged and the budget already spent, so the heal spent it
/// again (that is TST-FORGE-COMPLETION-RECEIPT-008's pin).
#[test]
fn a_failed_unit_is_recovered_by_the_next_process_not_replayed() {
    let store = MemoryStore::new();
    let failing = Arc::new(FailingLedger::new("evidence write failed"));
    let rt = compact_runtime(store.clone(), failing.clone());
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
        error.to_string().contains("evidence write failed"),
        "the unit's failure is reported: {error}"
    );
    assert_eq!(failing.calls(), 1, "the unit was attempted once");

    // The next process, with a ledger that can answer, applies the orphan — once.
    let healed = Arc::new(SharedLedger::default());
    let rt2 = compact_runtime(store.clone(), healed.clone());
    assert_eq!(
        rt2.reconcile_completions("story-1").expect("reconcile"),
        1,
        "a unit the failing ledger never committed is applied by the next process"
    );
    assert_eq!(healed.applied_count(), 1);
    let rt3 = compact_runtime(store.clone(), healed.clone());
    assert_eq!(
        rt3.reconcile_completions("story-1").expect("reconcile"),
        0,
        "and a third process applies nothing"
    );
    assert_eq!(healed.applied_count(), 1);
}

/// The runtime door must not turn a ledger failure into a completed task: `complete_role_task` propagates.
#[test]
fn a_ledger_failure_reaches_the_caller_of_complete_role_task() {
    let ledger = Arc::new(FailingLedger::new("database unreachable"));
    let rt = compact_runtime(MemoryStore::new(), ledger);
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
        "forge/src/bin/forge.rs must build its runtime with the receipt row; a process-local ledger there re-applies every completion in the instance history"
    );
}
