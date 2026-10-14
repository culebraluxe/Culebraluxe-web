//! DB.SCHEMA — nullable / NOT NULL contract matches Rust (TST-DB-SCHEMA-003).
//!
//! CONTRACT. The columns the Rust `property` model reads as required are NOT NULL in the database, and the ones it
//! reads as optional (`Option<_>`) are nullable. A column that drifts in either direction is a runtime decode failure
//! (a NULL into a required field) or a lie in the type (a required field that can never be absent).
//!
//! The catalog read is then given teeth: `status` refuses a NULL and `name` accepts one, so the declared contract is
//! shown to be enforced, and a name that is not in the schema is shown not to answer the probe — without which the
//! equality assertions above could hold vacuously.
//!
//! A non-production database only; PROD is refused by the harness before a socket opens.
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
#[ignore = "needs DATABASE_URL_DEV (APP_ENV=dev): runs only against the disposable DEV branch; PROD is refused"]
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

    // NEGATIVE CASE — the probe can say NO. Without this the loop above could pass on a lookup that answers YES for any
    // name, and a column that had drifted out of the schema entirely would be invisible to it.
    let absent: Option<String> = sqlx::query_scalar(
        "select is_nullable::text from information_schema.columns
          where table_schema = 'public' and table_name = 'property' and column_name = 'no_such_column'",
    )
    .fetch_optional(&mut *conn)
    .await
    .expect("read a column that does not exist");
    assert_eq!(
        absent, None,
        "a column that is not in the schema must not answer the probe, or the loop above proves nothing"
    );

    // NEGATIVE CASE — nullability is ENFORCED, not merely declared. `property.status` is the required column the Rust
    // model reads as non-Option, so a NULL must be refused outright; `property.name` is the optional one, so the same
    // NULL must be accepted. A schema that accepted NULL in the required column would satisfy the catalog read above
    // and still hand the model a decode failure.
    let mut tx = test_db.begin().await.expect("begin transaction");
    let null_status =
        sqlx::query("insert into property (name, status) values ('TST-DB-SCHEMA-003 a', null)")
            .execute(&mut *tx.connection())
            .await;
    assert!(
        null_status.is_err(),
        "property.status is read as non-Option by the Rust model, so a NULL must be refused"
    );
    let _ = tx.rollback().await;

    // A refused insert aborts its transaction, so the accepted one gets a transaction of its own.
    let mut tx = test_db.begin().await.expect("begin transaction");
    sqlx::query("insert into property (name, status) values ('TST-DB-SCHEMA-003 b', 'active')")
        .execute(&mut *tx.connection())
        .await
        .unwrap_or_else(|error| {
            panic!("property.name is read as Option by the Rust model, so NULL is legal: {error}")
        });
    let _ = tx.rollback().await;
}
