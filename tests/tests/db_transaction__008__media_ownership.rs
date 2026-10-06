//! DB.TRANSACTION — media + ownership (TST-DB-TRANSACTION-008).
//!
//! CONTRACT. A media asset must have a clear ownership relationship.  The
//! property_media table links media to properties with a role and sort order,
//! enforcing that a media asset is owned by exactly one property at a time for
//! a given role.  This test verifies the media ownership relationship.
//!
//! Level: L2 Persistence — exercise the same boundary production uses.  Use
//! only an isolated disposable Postgres/Neon test target; assert committed
//! database truth and rollback; PROD is forbidden.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_transaction__008__media_ownership

use test_harness::database::TestDatabase;
use test_harness::database::TestTransaction;

#[tokio::test]
async fn db_transaction_008__media_ownership() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)");

    // 1. Begin a transaction to keep state isolated.
    let mut tx = test_db.begin().await.expect("begin transaction");

    // 2. Insert a property.
    let property_id: String = sqlx::query_scalar::<_, String>(
        "INSERT INTO property (name, status) VALUES ('Test Property', 'active') RETURNING id::text",
    )
    .fetch_one(&mut *tx.connection())
    .await
    .expect("insert property");

    // 3. Insert a media row.
    let media_id: String = sqlx::query_scalar::<_, String>(
        "INSERT INTO media (file_data, filename, mime_type, media_type)
         VALUES ($1, 'test.jpg', 'image/jpeg', 'image')
         RETURNING id::text",
    )
    .bind(&b"image bytes"[..])
    .fetch_one(&mut *tx.connection())
    .await
    .expect("insert media row");

    // 4. Link the media to the property via property_media (ownership).
    sqlx::query(
        "INSERT INTO property_media (property_id, media_id, role, sort_order)
         VALUES ($1::uuid, $2::uuid, 'gallery', 0)",
    )
    .bind(&property_id)
    .bind(&media_id)
    .execute(&mut *tx.connection())
    .await
    .expect("link media to property");

    // 5. Verify the ownership relationship.
    let ownership_count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM property_media WHERE property_id = $1::uuid AND media_id = $2::uuid",
    )
    .bind(&property_id)
    .bind(&media_id)
    .fetch_one(&mut *tx.connection())
    .await
    .expect("count ownership records");
    assert_eq!(
        ownership_count.0, 1,
        "media must be owned by exactly one property"
    );

    // 6. Verify the role and sort order.
    let (role, sort_order): (String, i32) = sqlx::query_as(
        "SELECT role, sort_order FROM property_media WHERE property_id = $1::uuid AND media_id = $2::uuid",
    )
    .bind(&property_id)
    .bind(&media_id)
    .fetch_one(&mut *tx.connection())
    .await
    .expect("get ownership details");
    assert_eq!(role, "gallery", "ownership role must be gallery");
    assert_eq!(sort_order, 0, "ownership sort_order must be 0");

    // 7. Negative case: duplicate ownership (same property, media, role) must fail.
    let duplicate_result = sqlx::query(
        "INSERT INTO property_media (property_id, media_id, role, sort_order)
         VALUES ($1::uuid, $2::uuid, 'gallery', 1)",
    )
    .bind(&property_id)
    .bind(&media_id)
    .execute(&mut *tx.connection())
    .await;

    assert!(
        duplicate_result.is_err(),
        "duplicate property_media (property_id, media_id) must violate the primary key"
    );

    // 8. Roll back to leave no residual state.
    let _ = tx.rollback().await;
}
