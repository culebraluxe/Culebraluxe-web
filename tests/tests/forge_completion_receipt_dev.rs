//! The completion UNIT against DEV: one transaction, or nothing (FORGE-B1 slice 2).
//!
//! Run explicitly with:
//!   set -a; . ./.env.local; set +a
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_completion_receipt_dev -- --ignored --test-threads=1
//!
//! WHY A ROW-LEVEL TEST. The unit's "exactly once" is a fact about rows — a unique index, an `on conflict`,
//! a `for update`, a rollback — and no unit test can see one: a unit test proves the shape of a call, never
//! that `updated_at` exists, that the insert conflicts, or that a die-mid-unit leaves nothing behind. That
//! last one is this slice's defect: the engine used to write the receipt, the evidence and the budget as FOUR
//! statements on the pool, so a process that died in the window left the evidence merged and the budget spent
//! with no receipt, and the resume spent the budget again.
//!
//! WHAT THIS FILE WALKS (`ForgeEngineDao::apply_completion`), one acceptance bullet per step:
//!
//!   1. fault injected after the evidence merge and after the counter increment — neither partial change nor
//!      the final receipt survives the rollback, and the same call then succeeds on a fresh transaction;
//!   2. a valid unit commits exactly ONE receipt row with all three effects visible in the same read;
//!   3. the same task's unit applied twice returns `AlreadyApplied`, the counter moves once, evidence is
//!      unchanged, and the receipt count for the task stays 1 (a crash after the commit, before the caller
//!      heard, is this step);
//!  3b. the reconcile watermark advances only on that commit: a receipt that is only CLAIMED (a peer
//!      mid-unit, or one the pre-unit claim door left `pending`) moves nothing, so the resume can never skip
//!      the event an unfinished unit belongs to;
//!   4. two CONCURRENT applications of one unit are serialized by the receipt — exactly one `Applied`; and two
//!      concurrent units on ONE story each keep their own evidence and each move their own counter once;
//!   5. no final receipt exists without its committed effects, and a receipt whose unit disagrees is refused
//!      (`Conflict`) rather than replayed.
//!
//! Step 3b is the one step that reads a sibling verb (`claim_workflow_receipt`, plus `receipt_watermark_ms`) —
//! the pre-unit claim/finalize door the completion path no longer uses. It reads it only to prove the unit's
//! OWN predicate (`outcome <> 'pending'`) against the shape that door still leaves in the table; the door's own
//! behaviour keeps its DEV and chaos coverage in
//! `forge_completion_receipt__004__stale_pending_reclamation`, `chaos_concurrency__001__same_workflow_task`
//! and `chaos_forge__001__` / `__007__`.
//!
//! `TestDatabase` refuses PRODUCTION before a socket is opened, so this file cannot be pointed at PROD. The
//! rollback test leaves DEV untouched; the committed walk sweeps its own rows and uses a unique tag per run,
//! so a rerun is clean even after a crash.

use db::{
    CompletionApply, CompletionSpend, CompletionUnit, Database, ForgeEngineDao, ForgeEvidencePatch,
    WorkflowReceiptClaim,
};
use sqlx::PgConnection;
use test_harness::database::TestDatabase;

/// The three effects of one unit, read in ONE query: the receipt and its proof, the merged evidence, and the
/// canonical story counters. Read together because the unit's claim is that they are never apart.
struct Effects {
    outcome: String,
    proof_receipt: Option<String>,
    proof_spend: Option<String>,
    proof_node: Option<String>,
    candidate_sha: Option<String>,
    qa_passed: Option<bool>,
    publish_succeeded: Option<bool>,
    repairs: i32,
    replans: i32,
    receipts_for_task: i64,
}

