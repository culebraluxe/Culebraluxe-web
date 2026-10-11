//! PROPERTY.LAND — main road boundary (TST-PROPERTY-LAND-010).
//!
//! Contract: the property record carries the main-road boundary facts — `road_adjacency`
//! (which road the parcel borders), `road_frontage_feet` (how much frontage it has) and
//! `road_surface_type` (what that road is surfaced with). The three persist together
//! through the production admin boundary and read back as committed.
//!
//! Level: L3 Composition — property admin boundary.
//! Harness: PropertyHarness.
//! Requires DEV (`DATABASE_URL_DEV`); the harness refuses PROD before any socket.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_land__010__main_road_boundary

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
    road_adjacency: Option<&str>,
    road_frontage_feet: Option<&str>,
    road_surface_type: Option<&str>,
) -> SavePropertyAdminRequest {
    SavePropertyAdminRequest {
        property_id: property_id.to_string(),
        name: name.to_string(),
        status: "active".to_string(),
        road_adjacency: road_adjacency.map(|value| value.to_string()),
        road_frontage_feet: road_frontage_feet.map(|value| value.to_string()),
        road_surface_type: road_surface_type.map(|value| value.to_string()),
        ..Default::default()
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-LAND-010); the file and the assay use it.
async fn property_land_010__main_road_boundary() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the main-road-boundary proof runs only on an isolated DEV target"
    );
    let ctx = harness.test_context();
    let ns = harness.namespace().to_string();
    let marker = format!("TST-PROPERTY-LAND010-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. Positive: the main-road boundary facts persist together through the production admin boundary.
    // -----------------------------------------------------------------------------------------------------------
    let property_id = harness
        .seed_property(&marker, Some("land"))
        .await
        .expect("property seeds");

    let record = harness
        .property_service()
        .admin_save(
            &save_request(
                &property_id,
                &marker,
                Some("main_road"),
                Some("120"),
                Some("asphalt"),
            ),
            &ctx,
        )
        .await
        .expect("admin_save runs");
    assert_eq!(
        record.road_adjacency.as_deref(),
        Some("main_road"),
        "{HARNESS}: admin_save persists road_adjacency"
    );
    assert_eq!(
        record.road_frontage_feet.as_deref(),
        Some("120.00"),
        "{HARNESS}: admin_save persists road_frontage_feet"
    );
    assert_eq!(
        record.road_surface_type.as_deref(),
        Some("asphalt"),
        "{HARNESS}: admin_save persists road_surface_type"
    );

    // Read back through the production read path, not the write's return value.
    let fetched = harness
        .property_service()
        .admin_get(&property_id, &ctx)
        .await
        .expect("admin_get runs")
        .expect("the property is still there");
    assert_eq!(
        fetched.road_adjacency.as_deref(),
        Some("main_road"),
        "{HARNESS}: admin_get returns the persisted road_adjacency"
    );
    assert_eq!(
        fetched.road_frontage_feet.as_deref(),
        Some("120.00"),
        "{HARNESS}: admin_get returns the persisted road_frontage_feet"
    );
    assert_eq!(
        fetched.road_surface_type.as_deref(),
        Some("asphalt"),
        "{HARNESS}: admin_get returns the persisted road_surface_type"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. Positive: a parcel bordering a different road stores that road — the field is data, not a constant.
    // -----------------------------------------------------------------------------------------------------------
    let other_id = harness
        .seed_property(&format!("{marker}-other"), Some("land"))
        .await
        .expect("other property seeds");
    let fetched = harness
        .property_service()
        .admin_save(
            &save_request(
                &other_id,
                &format!("{marker}-other"),
                Some("county_road"),
                Some("85"),
                Some("gravel"),
            ),
            &ctx,
        )
        .await
        .expect("admin_save runs");
    assert_eq!(
        fetched.road_adjacency.as_deref(),
        Some("county_road"),
        "{HARNESS}: a different road adjacency persists as data"
    );
    assert_eq!(
        fetched.road_frontage_feet.as_deref(),
        Some("85.00"),
        "{HARNESS}: frontage persists for the other parcel"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. Negative: a parcel with no main-road boundary reads None for all three — the fields are optional.
    // -----------------------------------------------------------------------------------------------------------
    let plain_id = harness
        .seed_property(&format!("{marker}-plain"), Some("land"))
        .await
        .expect("plain property seeds");
    let fetched = harness
        .property_service()
        .admin_save(
            &save_request(&plain_id, &format!("{marker}-plain"), None, None, None),
            &ctx,
        )
        .await
        .expect("admin_save runs");
    assert_eq!(
        fetched.road_adjacency, None,
        "{HARNESS}: no road adjacency reads None"
    );
    assert_eq!(
        fetched.road_frontage_feet, None,
        "{HARNESS}: no frontage reads None"
    );
    assert_eq!(
        fetched.road_surface_type, None,
        "{HARNESS}: no surface type reads None"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. Negative: clearing the boundary (empty strings) stores NULL, not empty strings.
    // -----------------------------------------------------------------------------------------------------------
    let cleared = harness
        .property_service()
        .admin_save(
            &save_request(&property_id, &marker, Some(""), Some(""), Some("")),
            &ctx,
        )
        .await
        .expect("clearing admin_save runs");
    assert_eq!(
        cleared.road_adjacency, None,
        "{HARNESS}: an empty road_adjacency is stored as NULL"
    );
    assert_eq!(
        cleared.road_frontage_feet, None,
        "{HARNESS}: an empty frontage is stored as NULL"
    );

    // -----------------------------------------------------------------------------------------------------------
    // Cleanup.
    // -----------------------------------------------------------------------------------------------------------
    let removed = harness.cleanup(&marker).await.expect("cleanup properties");
    assert_eq!(
        removed, 3,
        "{HARNESS}: exactly three seeded properties removed"
    );
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("leftover count"),
        0
    );
}
