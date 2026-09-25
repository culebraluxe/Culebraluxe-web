use crate::{Database, DbFailure, DbResult, DbTransaction};
use chrono::{DateTime, Utc};
use domain::{
    CreatePropertyAdminRequest, FieldPatch, FindPropertyByAddressRequest, PersonPropertyContext,
    PersonPropertyRelation, Property, PropertyAddress, PropertyAddressPatch, PropertyAdminPage,
    PropertyAdminPageRequest, PropertyAdminRecord, PropertyAdminSummary, PropertyForPerson,
    PropertyStellarDetails, SavePropertyAdminRequest, SetPropertyDisplayNameRequest,
    SetPropertyStatusRequest, UpsertPropertyForPersonRequest,
};
use sqlx::{FromRow, PgConnection};

#[derive(Debug, FromRow)]
struct PropertyRow {
    id: String,
    name: Option<String>,
    legal_owner_name: Option<String>,
    catastro_number: Option<String>,
    registry_entry: Option<String>,
    finca_number: Option<String>,
    registry_section: Option<String>,
    status: String,
    archived_at: Option<DateTime<Utc>>,
    address_line1: Option<String>,
    location: Option<String>,
    street_number: Option<String>,
    street_name: Option<String>,
    unit_number: Option<String>,
    city: Option<String>,
    state_or_province: Option<String>,
    neighborhood: Option<String>,
    postal_code: Option<String>,
    country: Option<String>,
    iso_country_code: Option<String>,
}

#[derive(Debug, FromRow)]
struct PropertyRelationRow {
    id: String,
    name: Option<String>,
    legal_owner_name: Option<String>,
    catastro_number: Option<String>,
    registry_entry: Option<String>,
    finca_number: Option<String>,
    registry_section: Option<String>,
    status: String,
    archived_at: Option<DateTime<Utc>>,
    address_line1: Option<String>,
    location: Option<String>,
    street_number: Option<String>,
    street_name: Option<String>,
    unit_number: Option<String>,
    city: Option<String>,
    state_or_province: Option<String>,
    neighborhood: Option<String>,
    postal_code: Option<String>,
    country: Option<String>,
    iso_country_code: Option<String>,
    relation_type: String,
    relation_status: Option<String>,
}

#[derive(Debug, FromRow)]
struct PropertyAdminSummaryRow {
    id: String,
    name: String,
    status: String,
    location: Option<String>,
    list_price: Option<String>,
    property_type: Option<String>,
    is_active_listing: bool,
    is_published: bool,
    archived: bool,
    image_count: i64,
    video_count: i64,
}

#[derive(Debug, FromRow)]
struct PropertyAdminRecordRow {
    id: String,
    source_metadata: sqlx::types::Json<std::collections::BTreeMap<String, serde_json::Value>>,
    regrid_fields: sqlx::types::Json<std::collections::BTreeMap<String, serde_json::Value>>,
    stellar_property_id: Option<String>,
    stellar_updated_at: Option<String>,
    name: String,
    slug: Option<String>,
    status: String,
    featured: bool,
    is_active_listing: bool,
    is_published: bool,
    property_type: Option<String>,
    has_ocean_view: bool,
    has_bay_view: bool,
    has_beach_view: bool,
    has_harbor_view: bool,
    has_island_view: bool,
    has_mountain_view: bool,
    has_sunrise_view: bool,
    has_sunset_view: bool,
    has_water_access: bool,
    has_beach_access: bool,
    has_pool: bool,
    has_generator: bool,
    has_solar: bool,
    is_furnished: bool,
    is_gated: bool,
    list_price: Option<String>,
    original_list_price: Option<String>,
    location: Option<String>,
    address_line1: Option<String>,
    street_number: Option<String>,
    street_name: Option<String>,
    unit_number: Option<String>,
    city: Option<String>,
    state_or_province: Option<String>,
    neighborhood: Option<String>,
    postal_code: Option<String>,
    country: Option<String>,
    iso_country_code: Option<String>,
    latitude: Option<String>,
    longitude: Option<String>,
    bedrooms: Option<String>,
    bathrooms: Option<String>,
    bathrooms_full: Option<String>,
    bathrooms_half: Option<String>,
    square_feet: Option<String>,
    lot_size: Option<String>,
    lot_size_units: Option<String>,
    lot_size_acres: Option<String>,
    lot_size_sqft: Option<String>,
    road_frontage_feet: Option<String>,
    road_surface_type: Option<String>,
    lot_description: Option<String>,
    utilities_notes: Option<String>,
    catastro_number: Option<String>,
    buildability: Option<String>,
    slope_description: Option<String>,
    pool_potential: Option<String>,
    road_adjacency: Option<String>,
    utilities_availability: Option<String>,
    hoa_status: Option<String>,
    view_description: Option<String>,
    year_built: Option<String>,
    stories: Option<String>,
    parking_spaces: Option<String>,
    short_description: Option<String>,
    editorial_description: Option<String>,
    public_remarks: Option<String>,
    seo_title: Option<String>,
    seo_description: Option<String>,
    hero_title: Option<String>,
    tagline: Option<String>,
    architecture_notes: Option<String>,
    amenities_notes: Option<String>,
    lifestyle_notes: Option<String>,
    listing_agent_name: Option<String>,
    listing_agent_email: Option<String>,
    listing_agent_phone: Option<String>,
    listing_office: Option<String>,
    legal_owner_name: Option<String>,
    listing_identifier: Option<String>,
    registry_entry: Option<String>,
    finca_number: Option<String>,
    registry_section: Option<String>,
    seller_person_id: Option<String>,
    seller_name: Option<String>,
    archived: bool,
    image_count: i64,
    video_count: i64,
    document_count: i64,
    created_at: Option<String>,
    updated_at: Option<String>,
    listing_contract_date: Option<String>,
    expiration_date: Option<String>,
    listing_type: Option<String>,
    agent_mls_id: Option<String>,
    tax_id: Option<String>,
    tax_year: Option<String>,
    annual_tax: Option<String>,
    legal_description: Option<String>,
    zoning: Option<String>,
    total_area_sqft: Option<String>,
    heated_area_source: Option<String>,
    ownership_type: Option<String>,
    hoa_details: Option<String>,
    showing_instructions: Option<String>,
    occupant_type: Option<String>,
}