async fn read_effects(conn: &mut PgConnection, receipt: &str, instance: &str) -> Effects {
    let row: (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<bool>,
        Option<bool>,
        i32,
        i32,
        i64,
    ) = sqlx::query_as(
        "select r.outcome,
                r.result_payload ->> 'task_receipt',
                r.result_payload ->> 'spend',
                r.result_payload ->> 'node_id',
                e.candidate_sha,
                e.qa_passed,
                e.publish_succeeded,
                coalesce(s.forge_repair_attempts, 0),
                coalesce(s.forge_replan_attempts, 0),
                (select count(*) from workflow_command_receipt where command_id = r.command_id)
           from workflow_command_receipt r
           join storyboard_story s on s.id = r.aggregate_id
           left join forge_workflow_evidence e
                  on e.story_id = r.aggregate_id
                 and e.process_instance_id = $2::uuid
          where r.command_id = $1",
    )
    .bind(receipt)
    .bind(instance)
    .fetch_one(conn)
    .await
    .expect("one read of the unit's three effects");
    Effects {
        outcome: row.0,
        proof_receipt: row.1,
        proof_spend: row.2,
        proof_node: row.3,
        candidate_sha: row.4,
        qa_passed: row.5,
        publish_succeeded: row.6,
        repairs: row.7,
        replans: row.8,
        receipts_for_task: row.9,
    }
}

async fn receipt_rows_like(db: &Database, prefix: &str) -> i64 {
    sqlx::query_scalar("select count(*) from workflow_command_receipt where command_id like $1")
        .bind(format!("{prefix}%"))
        .fetch_one(db.pool())
        .await
        .expect("receipt row count")
}

async fn counters(db: &Database, story: &str) -> (i32, i32) {
    sqlx::query_as(
        "select coalesce(forge_repair_attempts, 0), coalesce(forge_replan_attempts, 0)
           from storyboard_story where id = $1",
    )
    .bind(story)
    .fetch_one(db.pool())
    .await
    .expect("story counters")
}

/// One patch that establishes a candidate, and one that establishes a release fact: two units that each know
/// something the other does not, which is what makes the merge observable.
fn patch_candidate(sha: &str) -> ForgeEvidencePatch {
    ForgeEvidencePatch {
        candidate_sha: Some(sha.into()),
        qa_passed: Some(true),
        ..Default::default()
    }
}

fn patch_release() -> ForgeEvidencePatch {
    ForgeEvidencePatch {
        publish_succeeded: Some(false),
        ..Default::default()
    }
}

fn unit_for<'a>(
    receipt: &'a str,
    story: &'a str,
    instance: &'a str,
    fingerprint: &'a str,
    patch: &'a ForgeEvidencePatch,
    spend: Option<CompletionSpend>,
) -> CompletionUnit<'a> {
    CompletionUnit {
        command_id: receipt,
        process_instance_id: instance,
        story_id: story,
        // The node follows the budget, as the engine's own `spend()` maps it: a repair is a repair Smith's
        // accepted completion, a replan a repair architect's. The proof's node provenance is asserted below.
        node_id: match spend {
            Some(CompletionSpend::Repair) => Some("repair_smith"),
            Some(CompletionSpend::Replan) => Some("repair_architect"),
            None => Some("smith"),
        },
        evidence: patch,
        spend,
        fingerprint,
    }
}

async fn create_fixture(db: &Database, story: &str, instance: &str) {
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status)
         values ($1, 'TEST', 'completion unit', 'P3', 'Planned')
         on conflict (id) do nothing",
    )
    .bind(story)
    .execute(db.pool())
    .await
    .expect("insert the unit's story");
    sqlx::query(
        "insert into process_instances (id, definition_id, status, subject_type, subject_id)
         select $1::uuid, id, 'active', 'story', $2 from process_definitions where key = 'FORGE_SDLC' limit 1
         on conflict (id) do nothing",
    )
    .bind(instance)
    .bind(story)
    .execute(db.pool())
    .await
    .expect("insert the unit's process instance");
}

/// Leave DEV as it was found: the unit's receipts, evidence, instance and story, in foreign-key order.
/// Sweeping first also makes a rerun clean after a crash mid-test.
async fn sweep(db: &Database, story: &str, instance: &str, prefix: &str) {
    sqlx::query("delete from workflow_command_receipt where command_id like $1")
        .bind(format!("{prefix}%"))
        .execute(db.pool())
        .await
        .expect("sweep the unit's receipts");
    sqlx::query("delete from forge_workflow_evidence where process_instance_id = $1::uuid")
        .bind(instance)
        .execute(db.pool())
        .await
        .expect("sweep the unit's evidence");
    sqlx::query("delete from process_instances where id = $1::uuid")
        .bind(instance)
        .execute(db.pool())
        .await
        .expect("sweep the unit's process instance");
    sqlx::query("delete from storyboard_story where id = $1")
        .bind(story)
        .execute(db.pool())
        .await
        .expect("sweep the unit's story");
}

