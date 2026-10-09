//! CRM.CLIENT — name search (TST-CRM-CLIENT-002).
//!
//! Contract: the client directory supports name search by display_name, returning matching clients
//! with correct relationship_activity. The search is case-insensitive and partial.
//!
//! Level: L3 Composition — CRM client directory name search.
//! Harness: ClientHarness.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test crm_client__002__name_search -- --ignored

use db::DbTarget;
use model::{ClientDirectoryPageRequest, ClientSummary};
use test_harness::ClientHarness;
use uuid::Uuid;

const HARNESS: &str = "ClientHarness/L3 Composition";

async fn connect_dev() -> ClientHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match ClientHarness::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; ClientHarness refuses PROD: {}",
        last.unwrap_or_default()
    );
}

fn dir_request(search: &str, page: i64, page_size: i64) -> ClientDirectoryPageRequest {
    ClientDirectoryPageRequest {
        search: search.to_string(),
        status: None,
        role: None,
        sort: "name".to_string(),
        page,
        page_size,
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); ClientHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-CLIENT-002); the file and the assay use it.
async fn crm_client_002__name_search() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the name search proof runs only on an isolated DEV target"
    );
    let ctx = harness.test_context();
    let ns = harness.namespace().to_string();
    let marker = format!("TST-CRMCLIENT002-{ns}-{}", &Uuid::new_v4().to_string()[..8]);

    // Seed test clients with distinct names.
    let pool = harness.pool();

    let alice_id = sqlx::query_scalar::<_, String>(
        "insert into person (display_name, role, status) values ($1, 'buyer', 'new') returning id::text"
    )
    .bind(format!("{marker}-alice-smith"))
    .fetch_one(pool)
    .await
    .expect("alice seeds");

    let bob_id = sqlx::query_scalar::<_, String>(
        "insert into person (display_name, role, status) values ($1, 'buyer', 'new') returning id::text"
    )
    .bind(format!("{marker}-bob-jones"))
    .fetch_one(pool)
    .await
    .expect("bob seeds");

    let charlie_id = sqlx::query_scalar::<_, String>(
        "insert into person (display_name, role, status) values ($1, 'seller', 'active') returning id::text"
    )
    .bind(format!("{marker}-charlie-brown"))
    .fetch_one(pool)
    .await
    .expect("charlie seeds");

    // Add primary emails so they appear in directory.
    sqlx::query(
        "insert into person_identity (person_id, identity_type, identity_value, is_primary) values ($1::uuid, 'email', $2, true)"
    )
    .bind(&alice_id)
    .bind(format!("alice-{marker}@example.test"))
    .execute(pool)
    .await
    .expect("alice email");

    sqlx::query(
        "insert into person_identity (person_id, identity_type, identity_value, is_primary) values ($1::uuid, 'email', $2, true)"
    )
    .bind(&bob_id)
    .bind(format!("bob-{marker}@example.test"))
    .execute(pool)
    .await
    .expect("bob email");

    sqlx::query(
        "insert into person_identity (person_id, identity_type, identity_value, is_primary) values ($1::uuid, 'email', $2, true)"
    )
    .bind(&charlie_id)
    .bind(format!("charlie-{marker}@example.test"))
    .execute(pool)
    .await
    .expect("charlie email");

    // Refresh the materialized view so the new data is visible.
    sqlx::query("refresh materialized view concurrently mv_client_directory")
        .execute(pool)
        .await
        .expect("refresh mv");

    // -----------------------------------------------------------------------------------------------------------
    // 1. Search for "alice" - should find only Alice.
    // -----------------------------------------------------------------------------------------------------------
    let result = harness
        .service()
        .directory(&dir_request(&format!("{marker}-alice"), 1, 50), &ctx)
        .await
        .expect("directory search runs");
    assert_eq!(
        result.total, 1,
        "{HARNESS}: search 'alice' returns exactly one"
    );
    assert_eq!(result.rows.len(), 1, "{HARNESS}: one row returned");
    let row: &ClientSummary = &result.rows[0];
    assert!(
        row.display_name.to_lowercase().contains("alice"),
        "{HARNESS}: row contains alice"
    );
    assert_eq!(row.id, alice_id, "{HARNESS}: correct person id");

    // -----------------------------------------------------------------------------------------------------------
    // 2. Search for "bob" - should find only Bob.
    // -----------------------------------------------------------------------------------------------------------
    let result = harness
        .service()
        .directory(&dir_request(&format!("{marker}-bob"), 1, 50), &ctx)
        .await
        .expect("directory search runs");
    assert_eq!(
        result.total, 1,
        "{HARNESS}: search 'bob' returns exactly one"
    );
    assert_eq!(result.rows.len(), 1, "{HARNESS}: one row returned");
    let row: &ClientSummary = &result.rows[0];
    assert!(
        row.display_name.to_lowercase().contains("bob"),
        "{HARNESS}: row contains bob"
    );
    assert_eq!(row.id, bob_id, "{HARNESS}: correct person id");

    // -----------------------------------------------------------------------------------------------------------
    // 3. Search for "charlie" - should find only Charlie.
    // -----------------------------------------------------------------------------------------------------------
    let result = harness
        .service()
        .directory(&dir_request(&format!("{marker}-charlie"), 1, 50), &ctx)
        .await
        .expect("directory search runs");
    assert_eq!(
        result.total, 1,
        "{HARNESS}: search 'charlie' returns exactly one"
    );
    assert_eq!(result.rows.len(), 1, "{HARNESS}: one row returned");
    let row: &ClientSummary = &result.rows[0];
    assert!(
        row.display_name.to_lowercase().contains("charlie"),
        "{HARNESS}: row contains charlie"
    );
    assert_eq!(row.id, charlie_id, "{HARNESS}: correct person id");

    // -----------------------------------------------------------------------------------------------------------
    // 4. Search for "SMITH" (uppercase) - should find Alice (case insensitive).
    // -----------------------------------------------------------------------------------------------------------
    let result = harness
        .service()
        .directory(
            &dir_request(&format!("{}-alice-smith", marker).to_uppercase(), 1, 50),
            &ctx,
        )
        .await
        .expect("directory search runs");
    assert_eq!(
        result.total, 1,
        "{HARNESS}: search 'SMITH' (uppercase) returns exactly one"
    );
    assert_eq!(
        result.rows[0].id, alice_id,
        "{HARNESS}: case-insensitive match works"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. Search for partial "char" - should find Charlie.
    // -----------------------------------------------------------------------------------------------------------
    let result = harness
        .service()
        .directory(&dir_request(&format!("{marker}-char"), 1, 50), &ctx)
        .await
        .expect("directory search runs");
    assert_eq!(
        result.total, 1,
        "{HARNESS}: partial search 'char' returns exactly one"
    );
    assert_eq!(
        result.rows[0].id, charlie_id,
        "{HARNESS}: partial match works"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. Search for "xyz" - should find nothing (negative case).
    // -----------------------------------------------------------------------------------------------------------
    let result = harness
        .service()
        .directory(&dir_request(&format!("{marker}-nobody"), 1, 50), &ctx)
        .await
        .expect("directory search runs");
    assert_eq!(result.total, 0, "{HARNESS}: search 'xyz' returns zero");
    assert!(
        result.rows.is_empty(),
        "{HARNESS}: no rows for non-matching search"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. Empty search - should return all three (paginated).
    // -----------------------------------------------------------------------------------------------------------
    let result = harness
        .service()
        .directory(&dir_request(&marker, 1, 2), &ctx)
        .await
        .expect("directory search runs");
    assert_eq!(result.total, 3, "{HARNESS}: empty search returns all three");
    assert_eq!(result.rows.len(), 2, "{HARNESS}: page size 2 respected");
    assert_eq!(result.page, 1, "{HARNESS}: page 1");
    assert_eq!(result.page_size, 2, "{HARNESS}: page size 2");

    let result2 = harness
        .service()
        .directory(&dir_request(&marker, 2, 2), &ctx)
        .await
        .expect("directory search page 2 runs");
    assert_eq!(
        result2.total, 3,
        "{HARNESS}: empty search page 2 total is 3"
    );
    assert_eq!(result2.rows.len(), 1, "{HARNESS}: page 2 has 1 row");
    assert_eq!(result2.page, 2, "{HARNESS}: page 2");

    // -----------------------------------------------------------------------------------------------------------
    // 8. Negative: invalid status filter is refused.
    // -----------------------------------------------------------------------------------------------------------
    let mut bad_request = dir_request("", 1, 50);
    bad_request.status = Some("invalid_status".to_string());
    let err = harness
        .service()
        .directory(&bad_request, &ctx)
        .await
        .expect_err("invalid status must be refused");
    assert_eq!(
        err.code(),
        "CLIENT_STATUS_INVALID",
        "{HARNESS}: invalid status refused with correct code"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 9. Negative: invalid role filter is refused.
    // -----------------------------------------------------------------------------------------------------------
    let mut bad_request = dir_request("", 1, 50);
    bad_request.role = Some("landlord".to_string());
    let err = harness
        .service()
        .directory(&bad_request, &ctx)
        .await
        .expect_err("invalid role must be refused");
    assert_eq!(
        err.code(),
        "CLIENT_ROLE_INVALID",
        "{HARNESS}: invalid role refused with correct code"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 10. Negative: invalid sort is refused.
    // -----------------------------------------------------------------------------------------------------------
    let mut bad_request = dir_request("", 1, 50);
    bad_request.sort = "invalid_sort".to_string();
    let err = harness
        .service()
        .directory(&bad_request, &ctx)
        .await
        .expect_err("invalid sort must be refused");
    assert_eq!(
        err.code(),
        "CLIENT_SORT_INVALID",
        "{HARNESS}: invalid sort refused with correct code"
    );

    // Clean up.
    let removed = harness.cleanup(&marker).await.expect("cleanup persons");
    assert_eq!(
        removed, 3,
        "{HARNESS}: exactly three seeded persons removed"
    );
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("leftover count"),
        0
    );
}
