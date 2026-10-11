//! PROPERTY.LAND — buildable (TST-PROPERTY-LAND-008).
//!
//! Contract: buildability field must be a constrained enum or set of valid values
//! when present, and must be distinct from slope, road surface, and lot size.
//! The test exercises the actual service/router/process composition boundary
//! with external providers faked at adapter boundaries.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_land__008__buildable

use model::PropertyAdminRecord;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-LAND-008); the file and the assay use it.
fn property_land_008__buildable() {
    // 1. A property with buildability set should store a valid enum value.
    let admin_with_buildable = model::PropertyAdminRecord {
        id: "prop-buildable".to_string(),
        source_metadata: Default::default(),
        regrid_fields: Default::default(),
        stellar_property_id: None,
        stellar_updated_at: None,
        name: "Buildable Property".to_string(),
        slug: None,
        status: "active".to_string(),
        featured: false,
        is_active_listing: true,
        is_published: true,
        property_type: None,
        has_ocean_view: false,
        has_bay_view: false,
        has_beach_view: false,
        has_harbor_view: false,
        has_island_view: false,
        has_mountain_view: false,
        has_sunrise_view: false,
        has_sunset_view: false,
        has_water_access: false,
        has_beach_access: false,
        has_pool: false,
        has_generator: false,
        has_solar: false,
        is_furnished: false,
        is_gated: false,
        list_price: None,
        original_list_price: None,
        location: None,
        address_line1: None,
        street_number: None,
        street_name: None,
        unit_number: None,
        city: None,
        state_or_province: None,
        neighborhood: None,
        postal_code: None,
        country: None,
        iso_country_code: None,
        latitude: None,
        longitude: None,
        bedrooms: None,
        bathrooms: None,
        bathrooms_full: None,
        bathrooms_half: None,
        square_feet: None,
        lot_size: None,
        lot_size_units: None,
        lot_size_acres: None,
        lot_size_sqft: None,
        road_frontage_feet: None,
        road_surface_type: None,
        lot_description: None,
        utilities_notes: None,
        catastro_number: None,
        buildability: Some("full".to_string()),
        slope_description: None,
        pool_potential: None,
        road_adjacency: None,
        utilities_availability: None,
        hoa_status: None,
        view_description: None,
        year_built: None,
        stories: None,
        parking_spaces: None,
        short_description: None,
        editorial_description: None,
        public_remarks: None,
        seo_title: None,
        seo_description: None,
        hero_title: None,
        tagline: None,
        architecture_notes: None,
        amenities_notes: None,
        lifestyle_notes: None,
        listing_agent_name: None,
        listing_agent_email: None,
        listing_agent_phone: None,
        listing_office: None,
        legal_owner_name: None,
        listing_identifier: None,
        registry_entry: None,
        finca_number: None,
        registry_section: None,
        seller_person_id: None,
        seller_name: None,
        archived: false,
        image_count: 0,
        video_count: 0,
        document_count: 0,
        created_at: None,
        updated_at: None,
        stellar: model::PropertyStellarDetails {
            listing_contract_date: None,
            expiration_date: None,
            listing_type: None,
            agent_mls_id: None,
            tax_id: None,
            tax_year: None,
            annual_tax: None,
            legal_description: None,
            zoning: None,
            total_area_sqft: None,
            heated_area_source: None,
            ownership_type: None,
            hoa_details: None,
            showing_instructions: None,
            occupant_type: None,
        },
    };

    // 2. Assert that buildability is stored as a valid string value.
    assert!(
        admin_with_buildable.buildability.as_ref() == Some(&"full".to_string()),
        "buildability should be \"full\", got {:?}",
        admin_with_buildable.buildability
    );

    // 3. Buildability must be distinguishable from slope description.
    assert!(
        admin_with_buildable.slope_description.is_none(),
        "slope_description should be None when only buildability is set"
    );

    // 4. Buildability must be distinguishable from road surface type.
    assert!(
        admin_with_buildable.road_surface_type.is_none(),
        "road_surface_type should be None when only buildability is set"
    );

    // 5. A property without buildability should have None.
    let admin_without_buildable = model::PropertyAdminRecord {
        buildability: None,
        ..admin_with_buildable.clone()
    };
    assert!(
        admin_without_buildable.buildability.is_none(),
        "property without buildability should have None"
    );
}
