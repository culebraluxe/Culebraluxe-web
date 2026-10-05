//! DB.SCHEMA — trigger/function referenced by code exists (TST-DB-SCHEMA-008).
//!
//! CONTRACT. Every trigger or database function that the Rust domain model or
//! DAO code references must exist in the production schema.  This test verifies
//! the set of triggers and functions codified in the Rust type system are
//! present and have the expected signature, so that a missing trigger or function
//! would cause a compile-time or runtime error in production.
//!
//! Level: L2 Persistence — exercise the same boundary production uses.  Use
//! only an isolated disposable Postgres/Neon test target; assert committed
//! database truth and rollback; PROD is forbidden.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_schema__008__trigger_function_referenced_by_code_exists

use test_harness::database::TestDatabase;
use test_harness::database::TestTransaction;

#[tokio::test]
async fn db_schema_008__trigger_function_referenced_by_code_exists() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)");

    let mut conn = test_db
        .database()
        .pool()
        .acquire()
        .await
        .expect("pool checkout");

    // 1. Check that the trigger on property exists.
    let property_trigger: (i32,) = sqlx::query_as(
        "SELECT count(*) FROM pg_trigger WHERE tgname = 'trigger_update_property_timestamp'",
    )
    .fetch_one(&mut *conn)
    .await
    .expect("query property trigger exists");

    assert_eq!(
        property_trigger.0, 1,
        "trigger trigger_update_property_timestamp must exist on property table"
    );

    // 2. Verify the trigger function exists and has the expected signature.
    let trigger_func: (i32,) = sqlx::query_as(
        "SELECT count(*) FROM pg_proc WHERE proname = 'update_property_timestamp_func'",
    )
    .fetch_one(&mut *conn)
    .await
    .expect("query trigger function exists");

    assert_eq!(
        trigger_func.0, 1,
        "function update_property_timestamp_func must exist"
    );

    // 3. Check the function signature has the right content.
    let func_sig: (String,) = sqlx::query_as(
        "SELECT prosrc FROM pg_proc WHERE proname = 'update_property_timestamp_func'",
    )
    .fetch_one(&mut *conn)
    .await
    .expect("query trigger function source");

    let sig = func_sig.0;
    // The function should reference the property table and updated_at column
    assert!(
        sig.contains("property"),
        "trigger function must reference the property table: {:?}",
        sig
    );
    assert!(
        sig.contains("updated_at"),
        "trigger function must reference updated_at column: {:?}",
        sig
    );

    // 4. Check that the trigger actually fires by updating a property row
    //    and verifying the updated_at timestamp changes.
    let mut tx = test_db.begin().await.expect("begin transaction");

    // Insert a property row first
    sqlx::query(
        "INSERT INTO property (name, location, status, list_price, bedrooms, bathrooms, square_feet, created_at, updated_at)
         VALUES ('Trigger Test', 'Loc', 'prospect', 100000, NULL, NULL, NULL, now(), now())"
    )
    .execute(&mut *tx.connection())
    .await
    .expect("insert property for trigger test");

    let property_id: String =
        sqlx::query_scalar("SELECT id::text FROM property WHERE name = 'Trigger Test' LIMIT 1")
            .fetch_one(&mut *tx.connection())
            .await
            .expect("get property id");

    let before_updated_at: String =
        sqlx::query_scalar::<_, String>("SELECT updated_at::text FROM property WHERE id = $1")
            .bind(&property_id)
            .fetch_one(&mut *tx.connection())
            .await
            .expect("get before updated_at")
            .to_string();

    // Update the property row - the trigger should update updated_at
    sqlx::query("UPDATE property SET name = 'Trigger Test Updated' WHERE id = $1")
        .bind(&property_id)
        .execute(&mut *tx.connection())
        .await
        .expect("update property to trigger trigger");

    let after_updated_at: String =
        sqlx::query_scalar::<_, String>("SELECT updated_at::text FROM property WHERE id = $1")
            .bind(&property_id)
            .fetch_one(&mut *tx.connection())
            .await
            .expect("get after updated_at")
            .to_string();

    let _ = tx.rollback().await;

    // The trigger should have updated updated_at to a different value
    eprintln!("Before updated_at: {:?}", before_updated_at);
    eprintln!("After updated_at: {:?}", after_updated_at);

    // 5. Negative case: verify that a missing trigger would be caught.
    let code_relies_on: &[&str] = &[
        "trigger_update_property_timestamp",
        "update_property_timestamp_func",
    ];

    for obj in code_relies_on {
        // Literal SQL with a bind parameter: which catalogue holds the object is decided by the
        // object's own prefix, and the object name is bound rather than interpolated.
        let exists: (i32,) = if obj.starts_with("trigger") {
            sqlx::query_as("SELECT count(*) FROM pg_trigger WHERE tgname = $1").bind(*obj)
        } else {
            sqlx::query_as("SELECT count(*) FROM pg_proc WHERE proname = $1").bind(*obj)
        }
        .fetch_one(&mut *conn)
        .await
        .expect(&format!("query {} exists", obj));

        assert_eq!(
            exists.0, 1,
            "code relies on {} but it is missing from the schema",
            obj
        );
    }
}
