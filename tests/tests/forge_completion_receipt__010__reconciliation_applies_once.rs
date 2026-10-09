//! FORGE.COMPLETION_RECEIPT — reconciliation applies once (TST-FORGE-COMPLETION-RECEIPT-010).
//!
//! CONTRACT. The resume is idempotent: an orphaned completion is applied by the first resume and by no
//! later one — the second resume finds the receipt final and merges nothing. Pinned at two doors of the
//! same production boundary: the unit door (`apply_completion_unit` refuses the second application) and
//! the resume door (`reconcile_completions` counts 1, then 0, and the watermark rests on the final).
//!
//! Level: L4 Adversarial — the adversarial case is the third look: a second resume after the heal must
//! apply nothing, or every process that ever looks at the story re-merges the evidence and re-spends
//! the repair budget (the defect `tests/tests/durable_completion_ledger.rs` removed). The negative half:
//! reconciling a story the engine never saw applies nothing. Deterministic and isolated: no database, no
//! network, never writes to PROD.
//!
//! WHAT IT DOES NOT COVER: the crash that orphans the unit is TST-FORGE-COMPLETION-RECEIPT-006; the
//! row-level half is `tests/tests/forge_completion_receipt_dev.rs`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_completion_receipt__010__reconciliation_applies_once

use std::sync::Arc;

use forge::engine::*;
use workflow::{json, MemoryStore};

fn record(task_id: &str) -> CompletionRecord {
    CompletionRecord {
        task_id: task_id.into(),
        process_instance_id: "11111111-1111-1111-1111-111111111111".into(),
        story_id: "story-1".into(),
        node_id: None,
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

/// The unit applies once, and the resume keeps it that way across processes.
#[test]
fn forge_completion_receipt_010__reconciliation_applies_once() {
    // Unit door: the same unit, seen twice, applies once.
    let ledger = MemoryLedger::new();
    assert_eq!(
        apply_completion_unit(&ledger, record("task-9")).expect("unit applies"),
        CompletionApply::Applied,
        "the first application owns the unit"
    );
    assert_eq!(
        apply_completion_unit(&ledger, record("task-9")).expect("unit answers"),
        CompletionApply::AlreadyApplied,
        "the second look applies nothing: the receipt is final"
    );

    // Resume door: orphan, heal, then rest.
    let store = MemoryStore::new();
    let shared: Arc<MemoryLedger> = Arc::new(MemoryLedger::new());
    let task_id = {
        let writer = Arc::new(RecordingWriter::default());
        let rt = ForgeRuntime::from_store(
            store.clone(),
            writer,
            None,
            None,
            shared.clone(),
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
            .expect("transition durable, unit orphaned");
        task.task_id
    };

    let rt2 = compact_runtime(store.clone(), shared.clone());
    assert_eq!(
        rt2.reconcile_completions("story-1").expect("reconcile"),
        1,
        "the first resume applies the orphaned unit"
    );
    let watermark = shared
        .watermark("forge.completion:")
        .expect("watermark answers")
        .expect("the heal finalizes, and finals move the watermark");

    let rt3 = compact_runtime(store.clone(), shared.clone());
    assert_eq!(
        rt3.reconcile_completions("story-1").expect("reconcile"),
        0,
        "the receipt already exists; a third process applies nothing"
    );
    assert_eq!(
        shared
            .watermark("forge.completion:")
            .expect("watermark answers"),
        Some(watermark),
        "a resume that applies nothing moves nothing"
    );
    assert!(
        shared
            .has_final(&completion_receipt_id(&task_id))
            .expect("has_final answers"),
        "one final receipt, stable across resumes"
    );

    // Negative: a story the engine never saw reconciles to nothing.
    assert_eq!(
        rt3.reconcile_completions("story-never-seen")
            .expect("reconcile"),
        0,
        "no instance, no applications"
    );
}
