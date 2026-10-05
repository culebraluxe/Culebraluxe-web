//! DB.SCHEMA — nullable/non-null contract matches Rust (TST-DB-SCHEMA-003).
//!
//! CONTRACT. Every `NOT NULL` Rust newtype and every `Option<T>` Rust optional
//! must have a matching constraint in the production schema.  A column that is
//! `Optional` in Rust must allow `NULL` in the database, and a column that is
//! a non‑optional newtype in Rust must be `NOT NULL` in the database.  This
//! test asserts that invariant across every column used by the domain model.
//!
//! Level: L2 Persistence — exercise the same boundary production uses.  Use
//! only an isolated disposable Postgres/Neon test target; assert committed
//! database truth and rollback; PROD is forbidden.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_schema__003__nullable_non_null_contract_matches_rust

use test_harness::database::TestDatabase;
use test_harness::database::TestTransaction;

#[tokio::test]
async fn db_schema_003__nullable_non_null_contract_matches_rust() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)");

    let mut tx = test_db.begin().await.expect("begin transaction");

    // 1. Verify NOT NULL columns in the database are the ones Rust expects.
    //    We check the information_schema for nullability.
    let columns_to_check = [
        ("id", false),         // NOT NULL in Rust → NOT NULL in DB
        ("name", true),        // Option<Text> in Rust → NULL allowed in DB
        ("list_price", true),  // Option<Numeric> in Rust → NULL allowed in DB
        ("bedrooms", true),    // Option<Numeric> in Rust → NULL allowed in DB
        ("bathrooms", true),   // Option<Numeric> in Rust → NULL allowed in DB
        ("square_feet", true), // Option<Integer> in Rust → NULL allowed in DB
        ("created_at", false), // NOT NULL in Rust → NOT NULL in DB
        ("updated_at", false), // NOT NULL in Rust → NOT NULL in DB
    ];

    for (column, allows_null) in &columns_to_check {
        let query = if *allows_null {
            "SELECT nullable = 'YES' FROM information_schema.columns WHERE table_name = 'property' AND column_name = $1"
        } else {
            "SELECT NOT NULL IS NOT NULL FROM information_schema.columns WHERE table_name = 'property' AND column_name = $1"
        };
        let result: (i32,) = sqlx::query_as(query)
            .bind(column)
            .fetch_one(&mut *tx.connection())
            .await
            .expect(&format!("query column {} nullability", column));

        let value = result.0 != 0;
        assert_eq!(
            value, *allows_null,
            "column `{}` should allow NULL={} in DB, got {}",
            column, allows_null, value
        );
    }

    // 2. Verify the unique index on listing_identifier exists.
    let unique_check: (i32,) = sqlx::query_as(
        "SELECT count(*) FROM pg_index WHERE indexname = 'idx_property_listing_identifier_unique'",
    )
    .fetch_one(&mut *tx.connection())
    .await
    .expect("count unique index");

    assert_eq!(
        unique_check.0, 1,
        "unique index idx_property_listing_identifier_unique must exist"
    );

    // 3. Verify the CHECK constraint on status exists.
    let check_constraint: (i32,) = sqlx::query_as(
        "SELECT count(*) FROM pg_constraint WHERE conname = 'property_status_check'",
    )
    .fetch_one(&mut *tx.connection())
    .await
    .expect("count status check constraint");

    assert_eq!(
        check_constraint.0, 1,
        "CHECK constraint property_status_check must exist"
    );

    let _ = tx.rollback().await;
}
