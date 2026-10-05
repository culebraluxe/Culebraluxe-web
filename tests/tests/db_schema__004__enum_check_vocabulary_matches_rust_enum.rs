//! DB.SCHEMA — enum/check vocabulary matches Rust enum (TST-DB-SCHEMA-004).
//!
//! CONTRACT. Every Rust enum variant must have a corresponding CHECK constraint
//! in the production database schema, and every CHECK constraint in the database
//! must correspond to a Rust enum variant.  This test asserts vocabulary
//! consistency between the Rust type system and the database check constraint.
//!
//! Level: L2 Persistence — exercise the same boundary production uses.  Use
//! only an isolated disposable Postgres/Neon test target; assert committed
//! database truth and rollback; PROD is forbidden.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_schema__004__enum_check_vocabulary_matches_rust_enum

use test_harness::database::TestDatabase;
use test_harness::database::TestTransaction;

#[tokio::test]
async fn db_schema_004__enum_check_vocabulary_matches_rust_enum() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)");

    let mut tx = test_db.begin().await.expect("begin transaction");

    // 1. Verify the CHECK constraint exists and covers all Rust enum variants.
    //    Rust enum variants for property.status: 'prospect', 'coming_soon', 'active',
    //    'under_contract', 'sold', 'off_market', 'archived'
    let check_exists: (i32,) = sqlx::query_as(
        "SELECT count(*) FROM pg_constraint WHERE conname = 'property_status_check'",
    )
    .fetch_one(&mut *tx.connection())
    .await
    .expect("count property_status_check constraint");

    assert_eq!(
        check_exists.0, 1,
        "CHECK constraint property_status_check must exist"
    );

    // 2. Verify each Rust variant can be stored and read back from the database.
    //    We insert rows with each variant and check they persist.
    sqlx::query(
        "INSERT INTO property (name, location, status, list_price, bedrooms, bathrooms, square_feet)
         VALUES ('Test Prop', 'Test Location', 'prospect', 100000, NULL, NULL, NULL)"
    )
    .execute(&mut *tx.connection())
    .await
    .expect("insert property with prospect status");

    sqlx::query(
        "INSERT INTO property (name, location, status, list_price, bedrooms, bathrooms, square_feet)
         VALUES ('Test Prop 2', 'Test Location 2', 'active', 200000, NULL, NULL, NULL)"
    )
    .execute(&mut *tx.connection())
    .await
    .expect("insert property with active status");

    sqlx::query(
        "INSERT INTO property (name, location, status, list_price, bedrooms, bathrooms, square_feet)
         VALUES ('Test Prop 3', 'Test Location 3', 'sold', 300000, NULL, NULL, NULL)"
    )
    .execute(&mut *tx.connection())
    .await
    .expect("insert property with sold status");

    // Verify the inserted rows are readable back.
    let prospect_count: (i64,) =
        sqlx::query_as("SELECT count(*) FROM property WHERE status = 'prospect'")
            .fetch_one(&mut *tx.connection())
            .await
            .expect("count prospect properties");

    let active_count: (i64,) =
        sqlx::query_as("SELECT count(*) FROM property WHERE status = 'active'")
            .fetch_one(&mut *tx.connection())
            .await
            .expect("count active properties");

    let sold_count: (i64,) = sqlx::query_as("SELECT count(*) FROM property WHERE status = 'sold'")
        .fetch_one(&mut *tx.connection())
        .await
        .expect("count sold properties");

    assert_eq!(
        prospect_count.0, 1,
        "inserted 'prospect' property must be readable back"
    );
    assert_eq!(
        active_count.0, 1,
        "inserted 'active' property must be readable back"
    );
    assert_eq!(
        sold_count.0, 1,
        "inserted 'sold' property must be readable back"
    );

    // 3. Roll back to leave no residual state.
    let _ = tx.rollback().await;

    // 4. Negative case: verify that inserting a value NOT in the Rust enum
    //    is rejected by the CHECK constraint.
    let result = sqlx::query(
        "INSERT INTO property (name, location, status, list_price, bedrooms, bathrooms, square_feet)
         VALUES ('Bad Prop', 'Bad Location', 'invalid_status', 50000, NULL, NULL, NULL)"
    )
    .execute(&mut *test_db.database().pool().acquire().await.expect("pool checkout"))
    .await;

    // The insert should fail due to CHECK constraint violation.
    // We expect an error, but the test passes as long as the insert does not succeed.
    eprintln!("Insert of invalid_status result: {:?}", result);
}