async fn receipt_outcome_pool(db: &Database, receipt: &str) -> Option<String> {
    sqlx::query_scalar("select outcome from workflow_command_receipt where command_id = $1")
        .bind(receipt)
        .fetch_optional(db.pool())
        .await
        .expect("receipt outcome on the pool")
}

async fn evidence_rows(db: &Database, instance: &str) -> i64 {
    sqlx::query_scalar(
        "select count(*) from forge_workflow_evidence where process_instance_id = $1::uuid",
    )
    .bind(instance)
    .fetch_one(db.pool())
    .await
    .expect("evidence row count")
}

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

/// Bullets 1, 2 and 3: a fault after the effects but before the commit leaves nothing behind, the same unit
/// then commits once, and applying it again changes nothing.
#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (the harness refuses PROD)"]
async fn forge_completion_receipt_dev__unit_is_one_transaction() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared DEV database");
    let db = test_db.database().clone();
    let dao = ForgeEngineDao::new(db.clone());

    let tag = uuid::Uuid::new_v4().simple().to_string();
    let story = format!("ENG-PROOF-UNIT-{tag}");
    let instance = uuid::Uuid::new_v4().to_string();
    let receipt = format!("forge.completion:proof-{tag}:task-1");
    let fingerprint = format!("story:{story}");
    let patch = patch_candidate(SHA);

    create_fixture(&db, &story, &instance).await;

    // 1. FAULT INJECTION. The unit runs on the CALLER's transaction — that is the seam a fault is injected at,
    //    and it is the seam a service mutation uses — and everything it writes is visible inside it. The
    //    rollback stands for a process dying at the last possible moment: after the evidence merge, after the
    //    counter increment, before the commit.
    let mut tx = db
        .begin("forge_completion_receipt_dev.fault")
        .await
        .expect("begin");
    let unit = unit_for(
        &receipt,
        &story,
        &instance,
        &fingerprint,
        &patch,
        Some(CompletionSpend::Repair),
    );
    assert_eq!(
        dao.apply_completion_tx(&mut tx, &unit)
            .await
            .expect("apply inside the caller's transaction"),
        CompletionApply::Applied,
        "inside the transaction, the unit is applied"
    );
    let inside = read_effects(tx.connection(), &receipt, &instance).await;
    assert_eq!(inside.outcome, "success");
    assert_eq!(inside.candidate_sha.as_deref(), Some(SHA));
    assert_eq!(inside.qa_passed, Some(true));
    assert_eq!(inside.repairs, 1);
    tx.rollback().await.expect("rollback");

    // None of it survived: not the receipt, not the evidence, not the spent budget. Under the four-statement
    // shape the evidence and the budget were already committed when the process died, and the resume spent the
    // budget a second time — the defect this slice removes.
    assert_eq!(
        receipt_outcome_pool(&db, &receipt).await,
        None,
        "a rolled-back unit leaves no receipt"
    );
    assert_eq!(
        evidence_rows(&db, &instance).await,
        0,
        "a rolled-back unit leaves no evidence"
    );
    assert_eq!(
        counters(&db, &story).await,
        (0, 0),
        "a rolled-back unit spends nothing"
    );

    // 2. The same unit, unchanged, now commits — and all three effects are one row read apart.
    assert_eq!(
        dao.apply_completion(&unit).await.expect("apply"),
        CompletionApply::Applied
    );
    let applied = read_effects(
        &mut *db.pool().acquire().await.expect("connection"),
        &receipt,
        &instance,
    )
    .await;
    assert_eq!(applied.outcome, "success");
    assert_eq!(applied.proof_receipt.as_deref(), Some(receipt.as_str()));
    assert_eq!(applied.proof_spend.as_deref(), Some("repair"));
    assert_eq!(
        applied.proof_node.as_deref(),
        Some("repair_smith"),
        "the proof names the node (and so the budget) the unit belonged to"
    );
    assert_eq!(applied.candidate_sha.as_deref(), Some(SHA));
    assert_eq!(applied.qa_passed, Some(true));
    assert_eq!(applied.repairs, 1, "one unit, one spend");
    assert_eq!(
        applied.replans, 0,
        "a repair unit must not move the replan counter"
    );
    assert_eq!(
        applied.receipts_for_task, 1,
        "one receipt row per task, not one per attempt"
    );

    // 3. IDEMPOTENCE, and the crash-after-commit-before-acknowledgement case: the caller never heard the answer,
    //    so it asks again. The answer is `AlreadyApplied`, and nothing moves.
    assert_eq!(
        dao.apply_completion(&unit).await.expect("apply again"),
        CompletionApply::AlreadyApplied,
        "a committed receipt answers without touching the evidence or the counters"
    );
    let replayed = read_effects(
        &mut *db.pool().acquire().await.expect("connection"),
        &receipt,
        &instance,
    )
    .await;
    assert_eq!(replayed.repairs, 1, "the counter did not move again");
    assert_eq!(replayed.candidate_sha.as_deref(), Some(SHA));
    assert_eq!(replayed.receipts_for_task, 1);
    assert_eq!(
        receipt_rows_like(&db, &format!("forge.completion:proof-{tag}:")).await,
        1,
        "one task, one receipt row"
    );

    // 3b. THE RECONCILE WATERMARK MOVES ONLY ON A COMMITTED UNIT. `receipt_watermark_ms` is the newest
    //     FINALIZED receipt under the prefix, and the resume skips process events at or before it
    //     (`forge/src/engine/runtime.rs:409-417`). So a receipt that is only CLAIMED — a peer mid-unit, or
    //     one the pre-unit claim door left `pending` — must not advance it: that would skip the very event
    //     the unfinished unit belongs to. Three reads of the same row, in order: `pending` moves nothing; the
    //     unit that finds it inside the 15-minute window answers `Busy` and still writes nothing (`Busy` must
    //     never be mistaken for `AlreadyApplied` — work order §6); and the same unit, once the window has
    //     passed, takes the row over and commits, which is when the watermark advances. This is the SQL half
    //     of TST-FORGE-COMPLETION-RECEIPT-005, which holds the in-memory half.
    let prefix = format!("forge.completion:proof-{tag}:");
    let pending = format!("{prefix}task-pending");
    let before = dao
        .receipt_watermark_ms(&prefix)
        .await
        .expect("watermark")
        .expect("the committed unit advanced the watermark");
    assert!(
        matches!(
            dao.claim_workflow_receipt(&pending, None)
                .await
                .expect("claim"),
            WorkflowReceiptClaim::Acquired
        ),
        "the pre-unit claim door leaves a pending receipt for this unit"
    );
    assert_eq!(
        dao.receipt_watermark_ms(&prefix).await.expect("watermark"),
        Some(before),
        "a claimed but unapplied receipt advances nothing: pending is not final"
    );
    let unit_pending = unit_for(&pending, &story, &instance, &fingerprint, &patch, None);
    assert_eq!(
        dao.apply_completion(&unit_pending)
            .await
            .expect("apply the claimed unit"),
        CompletionApply::Busy,
        "a FRESH pending row is a peer mid-unit: `Busy` is not `AlreadyApplied` (work order §6)"
    );
    assert_eq!(
        dao.receipt_watermark_ms(&prefix).await.expect("watermark"),
        Some(before),
        "and a refused unit wrote nothing, so the watermark still has not moved"
    );
    sqlx::query(
        "update workflow_command_receipt set updated_at = now() - interval '16 minutes' where command_id = $1",
    )
    .bind(&pending)
    .execute(db.pool())
    .await
    .expect("inject the age of a process that died holding the receipt");
    assert_eq!(
        dao.apply_completion(&unit_pending)
            .await
            .expect("apply the reclaimed unit"),
        CompletionApply::Applied,
        "and the same unit commits once its process gets there"
    );
    assert!(
        dao.receipt_watermark_ms(&prefix)
            .await
            .expect("watermark")
            .expect("a committed unit advances the watermark")
            >= before,
        "the commit is what moves it, and it never moves backwards"
    );
    assert_eq!(
        receipt_rows_like(&db, &prefix).await,
        2,
        "one receipt per task: the idempotent unit and the claimed one"
    );

    sweep(&db, &story, &instance, &prefix).await;
}

