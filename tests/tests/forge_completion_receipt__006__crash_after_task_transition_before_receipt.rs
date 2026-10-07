//! FORGE.COMPLETION_RECEIPT — crash after task transition before receipt
//! (TST-FORGE-COMPLETION-RECEIPT-006).
//!
//! CONTRACT. The transition is durable, the evidence is not written, no receipt: when a process wins a
//! task transition and dies before the completion unit runs, the next process's resume must apply the
//! orphaned unit exactly once. This is the interrupted-sequence contract
//! (`legacy/workflow_app/tests/interrupted-sequences.test.ts`, cited by
//! `forge/src/engine/completion.rs:136-140`) driven through the production resume door
//! (`ForgeRuntime::reconcile_completions`), with the two runtimes sharing one `MemoryStore` (its clones
//! share state: one durable engine) and one production `MemoryLedger` standing in for the receipt row.
//!
//! Level: L4 Adversarial — process kill (the first runtime is dropped mid-window, nothing finalized) and
//! restart (a fresh runtime on the same store). The negative half: a story with no won transitions
//! reconciles to nothing — the resume must not invent work. Deterministic and isolated: no database, no
//! network, never writes to PROD.
//!
//! WHAT IT DOES NOT COVER: the row-level half (the SQL really behaving this way) is
//! `tests/tests/forge_completion_receipt_dev.rs`; multi-process idempotence after the heal is
//! TST-FORGE-COMPLETION-RECEIPT-010.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_completion_receipt__006__crash_after_task_transition_before_receipt

use std::sync::Arc;

use forge::engine::*;
use workflow::{json, MemoryStore};

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

/// Transition durable, receipt absent: the resume applies the orphaned unit, once.
#[test]
fn forge_completion_receipt_006__crash_after_task_transition_before_receipt() {
    let store = MemoryStore::new();
    let ledger: Arc<MemoryLedger> = Arc::new(MemoryLedger::new());

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
            .expect("the transition is durable even though the unit never runs");
        task.task_id
        // Dropped here: no completion unit, no receipt, no goodbye. This is the crash.
    };

    assert!(
        !ledger
            .has_final(&completion_receipt_id(&task_id))
            .expect("has_final answers"),
        "absence of a receipt IS the crash window: the transition won, nothing applied"
    );

    // Process 2 — the resume heals the orphan, exactly once.
    let mut rt2 = compact_runtime(store.clone(), ledger.clone());
    assert_eq!(
        rt2.reconcile_completions("story-1").expect("reconcile"),
        1,
        "the orphaned unit is applied by the next process"
    );
    assert!(
        ledger
            .has_final(&completion_receipt_id(&task_id))
            .expect("has_final answers"),
        "the heal leaves a final receipt"
    );

    // Negative: the resume invents nothing. A story with no won transitions reconciles to zero.
    rt2.start_story("story-2", "FEATURE", feature_story())
        .expect("start");
    assert_eq!(
        rt2.reconcile_completions("story-2").expect("reconcile"),
        0,
        "no completed tasks, no applications: the resume heals orphans, never invents units"
    );
}
