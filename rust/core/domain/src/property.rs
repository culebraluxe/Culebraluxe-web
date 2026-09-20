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

/// Preserves the TypeScript ingress distinction:
/// omitted => leave current value unchanged; Set(None) => explicitly clear it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum FieldPatch<T> {
    #[default]
    Unchanged,
    Set(Option<T>),
}

impl<T: Clone> FieldPatch<T> {
    pub fn apply(&self, current: Option<T>) -> Option<T> {
        match self {
            Self::Unchanged => current,
            Self::Set(value) => value.clone(),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_patch_distinguishes_unchanged_from_clear() {
        assert_eq!(
            FieldPatch::<String>::Unchanged.apply(Some("old".into())),
            Some("old".into())
        );
        assert_eq!(FieldPatch::<String>::Set(None).apply(Some("old".into())), None);
        assert_eq!(
            FieldPatch::Set(Some("new".to_string())).apply(Some("old".into())),
            Some("new".into())
        );
    }
}
