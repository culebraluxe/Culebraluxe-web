use crate::FieldPatch;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyAddress {
    pub address_line1: Option<String>,
    pub city: Option<String>,
    pub state_or_province: Option<String>,
    pub neighborhood: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub iso_country_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonPropertyRelation {
    Address,
    LegalAddress,
    PhysicalProperty,
    Interest,
}

impl PersonPropertyRelation {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Address => "address",
            Self::LegalAddress => "legal_address",
            Self::PhysicalProperty => "physical_property",
            Self::Interest => "interest",
        }
    }
}

impl TryFrom<&str> for PersonPropertyRelation {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "address" => Ok(Self::Address),
            "legal_address" => Ok(Self::LegalAddress),
            "physical_property" => Ok(Self::PhysicalProperty),
            "interest" => Ok(Self::Interest),
            other => Err(format!("unknown Person/Property relation: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Property {
    pub id: String,
    pub display_name: String,
    pub local_name: Option<String>,
    pub legal_owner_name: Option<String>,
    pub catastro_number: Option<String>,
    pub registry_entry: Option<String>,
    pub finca_number: Option<String>,
    pub registry_section: Option<String>,
    pub address_line1: Option<String>,
    pub municipality: Option<String>,
    pub address: PropertyAddress,
    pub status: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyForPerson {
    pub relation: PersonPropertyRelation,
    pub relation_status: Option<String>,
    pub property: Property,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonPropertyContext {
    pub person_id: String,
    pub properties: Vec<PropertyForPerson>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindPropertyByAddressRequest {
    pub address_line1: String,
    pub municipality: Option<String>,
    pub state_or_province: Option<String>,
    pub postal_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsertPropertyForPersonRequest {
    pub person_id: String,
    pub relation: PersonPropertyRelation,
    pub property_id: Option<String>,
    pub relation_status: Option<String>,
    pub address: Option<PropertyAddressPatch>,
    pub local_name: FieldPatch<String>,
    pub legal_owner_name: FieldPatch<String>,
    pub catastro_number: FieldPatch<String>,
    pub registry_entry: FieldPatch<String>,
    pub finca_number: FieldPatch<String>,
    pub registry_section: FieldPatch<String>,
    pub source_type: Option<String>,
    pub source_key: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropertyAddressPatch {
    pub address_line1: FieldPatch<String>,
    pub city: FieldPatch<String>,
    pub state_or_province: FieldPatch<String>,
    pub neighborhood: FieldPatch<String>,
    pub postal_code: FieldPatch<String>,
    pub country: FieldPatch<String>,
    pub iso_country_code: FieldPatch<String>,
}


#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyAdminSummary {
    pub id: String,
    pub name: String,
    pub status: String,
    pub location: Option<String>,
    pub list_price: Option<String>,
    pub property_type: Option<String>,
    pub is_active_listing: bool,
    pub is_published: bool,
    pub archived: bool,
    pub image_count: i64,
    pub video_count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyStellarDetails {
    pub listing_contract_date: Option<String>,
    pub expiration_date: Option<String>,
    pub listing_type: Option<String>,
    pub agent_mls_id: Option<String>,
    pub tax_id: Option<String>,
    pub tax_year: Option<String>,
    pub annual_tax: Option<String>,
    pub legal_description: Option<String>,
    pub zoning: Option<String>,
    pub total_area_sqft: Option<String>,
    pub heated_area_source: Option<String>,
    pub ownership_type: Option<String>,
    pub hoa_details: Option<String>,
    pub showing_instructions: Option<String>,
    pub occupant_type: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyAdminRecord {
    pub id: String,
    /// System and imported columns are visible in OPS without becoming writable input.
    pub source_metadata: std::collections::BTreeMap<String, serde_json::Value>,
    pub regrid_fields: std::collections::BTreeMap<String, serde_json::Value>,
    pub stellar_property_id: Option<String>,
    pub stellar_updated_at: Option<String>,
    pub name: String,
    pub slug: Option<String>,
    pub status: String,
    pub featured: bool,
    pub is_active_listing: bool,
    pub is_published: bool,
    pub property_type: Option<String>,
    pub has_ocean_view: bool,
    pub has_bay_view: bool,
    pub has_beach_view: bool,
    pub has_harbor_view: bool,
    pub has_island_view: bool,
    pub has_mountain_view: bool,
    pub has_sunrise_view: bool,
    pub has_sunset_view: bool,
    pub has_water_access: bool,
    pub has_beach_access: bool,
    pub has_pool: bool,
    pub has_generator: bool,
    pub has_solar: bool,
    pub is_furnished: bool,
    pub is_gated: bool,
    pub list_price: Option<String>,
    pub original_list_price: Option<String>,
    pub location: Option<String>,
    pub address_line1: Option<String>,
    pub street_number: Option<String>,
    pub street_name: Option<String>,
    pub unit_number: Option<String>,
    pub city: Option<String>,
    pub state_or_province: Option<String>,
    pub neighborhood: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub iso_country_code: Option<String>,
    pub latitude: Option<String>,
    pub longitude: Option<String>,
    pub bedrooms: Option<String>,
    pub bathrooms: Option<String>,
    pub bathrooms_full: Option<String>,
    pub bathrooms_half: Option<String>,
    pub square_feet: Option<String>,
    pub lot_size: Option<String>,
    pub lot_size_units: Option<String>,
    pub lot_size_sqft: Option<String>,
    pub road_frontage_feet: Option<String>,
    pub road_surface_type: Option<String>,
    pub lot_description: Option<String>,
    pub utilities_notes: Option<String>,
    pub catastro_number: Option<String>,
    pub buildability: Option<String>,
    pub slope_description: Option<String>,
    pub pool_potential: Option<String>,
    pub road_adjacency: Option<String>,
    pub utilities_availability: Option<String>,
    pub hoa_status: Option<String>,
    pub view_description: Option<String>,
    pub year_built: Option<String>,
    pub stories: Option<String>,
    pub parking_spaces: Option<String>,
    pub short_description: Option<String>,
    pub editorial_description: Option<String>,
    pub public_remarks: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub hero_title: Option<String>,
    pub tagline: Option<String>,
    pub architecture_notes: Option<String>,
    pub amenities_notes: Option<String>,
    pub lifestyle_notes: Option<String>,
    pub listing_agent_name: Option<String>,
    pub listing_agent_email: Option<String>,
    pub listing_agent_phone: Option<String>,
    pub listing_office: Option<String>,
    pub legal_owner_name: Option<String>,
    pub listing_identifier: Option<String>,
    pub registry_entry: Option<String>,
    pub finca_number: Option<String>,
    pub registry_section: Option<String>,
    pub seller_person_id: Option<String>,
    pub seller_name: Option<String>,
    pub archived: bool,
    pub image_count: i64,
    pub video_count: i64,
    pub document_count: i64,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub stellar: PropertyStellarDetails,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyAdminPageRequest {
    pub search: String,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyAdminPage {
    pub rows: Vec<PropertyAdminSummary>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatePropertyAdminRequest {
    pub name: String,
    pub property_type: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SavePropertyAdminRequest {
    pub property_id: String,
    pub name: String,
    pub slug: Option<String>,
    pub status: String,
    pub featured: bool,
    pub is_active_listing: bool,
    pub is_published: bool,
    pub property_type: Option<String>,
    pub has_ocean_view: bool,
    pub has_bay_view: bool,
    pub has_beach_view: bool,
    pub has_harbor_view: bool,
    pub has_island_view: bool,
    pub has_mountain_view: bool,
    pub has_sunrise_view: bool,
    pub has_sunset_view: bool,
    pub has_water_access: bool,
    pub has_beach_access: bool,
    pub has_pool: bool,
    pub has_generator: bool,
    pub has_solar: bool,
    pub is_furnished: bool,
    pub is_gated: bool,
    pub list_price: Option<String>,
    pub original_list_price: Option<String>,
    pub location: Option<String>,
    pub address_line1: Option<String>,
    pub street_number: Option<String>,
    pub street_name: Option<String>,
    pub unit_number: Option<String>,
    pub city: Option<String>,
    pub state_or_province: Option<String>,
    pub neighborhood: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub iso_country_code: Option<String>,
    pub latitude: Option<String>,
    pub longitude: Option<String>,
    pub bedrooms: Option<String>,
    pub bathrooms: Option<String>,
    pub bathrooms_full: Option<String>,
    pub bathrooms_half: Option<String>,
    pub square_feet: Option<String>,
    pub lot_size: Option<String>,
    pub lot_size_units: Option<String>,
    pub lot_size_sqft: Option<String>,
    pub road_frontage_feet: Option<String>,
    pub road_surface_type: Option<String>,
    pub lot_description: Option<String>,
    pub utilities_notes: Option<String>,
    pub catastro_number: Option<String>,
    pub buildability: Option<String>,
    pub slope_description: Option<String>,
    pub pool_potential: Option<String>,
    pub road_adjacency: Option<String>,
    pub utilities_availability: Option<String>,
    pub hoa_status: Option<String>,
    pub view_description: Option<String>,
    pub year_built: Option<String>,
    pub stories: Option<String>,
    pub parking_spaces: Option<String>,
    pub short_description: Option<String>,
    pub editorial_description: Option<String>,
    pub public_remarks: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub hero_title: Option<String>,
    pub tagline: Option<String>,
    pub architecture_notes: Option<String>,
    pub amenities_notes: Option<String>,
    pub lifestyle_notes: Option<String>,
    pub listing_agent_name: Option<String>,
    pub listing_agent_email: Option<String>,
    pub listing_agent_phone: Option<String>,
    pub listing_office: Option<String>,
    pub legal_owner_name: Option<String>,
    pub listing_identifier: Option<String>,
    pub registry_entry: Option<String>,
    pub finca_number: Option<String>,
    pub registry_section: Option<String>,
    pub seller_person_id: Option<String>,
    pub archived: bool,
    pub stellar: PropertyStellarDetails,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetPropertyDisplayNameRequest {
    pub property_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetPropertyStatusRequest {
    pub property_id: String,
    pub status: String,
}
