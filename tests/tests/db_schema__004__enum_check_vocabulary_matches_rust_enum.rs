//! DB.SCHEMA — enum/CHECK vocabulary matches the Rust vocabulary (TST-DB-SCHEMA-004).
//!
//! CONTRACT. The statuses the Rust service accepts for a property (`validate_admin_save`'s `STATUSES`,
//! `web/src/properties/mod.rs`) are exactly the statuses the database CHECK `property_status_check` accepts. A status
//! Rust allows and the database refuses is a 500 on save; a status the database allows and Rust refuses is a value no
//! screen can ever write or clear.
//!
//! Both sides are READ: the Rust list is parsed out of its source (it is a function-local `const`, not importable), the
//! database list out of the constraint definition. Then every Rust status is written to a real row inside a transaction
//! that is rolled back, so the CHECK is exercised and not only read.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_schema__004__enum_check_vocabulary_matches_rust_enum

use std::collections::BTreeSet;
use std::path::PathBuf;

use test_harness::database::TestDatabase;

/// Every single-quoted word in `text`, in order.
fn quoted(text: &str) -> Vec<String> {
    text.split('\'')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

/// The strings of `const STATUSES: &[&str] = &[ ... ];` in `validate_admin_save`.
fn rust_property_statuses() -> BTreeSet<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/src/properties/mod.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let start = source
        .find("const STATUSES: &[&str] = &[")
        .expect("validate_admin_save declares its STATUSES");
    let body = &source[start..];
    let body = &body[..body.find("];").expect("the STATUSES list closes")];
    body.split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (APP_ENV=dev): runs only against the disposable DEV branch; PROD is refused"]
async fn db_schema_004__enum_check_vocabulary_matches_rust_enum() {
    let test_db = TestDatabase::connect_from_env().await.expect(
        "a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)",
    );

    let rust = rust_property_statuses();
    assert!(
        rust.len() >= 5,
        "the Rust status list was not read: {rust:?}"
    );

    let definition: String = sqlx::query_scalar(
        "select pg_get_constraintdef(oid) from pg_constraint
          where conname = 'property_status_check' and conrelid = 'property'::regclass",
    )
    .fetch_one(test_db.database().pool())
    .await
    .expect("property_status_check exists");
    let database: BTreeSet<String> = quoted(&definition)
        .into_iter()
        .filter(|word| !word.is_empty())
        .collect();

    assert_eq!(
        rust, database,
        "the Rust status list and property_status_check must name the same statuses\n  rust:     {rust:?}\n  database: {database:?}"
    );

    // Exercise the constraint itself: every Rust status is storable, and a status outside the vocabulary is refused.
    let mut tx = test_db.begin().await.expect("begin transaction");
    for status in &rust {
        sqlx::query("insert into property (name, status) values ($1, $2)")
            .bind(format!("TST-DB-SCHEMA-004 {status}"))
            .bind(status)
            .execute(&mut *tx.connection())
            .await
            .unwrap_or_else(|error| {
                panic!("status {status:?} is in the vocabulary but was refused: {error}")
            });
    }
    let stored: i64 =
        sqlx::query_scalar("select count(*) from property where name like 'TST-DB-SCHEMA-004 %'")
            .fetch_one(&mut *tx.connection())
            .await
            .expect("count the rows just written");
    assert_eq!(stored as usize, rust.len(), "one row per Rust status");
    tx.rollback().await.expect("rollback leaves no row behind");

    // A refused insert aborts its transaction, so the negative case gets a transaction of its own.
    let mut tx = test_db.begin().await.expect("begin transaction");
    let refused = sqlx::query(
        "insert into property (name, status) values ('TST-DB-SCHEMA-004 bad', 'not_a_status')",
    )
    .execute(&mut *tx.connection())
    .await;
    assert!(
        refused.is_err(),
        "a status outside the vocabulary must be refused by the CHECK"
    );
    let _ = tx.rollback().await;
}
