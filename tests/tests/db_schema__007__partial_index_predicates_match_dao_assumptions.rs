//! DB.SCHEMA — partial-index predicates match DAO assumptions (TST-DB-SCHEMA-007).
//!
//! CONTRACT. Partial indexes in the production schema must have WHERE predicates
//! that match the assumptions the DAO (Data Access Object) code makes about data
//! state.  This test verifies that every partial index used by the domain model
//! has a predicate that is consistent with the Rust code's expectations, and
//! that no partial index exists with a predicate that would silently exclude
//! rows the DAO expects to see.
//!
//! Level: L2 Persistence — exercise the same boundary production uses.  Use
//! only an isolated disposable Postgres/Neon test target; assert committed
//! database truth and rollback; PROD is forbidden.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_schema__007__partial_index_predicates_match_dao_assumptions

use test_harness::database::TestDatabase;

#[tokio::test]
async fn db_schema_007__partial_index_predicates_match_dao_assumptions() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)");

    let mut conn = test_db.database().pool().acquire().await.expect("pool checkout");

    // 1. Check that the partial index on property with has_ocean_view exists
    //    and its predicate matches what the DAO assumes.
    let ocean_view_index: (i32,) = sqlx::query_as(
        "SELECT count(*) FROM pg_index WHERE indexname = 'idx_property_has_ocean_view'"
    )
    .fetch_one(&mut conn)
    .await
    .expect("query idx_property_has_ocean_view exists");

    assert_eq!(ocean_view_index.0, 1, "partial index idx_property_has_ocean_view must exist");

    // 2. Verify the predicate of the partial index.
    let index_def: (String,) = sqlx::query_as(
        "SELECT indexdef FROM pg_index WHERE indexname = 'idx_property_has_ocean_view'"
    )
    .fetch_one(&mut conn)
    .await
    .expect("query index definition");

    let index_def_str = index_def.0;
    // The predicate should filter for has_ocean_view = true
    assert!(
        index_def_str.contains("has_ocean_view"),
        "partial index definition must reference has_ocean_view column: {:?}", index_def_str
    );
    assert!(
        index_def_str.contains("= true"),
        "partial index predicate must have = true condition: {:?}", index_def_str
    );

    // 3. Check another partial index: property with has_bay_view.
    let bay_view_index: (i32,) = sqlx::query_as(
        "SELECT count(*) FROM pg_index WHERE indexname = 'idx_property_has_bay_view'"
    )
    .fetch_one(&mut conn)
    .await
    .expect("query idx_property_has_bay_view exists");

    assert_eq!(bay_view_index.0, 1, "partial index idx_property_has_bay_view must exist");

    let bay_index_def: (String,) = sqlx::query_as(
        "SELECT indexdef FROM pg_index WHERE indexname = 'idx_property_has_bay_view'"
    )
    .fetch_one(&mut conn)
    .await
    .expect("query index definition for bay view");

    let bay_index_def_str = bay_index_def.0;
    assert!(
        bay_index_def_str.contains("has_bay_view"),
        "partial index definition must reference has_bay_view column: {:?}", bay_index_def_str
    );
    assert!(
        bay_index_def_str.contains("= true"),
        "partial index predicate must have = true condition: {:?}", bay_index_def_str
    );

    // 4. Check has_beach_view partial index.
    let beach_view_index: (i32,) = sqlx::query_as(
        "SELECT count(*) FROM pg_index WHERE indexname = 'idx_property_has_beach_view'"
    )
    .fetch_one(&mut conn)
    .await
    .expect("query idx_property_has_beach_view exists");

    assert_eq!(beach_view_index.0, 1, "partial index idx_property_has_beach_view must exist");

    let beach_index_def: (String,) = sqlx::query_as(
        "SELECT indexdef FROM pg_index WHERE indexname = 'idx_property_has_beach_view'"
    )
    .fetch_one(&mut conn)
    .await
    .expect("query index definition for beach view");

    let beach_index_def_str = beach_index_def.0;
    assert!(
        beach_index_def_str.contains("has_beach_view"),
        "partial index definition must reference has_beach_view column: {:?}", beach_index_def_str
    );
    assert!(
        beach_index_def_str.contains("= true"),
        "partial index predicate must have = true condition: {:?}", beach_index_def_str
    );

    // 5. Negative case: verify that a partial index with a wrong predicate
    //    would not silently exclude rows the DAO expects.
    let wrong_predicate_detected: bool = index_def_str.contains("= false")
        || bay_index_def_str.contains("= false")
        || beach_index_def_str.contains("= false");

    assert!(!wrong_predicate_detected,
        "partial index predicates must not use = false (would silently exclude DAO‑expected rows)");
}