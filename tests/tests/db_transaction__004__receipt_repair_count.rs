//! DB.TRANSACTION — receipt + repair count (TST-DB-TRANSACTION-004).
//!
//! CONTRACT. The repair budget is a column on the STORY (`storyboard_story.forge_repair_attempts`, migration 114), not
//! a counter in a process, so it survives the one-process-per-dispatch engine and is the same number for every reader:
//!
//!   * a completion of `repair_smith` / `fast_repair_smith` increments it by exactly one per application;
//!   * the QA failure route READS it (`ForgeEngineDao::story_repair_counts`), and a fresh instance or a story reset
//!     returns it to zero (`reset_forge_attempts`) — a count that is only ever written would never stop a loop;
//!   * it is part of the unit: a rolled-back unit leaves the count where it was.
//!
//! The statements are the DAO's, run inside a transaction that is rolled back. A non-production database only.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_transaction__004__receipt_repair_count

use test_harness::database::TestDatabase;

const STORY: &str = "TST-DB-TRANSACTION-004";

const INCREMENT: &str = "update storyboard_story
                            set forge_repair_attempts = coalesce(forge_repair_attempts, 0) + 1
                          where id = $1";
const READ: &str = "select coalesce(forge_repair_attempts, 0), coalesce(forge_replan_attempts, 0)
                      from storyboard_story where id = $1";
const RESET: &str = "update storyboard_story set forge_repair_attempts = 0, forge_replan_attempts = 0, updated_at = now()
                      where id = $1 and (forge_repair_attempts <> 0 or forge_replan_attempts <> 0)";

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (APP_ENV=dev): runs only against the disposable DEV branch; PROD is refused"]
async fn db_transaction_004__receipt_repair_count() {
    let test_db = TestDatabase::connect_from_env().await.expect(
        "a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)",
    );
    let mut tx = test_db.begin().await.expect("begin transaction");
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status)
         values ($1, 'TEST', 'repair budget', 'P3', 'Planned')",
    )
    .bind(STORY)
    .execute(&mut *tx.connection())
    .await
    .expect("insert a story");

    let start: (i32, i32) = sqlx::query_as(READ)
        .bind(STORY)
        .fetch_one(&mut *tx.connection())
        .await
        .expect("read the starting budget");
    assert_eq!(start, (0, 0), "a new story has spent nothing");

    for expected in 1..=3 {
        sqlx::query(INCREMENT)
            .bind(STORY)
            .execute(&mut *tx.connection())
            .await
            .expect("count a repair");
        let (repairs, replans): (i32, i32) = sqlx::query_as(READ)
            .bind(STORY)
            .fetch_one(&mut *tx.connection())
            .await
            .expect("read the budget");
        assert_eq!(
            (repairs, replans),
            (expected, 0),
            "each application counts exactly one repair"
        );
    }

    // A fresh attempt refills it — and only touches a story that has spent something.
    let reset = sqlx::query(RESET)
        .bind(STORY)
        .execute(&mut *tx.connection())
        .await
        .expect("reset the budget");
    assert_eq!(
        reset.rows_affected(),
        1,
        "a story that spent repairs is refilled"
    );
    let refilled: (i32, i32) = sqlx::query_as(READ)
        .bind(STORY)
        .fetch_one(&mut *tx.connection())
        .await
        .expect("read the refilled budget");
    assert_eq!(
        refilled,
        (0, 0),
        "a fresh attempt starts with a full budget"
    );
    let untouched = sqlx::query(RESET)
        .bind(STORY)
        .execute(&mut *tx.connection())
        .await
        .expect("reset a full budget");
    assert_eq!(
        untouched.rows_affected(),
        0,
        "resetting a story that spent nothing writes nothing"
    );

    sqlx::query(INCREMENT)
        .bind(STORY)
        .execute(&mut *tx.connection())
        .await
        .expect("count one more repair");
    tx.rollback().await.expect("rollback");

    // The count is part of the unit: nothing of it survives a rollback (the story itself was part of it).
    let left: Option<i32> =
        sqlx::query_scalar("select forge_repair_attempts from storyboard_story where id = $1")
            .bind(STORY)
            .fetch_optional(test_db.database().pool())
            .await
            .expect("read after rollback");
    assert_eq!(
        left, None,
        "a rolled-back unit leaves no story and no count"
    );
}
