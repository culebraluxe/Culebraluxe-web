//! The Data Workbench's pure rules: which section a record type opens on, and the editable form a loaded record starts
//! from. Pure, so they are tested on the host and shared by the screen and the old loop.

use crate::model::PortalOpsWorkbenchPage;

pub fn ops_default_section(entity: &str) -> String {
    match entity {
        "person" => "identity".into(),
        "project" => "project".into(),
        _ => "property".into(),
    }
}

pub fn ops_form(page: &PortalOpsWorkbenchPage) -> std::collections::BTreeMap<String, String> {
    let mut form = std::collections::BTreeMap::new();
    match page.entity.as_str() {
        "person" => {
            if let Some(person) = page.person.as_ref() {
                put(&mut form, "displayName", Some(&person.display_name));
                put(&mut form, "status", Some(&person.status));
                put(&mut form, "company", person.company.as_deref());
                put(&mut form, "role", Some(&person.role));
                put(&mut form, "location", person.location.as_deref());
                put(&mut form, "email", person.email.as_deref());
                put(&mut form, "phone", person.phone.as_deref());
            }
        }
        "project" => {
            if let Some(project) = page.project.as_ref() {
                put(&mut form, "name", Some(&project.name));
                put(&mut form, "owner", project.owner.as_deref());
                put(&mut form, "status", Some(&project.status));
                put(&mut form, "description", Some(&project.description));
                form.insert("areas".into(), project.areas.join(", "));
                put(&mut form, "projectType", project.project_type.as_deref());
                put(&mut form, "playbookId", project.playbook_id.as_deref());
                form.insert(
                    "playbookVersion".into(),
                    project
                        .playbook_version
                        .map(|value| value.to_string())
                        .unwrap_or_default(),
                );
                put(&mut form, "personId", project.person_id.as_deref());
                put(&mut form, "propertyId", project.property_id.as_deref());
                put(&mut form, "contractId", project.contract_id.as_deref());
                put(&mut form, "startsAt", project.starts_at.as_deref());
                put(&mut form, "endsAt", project.ends_at.as_deref());
            }
        }
        _ => {
            if let Some(property) = page.property.as_ref() {
                put(&mut form, "name", Some(&property.name));
                put(&mut form, "slug", property.slug.as_deref());
                put(&mut form, "status", Some(&property.status));
                form.insert("featured".into(), property.featured.to_string());
                form.insert(
                    "isActiveListing".into(),
                    property.is_active_listing.to_string(),
                );
                form.insert("isPublished".into(), property.is_published.to_string());
                put(&mut form, "propertyType", property.property_type.as_deref());
                form.insert("hasOceanView".into(), property.has_ocean_view.to_string());
                form.insert("hasBayView".into(), property.has_bay_view.to_string());
                form.insert("hasBeachView".into(), property.has_beach_view.to_string());
                form.insert("hasHarborView".into(), property.has_harbor_view.to_string());
                form.insert("hasIslandView".into(), property.has_island_view.to_string());
                form.insert(
                    "hasMountainView".into(),
                    property.has_mountain_view.to_string(),
                );
                form.insert(
                    "hasSunriseView".into(),
                    property.has_sunrise_view.to_string(),
                );
                form.insert("hasSunsetView".into(), property.has_sunset_view.to_string());
                form.insert(
                    "hasWaterAccess".into(),
                    property.has_water_access.to_string(),
                );
                form.insert(
                    "hasBeachAccess".into(),
                    property.has_beach_access.to_string(),
                );
                form.insert("hasPool".into(), property.has_pool.to_string());
                form.insert("hasGenerator".into(), property.has_generator.to_string());
                form.insert("hasSolar".into(), property.has_solar.to_string());
                form.insert("isFurnished".into(), property.is_furnished.to_string());
                form.insert("isGated".into(), property.is_gated.to_string());

                put(&mut form, "listPrice", property.list_price.as_deref());
                put(
                    &mut form,
                    "originalListPrice",
                    property.original_list_price.as_deref(),
                );
                put(&mut form, "location", property.location.as_deref());
                put(&mut form, "addressLine1", property.address_line1.as_deref());
                put(&mut form, "streetNumber", property.street_number.as_deref());
                put(&mut form, "streetName", property.street_name.as_deref());
                put(&mut form, "unitNumber", property.unit_number.as_deref());
                put(&mut form, "city", property.city.as_deref());
                put(
                    &mut form,
                    "stateOrProvince",
                    property.state_or_province.as_deref(),
                );
                put(&mut form, "neighborhood", property.neighborhood.as_deref());
                put(&mut form, "postalCode", property.postal_code.as_deref());
                put(&mut form, "country", property.country.as_deref());
                put(
                    &mut form,
                    "isoCountryCode",
                    property.iso_country_code.as_deref(),
                );
                put(&mut form, "latitude", property.latitude.as_deref());
                put(&mut form, "longitude", property.longitude.as_deref());
                put(&mut form, "bedrooms", property.bedrooms.as_deref());
                put(&mut form, "bathrooms", property.bathrooms.as_deref());
                put(
                    &mut form,
                    "bathroomsFull",
                    property.bathrooms_full.as_deref(),
                );
                put(
                    &mut form,
                    "bathroomsHalf",
                    property.bathrooms_half.as_deref(),
                );
                put(&mut form, "squareFeet", property.square_feet.as_deref());
                put(&mut form, "lotSize", property.lot_size.as_deref());
                put(
                    &mut form,
                    "lotSizeUnits",
                    property.lot_size_units.as_deref(),
                );
                put(
                    &mut form,
                    "lotSizeAcres",
                    property.lot_size_acres.as_deref(),
                );
                put(&mut form, "lotSizeSqft", property.lot_size_sqft.as_deref());
                put(
                    &mut form,
                    "roadFrontageFeet",
                    property.road_frontage_feet.as_deref(),
                );
                put(
                    &mut form,
                    "roadSurfaceType",
                    property.road_surface_type.as_deref(),
                );
                put(
                    &mut form,
                    "lotDescription",
                    property.lot_description.as_deref(),
                );
                put(
                    &mut form,
                    "utilitiesNotes",
                    property.utilities_notes.as_deref(),
                );
                put(
                    &mut form,
                    "catastroNumber",
                    property.catastro_number.as_deref(),
                );
                put(&mut form, "buildability", property.buildability.as_deref());
                put(
                    &mut form,
                    "slopeDescription",
                    property.slope_description.as_deref(),
                );
                put(
                    &mut form,
                    "poolPotential",
                    property.pool_potential.as_deref(),
                );
                put(
                    &mut form,
                    "roadAdjacency",
                    property.road_adjacency.as_deref(),
                );
                put(
                    &mut form,
                    "utilitiesAvailability",
                    property.utilities_availability.as_deref(),
                );
                put(&mut form, "hoaStatus", property.hoa_status.as_deref());
                put(
                    &mut form,
                    "viewDescription",
                    property.view_description.as_deref(),
                );
                put(&mut form, "yearBuilt", property.year_built.as_deref());
                put(&mut form, "stories", property.stories.as_deref());
                put(
                    &mut form,
                    "parkingSpaces",
                    property.parking_spaces.as_deref(),
                );
                put(
                    &mut form,
                    "shortDescription",
                    property.short_description.as_deref(),
                );
                put(
                    &mut form,
                    "editorialDescription",
                    property.editorial_description.as_deref(),
                );
                put(
                    &mut form,
                    "publicRemarks",
                    property.public_remarks.as_deref(),
                );
                put(&mut form, "seoTitle", property.seo_title.as_deref());
                put(
                    &mut form,
                    "seoDescription",
                    property.seo_description.as_deref(),
                );
                put(&mut form, "heroTitle", property.hero_title.as_deref());
                put(&mut form, "tagline", property.tagline.as_deref());
                put(
                    &mut form,
                    "architectureNotes",
                    property.architecture_notes.as_deref(),
                );
                put(
                    &mut form,
                    "amenitiesNotes",
                    property.amenities_notes.as_deref(),
                );
                put(
                    &mut form,
                    "lifestyleNotes",
                    property.lifestyle_notes.as_deref(),
                );
                put(
                    &mut form,
                    "listingAgentName",
                    property.listing_agent_name.as_deref(),
                );
                put(
                    &mut form,
                    "listingAgentEmail",
                    property.listing_agent_email.as_deref(),
                );
                put(
                    &mut form,
                    "listingAgentPhone",
                    property.listing_agent_phone.as_deref(),
                );
                put(
                    &mut form,
                    "listingOffice",
                    property.listing_office.as_deref(),
                );
                put(
                    &mut form,
                    "legalOwnerName",
                    property.legal_owner_name.as_deref(),
                );
                put(
                    &mut form,
                    "listingIdentifier",
                    property.listing_identifier.as_deref(),
                );
                put(
                    &mut form,
                    "registryEntry",
                    property.registry_entry.as_deref(),
                );
                put(&mut form, "fincaNumber", property.finca_number.as_deref());
                put(
                    &mut form,
                    "registrySection",
                    property.registry_section.as_deref(),
                );
                put(
                    &mut form,
                    "sellerPersonId",
                    property.seller_person_id.as_deref(),
                );
                form.insert("archived".into(), property.archived.to_string());

                let stellar = &property.stellar;
                put(
                    &mut form,
                    "listingContractDate",
                    stellar.listing_contract_date.as_deref(),
                );
                put(
                    &mut form,
                    "expirationDate",
                    stellar.expiration_date.as_deref(),
                );
                put(&mut form, "listingType", stellar.listing_type.as_deref());
                put(&mut form, "agentMlsId", stellar.agent_mls_id.as_deref());
                put(&mut form, "taxId", stellar.tax_id.as_deref());
                put(&mut form, "taxYear", stellar.tax_year.as_deref());
                put(&mut form, "annualTax", stellar.annual_tax.as_deref());
                put(
                    &mut form,
                    "legalDescription",
                    stellar.legal_description.as_deref(),
                );
                put(&mut form, "zoning", stellar.zoning.as_deref());
                put(
                    &mut form,
                    "totalAreaSqft",
                    stellar.total_area_sqft.as_deref(),
                );
                put(
                    &mut form,
                    "heatedAreaSource",
                    stellar.heated_area_source.as_deref(),
                );
                put(
                    &mut form,
                    "ownershipType",
                    stellar.ownership_type.as_deref(),
                );
                put(&mut form, "hoaDetails", stellar.hoa_details.as_deref());
                put(
                    &mut form,
                    "showingInstructions",
                    stellar.showing_instructions.as_deref(),
                );
                put(&mut form, "occupantType", stellar.occupant_type.as_deref());
            }
        }
    }
    form
}

fn put(form: &mut std::collections::BTreeMap<String, String>, key: &str, value: Option<&str>) {
    form.insert(key.to_string(), value.unwrap_or_default().to_string());
}
