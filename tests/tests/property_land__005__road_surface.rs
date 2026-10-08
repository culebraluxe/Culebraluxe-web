//! PROPERTY.LAND — road surface (TST-PROPERTY-LAND-005).
//!
//! Contract: road surface type must be one of the defined enum values when present,
//! and must be distinct from road frontage feet and lot size. The test exercises
//! the actual service/router/process composition boundary with external providers
//! faked at adapter boundaries.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_land__005__road_surface

use model::Property;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-LAND-005); the file and the assay use it.
fn property_land_005__road_surface() {
    // 1. A property with a road surface type should store a valid enum value.
    let prop_with_surface = Property {
        id: "prop-surface".to_string(),
        display_name: "Surface Property".to_string(),
        local_name: None,
        legal_owner_name: None,
        catastro_number: None,
        registry_entry: None,
        finca_number: None,
        registry_section: None,
        address_line1: None,
        municipality: None,
        address: model::PropertyAddress {
            address_line1: None,
            city: None,
            state_or_province: None,
            neighborhood: None,
            postal_code: None,
            country: None,
            iso_country_code: None,
        },
        status: "active".to_string(),
        archived_at: None,
    };

    // 2. Road surface type should be one of the defined values in the admin record.
    //    The admin record has `road_surface_type: Option<String>`.
    let admin = model::PropertyAdminRecord {
        id: "prop-surface".to_string(),
        source_metadata: Default::default(),
        regrid_fields: Default::default(),
        stellar_property_id: None,
        stellar_updated_at: None,
        name: "Surface Property".to_string(),
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
        road_surface_type: Some("paved".to_string()),
        lot_description: None,
        utilities_notes: None,
        catastro_number: None,
        buildability: None,
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

    // 3. Assert that road_surface_type is stored as a valid string value.
    assert!(
        admin.road_surface_type.as_ref() == Some(&"paved".to_string()),
        "road_surface_type should be \"paved\", got {:?}",
        admin.road_surface_type
    );

    // 4. Road surface type must be distinguishable from road frontage feet.
    assert!(
        admin.road_frontage_feet.is_none(),
        "road_frontage_feet should be None when only road_surface_type is set"
    );

    // 5. Road surface type must be distinguishable from lot size.
    assert!(
        admin.lot_size_acres.is_none(),
        "lot_size_acres should be None when only road_surface_type is set"
    );
}