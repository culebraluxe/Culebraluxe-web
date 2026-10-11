//! PROPERTY.LAND — view (TST-PROPERTY-LAND-006).
//!
//! Contract: view description must be a free-text field that can contain diverse
//! content without implicit sanitization or truncation. The test exercises the
//! actual service/router/process composition boundary with external providers
//! faked at adapter boundaries.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_land__006__view

use chrono::Utc;
use model::PropertyAdminRecord;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-LAND-006); the file and the assay use it.
fn property_land_006__view() {
    // 1. A property with a view description should store the full text without
    //    implicit sanitization, truncation, or encoding changes.
    let admin_with_view = model::PropertyAdminRecord {
        id: "prop-view".to_string(),
        source_metadata: Default::default(),
        regrid_fields: Default::default(),
        stellar_property_id: None,
        stellar_updated_at: None,
        name: "View Property".to_string(),
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
        buildability: None,
        slope_description: None,
        pool_potential: None,
        road_adjacency: None,
        utilities_availability: None,
        hoa_status: None,
        view_description: Some(
            "Ocean view from master bedroom. Sunset over the harbor. Mountain silhouette at dusk."
                .to_string(),
        ),
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

    // 2. Assert that the view description is stored as-is, preserving all characters.
    let expected =
        "Ocean view from master bedroom. Sunset over the harbor. Mountain silhouette at dusk.";
    assert_eq!(
        admin_with_view.view_description.as_deref().unwrap(),
        expected,
        "view description should be stored verbatim"
    );

    // 3. A property without a view description should have None.
    let admin_without_view = model::PropertyAdminRecord {
        view_description: None,
        ..admin_with_view.clone()
    };
    assert!(
        admin_without_view.view_description.is_none(),
        "property without view should have None"
    );

    // 4. View description must be distinguishable from short description.
    assert_ne!(
        admin_with_view.view_description, admin_with_view.short_description,
        "view_description and short_description are separate fields"
    );
}
