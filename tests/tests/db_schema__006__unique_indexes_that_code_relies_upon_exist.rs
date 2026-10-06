//! DB.SCHEMA — the unique indexes the code relies upon exist and enforce (TST-DB-SCHEMA-006).
//!
//! CONTRACT. Code that dedupes by writing and catching a conflict (`person_identity`, the property identifiers, the
//! one-open-work-item-per-story rule the dispatch trigger leans on) is only correct while the unique index behind it
//! exists, covers exactly those columns, and refuses the duplicate. Each is read from `pg_indexes` by NAME and COLUMNS,
//! and the two the engine's dispatch and the property import depend on are exercised with a real duplicate inside a
//! transaction that is rolled back.
//!
//! A non-production database only; PROD is refused by the harness before a socket opens.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_schema__006__unique_indexes_that_code_relies_upon_exist

use test_harness::database::TestDatabase;

/// `(table, index, the column list it must cover)`.
const UNIQUE_INDEXES: [(&str, &str, &str); 7] = [
    (
        "property",
        "property_listing_identifier_unique",
        "(listing_identifier)",
    ),
    ("property", "idx_property_slug_unique", "(slug)"),
    (
        "property",
        "idx_property_regrid_ll_uuid_unique",
        "(regrid_ll_uuid)",
    ),
    (
        "person_identity",
        "person_identity_unique",
        "(identity_type, identity_value)",
    ),
    (
        "agent_work_item",
        "agent_work_item_one_serial_active_per_story",
        "(story_id)",
    ),
    (
        "agent_work_item",
        "agent_work_item_one_parallel_slot",
        "(story_id, parallel_group_id, parallel_slot)",
    ),
    (
        "workflow_command_receipt",
        "workflow_command_receipt_pkey",
        "(command_id)",
    ),
];

#[tokio::test]
async fn db_schema_006__unique_indexes_that_code_relies_upon_exist() {
    let test_db = TestDatabase::connect_from_env().await.expect(
        "a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)",
    );
    let pool = test_db.database().pool();

    // 1. Every index exists, is UNIQUE, and covers the columns the code assumes.
    for (table, index, columns) in UNIQUE_INDEXES {
        let definition: Option<String> = sqlx::query_scalar(
            "select indexdef::text from pg_indexes
              where schemaname = 'public' and tablename = $1 and indexname = $2",
        )
        .bind(table)
        .bind(index)
        .fetch_optional(pool)
        .await
        .expect("read pg_indexes");
        let definition =
            definition.unwrap_or_else(|| panic!("unique index {index} on {table} must exist"));
        assert!(
            definition.contains("UNIQUE"),
            "{index} must be UNIQUE: {definition}"
        );
        assert!(
            definition.contains(columns),
            "{index} must cover {columns}: {definition}"
        );
    }

    // 2. The property identifier index refuses a duplicate. (Its own transaction: a refused insert aborts one.)
    let mut tx = test_db.begin().await.expect("begin transaction");
    sqlx::query("insert into property (name, listing_identifier) values ('TST-DB-SCHEMA-006 a', 'tst-db-schema-006-dup')")
        .execute(&mut *tx.connection())
        .await
        .expect("the first property with an identifier is accepted");
    let duplicate = sqlx::query(
        "insert into property (name, listing_identifier) values ('TST-DB-SCHEMA-006 b', 'tst-db-schema-006-dup')",
    )
    .execute(&mut *tx.connection())
    .await;
    assert!(
        duplicate.is_err(),
        "a second property with the same listing_identifier must be refused"
    );
    let _ = tx.rollback().await;

    // 3. At most ONE open serial work item per story: the rule the dispatch trigger's `on conflict do nothing` leans on.
    let mut tx = test_db.begin().await.expect("begin transaction");
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status)
         values ('TST-DB-SCHEMA-006', 'TEST', 'unique index probe', 'P3', 'Planned')",
    )
    .execute(&mut *tx.connection())
    .await
    .expect("insert a story");
    sqlx::query(
        "insert into agent_work_item (story_id, state) values ('TST-DB-SCHEMA-006', 'Ready')",
    )
    .execute(&mut *tx.connection())
    .await
    .expect("the first open serial work item is accepted");
    let second = sqlx::query(
        "insert into agent_work_item (story_id, state) values ('TST-DB-SCHEMA-006', 'Ready')",
    )
    .execute(&mut *tx.connection())
    .await;
    assert!(
        second.is_err(),
        "a second open serial work item for one story must be refused by agent_work_item_one_serial_active_per_story"
    );
    let _ = tx.rollback().await;
}
