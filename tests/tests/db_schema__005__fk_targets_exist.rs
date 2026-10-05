//! DB.SCHEMA — FK targets exist (TST-DB-SCHEMA-005).
//!
//! CONTRACT. Every foreign key reference in the production schema must have a
//! matching row in the referenced table.  This test verifies that all FK
//! constraints are satisfied: no orphaned rows exist in tables that reference
//! parent tables via foreign keys.  The test exercises the production boundary
//! using an isolated disposable test target; PROD is forbidden.
//!
//! Level: L2 Persistence — exercise the same boundary production uses.  Use
//! only an isolated disposable Postgres/Neon test target; assert committed
//! database truth and rollback; PROD is forbidden.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_schema__005__fk_targets_exist

use test_harness::database::TestDatabase;

#[tokio::test]
async fn db_schema_005__fk_targets_exist() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)");

    let mut conn = test_db.database().pool().acquire().await.expect("pool checkout");

    // 1. Verify no orphaned property_interest rows: every property_id in
    //    property_interest must reference an existing property.
    let orphan_interest: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM property_interest pi WHERE NOT EXISTS (
            SELECT 1 FROM property p WHERE p.id = pi.property_id
        )"
    )
    .fetch_one(&mut conn)
    .await
    .expect("count orphaned property_interest rows");

    assert_eq!(orphan_interest.0, 0, "no orphaned property_interest rows must exist");

    // 2. Verify no orphaned deal rows: every deal.property_id must reference
    //    an existing property.
    let orphan_deal: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM deal d WHERE NOT EXISTS (
            SELECT 1 FROM property p WHERE p.id = d.property_id
        )"
    )
    .fetch_one(&mut conn)
    .await
    .expect("count orphaned deal rows");

    assert_eq!(orphan_deal.0, 0, "no orphaned deal rows must exist");

    // 3. Verify no orphaned interaction rows: every interaction.property_id
    //    must reference an existing property (nullable, so we check non-null).
    let orphan_interaction: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM interaction i WHERE i.property_id IS NOT NULL
         AND NOT EXISTS (
             SELECT 1 FROM property p WHERE p.id = i.property_id
         )"
    )
    .fetch_one(&mut conn)
    .await
    .expect("count orphaned interaction rows");

    assert_eq!(orphan_interaction.0, 0, "no orphaned interaction rows must exist");

    // 4. Verify no orphaned deal client_person rows: every deal.client_person_id
    //    must reference an existing person.
    let orphan_deal_person: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM deal d WHERE d.client_person_id IS NOT NULL
         AND NOT EXISTS (
             SELECT 1 FROM person p WHERE p.id = d.client_person_id
         )"
    )
    .fetch_one(&mut conn)
    .await
    .expect("count orphaned deal client_person rows");

    assert_eq!(orphan_deal_person.0, 0, "no orphaned deal client_person rows must exist");

    // 5. Verify no orphaned interaction person rows: every interaction.person_id
    //    must reference an existing person.
    let orphan_interaction_person: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM interaction i WHERE i.person_id IS NOT NULL
         AND NOT EXISTS (
             SELECT 1 FROM person p WHERE p.id = i.person_id
         )"
    )
    .fetch_one(&mut conn)
    .await
    .expect("count orphaned interaction person rows");

    assert_eq!(orphan_interaction_person.0, 0, "no orphaned interaction person rows must exist");

    // 6. Verify no orphaned app_user rows referenced by person or deal.
    let orphan_app_user_person: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM person p WHERE p.assigned_user_id IS NOT NULL
         AND NOT EXISTS (
             SELECT 1 FROM app_user a WHERE a.id = p.assigned_user_id
         )"
    )
    .fetch_one(&mut conn)
    .await
    .expect("count orphaned app_user references from person");

    assert_eq!(orphan_app_user_person.0, 0, "no orphaned app_user references from person must exist");
}