fn map_admin_summary(row: PropertyAdminSummaryRow) -> PropertyAdminSummary {
    PropertyAdminSummary {
        id: row.id,
        name: row.name,
        status: row.status,
        location: row.location,
        list_price: row.list_price,
        property_type: row.property_type,
        is_active_listing: row.is_active_listing,
        is_published: row.is_published,
        archived: row.archived,
        image_count: row.image_count,
        video_count: row.video_count,
    }
}

fn map_admin_record(row: PropertyAdminRecordRow) -> PropertyAdminRecord {
    PropertyAdminRecord {
        id: row.id,
        source_metadata: row.source_metadata.0,
        regrid_fields: row.regrid_fields.0,
        stellar_property_id: row.stellar_property_id,
        stellar_updated_at: row.stellar_updated_at,
        name: row.name,
        slug: row.slug,
        status: row.status,
        featured: row.featured,
        is_active_listing: row.is_active_listing,
        is_published: row.is_published,
        property_type: row.property_type,
        has_ocean_view: row.has_ocean_view,
        has_bay_view: row.has_bay_view,
        has_beach_view: row.has_beach_view,
        has_harbor_view: row.has_harbor_view,
        has_island_view: row.has_island_view,
        has_mountain_view: row.has_mountain_view,
        has_sunrise_view: row.has_sunrise_view,
        has_sunset_view: row.has_sunset_view,
        has_water_access: row.has_water_access,
        has_beach_access: row.has_beach_access,
        has_pool: row.has_pool,
        has_generator: row.has_generator,
        has_solar: row.has_solar,
        is_furnished: row.is_furnished,
        is_gated: row.is_gated,
        list_price: row.list_price,
        original_list_price: row.original_list_price,
        location: row.location,
        address_line1: row.address_line1,
        street_number: row.street_number,
        street_name: row.street_name,
        unit_number: row.unit_number,
        city: row.city,
        state_or_province: row.state_or_province,
        neighborhood: row.neighborhood,
        postal_code: row.postal_code,
        country: row.country,
        iso_country_code: row.iso_country_code,
        latitude: row.latitude,
        longitude: row.longitude,
        bedrooms: row.bedrooms,
        bathrooms: row.bathrooms,
        bathrooms_full: row.bathrooms_full,
        bathrooms_half: row.bathrooms_half,
        square_feet: row.square_feet,
        lot_size: row.lot_size,
        lot_size_units: row.lot_size_units,
        lot_size_acres: row.lot_size_acres,
        lot_size_sqft: row.lot_size_sqft,
        road_frontage_feet: row.road_frontage_feet,
        road_surface_type: row.road_surface_type,
        lot_description: row.lot_description,
        utilities_notes: row.utilities_notes,
        catastro_number: row.catastro_number,
        buildability: row.buildability,
        slope_description: row.slope_description,
        pool_potential: row.pool_potential,
        road_adjacency: row.road_adjacency,
        utilities_availability: row.utilities_availability,
        hoa_status: row.hoa_status,
        view_description: row.view_description,
        year_built: row.year_built,
        stories: row.stories,
        parking_spaces: row.parking_spaces,
        short_description: row.short_description,
        editorial_description: row.editorial_description,
        public_remarks: row.public_remarks,
        seo_title: row.seo_title,
        seo_description: row.seo_description,
        hero_title: row.hero_title,
        tagline: row.tagline,
        architecture_notes: row.architecture_notes,
        amenities_notes: row.amenities_notes,
        lifestyle_notes: row.lifestyle_notes,
        listing_agent_name: row.listing_agent_name,
        listing_agent_email: row.listing_agent_email,
        listing_agent_phone: row.listing_agent_phone,
        listing_office: row.listing_office,
        legal_owner_name: row.legal_owner_name,
        listing_identifier: row.listing_identifier,
        registry_entry: row.registry_entry,
        finca_number: row.finca_number,
        registry_section: row.registry_section,
        seller_person_id: row.seller_person_id,
        seller_name: row.seller_name,
        archived: row.archived,
        image_count: row.image_count,
        video_count: row.video_count,
        document_count: row.document_count,
        created_at: row.created_at,
        updated_at: row.updated_at,
        stellar: PropertyStellarDetails {
            listing_contract_date: row.listing_contract_date,
            expiration_date: row.expiration_date,
            listing_type: row.listing_type,
            agent_mls_id: row.agent_mls_id,
            tax_id: row.tax_id,
            tax_year: row.tax_year,
            annual_tax: row.annual_tax,
            legal_description: row.legal_description,
            zoning: row.zoning,
            total_area_sqft: row.total_area_sqft,
            heated_area_source: row.heated_area_source,
            ownership_type: row.ownership_type,
            hoa_details: row.hoa_details,
            showing_instructions: row.showing_instructions,
            occupant_type: row.occupant_type,
        },
    }
}

