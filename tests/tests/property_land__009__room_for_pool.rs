//! PROPERTY.LAND — room for pool (TST-PROPERTY-LAND-009).
//!
//! Contract: the property record carries a `pool_potential` field describing whether the
//! land has room for a pool. The field is persisted and retrievable through the production
//! admin boundary (`PropertyService::admin_save` / `admin_get`), it is optional (a property
//! without pool potential reads `None`, not a default), and it survives unrelated saves.
//!
//! Level: L3 Composition — property admin boundary.
//! Harness: PropertyHarness.
//! Requires DEV (`DATABASE_URL_DEV`); the harness refuses PROD before any socket.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_land__009__room_for_pool

use db::DbTarget;
use model::SavePropertyAdminRequest;
use test_harness::PropertyHarness;

const HARNESS: &str = "PropertyHarness/L3 Composition";

async fn connect_dev() -> PropertyHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match PropertyHarness::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; PropertyHarness refuses PROD: {}",
        last.unwrap_or_default()
    );
}

fn save_request(
    property_id: &str,
    name: &str,
    pool_potential: Option<&str>,
) -> SavePropertyAdminRequest {
    SavePropertyAdminRequest {
        property_id: property_id.to_string(),
        name: name.to_string(),
        status: "active".to_string(),
        pool_potential: pool_potential.map(|value| value.to_string()),
        ..Default::default()
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-LAND-009); the file and the assay use it.
async fn property_land_009__room_for_pool() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the room-for-pool proof runs only on an isolated DEV target"
    );
    let ctx = harness.test_context();
    let ns = harness.namespace().to_string();
    let marker = format!("TST-PROPERTY-LAND009-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. Positive: pool_potential persists through the production admin boundary.
    // -----------------------------------------------------------------------------------------------------------
    let property_id = harness
        .seed_property(&marker, Some("land"))
        .await
        .expect("property seeds");

    let record = harness
        .property_service()
        .admin_save(
            &save_request(&property_id, &marker, Some("Ample room for a pool")),
            &ctx,
        )
        .await
        .expect("admin_save runs");
    assert_eq!(
        record.pool_potential.as_deref(),
        Some("Ample room for a pool"),
        "{HARNESS}: admin_save persists pool_potential"
    );

    // Read back through the production read path, not the write's return value.
    let fetched = harness
        .property_service()
        .admin_get(&property_id, &ctx)
        .await
        .expect("admin_get runs")
        .expect("the property is still there");
    assert_eq!(
        fetched.pool_potential.as_deref(),
        Some("Ample room for a pool"),
        "{HARNESS}: admin_get returns the persisted pool_potential"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. Positive: pool_potential survives an unrelated save (it is not cleared by other fields).
    // -----------------------------------------------------------------------------------------------------------
    let mut unrelated = save_request(&property_id, &marker, Some("Ample room for a pool"));
    unrelated.short_description = Some("A sunny parcel.".to_string());
    let fetched = harness
        .property_service()
        .admin_save(&unrelated, &ctx)
        .await
        .expect("second admin_save runs");
    assert_eq!(
        fetched.pool_potential.as_deref(),
        Some("Ample room for a pool"),
        "{HARNESS}: pool_potential survives an unrelated save"
    );
    assert_eq!(
        fetched.short_description.as_deref(),
        Some("A sunny parcel."),
        "{HARNESS}: the unrelated field also persisted"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. Negative: a property saved WITHOUT pool_potential reads None — the field is optional, not defaulted.
    // -----------------------------------------------------------------------------------------------------------
    let plain_id = harness
        .seed_property(&format!("{marker}-plain"), Some("land"))
        .await
        .expect("plain property seeds");
    let fetched = harness
        .property_service()
        .admin_save(
            &save_request(&plain_id, &format!("{marker}-plain"), None),
            &ctx,
        )
        .await
        .expect("admin_save runs");
    assert_eq!(
        fetched.pool_potential, None,
        "{HARNESS}: a property without pool_potential reads None, not a default"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. Negative: clearing pool_potential (empty string) stores NULL, not an empty string.
    // -----------------------------------------------------------------------------------------------------------
    let cleared = harness
        .property_service()
        .admin_save(&save_request(&property_id, &marker, Some("")), &ctx)
        .await
        .expect("clearing admin_save runs");
    assert_eq!(
        cleared.pool_potential, None,
        "{HARNESS}: an empty pool_potential is stored as NULL"
    );

    // -----------------------------------------------------------------------------------------------------------
    // Cleanup.
    // -----------------------------------------------------------------------------------------------------------
    let removed = harness.cleanup(&marker).await.expect("cleanup properties");
    assert_eq!(
        removed, 2,
        "{HARNESS}: exactly two seeded properties removed"
    );
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("leftover count"),
        0
    );
}
