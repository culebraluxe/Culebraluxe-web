//! DB.TRANSACTION — receipt + evidence (TST-DB-TRANSACTION-003).
//!
//! CONTRACT. A completion unit is claimed BEFORE its writes and finalized AFTER them, and what it writes —
//! the evidence merge — never erases what an earlier unit established:
//!
//!   * the receipt is a claim: the first claimant gets it, a second is refused while it is pending
//!     (`insert … on conflict do nothing`), and once finalized it is the proof the unit ran;
//!   * evidence merges with `coalesce(new, existing)`: a later unit that knows nothing about `qa_passed` leaves it
//!     as it was, and adds only what it does know (`candidate_sha`);
//!   * all of it is one transaction's worth: a rollback leaves neither a receipt nor an evidence row.
//!
//! The statements are the DAO's (`claim_workflow_receipt`, `merge_workflow_evidence`, `finalize_workflow_receipt`),
//! run inside a transaction that is rolled back. A non-production database only.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_transaction__003__receipt_evidence

use test_harness::database::TestDatabase;

const STORY: &str = "TST-DB-TRANSACTION-003";
const RECEIPT: &str = "forge.completion:tst-db-transaction-003";
const INSTANCE: &str = "0b3c1f7e-3d4a-4a62-9a0e-00000000e003";

const CLAIM: &str = "insert into workflow_command_receipt (command_id, outcome, aggregate_id, message, actor_app_user_id)
                     values ($1, 'pending', null, null, null)
                     on conflict (command_id) do nothing
                     returning command_id";

const MERGE: &str = "insert into forge_workflow_evidence (process_instance_id, story_id, qa_passed, candidate_sha)
                     values ($1::uuid, $2, $3, $4)
                     on conflict (process_instance_id) do update set
                        qa_passed = coalesce(excluded.qa_passed, forge_workflow_evidence.qa_passed),
                        candidate_sha = coalesce(excluded.candidate_sha, forge_workflow_evidence.candidate_sha),
                        updated_at = now()";

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (APP_ENV=dev): runs only against the disposable DEV branch; PROD is refused"]
async fn db_transaction_003__receipt_evidence() {
    let test_db = TestDatabase::connect_from_env().await.expect(
        "a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)",
    );
    let mut tx = test_db.begin().await.expect("begin transaction");
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status)
         values ($1, 'TEST', 'receipt + evidence', 'P3', 'Planned')",
    )
    .bind(STORY)
    .execute(&mut *tx.connection())
    .await
    .expect("insert a story for the evidence to name");
    // Evidence belongs to a process instance (foreign key), so the unit has one: a throwaway instance of the Forge
    // definition, inside this transaction.
    sqlx::query(
        "insert into process_instances (id, definition_id, status, subject_type, subject_id)
         select $1::uuid, id, 'active', 'story', $2 from process_definitions where key = 'FORGE_SDLC' limit 1",
    )
    .bind(INSTANCE)
    .bind(STORY)
    .execute(&mut *tx.connection())
    .await
    .expect("insert a process instance for the evidence to belong to");

    // The claim: first wins, second is refused while it is pending.
    let first: Option<String> = sqlx::query_scalar(CLAIM)
        .bind(RECEIPT)
        .fetch_optional(&mut *tx.connection())
        .await
        .expect("first claim");
    assert_eq!(
        first.as_deref(),
        Some(RECEIPT),
        "the first claimant owns the unit"
    );
    let second: Option<String> = sqlx::query_scalar(CLAIM)
        .bind(RECEIPT)
        .fetch_optional(&mut *tx.connection())
        .await
        .expect("second claim");
    assert_eq!(
        second, None,
        "a second claimant is refused while the receipt is pending"
    );

    // Evidence: the first unit establishes QA passed; a later one that knows only a candidate leaves it standing.
    sqlx::query(MERGE)
        .bind(INSTANCE)
        .bind(STORY)
        .bind(Some(true))
        .bind(None::<String>)
        .execute(&mut *tx.connection())
        .await
        .expect("first evidence merge");
    sqlx::query(MERGE)
        .bind(INSTANCE)
        .bind(STORY)
        .bind(None::<bool>)
        .bind(Some("0123456789abcdef0123456789abcdef01234567"))
        .execute(&mut *tx.connection())
        .await
        .expect("second evidence merge");
    let (qa_passed, candidate, rows): (Option<bool>, Option<String>, i64) = sqlx::query_as(
        "select max(qa_passed::int)::int = 1, max(candidate_sha), count(*) from forge_workflow_evidence where story_id = $1",
    )
    .bind(STORY)
    .fetch_one(&mut *tx.connection())
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
    assert_eq!(
        candidate.as_deref(),
        Some("0123456789abcdef0123456789abcdef01234567"),
        "and it adds what it knows"
    );

    // Finalize: the receipt stops being pending and is the proof the unit ran.
    let finalized = sqlx::query(
        "update workflow_command_receipt set outcome = 'Success', message = 'applied', updated_at = now() where command_id = $1",
    )
    .bind(RECEIPT)
    .execute(&mut *tx.connection())
    .await
    .expect("finalize the receipt");
    assert_eq!(
        finalized.rows_affected(),
        1,
        "finalize touches exactly the claimed receipt"
    );
    let outcome: String =
        sqlx::query_scalar("select outcome from workflow_command_receipt where command_id = $1")
            .bind(RECEIPT)
            .fetch_one(&mut *tx.connection())
            .await
            .expect("read the receipt");
    assert_eq!(outcome, "Success");
    tx.rollback().await.expect("rollback");

    // Nothing survived the rollback: the receipt and its evidence go together or not at all.
    let receipts: i64 =
        sqlx::query_scalar("select count(*) from workflow_command_receipt where command_id = $1")
            .bind(RECEIPT)
            .fetch_one(test_db.database().pool())
            .await
            .expect("count receipts after rollback");
    let evidence: i64 =
        sqlx::query_scalar("select count(*) from forge_workflow_evidence where story_id = $1")
            .bind(STORY)
            .fetch_one(test_db.database().pool())
            .await
            .expect("count evidence after rollback");
    assert_eq!(
        (receipts, evidence),
        (0, 0),
        "a rolled-back unit leaves neither a receipt nor evidence"
    );
}
