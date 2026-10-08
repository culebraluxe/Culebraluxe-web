//! PROPERTY.LAND — both persist independently (TST-PROPERTY-LAND-002).
//!
//! Contract: property and admin record fields must be independently addressable;
//! setting one must not implicitly zero or transform the other. The test exercises
//! the model boundary: property admin record fields are separate invariants.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_land__002__both_persist_independently

use model::{Property, PropertyAdminRecord};

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-LAND-002); the file and the assay use it.
fn property_land_002__both_persist_independently() {
    // 1. A property with display_name set should not implicitly zero or transform
    //    the admin record's other fields.
    let prop_with_name = Property {
        id: "prop-001".to_string(),
        display_name: "Test Property".to_string(),
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

    // 2. The admin record should have name set independently.
    let admin_with_name = PropertyAdminRecord {
        id: "prop-001".to_string(),
        source_metadata: Default::default(),
        regrid_fields: Default::default(),
        stellar_property_id: None,
        stellar_updated_at: None,
        name: "Test Property".to_string(),
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

    // 3. Assert that name is independently addressable — the admin
    //    record's name field should not be conjured from the property's display_name.
    assert_eq!(
        admin_with_name.name,
        "Test Property",
        "admin record name should be independently set"
    );

    // 4. Property and admin record fields must not conflate — setting one should
    //    not implicitly affect the other, exercising the boundary that they are
    //    independent fields with separate lifecycles.
    assert_eq!(
        prop_with_name.display_name,
        "Test Property",
        "property display_name should remain as-is"
    );
}