macro_rules! property_sql {
    ($prefix:literal, $suffix:literal) => {
        concat!(
            $prefix,
            "p.id::text as id, p.name, p.legal_owner_name, p.catastro_number, ",
            "p.registry_entry, p.finca_number, p.registry_section, ",
            "p.status, p.archived_at, p.address_line1, p.location, ",
            "p.street_number, p.street_name, p.unit_number, p.city, p.state_or_province, ",
            "p.neighborhood, p.postal_code, p.country, p.iso_country_code",
            $suffix
        )
    };
}

fn compact(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}

fn one_line(value: Option<&str>) -> Option<String> {
    let parts: Vec<&str> = value?
        .lines()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    (!parts.is_empty()).then(|| parts.join(", "))
}

fn canonical_address_line(row: &PropertyRow) -> Option<String> {
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

fn map_property(row: PropertyRow) -> Property {
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

fn map_relation(row: PropertyRelationRow) -> DbResult<PropertyForPerson> {
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

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn apply_patch(patch: &FieldPatch<String>, current: Option<String>) -> Option<String> {
    match patch {
        FieldPatch::Unchanged => current,
        FieldPatch::Set(value) => compact(value.as_deref()),
    }
}

fn explicit_patch_value(patch: &FieldPatch<String>) -> Option<String> {
    match patch {
        FieldPatch::Unchanged => None,
        FieldPatch::Set(value) => compact(value.as_deref()),
    }
}

fn merge_address(
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

async fn get_on(connection: &mut PgConnection, property_id: &str) -> DbResult<Option<Property>> {
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

async fn find_by_address_on(
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
    db: Database,
}

impl PropertyDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get(&self, property_id: &str) -> DbResult<Option<Property>> {
        let row = sqlx::query_as::<_, PropertyRow>(property_sql!(
            "select ",
            " from property p where p.id = $1::uuid limit 1"
        ))
        .bind(property_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("property.get", &error))?;
        Ok(row.map(map_property))
    }

    pub async fn find_by_address(
        &self,
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
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("property.find_by_address", &error))?;

        let target = normalize(&request.address_line1);
        Ok(rows
            .into_iter()
            .find(|row| normalize(canonical_address_line(row).as_deref().unwrap_or("")) == target)
            .map(map_property))
    }

    pub async fn for_person(&self, person_id: &str) -> DbResult<PersonPropertyContext> {
        let canonical = sqlx::query_as::<_, PropertyRelationRow>(property_sql!(
            "select ",
            ", pp.relation_type, pp.relation_status
             from person_property pp
             join property p on p.id = pp.property_id
             where pp.person_id = $1::uuid and p.archived_at is null"
        ))
        .bind(person_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("property.for_person.canonical", &error))?;

        let interests = sqlx::query_as::<_, PropertyRelationRow>(property_sql!(
            "select ",
            ", 'interest'::text as relation_type, pi.status as relation_status
             from property_interest pi
             join property p on p.id = pi.property_id
             where pi.person_id = $1::uuid and p.archived_at is null"
        ))
        .bind(person_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("property.for_person.interest", &error))?;

        let sellers = sqlx::query_as::<_, PropertyRelationRow>(property_sql!(
            "select ",
            ", 'physical_property'::text as relation_type, null::text as relation_status
             from property p
             where p.seller_person_id = $1::uuid and p.archived_at is null"
        ))
        .bind(person_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("property.for_person.seller", &error))?;

        let mut properties = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for row in canonical.into_iter().chain(interests).chain(sellers) {
            let linked = map_relation(row)?;
            let key = format!("{}:{}", linked.property.id, linked.relation.as_str());
            if seen.insert(key) {
                properties.push(linked);
            }
        }
        Ok(PersonPropertyContext {
            person_id: person_id.to_owned(),
            properties,
        })
    }

    pub async fn upsert_for_person(
        &self,
        request: &UpsertPropertyForPersonRequest,
    ) -> DbResult<PropertyForPerson> {
        let mut tx = self.db.begin("property.upsert_for_person").await?;
        let result = upsert_for_person_tx(&mut tx, request).await;
        match result {
            Ok(value) => {
                tx.commit().await?;
                Ok(value)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }

    pub async fn set_display_name(
        &self,
        request: &SetPropertyDisplayNameRequest,
    ) -> DbResult<Option<Property>> {
        let row = sqlx::query_as::<_, PropertyRow>(property_sql!(
            "update property p set name=$2, updated_at=now()
             where p.id=$1::uuid returning ",
            ""
        ))
        .bind(&request.property_id)
        .bind(request.display_name.trim())
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("property.set_display_name", &error))?;
        Ok(row.map(map_property))
    }

    pub async fn set_status(
        &self,
        request: &SetPropertyStatusRequest,
    ) -> DbResult<Option<Property>> {
        let row = sqlx::query_as::<_, PropertyRow>(property_sql!(
            "update property p set status=$2, updated_at=now()
             where p.id=$1::uuid returning ",
            ""
        ))
        .bind(&request.property_id)
        .bind(request.status.trim())
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("property.set_status", &error))?;
        Ok(row.map(map_property))
    }

    pub async fn admin_page(
        &self,
        request: &PropertyAdminPageRequest,
    ) -> DbResult<PropertyAdminPage> {
        let search = compact(Some(request.search.as_str())).map(|value| format!("%{value}%"));
        let page = request.page.max(1);
        let page_size = request.page_size.clamp(1, 100);
        let offset = (page - 1) * page_size;

        let total = sqlx::query_scalar::<_, i64>(
            r#"
            select count(*)::bigint
            from property p
            where ($1::text is null or (
                coalesce(p.name, '') ilike $1
                or coalesce(p.slug, '') ilike $1
                or coalesce(p.location, '') ilike $1
                or coalesce(p.city, '') ilike $1
                or coalesce(p.neighborhood, '') ilike $1
                or coalesce(p.listing_identifier, '') ilike $1
                or coalesce(p.catastro_number, '') ilike $1
            ))
            "#,
        )
        .bind(search.as_deref())
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("property.admin.count", &error))?;

        let rows = sqlx::query_as::<_, PropertyAdminSummaryRow>(
            r#"
            select
                p.id::text as id,
                coalesce(nullif(trim(p.name), ''), 'Unnamed property') as name,
                p.status,
                coalesce(p.location, p.address_line1, p.city) as location,
                p.list_price::text as list_price,
                p.property_type,
                p.is_active_listing,
                p.is_published,
                (p.archived_at is not null) as archived,
                (select count(*)::bigint from property_media pm join media m on m.id = pm.media_id where pm.property_id = p.id and m.media_type = 'image') as image_count,
                (select count(*)::bigint from property_media pm join media m on m.id = pm.media_id where pm.property_id = p.id and m.media_type = 'video') as video_count
            from property p
            where ($1::text is null or (
                coalesce(p.name, '') ilike $1
                or coalesce(p.slug, '') ilike $1
                or coalesce(p.location, '') ilike $1
                or coalesce(p.city, '') ilike $1
                or coalesce(p.neighborhood, '') ilike $1
                or coalesce(p.listing_identifier, '') ilike $1
                or coalesce(p.catastro_number, '') ilike $1
            ))
            order by
                case when p.archived_at is null then 0 else 1 end,
                case when p.is_active_listing then 0 else 1 end,
                coalesce(nullif(trim(p.name), ''), p.address_line1, p.id::text)
            limit $2 offset $3
            "#,
        )
        .bind(search.as_deref())
        .bind(page_size)
        .bind(offset)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("property.admin.page", &error))?;

        Ok(PropertyAdminPage {
            rows: rows.into_iter().map(map_admin_summary).collect(),
            total,
            page,
            page_size,
        })
    }

    pub async fn admin_get(&self, property_id: &str) -> DbResult<Option<PropertyAdminRecord>> {
        let row = sqlx::query_as::<_, PropertyAdminRecordRow>(
            r#"
            select
                p.id::text as id,
                jsonb_build_object(
                    'id', p.id, 'listing_user_id', p.listing_user_id,
                    'created_at', p.created_at, 'updated_at', p.updated_at,
                    'source_type', p.source_type, 'source_provider', p.source_provider,
                    'source_listing_key', p.source_listing_key,
                    'source_modified_at', p.source_modified_at,
                    'last_synced_at', p.last_synced_at,
                    'catastro_source', p.catastro_source,
                    'lot_size', p.lot_size,
                    'lot_size_units', p.lot_size_units
                ) as source_metadata,
                coalesce((select jsonb_object_agg(key, value)
                    from jsonb_each(to_jsonb(p)) where key like 'regrid_%'), '{}'::jsonb) as regrid_fields,
                s.property_id::text as stellar_property_id,
                s.updated_at::text as stellar_updated_at,
                coalesce(nullif(trim(p.name), ''), 'Unnamed property') as name,
                p.slug,
                p.status,
                p.featured,
                p.is_active_listing,
                p.is_published,
                p.property_type,
                p.has_ocean_view,
                p.has_bay_view,
                p.has_beach_view,
                p.has_harbor_view,
                p.has_island_view,
                p.has_mountain_view,
                p.has_sunrise_view,
                p.has_sunset_view,
                p.has_water_access,
                p.has_beach_access,
                p.has_pool,
                p.has_generator,
                p.has_solar,
                p.is_furnished,
                p.is_gated,
                p.list_price::text as list_price,
                p.original_list_price::text as original_list_price,
                p.location,
                p.address_line1,
                p.street_number,
                p.street_name,
                p.unit_number,
                p.city,
                p.state_or_province,
                p.neighborhood,
                p.postal_code,
                p.country,
                p.iso_country_code,
                p.latitude::text as latitude,
                p.longitude::text as longitude,
                p.bedrooms::text as bedrooms,
                p.bathrooms::text as bathrooms,
                p.bathrooms_full::text as bathrooms_full,
                p.bathrooms_half::text as bathrooms_half,
                p.square_feet::text as square_feet,
                p.lot_size::text as lot_size,
                p.lot_size_units,
                p.lot_size_acres::text as lot_size_acres,
                p.lot_size_sqft::text as lot_size_sqft,
                p.road_frontage_feet::text as road_frontage_feet,
                p.road_surface_type,
                p.lot_description,
                p.utilities_notes,
                p.catastro_number,
                p.buildability,
                p.slope_description,
                p.pool_potential,
                p.road_adjacency,
                p.utilities_availability,
                p.hoa_status,
                p.view_description,
                p.year_built::text as year_built,
                p.stories::text as stories,
                p.parking_spaces::text as parking_spaces,
                p.short_description,
                p.editorial_description,
                p.public_remarks,
                p.seo_title,
                p.seo_description,
                p.hero_title,
                p.tagline,
                p.architecture_notes,
                p.amenities_notes,
                p.lifestyle_notes,
                p.listing_agent_name,
                p.listing_agent_email,
                p.listing_agent_phone,
                p.listing_office,
                p.legal_owner_name,
                p.listing_identifier,
                p.registry_entry,
                p.finca_number,
                p.registry_section,
                p.seller_person_id::text as seller_person_id,
                seller.display_name as seller_name,
                (p.archived_at is not null) as archived,
                (select count(*)::bigint from property_media pm join media m on m.id = pm.media_id where pm.property_id = p.id and m.media_type = 'image') as image_count,
                (select count(*)::bigint from property_media pm join media m on m.id = pm.media_id where pm.property_id = p.id and m.media_type = 'video') as video_count,
                (select count(*)::bigint from property_media pm join media m on m.id = pm.media_id where pm.property_id = p.id and m.media_type = 'document') as document_count,
                p.created_at::text as created_at,
                p.updated_at::text as updated_at,
                s.listing_contract_date::text as listing_contract_date,
                s.expiration_date::text as expiration_date,
                s.listing_type,
                s.agent_mls_id,
                s.tax_id,
                s.tax_year::text as tax_year,
                s.annual_tax::text as annual_tax,
                s.legal_description,
                s.zoning,
                s.total_area_sqft::text as total_area_sqft,
                s.heated_area_source,
                s.ownership_type,
                s.hoa_details,
                s.showing_instructions,
                s.occupant_type
            from property p
            left join person seller on seller.id = p.seller_person_id
            left join property_stellar_listing s on s.property_id = p.id
            where p.id = $1::uuid
            limit 1
            "#,
        )
        .bind(property_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("property.admin.get", &error))?;

        Ok(row.map(map_admin_record))
    }

    pub async fn admin_create(
        &self,
        request: &CreatePropertyAdminRequest,
    ) -> DbResult<PropertyAdminRecord> {
        let id = uuid::Uuid::new_v4().to_string();
        // THE SLUG IS GENERATED HERE BECAUSE NOTHING ELSE SHOULD HAVE TO KNOW IT EXISTS.
        //
        // A listing without a slug is invisible to buyers: the public rows route drops any row whose slug is null,
        // because a listing nobody can open must not be offered as one. This insert used to omit the column
        // entirely, so every Property created in OPS was born unreachable — correct on screen, absent from the site,
        // and nothing said why. Deriving it from the name means a new Property has an address before anyone looks
        // for one, and the URL is still editable afterwards.
        //
        // A name that is already taken gets the row's own id appended rather than failing the insert: two listings
        // can legitimately share a name, and colliding URLs are worse than an ugly one.
        sqlx::query(
            r#"
            with candidate as (
                select nullif(trim(both '-' from regexp_replace(lower($2), '[^a-z0-9]+', '-', 'g')), '') as base
            )
            insert into property (id, name, property_type, status, is_active_listing, is_published, slug)
            values (
                $1::uuid, $2, $3, 'prospect', false, false,
                (
                    select case
                        when candidate.base is null then null
                        when exists (select 1 from property p where p.slug = candidate.base)
                            -- $1 IS A UUID, AND substr() HAS NO uuid OVERLOAD. Without the cast this expression
                            -- raises "function substr(uuid, integer, integer) does not exist" — which would have
                            -- failed the INSERT for any Property whose name collides with an existing slug. Caught by
                            -- running the statement against the real database instead of trusting the shape of it.
                            then candidate.base || '-' || substr($1::text, 1, 6)
                        else candidate.base
                    end
                    from candidate
                )
            )
            "#,
        )
        .bind(&id)
        .bind(request.name.trim())
        .bind(
            request
                .property_type
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        )
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("property.admin.create", &error))?;

        self.admin_get(&id).await?.ok_or_else(|| {
            DbFailure::schema_mismatch(
                "property.admin.create",
                "created Property could not be reloaded",
            )
        })
    }

    pub async fn admin_save(
        &self,
        request: &SavePropertyAdminRequest,
    ) -> DbResult<Option<PropertyAdminRecord>> {
        let mut tx = self.db.begin("property.admin.save").await?;

        let updated = sqlx::query_scalar::<_, String>(
            r#"
            update property
            set
                name = $2,
                slug = nullif($3::text, ''),
                status = $4,
                featured = $5,
                -- STATUS IS THE ONLY SWITCH. These two columns mirror it rather than deciding anything, so a form can no
                -- longer set "active listing" and "published" independently of the status and quietly create a Property
                -- that is active and invisible, or archived and published. Nothing on the website reads them; they are
                -- kept in step here only so the data cannot contradict itself.
                is_active_listing = ($4 in ('active', 'under_contract', 'sold')),
                is_published = ($4 in ('active', 'under_contract', 'sold')),
                property_type = nullif($8::text, ''),
                list_price = nullif($9::text, '')::numeric,
                location = nullif($10::text, ''),
                address_line1 = nullif($11::text, ''),
                street_number = nullif($12::text, ''),
                street_name = nullif($13::text, ''),
                unit_number = nullif($14::text, ''),
                city = nullif($15::text, ''),
                state_or_province = nullif($16::text, ''),
                neighborhood = nullif($17::text, ''),
                postal_code = nullif($18::text, ''),
                country = nullif($19::text, ''),
                iso_country_code = nullif($20::text, ''),
                latitude = nullif($21::text, '')::numeric,
                longitude = nullif($22::text, '')::numeric,
                bedrooms = nullif($23::text, '')::numeric,
                bathrooms = nullif($24::text, '')::numeric,
                bathrooms_full = nullif($25::text, '')::numeric,
                bathrooms_half = nullif($26::text, '')::numeric,
                square_feet = nullif($27::text, '')::integer,
                lot_size = nullif($28::text, '')::numeric,
                lot_size_units = nullif($29::text, ''),
                year_built = nullif($30::text, '')::integer,
                stories = nullif($31::text, '')::numeric,
                parking_spaces = nullif($32::text, '')::integer,
                short_description = nullif($33::text, ''),
                editorial_description = nullif($34::text, ''),
                public_remarks = nullif($35::text, ''),
                listing_agent_name = nullif($36::text, ''),
                listing_agent_email = nullif($37::text, ''),
                listing_agent_phone = nullif($38::text, ''),
                listing_office = nullif($39::text, ''),
                legal_owner_name = nullif($40::text, ''),
                listing_identifier = nullif($41::text, ''),
                registry_entry = nullif($42::text, ''),
                finca_number = nullif($43::text, ''),
                registry_section = nullif($44::text, ''),
                seller_person_id = nullif($45::text, '')::uuid,
                archived_at = case when $4 = 'archived' or $46 then coalesce(archived_at, now()) else null end,
                updated_at = now(),
                has_ocean_view = $47,
                has_bay_view = $48,
                has_beach_view = $49,
                has_harbor_view = $50,
                has_island_view = $51,
                has_mountain_view = $52,
                has_sunrise_view = $53,
                has_sunset_view = $54,
                has_water_access = $55,
                has_beach_access = $56,
                has_pool = $57,
                has_generator = $58,
                has_solar = $59,
                is_furnished = $60,
                is_gated = $61,
                hero_title = nullif($62::text, ''),
                tagline = nullif($63::text, ''),
                architecture_notes = nullif($64::text, ''),
                amenities_notes = nullif($65::text, ''),
                lifestyle_notes = nullif($66::text, ''),
                lot_size_sqft = nullif($67::text, '')::numeric,
                road_frontage_feet = nullif($68::text, '')::numeric,
                road_surface_type = nullif($69::text, ''),
                lot_description = nullif($70::text, ''),
                utilities_notes = nullif($71::text, ''),
                original_list_price = nullif($72::text, '')::numeric,
                seo_title = nullif($73::text, ''),
                seo_description = nullif($74::text, ''),
                catastro_number = nullif($75::text, ''),
                buildability = nullif($76::text, ''),
                slope_description = nullif($77::text, ''),
                pool_potential = nullif($78::text, ''),
                road_adjacency = nullif($79::text, ''),
                utilities_availability = nullif($80::text, ''),
                hoa_status = nullif($81::text, ''),
                view_description = nullif($82::text, ''),
                lot_size_acres = nullif($83::text, '')::numeric
            where id = $1::uuid
            returning id::text
            "#,
        )
        .bind(&request.property_id)
        .bind(request.name.trim())
        .bind(request.slug.as_deref())
        .bind(request.status.trim())
        .bind(request.featured)
        .bind(request.is_active_listing)
        .bind(request.is_published)
        .bind(request.property_type.as_deref())
        .bind(request.list_price.as_deref())
        .bind(request.location.as_deref())
        .bind(request.address_line1.as_deref())
        .bind(request.street_number.as_deref())
        .bind(request.street_name.as_deref())
        .bind(request.unit_number.as_deref())
        .bind(request.city.as_deref())
        .bind(request.state_or_province.as_deref())
        .bind(request.neighborhood.as_deref())
        .bind(request.postal_code.as_deref())
        .bind(request.country.as_deref())
        .bind(request.iso_country_code.as_deref())
        .bind(request.latitude.as_deref())
        .bind(request.longitude.as_deref())
        .bind(request.bedrooms.as_deref())
        .bind(request.bathrooms.as_deref())
        .bind(request.bathrooms_full.as_deref())
        .bind(request.bathrooms_half.as_deref())
        .bind(request.square_feet.as_deref())
        .bind(request.lot_size.as_deref())
        .bind(request.lot_size_units.as_deref())
        .bind(request.year_built.as_deref())
        .bind(request.stories.as_deref())
        .bind(request.parking_spaces.as_deref())
        .bind(request.short_description.as_deref())
        .bind(request.editorial_description.as_deref())
        .bind(request.public_remarks.as_deref())
        .bind(request.listing_agent_name.as_deref())
        .bind(request.listing_agent_email.as_deref())
        .bind(request.listing_agent_phone.as_deref())
        .bind(request.listing_office.as_deref())
        .bind(request.legal_owner_name.as_deref())
        .bind(request.listing_identifier.as_deref())
        .bind(request.registry_entry.as_deref())
        .bind(request.finca_number.as_deref())
        .bind(request.registry_section.as_deref())
        .bind(request.seller_person_id.as_deref())
        .bind(request.archived)
        .bind(request.has_ocean_view)
        .bind(request.has_bay_view)
        .bind(request.has_beach_view)
        .bind(request.has_harbor_view)
        .bind(request.has_island_view)
        .bind(request.has_mountain_view)
        .bind(request.has_sunrise_view)
        .bind(request.has_sunset_view)
        .bind(request.has_water_access)
        .bind(request.has_beach_access)
        .bind(request.has_pool)
        .bind(request.has_generator)
        .bind(request.has_solar)
        .bind(request.is_furnished)
        .bind(request.is_gated)
        .bind(request.hero_title.as_deref())
        .bind(request.tagline.as_deref())
        .bind(request.architecture_notes.as_deref())
        .bind(request.amenities_notes.as_deref())
        .bind(request.lifestyle_notes.as_deref())
        .bind(request.lot_size_sqft.as_deref())
        .bind(request.road_frontage_feet.as_deref())
        .bind(request.road_surface_type.as_deref())
        .bind(request.lot_description.as_deref())
        .bind(request.utilities_notes.as_deref())
        .bind(request.original_list_price.as_deref())
        .bind(request.seo_title.as_deref())
        .bind(request.seo_description.as_deref())
        .bind(request.catastro_number.as_deref())
        .bind(request.buildability.as_deref())
        .bind(request.slope_description.as_deref())
        .bind(request.pool_potential.as_deref())
        .bind(request.road_adjacency.as_deref())
        .bind(request.utilities_availability.as_deref())
        .bind(request.hoa_status.as_deref())
        .bind(request.view_description.as_deref())
        .bind(request.lot_size_acres.as_deref())
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("property.admin.save.property", &error))?;

        if updated.is_none() {
            let _ = tx.rollback().await;
            return Ok(None);
        }

        let stellar = &request.stellar;
        sqlx::query(
            r#"
            insert into property_stellar_listing (
                property_id, listing_contract_date, expiration_date, listing_type, agent_mls_id,
                tax_id, tax_year, annual_tax, legal_description, zoning, total_area_sqft,
                heated_area_source, ownership_type, hoa_details, showing_instructions, occupant_type
            ) values (
                $1::uuid,
                nullif($2::text, '')::date,
                nullif($3::text, '')::date,
                nullif($4::text, ''),
                nullif($5::text, ''),
                nullif($6::text, ''),
                nullif($7::text, '')::integer,
                nullif($8::text, '')::numeric,
                nullif($9::text, ''),
                nullif($10::text, ''),
                nullif($11::text, '')::numeric,
                nullif($12::text, ''),
                nullif($13::text, ''),
                nullif($14::text, ''),
                nullif($15::text, ''),
                nullif($16::text, '')
            )
            on conflict (property_id) do update set
                listing_contract_date = excluded.listing_contract_date,
                expiration_date = excluded.expiration_date,
                listing_type = excluded.listing_type,
                agent_mls_id = excluded.agent_mls_id,
                tax_id = excluded.tax_id,
                tax_year = excluded.tax_year,
                annual_tax = excluded.annual_tax,
                legal_description = excluded.legal_description,
                zoning = excluded.zoning,
                total_area_sqft = excluded.total_area_sqft,
                heated_area_source = excluded.heated_area_source,
                ownership_type = excluded.ownership_type,
                hoa_details = excluded.hoa_details,
                showing_instructions = excluded.showing_instructions,
                occupant_type = excluded.occupant_type,
                updated_at = now()
            "#,
        )
        .bind(&request.property_id)
        .bind(stellar.listing_contract_date.as_deref())
        .bind(stellar.expiration_date.as_deref())
        .bind(stellar.listing_type.as_deref())
        .bind(stellar.agent_mls_id.as_deref())
        .bind(stellar.tax_id.as_deref())
        .bind(stellar.tax_year.as_deref())
        .bind(stellar.annual_tax.as_deref())
        .bind(stellar.legal_description.as_deref())
        .bind(stellar.zoning.as_deref())
        .bind(stellar.total_area_sqft.as_deref())
        .bind(stellar.heated_area_source.as_deref())
        .bind(stellar.ownership_type.as_deref())
        .bind(stellar.hoa_details.as_deref())
        .bind(stellar.showing_instructions.as_deref())
        .bind(stellar.occupant_type.as_deref())
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("property.admin.save.stellar", &error))?;

        tx.commit().await?;
        self.admin_get(&request.property_id).await
    }
}

