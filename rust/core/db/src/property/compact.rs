//! Moved from `property.rs` (move only): compact, one_line, canonical_address_line, map_property, map_relation, into_relation, normalize, apply_patch, explicit_patch_value, merge_address, get_on, find_by_address_on, PropertyDao, PropertyReadCache.

#[allow(unused_imports)]
use super::*;

pub(super) fn compact(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}

pub(super) fn one_line(value: Option<&str>) -> Option<String> {
    let parts: Vec<&str> = value?
        .lines()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    (!parts.is_empty()).then(|| parts.join(", "))
}

pub(super) fn canonical_address_line(row: &PropertyRow) -> Option<String> {
    if let Some(value) = one_line(row.address_line1.as_deref()) {
        return Some(value);
    }
    let street = [
        compact(row.street_number.as_deref()),
        compact(row.street_name.as_deref()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    if !street.is_empty() {
        return compact(row.unit_number.as_deref())
            .map(|unit| format!("{street}, {unit}"))
            .or(Some(street));
    }
    one_line(row.location.as_deref())
}

pub(super) fn map_property(row: PropertyRow) -> Property {
    let address_line1 = canonical_address_line(&row);
    let address = PropertyAddress {
        address_line1: address_line1.clone(),
        city: compact(row.city.as_deref()),
        state_or_province: compact(row.state_or_province.as_deref()),
        neighborhood: compact(row.neighborhood.as_deref()),
        postal_code: compact(row.postal_code.as_deref()),
        country: compact(row.country.as_deref()),
        iso_country_code: compact(row.iso_country_code.as_deref()),
    };
    let local_name = compact(row.name.as_deref());
    Property {
        id: row.id,
        display_name: local_name
            .clone()
            .or_else(|| address.address_line1.clone())
            .unwrap_or_else(|| "Property".into()),
        local_name,
        legal_owner_name: compact(row.legal_owner_name.as_deref()),
        catastro_number: compact(row.catastro_number.as_deref()),
        registry_entry: compact(row.registry_entry.as_deref()),
        finca_number: compact(row.finca_number.as_deref()),
        registry_section: compact(row.registry_section.as_deref()),
        address_line1,
        municipality: address.city.clone(),
        address,
        status: row.status,
        archived_at: row.archived_at.map(|value| value.to_rfc3339()),
    }
}

pub(super) fn map_relation(row: PropertyRelationRow) -> DbResult<PropertyForPerson> {
    let relation = PersonPropertyRelation::try_from(row.relation_type.as_str())
        .map_err(|error| DbFailure::schema_mismatch("property.map_relation", error))?;
    let property = map_property(PropertyRow {
        id: row.id,
        name: row.name,
        legal_owner_name: row.legal_owner_name,
        catastro_number: row.catastro_number,
        registry_entry: row.registry_entry,
        finca_number: row.finca_number,
        registry_section: row.registry_section,
        status: row.status,
        archived_at: row.archived_at,
        address_line1: row.address_line1,
        location: row.location,
        street_number: row.street_number,
        street_name: row.street_name,
        unit_number: row.unit_number,
        city: row.city,
        state_or_province: row.state_or_province,
        neighborhood: row.neighborhood,
        postal_code: row.postal_code,
        country: row.country,
        iso_country_code: row.iso_country_code,
    });
    Ok(PropertyForPerson {
        relation,
        relation_status: row.relation_status,
        property,
    })
}

impl CachedPropertyRelationRow {
    pub(super) fn into_relation(self) -> PropertyRelationRow {
        PropertyRelationRow {
            id: self.id,
            name: self.name,
            legal_owner_name: self.legal_owner_name,
            catastro_number: self.catastro_number,
            registry_entry: self.registry_entry,
            finca_number: self.finca_number,
            registry_section: self.registry_section,
            status: self.status,
            archived_at: self.archived_at,
            address_line1: self.address_line1,
            location: self.location,
            street_number: self.street_number,
            street_name: self.street_name,
            unit_number: self.unit_number,
            city: self.city,
            state_or_province: self.state_or_province,
            neighborhood: self.neighborhood,
            postal_code: self.postal_code,
            country: self.country,
            iso_country_code: self.iso_country_code,
            relation_type: self.relation_type,
            relation_status: self.relation_status,
        }
    }
}

pub(super) fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub(super) fn apply_patch(patch: &FieldPatch<String>, current: Option<String>) -> Option<String> {
    match patch {
        FieldPatch::Unchanged => current,
        FieldPatch::Set(value) => compact(value.as_deref()),
    }
}

pub(super) fn explicit_patch_value(patch: &FieldPatch<String>) -> Option<String> {
    match patch {
        FieldPatch::Unchanged => None,
        FieldPatch::Set(value) => compact(value.as_deref()),
    }
}

pub(super) fn merge_address(
    current: Option<&PropertyAddress>,
    patch: Option<&PropertyAddressPatch>,
) -> PropertyAddress {
    let empty = PropertyAddress {
        address_line1: None,
        city: None,
        state_or_province: None,
        neighborhood: None,
        postal_code: None,
        country: None,
        iso_country_code: None,
    };
    let base = current.unwrap_or(&empty);
    let Some(patch) = patch else {
        return base.clone();
    };
    PropertyAddress {
        address_line1: apply_patch(&patch.address_line1, base.address_line1.clone()),
        city: apply_patch(&patch.city, base.city.clone()),
        state_or_province: apply_patch(&patch.state_or_province, base.state_or_province.clone()),
        neighborhood: apply_patch(&patch.neighborhood, base.neighborhood.clone()),
        postal_code: apply_patch(&patch.postal_code, base.postal_code.clone()),
        country: apply_patch(&patch.country, base.country.clone()),
        iso_country_code: apply_patch(&patch.iso_country_code, base.iso_country_code.clone()),
    }
}

pub(super) async fn get_on(connection: &mut PgConnection, property_id: &str) -> DbResult<Option<Property>> {
    let row = sqlx::query_as::<_, PropertyRow>(property_sql!(
        "select ",
        " from property p where p.id = $1::uuid limit 1"
    ))
    .bind(property_id)
    .fetch_optional(connection)
    .await
    .map_err(|error| DbFailure::from_sqlx("property.get", &error))?;
    Ok(row.map(map_property))
}

pub(super) async fn find_by_address_on(
    connection: &mut PgConnection,
    request: &FindPropertyByAddressRequest,
) -> DbResult<Option<Property>> {
    let rows = sqlx::query_as::<_, PropertyRow>(property_sql!(
        "select ",
        " from property p
          where p.archived_at is null
            and ($1::text is null or lower(trim(coalesce(p.city, ''))) = lower(trim($1)))
            and ($2::text is null or lower(trim(coalesce(p.state_or_province, ''))) = lower(trim($2)))
            and ($3::text is null or lower(trim(coalesce(p.postal_code, ''))) = lower(trim($3)))
          order by p.updated_at desc nulls last, p.id asc
          limit 250"
    ))
    .bind(request.municipality.as_deref())
    .bind(request.state_or_province.as_deref())
    .bind(request.postal_code.as_deref())
    .fetch_all(connection)
    .await
    .map_err(|error| DbFailure::from_sqlx("property.find_by_address", &error))?;

    let target = normalize(&request.address_line1);
    Ok(rows
        .into_iter()
        .find(|row| normalize(canonical_address_line(row).as_deref().unwrap_or("")) == target)
        .map(map_property))
}

#[derive(Clone)]
pub struct PropertyDao {
    pub(super) db: Database,
    pub(super) read_cache: Arc<RwLock<Option<PropertyReadCache>>>,
}

#[derive(Clone, Default)]
pub(super) struct PropertyReadCache {
    pub(super) directory: Vec<PropertyAdminSummary>,
    pub(super) by_person: HashMap<String, PersonPropertyContext>,
}
