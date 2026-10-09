//! DB.TRANSACTION — receipt + evidence (TST-DB-TRANSACTION-003).
//!
//! CONTRACT. A completion unit is ONE transaction: the receipt, the evidence merge and the story's counters
//! commit together or not at all, and the receipt is the unit's idempotence key.
//!
//!   * a second application of the same unit answers `AlreadyApplied` and writes nothing — there is no state
//!     in which a second winner exists;
//!   * evidence merges with `coalesce(new, existing)`: a later unit that knows nothing about `qa_passed`
//!     leaves it as it was, and adds only what it does know (`candidate_sha`);
//!   * a rollback leaves neither a receipt nor an evidence row, which is the fault the multi-statement shape
//!     this test was written for used to leave behind (FORGE-B1 slice 2).
//!
//! The statement is the production routine's (`ForgeEngineDao::apply_completion_tx`, `db/src/forge_engine.rs`,
//! the caller's-transaction door of `apply_completion`), run on a transaction this harness only rolls back —
//! the caller's transaction IS the fault seam the slice's first acceptance bullet asks for. A non-production
//! database only.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_transaction__003__receipt_evidence -- --ignored

use db::{CompletionApply, CompletionSpend, CompletionUnit, ForgeEngineDao, ForgeEvidencePatch};
use test_harness::database::TestDatabase;

const STORY: &str = "TST-DB-TRANSACTION-003";
const INSTANCE: &str = "0b3c1f7e-3d4a-4a62-9a0e-00000000e003";
const FINGERPRINT: &str = "story:TST-DB-TRANSACTION-003";
const FIRST: &str = "forge.completion:tst-db-transaction-003:task-1";
const SECOND: &str = "forge.completion:tst-db-transaction-003:task-2";
const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

fn unit<'a>(
    receipt: &'a str,
    patch: &'a ForgeEvidencePatch,
    spend: Option<CompletionSpend>,
) -> CompletionUnit<'a> {
    CompletionUnit {
        command_id: receipt,
        process_instance_id: INSTANCE,
        story_id: STORY,
        // The node the spend belongs to, so the receipt's proof carries node provenance, not only the story.
        node_id: Some("repair_smith"),
        evidence: patch,
        spend,
        fingerprint: FINGERPRINT,
        assay_receipt: None,
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (APP_ENV=dev): runs only against the disposable DEV branch; PROD is refused"]
async fn db_transaction_003__receipt_evidence() {
    let test_db = TestDatabase::connect_from_env().await.expect(
        "a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)",
    );
    let db = test_db.database().clone();
    let dao = ForgeEngineDao::new(db.clone());
    let mut tx = db
        .begin("db_transaction_003.receipt_evidence")
        .await
        .expect("begin");

    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status)
         values ($1, 'TEST', 'receipt + evidence', 'P3', 'Planned')",
    )
    .bind(STORY)
    .execute(tx.connection())
    .await
    .expect("insert a story for the unit's counters and the evidence to name");

    // Evidence belongs to a process instance (foreign key), so the unit has one: a throwaway instance of the
    // Forge definition, inside this transaction.
    sqlx::query(
        "insert into process_instances (id, definition_id, status, subject_type, subject_id)
         select $1::uuid, id, 'active', 'story', $2 from process_definitions where key = 'FORGE_SDLC' limit 1",
    )
    .bind(INSTANCE)
    .bind(STORY)
    .execute(tx.connection())
    .await
    .expect("insert a process instance for the evidence to belong to");

    // The first unit, the one that knows QA passed and spends a repair. Applied — the routine's own answer,
    // and the receipt it commits is what makes the second look at it a duplicate.
    let qa_only = ForgeEvidencePatch {
        qa_passed: Some(true),
        ..Default::default()
    };
    let first = dao
        .apply_completion_tx(
            &mut tx,
            &unit(FIRST, &qa_only, Some(CompletionSpend::Repair)),
        )
        .await
        .expect("the first unit applies");
    assert_eq!(first, CompletionApply::Applied);

    // The same unit again: no second winner, and nothing written by the look.
    let again = dao
        .apply_completion_tx(
            &mut tx,
            &unit(FIRST, &qa_only, Some(CompletionSpend::Repair)),
        )
        .await
        .expect("a duplicate is answered, not errored");
    assert_eq!(
        again,
        CompletionApply::AlreadyApplied,
        "a committed unit is never applied twice"
    );

    // A second unit on the same instance knows only a candidate: it adds what it knows and leaves what the
    // first established standing.
    let candidate_only = ForgeEvidencePatch {
        candidate_sha: Some(SHA.into()),
        ..Default::default()
    };
    let second = dao
        .apply_completion_tx(&mut tx, &unit(SECOND, &candidate_only, None))
        .await
        .expect("the second unit applies");
    assert_eq!(second, CompletionApply::Applied);

    let (qa_passed, candidate, rows): (Option<bool>, Option<String>, i64) = sqlx::query_as(
        "select max(qa_passed::int)::int = 1, max(candidate_sha), count(*)
           from forge_workflow_evidence where story_id = $1",
    )
    .bind(STORY)
    .fetch_one(tx.connection())
    .await
    .expect("read the merged evidence");
    assert_eq!(
        rows, 1,
        "both units merged into one evidence row per instance"
    );
    assert_eq!(
        qa_passed,
        Some(true),
        "a later unit that says nothing about qa_passed must not erase it"
    );
    assert_eq!(candidate.as_deref(), Some(SHA), "and it adds what it knows");

    // The receipt is final and carries its proof: the unit's effects are what it names. The payload is read
    // as text (`->>`), so this test needs no JSON crate of its own.
    let (outcome, proof, node): (String, Option<String>, Option<String>) = sqlx::query_as(
        "select outcome, result_payload ->> 'task_receipt', result_payload ->> 'node_id'
           from workflow_command_receipt where command_id = $1",
    )
    .bind(FIRST)
    .fetch_one(tx.connection())
    .await
    .expect("read the receipt");
    assert_eq!(outcome, "success", "a committed receipt is not pending");
    assert_eq!(
        proof.as_deref(),
        Some(FIRST),
        "and its proof names the unit it proves"
    );
    assert_eq!(
        node.as_deref(),
        Some("repair_smith"),
        "and the node whose budget it spent, not only the story it belongs to"
    );

    // One spend for the one unit that spends: the duplicate did not increment, and the second unit spends
    // none.
    let (repairs, replans): (i32, i32) = sqlx::query_as(
        "select coalesce(forge_repair_attempts,0), coalesce(forge_replan_attempts,0)
           from storyboard_story where id = $1",
    )
    .bind(STORY)
    .fetch_one(tx.connection())
    .await
    .expect("read the story's counters");
    assert_eq!(
        (repairs, replans),
        (1, 0),
        "one unit, one increment — a duplicate spends nothing"
    );

    tx.rollback().await.expect("rollback");

    // Nothing survived the rollback: the receipt and its evidence go together or not at all.
    let receipts: i64 =
        sqlx::query_scalar("select count(*) from workflow_command_receipt where command_id = $1")
            .bind(FIRST)
            .fetch_one(db.pool())
            .await
            .expect("count receipts after rollback");
    let evidence: i64 =
        sqlx::query_scalar("select count(*) from forge_workflow_evidence where story_id = $1")
            .bind(STORY)
            .fetch_one(db.pool())
            .await
            .expect("count evidence after rollback");
    assert_eq!(
        (receipts, evidence),
        (0, 0),
        "a rolled-back unit leaves neither a receipt nor evidence"
    );
}