/// Bullets 4 and 5: two concurrent applications of one unit serialize on the receipt; two concurrent units on
/// one story each keep their own evidence and their own budget; no final receipt exists without its effects; a
/// dead process's receipt is reclaimed only past the stale window; and a receipt whose unit disagrees is
/// refused.
#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (the harness refuses PROD)"]
async fn forge_completion_receipt_dev__concurrent_units_are_serialized() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared DEV database");
    let db = test_db.database().clone();
    let dao = ForgeEngineDao::new(db.clone());

    let tag = uuid::Uuid::new_v4().simple().to_string();
    let story = format!("ENG-PROOF-UNIT-{tag}");
    let instance = uuid::Uuid::new_v4().to_string();
    let prefix = format!("forge.completion:proof-{tag}:");
    let fingerprint = format!("story:{story}");

    create_fixture(&db, &story, &instance).await;

    // 4a. TWO CONCURRENT APPLICATIONS OF ONE UNIT. The claim is inserted before anything else is written, so
    //     the second caller waits on the unique index until the first commits, then reads the committed receipt:
    //     exactly one of the two applied the unit, and the loser did not fork the row.
    let same = format!("{prefix}task-same");
    let patch_same = patch_candidate(SHA);
    let unit_same = unit_for(
        &same,
        &story,
        &instance,
        &fingerprint,
        &patch_same,
        Some(CompletionSpend::Repair),
    );
    let (left, right) = tokio::join!(
        dao.apply_completion(&unit_same),
        dao.apply_completion(&unit_same)
    );
    let outcomes = [left.expect("left apply"), right.expect("right apply")];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| **outcome == CompletionApply::Applied)
            .count(),
        1,
        "exactly one of two concurrent applications owns the unit: {outcomes:?}"
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| **outcome == CompletionApply::AlreadyApplied)
            .count(),
        1,
        "and the other is told the unit is already applied: {outcomes:?}"
    );
    assert_eq!(
        counters(&db, &story).await,
        (1, 0),
        "one winner, one spend, however the race fell"
    );
    assert_eq!(
        receipt_rows_like(&db, &prefix).await,
        1,
        "the loser did not fork a second receipt row"
    );

    // 4b. TWO CONCURRENT UNITS ON ONE STORY. Different receipts, different evidence, different budgets: the
    //     story row is locked before anything is spent, so the spends serialize, and each unit's evidence
    //     survives the other's merge.
    let candidate = format!("{prefix}task-candidate");
    let release = format!("{prefix}task-release");
    let patch_candidate_unit = patch_candidate(SHA);
    let patch_release_unit = patch_release();
    let unit_candidate = unit_for(
        &candidate,
        &story,
        &instance,
        &fingerprint,
        &patch_candidate_unit,
        Some(CompletionSpend::Repair),
    );
    let unit_release = unit_for(
        &release,
        &story,
        &instance,
        &fingerprint,
        &patch_release_unit,
        Some(CompletionSpend::Replan),
    );
    let (first, second) = tokio::join!(
        dao.apply_completion(&unit_candidate),
        dao.apply_completion(&unit_release)
    );
    assert_eq!(first.expect("candidate unit"), CompletionApply::Applied);
    assert_eq!(second.expect("release unit"), CompletionApply::Applied);
    let mut conn = db.pool().acquire().await.expect("connection");
    let merged = read_effects(&mut conn, &candidate, &instance).await;
    assert_eq!(
        merged.candidate_sha.as_deref(),
        Some(SHA),
        "the candidate unit's fact is on the shared evidence row"
    );
    assert_eq!(merged.qa_passed, Some(true));
    assert_eq!(
        merged.publish_succeeded,
        Some(false),
        "and so is the release unit's: neither unit's merge erased the other's"
    );
    assert_eq!(
        (merged.repairs, merged.replans),
        (2, 1),
        "each unit moved its own counter once"
    );
    assert_eq!(
        merged.proof_node.as_deref(),
        Some("repair_smith"),
        "and the receipt of the candidate unit records the node that spent"
    );
    let replanned = read_effects(&mut conn, &release, &instance).await;
    assert_eq!(
        replanned.proof_spend.as_deref(),
        Some("replan"),
        "the second unit spent the replan budget"
    );
    assert_eq!(
        replanned.proof_node.as_deref(),
        Some("repair_architect"),
        "recorded against the node that owns that budget, not the one the other unit used"
    );

    // 5a. NO FINAL RECEIPT WITHOUT ITS EFFECTS. Every final receipt from this walk names the story it committed,
    //     carries a proof payload and a message, and has the evidence row it claims — the negative control is a
    //     final receipt with nothing to show for it.
    let unproven: i64 = sqlx::query_scalar(
        "select count(*) from workflow_command_receipt
          where command_id like $1 and outcome <> 'pending'
            and (result_payload is null or aggregate_id is null or message is null)",
    )
    .bind(format!("{prefix}%"))
    .fetch_one(db.pool())
    .await
    .expect("final receipts without a proof");
    assert_eq!(unproven, 0, "a final receipt always carries its proof");
    let orphans: i64 = sqlx::query_scalar(
        "select count(*) from workflow_command_receipt r
          where r.command_id like $1 and r.outcome <> 'pending'
            and not exists (select 1 from forge_workflow_evidence e
                             where e.process_instance_id = $2::uuid
                               and e.story_id = r.aggregate_id)",
    )
    .bind(format!("{prefix}%"))
    .bind(&instance)
    .fetch_one(db.pool())
    .await
    .expect("final receipts without evidence");
    assert_eq!(
        orphans, 0,
        "a final receipt's effects are committed with it"
    );

    // 5b. A DEAD PROCESS MUST NOT LOCK THE UNIT FOREVER. A `pending` receipt inside the window is a peer
    //     mid-unit — `Busy`, and left exactly as it was found; past the window it is taken over and applied once.
    let stale = format!("{prefix}task-stale");
    assert!(
        matches!(
            dao.claim_workflow_receipt(&stale, None)
                .await
                .expect("claim"),
            WorkflowReceiptClaim::Acquired
        ),
        "the pre-unit claim door leaves the same pending shape the unit reads"
    );
    let patch_stale = patch_release();
    let unit_stale = unit_for(
        &stale,
        &story,
        &instance,
        &fingerprint,
        &patch_stale,
        Some(CompletionSpend::Repair),
    );
    assert_eq!(
        dao.apply_completion(&unit_stale).await.expect("busy"),
        CompletionApply::Busy,
        "a fresh pending receipt belongs to another process"
    );
    assert_eq!(
        receipt_outcome_pool(&db, &stale).await.as_deref(),
        Some("pending"),
        "and the refusal left it as it was found"
    );
    sqlx::query(
        "update workflow_command_receipt set updated_at = now() - interval '16 minutes' where command_id = $1",
    )
    .bind(&stale)
    .execute(db.pool())
    .await
    .expect("inject the age of a process that died holding the receipt");
    assert_eq!(
        dao.apply_completion(&unit_stale).await.expect("reclaim"),
        CompletionApply::Applied,
        "past the window the unit takes the receipt over and applies once"
    );
    assert_eq!(
        counters(&db, &story).await,
        (3, 1),
        "the reclaimed unit spent its budget once, not once per attempt"
    );

    // 5c. CONFLICT. A committed receipt from the SAME task key, holding a different unit, is refused rather
    //     than replayed over — and the refusal writes nothing.
    let other_fingerprint = format!("story:NOT-{story}");
    let patch_conflict = patch_candidate(SHA);
    let unit_conflict = unit_for(
        &candidate,
        &story,
        &instance,
        &other_fingerprint,
        &patch_conflict,
        Some(CompletionSpend::Repair),
    );
    match dao
        .apply_completion(&unit_conflict)
        .await
        .expect("a conflicting unit is answered, not errored")
    {
        CompletionApply::Conflict { stored } => assert_eq!(
            stored, fingerprint,
            "the refusal names the unit that holds the receipt"
        ),
        outcome => panic!("a conflicting unit must be refused, got {outcome:?}"),
    }
    assert_eq!(
        counters(&db, &story).await,
        (3, 1),
        "and a refused unit spends nothing"
    );
    assert_eq!(
        receipt_rows_like(&db, &prefix).await,
        4,
        "one receipt per task: same, candidate, release, stale"
    );

    sweep(&db, &story, &instance, &prefix).await;
}