async fn upsert_for_person_tx(
    tx: &mut DbTransaction,
    request: &UpsertPropertyForPersonRequest,
) -> DbResult<PropertyForPerson> {
    let mut current = match request
        .property_id
        .as_deref()
        .and_then(|v| compact(Some(v)))
    {
        Some(id) => get_on(tx.connection(), &id).await?,
        None => None,
    };

    if current.is_none() && request.property_id.is_none() {
        if let Some(line) = request
            .address
            .as_ref()
            .and_then(|address| explicit_patch_value(&address.address_line1))
        {
            current = find_by_address_on(
                tx.connection(),
                &FindPropertyByAddressRequest {
                    address_line1: line,
                    municipality: request
                        .address
                        .as_ref()
                        .and_then(|value| explicit_patch_value(&value.city)),
                    state_or_province: request
                        .address
                        .as_ref()
                        .and_then(|value| explicit_patch_value(&value.state_or_province)),
                    postal_code: request
                        .address
                        .as_ref()
                        .and_then(|value| explicit_patch_value(&value.postal_code)),
                },
            )
            .await?;
        }
    }

    let address = merge_address(
        current.as_ref().map(|value| &value.address),
        request.address.as_ref(),
    );
    let local_name = apply_patch(
        &request.local_name,
        current.as_ref().and_then(|value| value.local_name.clone()),
    );
    let legal_owner_name = apply_patch(
        &request.legal_owner_name,
        current
            .as_ref()
            .and_then(|value| value.legal_owner_name.clone()),
    );
    let catastro_number = apply_patch(
        &request.catastro_number,
        current
            .as_ref()
            .and_then(|value| value.catastro_number.clone()),
    );
    let registry_entry = apply_patch(
        &request.registry_entry,
        current
            .as_ref()
            .and_then(|value| value.registry_entry.clone()),
    );
    let finca_number = apply_patch(
        &request.finca_number,
        current
            .as_ref()
            .and_then(|value| value.finca_number.clone()),
    );
    let registry_section = apply_patch(
        &request.registry_section,
        current
            .as_ref()
            .and_then(|value| value.registry_section.clone()),
    );

    if current.is_none() && address.address_line1.is_none() && local_name.is_none() {
        return Err(DbFailure::schema_mismatch(
            "property.upsert_for_person",
            "Property requires an address or local name before creation",
        ));
    }

    let row = if let Some(existing) = current {
        sqlx::query_as::<_, PropertyRow>(property_sql!(
            "update property p
             set name=$2, legal_owner_name=$3, catastro_number=$4,
                 registry_entry=$5, finca_number=$6, registry_section=$7,
                 address_line1=$8, city=$9, state_or_province=$10,
                 neighborhood=$11, postal_code=$12, country=$13,
                 iso_country_code=$14, updated_at=now()
             where p.id=$1::uuid returning ",
            ""
        ))
        .bind(&existing.id)
        .bind(local_name)
        .bind(legal_owner_name)
        .bind(catastro_number)
        .bind(registry_entry)
        .bind(finca_number)
        .bind(registry_section)
        .bind(address.address_line1.clone())
        .bind(address.city.clone())
        .bind(address.state_or_province.clone())
        .bind(address.neighborhood.clone())
        .bind(address.postal_code.clone())
        .bind(address.country.clone())
        .bind(address.iso_country_code.clone())
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("property.upsert_for_person.update", &error))?
    } else {
        sqlx::query_as::<_, PropertyRow>(property_sql!(
            "insert into property as p (
                 name, legal_owner_name, catastro_number, registry_entry,
                 finca_number, registry_section, address_line1, city,
                 state_or_province, neighborhood, postal_code, country,
                 iso_country_code, source_type
             )
             values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
             returning ",
            ""
        ))
        .bind(local_name)
        .bind(legal_owner_name)
        .bind(catastro_number)
        .bind(registry_entry)
        .bind(finca_number)
        .bind(registry_section)
        .bind(address.address_line1.clone())
        .bind(address.city.clone())
        .bind(address.state_or_province.clone())
        .bind(address.neighborhood.clone())
        .bind(address.postal_code.clone())
        .bind(address.country.clone())
        .bind(address.iso_country_code.clone())
        .bind(request.source_type.as_deref().unwrap_or("manual"))
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("property.upsert_for_person.insert", &error))?
    };

    let property = map_property(row);
    sqlx::query(
        r#"
        insert into person_property (
            person_id, property_id, relation_type, relation_status, source_type, source_key
        )
        values ($1::uuid,$2::uuid,$3,$4,$5,$6)
        on conflict (person_id, property_id, relation_type)
        do update set relation_status=excluded.relation_status,
                      source_type=excluded.source_type,
                      source_key=excluded.source_key,
                      updated_at=now()
        "#,
    )
    .bind(&request.person_id)
    .bind(&property.id)
    .bind(request.relation.as_str())
    .bind(request.relation_status.as_deref())
    .bind(request.source_type.as_deref().unwrap_or("manual"))
    .bind(request.source_key.as_deref())
    .execute(tx.connection())
    .await
    .map_err(|error| DbFailure::from_sqlx("property.upsert_for_person.relation", &error))?;

    Ok(PropertyForPerson {
        relation: request.relation.clone(),
        relation_status: request.relation_status.clone(),
        property,
    })
}
