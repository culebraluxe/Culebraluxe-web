//! PROPERTY.LAND — land fields also available to house when applicable (TST-PROPERTY-LAND-011).
//!
//! Contract: the land fields — lot_size, lot_size_units, lot_size_acres, lot_size_sqft,
//! road_frontage_feet, road_surface_type, lot_description, buildability, slope_description,
//! pool_potential, road_adjacency, utilities_availability — live on the property record
//! itself, not on a land-only subtype. A house that has them (a lot with a pool, a
//! frontage, a buildable slope) persists and reads them back through the production admin
//! boundary exactly as a land parcel does.
//!
//! Level: L3 Composition — property admin boundary.
//! Harness: PropertyHarness.
//! Requires DEV (`DATABASE_URL_DEV`); the harness refuses PROD before any socket.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_land__011__land_fields_also_available_to_house_when_applicable

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

/// A save request carrying the full set of land fields, so the test proves they all persist
/// on one record regardless of the property's type.
fn land_fields_request(
    property_id: &str,
    name: &str,
    property_type: &str,
) -> SavePropertyAdminRequest {
    SavePropertyAdminRequest {
        property_id: property_id.to_string(),
        name: name.to_string(),
        status: "active".to_string(),
        property_type: Some(property_type.to_string()),
        lot_size: Some("0.42".to_string()),
        lot_size_units: Some("acres".to_string()),
        lot_size_acres: Some("0.42".to_string()),
        lot_size_sqft: Some("18277".to_string()),
        road_frontage_feet: Some("65".to_string()),
        road_surface_type: Some("paved".to_string()),
        lot_description: Some("Level corner lot with mature trees.".to_string()),
        buildability: Some("Buildable with setback variance.".to_string()),
        slope_description: Some("Level to gentle.".to_string()),
        pool_potential: Some("Room for a pool.".to_string()),
        road_adjacency: Some("main_road".to_string()),
        utilities_availability: Some("All public utilities at lot line.".to_string()),
        ..Default::default()
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-LAND-011); the file and the assay use it.
async fn property_land_011__land_fields_also_available_to_house_when_applicable() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the land-fields-on-house proof runs only on an isolated DEV target"
    );
    let ctx = harness.test_context();
    let ns = harness.namespace().to_string();
    let marker = format!("TST-PROPERTY-LAND011-{ns}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. Positive: a HOUSE persists the full set of land fields — they are not land-only.
    // -----------------------------------------------------------------------------------------------------------
    let house_id = harness
        .seed_property(&marker, Some("house"))
        .await
        .expect("house property seeds");

    let record = harness
        .property_service()
        .admin_save(&land_fields_request(&house_id, &marker, "house"), &ctx)
        .await
        .expect("admin_save runs");
    assert_eq!(
        record.property_type.as_deref(),
        Some("house"),
        "{HARNESS}: the record is still a house"
    );
    assert_eq!(
        record.lot_size.as_deref(),
        Some("0.42"),
        "{HARNESS}: lot_size persists on a house"
    );
    assert_eq!(
        record.lot_size_units.as_deref(),
        Some("acres"),
        "{HARNESS}: lot_size_units persists on a house"
    );
    assert_eq!(
        record.lot_size_acres.as_deref(),
        Some("0.4200"),
        "{HARNESS}: lot_size_acres persists on a house"
    );
    assert_eq!(
        record.lot_size_sqft.as_deref(),
        Some("18277.00"),
        "{HARNESS}: lot_size_sqft persists on a house"
    );
    assert_eq!(
        record.road_frontage_feet.as_deref(),
        Some("65.00"),
        "{HARNESS}: road_frontage_feet persists on a house"
    );
    assert_eq!(
        record.road_surface_type.as_deref(),
        Some("paved"),
        "{HARNESS}: road_surface_type persists on a house"
    );
    assert_eq!(
        record.lot_description.as_deref(),
        Some("Level corner lot with mature trees."),
        "{HARNESS}: lot_description persists on a house"
    );
    assert_eq!(
        record.buildability.as_deref(),
        Some("Buildable with setback variance."),
        "{HARNESS}: buildability persists on a house"
    );
    assert_eq!(
        record.slope_description.as_deref(),
        Some("Level to gentle."),
        "{HARNESS}: slope_description persists on a house"
    );
    assert_eq!(
        record.pool_potential.as_deref(),
        Some("Room for a pool."),
        "{HARNESS}: pool_potential persists on a house"
    );
    assert_eq!(
        record.road_adjacency.as_deref(),
        Some("main_road"),
        "{HARNESS}: road_adjacency persists on a house"
    );
    assert_eq!(
        record.utilities_availability.as_deref(),
        Some("All public utilities at lot line."),
        "{HARNESS}: utilities_availability persists on a house"
    );

    // Read back through the production read path, not the write's return value.
    let fetched = harness
        .property_service()
        .admin_get(&house_id, &ctx)
        .await
        .expect("admin_get runs")
        .expect("the house is still there");
    assert_eq!(
        fetched.lot_size.as_deref(),
        Some("0.42"),
        "{HARNESS}: admin_get returns the house's lot_size"
    );
    assert_eq!(
        fetched.lot_size_units.as_deref(),
        Some("acres"),
        "{HARNESS}: admin_get returns the house's lot_size_units"
    );
    assert_eq!(
        fetched.lot_size_acres.as_deref(),
        Some("0.4200"),
        "{HARNESS}: admin_get returns the house's lot_size_acres"
    );
    assert_eq!(
        fetched.lot_size_sqft.as_deref(),
        Some("18277.00"),
        "{HARNESS}: admin_get returns the house's lot_size_sqft"
    );
    assert_eq!(
        fetched.road_frontage_feet.as_deref(),
        Some("65.00"),
        "{HARNESS}: admin_get returns the house's road_frontage_feet"
    );
    assert_eq!(
        fetched.road_surface_type.as_deref(),
        Some("paved"),
        "{HARNESS}: admin_get returns the house's road_surface_type"
    );
    assert_eq!(
        fetched.lot_description.as_deref(),
        Some("Level corner lot with mature trees."),
        "{HARNESS}: admin_get returns the house's lot_description"
    );
    assert_eq!(
        fetched.buildability.as_deref(),
        Some("Buildable with setback variance."),
        "{HARNESS}: admin_get returns the house's buildability"
    );
    assert_eq!(
        fetched.slope_description.as_deref(),
        Some("Level to gentle."),
        "{HARNESS}: admin_get returns the house's slope_description"
    );
    assert_eq!(
        fetched.pool_potential.as_deref(),
        Some("Room for a pool."),
        "{HARNESS}: admin_get returns the house's pool_potential"
    );
    assert_eq!(
        fetched.road_adjacency.as_deref(),
        Some("main_road"),
        "{HARNESS}: admin_get returns the house's road_adjacency"
    );
    assert_eq!(
        fetched.utilities_availability.as_deref(),
        Some("All public utilities at lot line."),
        "{HARNESS}: admin_get returns the house's utilities_availability"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. Positive: a LAND parcel carries the same fields — the fields are universal, not house-only.
    // -----------------------------------------------------------------------------------------------------------
    let land_id = harness
        .seed_property(&format!("{marker}-land"), Some("land"))
        .await
        .expect("land property seeds");
    let fetched = harness
        .property_service()
        .admin_save(
            &land_fields_request(&land_id, &format!("{marker}-land"), "land"),
            &ctx,
        )
        .await
        .expect("admin_save runs");
    assert_eq!(
        fetched.property_type.as_deref(),
        Some("land"),
        "{HARNESS}: the record is still a land parcel"
    );
    assert_eq!(
        fetched.lot_size.as_deref(),
        Some("0.42"),
        "{HARNESS}: lot_size persists on a land parcel too"
    );
    assert_eq!(
        fetched.pool_potential.as_deref(),
        Some("Room for a pool."),
        "{HARNESS}: pool_potential persists on a land parcel too"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. Negative: a house WITHOUT land fields reads None for all of them — the fields are optional,
    //    not forced onto every house.
    // -----------------------------------------------------------------------------------------------------------
    let bare_id = harness
        .seed_property(&format!("{marker}-bare"), Some("house"))
        .await
        .expect("bare house seeds");
    let bare = SavePropertyAdminRequest {
        property_id: bare_id.clone(),
        name: format!("{marker}-bare"),
        status: "active".to_string(),
        property_type: Some("house".to_string()),
        ..Default::default()
    };
    let fetched = harness
        .property_service()
        .admin_save(&bare, &ctx)
        .await
        .expect("admin_save runs");
    assert_eq!(
        fetched.lot_size, None,
        "{HARNESS}: a house without land fields reads None for lot_size"
    );
    assert_eq!(
        fetched.road_frontage_feet, None,
        "{HARNESS}: a house without land fields reads None for road_frontage_feet"
    );
    assert_eq!(
        fetched.pool_potential, None,
        "{HARNESS}: a house without land fields reads None for pool_potential"
    );
    assert_eq!(
        fetched.utilities_availability, None,
        "{HARNESS}: a house without land fields reads None for utilities_availability"
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
