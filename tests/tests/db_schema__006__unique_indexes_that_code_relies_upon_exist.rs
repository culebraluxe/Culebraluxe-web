//! DB.SCHEMA — unique indexes that code relies upon exist (TST-DB-SCHEMA-006).
//!
//! CONTRACT. Every unique index that the Rust domain model or DAO code depends
//! upon must exist in the production schema.  This test verifies the set of
//! unique indexes codified in the Rust type system are present in the database,
//! and that they enforce uniqueness as expected.  The test exercises the
//! production boundary using an isolated disposable test target; PROD is
//! forbidden.
//!
//! Level: L2 Persistence — exercise the same boundary production uses.  Use
//! only an isolated disposable Postgres/Neon test target; assert committed
//! database truth and rollback; PROD is forbidden.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_schema__006__unique_indexes_that_code_relies_upon_exist

use test_harness::database::TestDatabase;

#[tokio::test]
async fn db_schema_006__unique_indexes_that_code_relies_upon_exist() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)");

    let mut conn = test_db.database().pool().acquire().await.expect("pool checkout");

    // 1. The property table has a unique index on listing_identifier.
    let listing_id_unique: (i32,) = sqlx::query_as(
        "SELECT count(*) FROM pg_index WHERE indexname = 'idx_property_listing_identifier_unique'"
    )
    .fetch_one(&mut conn)
    .await
    .expect("query idx_property_listing_identifier_unique exists");

    assert_eq!(listing_id_unique.0, 1, "unique index idx_property_listing_identifier_unique must exist on property.listing_identifier");

    // 2. The property table has a unique constraint on name (business rule).
    let name_unique: (i32,) = sqlx::query_as(
        "SELECT count(*) FROM pg_constraint WHERE conname = 'property_name_unique'"
    )
    .fetch_one(&mut conn)
    .await
    .expect("query property_name_unique constraint exists");

    assert_eq!(name_unique.0, 1, "unique constraint on property.name must exist");

    // 3. The person_identity table has a unique constraint on (identity_type, identity_value).
    let person_identity_unique: (i32,) = sqlx::query_as(
        "SELECT count(*) FROM pg_constraint WHERE conname = 'person_identity_unique'"
    )
    .fetch_one(&mut conn)
    .await
    .expect("query person_identity_unique constraint exists");

    assert_eq!(person_identity_unique.0, 1, "unique constraint on person_identity (identity_type, identity_value) must exist");

    // 4. Verify the unique index actually enforces uniqueness.
    let tx = test_db.begin().await.expect("begin transaction");

    let result1 = sqlx::query(
        "INSERT INTO property (name, location, status, list_price, bedrooms, bathrooms, square_feet, listing_identifier)
         VALUES ('Dup Prop', 'Loc', 'prospect', 100000, NULL, NULL, NULL, 'dup-identifier')"
    )
    .execute(&*tx.connection())
    .await;

    assert!(
        result1.is_ok(),
        "first insert of property with listing_identifier should succeed"
    );

    let result2 = sqlx::query(
        "INSERT INTO property (name, location, status, list_price, bedrooms, bathrooms, square_feet, listing_identifier)
         VALUES ('Dup Prop 2', 'Loc 2', 'prospect', 200000, NULL, NULL, NULL, 'dup-identifier')"
    )
    .execute(&*tx.connection())
    .await;

    let _ = tx.rollback().await;

    assert!(
        result2.is_err(),
        "second insert of property with same listing_identifier must fail due to unique constraint"
    );

    // 5. Verify that code relies on these indexes exist.
    let code_relies_on: &[&str] = &["idx_property_listing_identifier_unique", "property_name_unique", "person_identity_unique"];

    for idx in code_relies_on {
        let exists: (i32,) = sqlx::query_as(
            &format!("SELECT count(*) FROM pg_index WHERE indexname = '{idx}'")
        )
        .fetch_one(&mut conn)
        .await
        .expect(&format!("query index {} exists", idx));

        assert_eq!(exists.0, 1, "code relies on unique index {} but it is missing from the schema", idx);
    }
}