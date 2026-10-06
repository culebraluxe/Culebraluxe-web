//! DB.SCHEMA — nullable / NOT NULL contract matches Rust (TST-DB-SCHEMA-003).
//!
//! CONTRACT. The columns the Rust `property` model reads as required are NOT NULL in the database, and the ones it
//! reads as optional (`Option<_>`) are nullable. A column that drifts in either direction is a runtime decode failure
//! (a NULL into a required field) or a lie in the type (a required field that can never be absent).
//!
//! Read from `information_schema.columns`; no row is written. A non-production database only; PROD is refused by the
//! harness before a socket opens.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_schema__003__nullable_non_null_contract_matches_rust

use test_harness::database::TestDatabase;

/// `(column, nullable)` as the Rust `property` model treats each one.
const PROPERTY_COLUMNS: [(&str, bool); 9] = [
    ("id", false),
    ("status", false),
    ("created_at", false),
    ("updated_at", false),
    ("name", true),
    ("list_price", true),
    ("bedrooms", true),
    ("bathrooms", true),
    ("square_feet", true),
];

#[tokio::test]
async fn db_schema_003__nullable_non_null_contract_matches_rust() {
    let test_db = TestDatabase::connect_from_env().await.expect(
        "a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)",
    );
    let mut conn = test_db
        .database()
        .pool()
        .acquire()
        .await
        .expect("pool checkout");

    for (column, nullable) in PROPERTY_COLUMNS {
        let is_nullable: String = sqlx::query_scalar(
            "select is_nullable::text from information_schema.columns
              where table_schema = 'public' and table_name = 'property' and column_name = $1",
        )
        .bind(column)
        .fetch_one(&mut *conn)
        .await
        .unwrap_or_else(|error| panic!("property.{column} is not in the schema: {error}"));
        assert_eq!(
            is_nullable == "YES",
            nullable,
            "property.{column}: the Rust model expects nullable={nullable}, the database says is_nullable={is_nullable}"
        );
    }

    // The status vocabulary is enforced by a CHECK, not by convention (004 pins its contents).
    let checks: i64 = sqlx::query_scalar(
        "select count(*) from pg_constraint
          where conname = 'property_status_check' and conrelid = 'property'::regclass and contype = 'c'",
    )
    .fetch_one(&mut *conn)
    .await
    .expect("count property_status_check");
    assert_eq!(
        checks, 1,
        "property.status must be guarded by property_status_check"
    );
}
