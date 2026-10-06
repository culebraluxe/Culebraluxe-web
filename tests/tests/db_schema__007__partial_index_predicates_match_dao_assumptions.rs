//! DB.SCHEMA — partial-index predicates match the DAO's assumptions (TST-DB-SCHEMA-007).
//!
//! CONTRACT. A partial index only serves, or only constrains, the rows its WHERE predicate admits. The code assumes the
//! predicates below; if one is loosened the "unique when present" rule silently becomes "unique always" (and breaks
//! every row without the value), and if one is tightened the index stops covering the rows a query reads.
//!
//!   * `property.slug` / `regrid_ll_uuid`: unique ONLY WHERE the value is not null — most properties have neither;
//!   * `property.is_published`: the publication index covers `is_published = true`, the public site's read;
//!   * `agent_work_item` one-open-per-story: the OPEN states only (`Ready/Claimed/Running/Paused`), so a finished item
//!     never blocks the next run, and serial rows only (`parallel_group_id is null`).
//!
//! Read from `pg_indexes`, then the null-slug rule is exercised in a transaction that is rolled back.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_schema__007__partial_index_predicates_match_dao_assumptions

use test_harness::database::TestDatabase;

/// `(index, predicate fragments its definition must contain)`.
const PARTIAL_INDEXES: [(&str, &[&str]); 5] = [
    ("idx_property_slug_unique", &["WHERE", "slug IS NOT NULL"]),
    (
        "idx_property_regrid_ll_uuid_unique",
        &["WHERE", "regrid_ll_uuid IS NOT NULL"],
    ),
    (
        "idx_property_publication",
        &["WHERE", "is_published = true"],
    ),
    (
        "agent_work_item_one_serial_active_per_story",
        &[
            "'Ready'",
            "'Claimed'",
            "'Running'",
            "'Paused'",
            "parallel_group_id IS NULL",
        ],
    ),
    (
        "agent_work_item_one_parallel_slot",
        &[
            "'Ready'",
            "'Claimed'",
            "'Running'",
            "'Paused'",
            "parallel_group_id IS NOT NULL",
        ],
    ),
];

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (APP_ENV=dev): runs only against the disposable DEV branch; PROD is refused"]
async fn db_schema_007__partial_index_predicates_match_dao_assumptions() {
    let test_db = TestDatabase::connect_from_env().await.expect(
        "a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)",
    );
    let pool = test_db.database().pool();

    for (index, fragments) in PARTIAL_INDEXES {
        let definition: String = sqlx::query_scalar(
            "select indexdef::text from pg_indexes where schemaname = 'public' and indexname = $1",
        )
        .bind(index)
        .fetch_optional(pool)
        .await
        .expect("read pg_indexes")
        .unwrap_or_else(|| panic!("partial index {index} must exist"));
        for fragment in fragments {
            assert!(
                definition.contains(fragment),
                "{index}: the predicate the code assumes ({fragment}) is missing from its definition: {definition}"
            );
        }
    }

    // The null-slug rule, exercised: two properties with NO slug coexist, two with the SAME slug do not.
    let mut tx = test_db.begin().await.expect("begin transaction");
    for name in ["TST-DB-SCHEMA-007 a", "TST-DB-SCHEMA-007 b"] {
        sqlx::query("insert into property (name, slug) values ($1, null)")
            .bind(name)
            .execute(&mut *tx.connection())
            .await
            .expect("properties without a slug never collide");
    }
    sqlx::query(
        "insert into property (name, slug) values ('TST-DB-SCHEMA-007 c', 'tst-db-schema-007')",
    )
    .execute(&mut *tx.connection())
    .await
    .expect("the first property with a slug is accepted");
    let duplicate = sqlx::query(
        "insert into property (name, slug) values ('TST-DB-SCHEMA-007 d', 'tst-db-schema-007')",
    )
    .execute(&mut *tx.connection())
    .await;
    assert!(duplicate.is_err(), "the same slug twice must be refused");
    let _ = tx.rollback().await;
}
