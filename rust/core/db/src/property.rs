use crate::{Database, DbFailure, DbResult, DbTransaction};
use chrono::{DateTime, Utc};
use domain::{
    FieldPatch, FindPropertyByAddressRequest, PersonPropertyContext, PersonPropertyRelation,
    Property, PropertyAddress, PropertyAddressPatch, PropertyForPerson,
    SetPropertyDisplayNameRequest, SetPropertyStatusRequest, UpsertPropertyForPersonRequest,
};
use sqlx::{FromRow, PgConnection};

#[derive(Debug, FromRow)]
struct PropertyRow {
    id: String,
    name: Option<String>,
    legal_owner_name: Option<String>,
    listing_identifier: Option<String>,
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
    listing_identifier: Option<String>,
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

fn compact(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn one_line(value: Option<&str>) -> Option<String> {
    let value = value?;
    let parts: Vec<&str> = value
        .lines()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
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
        return match compact(row.unit_number.as_deref()) {
            Some(unit) => Some(format!("{street}, {unit}")),
            None => Some(street),
        };
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
    let display_name = local_name
        .clone()
        .or_else(|| address.address_line1.clone())
        .unwrap_or_else(|| "Property".into());
    Property {
        id: row.id,
        display_name,
        local_name,
        legal_owner_name: compact(row.legal_owner_name.as_deref()),
        catastro_number: compact(row.listing_identifier.as_deref()),
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

fn relation_row_to_property(row: &PropertyRelationRow) -> Property {
    map_property(PropertyRow {
        id: row.id.clone(),
        name: row.name.clone(),
        legal_owner_name: row.legal_owner_name.clone(),
        listing_identifier: row.listing_identifier.clone(),
        registry_entry: row.registry_entry.clone(),
        finca_number: row.finca_number.clone(),
        registry_section: row.registry_section.clone(),
        status: row.status.clone(),
        archived_at: row.archived_at,
        address_line1: row.address_line1.clone(),
        location: row.location.clone(),
        street_number: row.street_number.clone(),
        street_name: row.street_name.clone(),
        unit_number: row.unit_number.clone(),
        city: row.city.clone(),
        state_or_province: row.state_or_province.clone(),
        neighborhood: row.neighborhood.clone(),
        postal_code: row.postal_code.clone(),
        country: row.country.clone(),
        iso_country_code: row.iso_country_code.clone(),
    })
}

const PROPERTY_COLUMNS: &str = r#"
    p.id::text as id, p.name, p.legal_owner_name, p.listing_identifier,
    p.registry_entry, p.finca_number, p.registry_section,
    p.status, p.archived_at, p.address_line1, p.location,
    p.street_number, p.street_name, p.unit_number, p.city, p.state_or_province,
    p.neighborhood, p.postal_code, p.country, p.iso_country_code
"#;

async fn get_on(connection: &mut PgConnection, property_id: &str) -> DbResult<Option<Property>> {
    let query = format!("select {PROPERTY_COLUMNS} from property p where p.id = $1::uuid limit 1");
    let row = sqlx::query_as::<_, PropertyRow>(&query)
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
    let query = format!(
        r#"
        select {PROPERTY_COLUMNS}
        from property p
        where p.archived_at is null
          and ($1::text is null or lower(trim(coalesce(p.city, ''))) = lower(trim($1)))
          and ($2::text is null or lower(trim(coalesce(p.state_or_province, ''))) = lower(trim($2)))
          and ($3::text is null or lower(trim(coalesce(p.postal_code, ''))) = lower(trim($3)))
        order by p.updated_at desc nulls last, p.id asc
        limit 250
        "#
    );
    let rows = sqlx::query_as::<_, PropertyRow>(&query)
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

#[derive(Clone)]
pub struct PropertyDao {
    db: Database,
}

impl PropertyDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get(&self, property_id: &str) -> DbResult<Option<Property>> {
        let query =
            format!("select {PROPERTY_COLUMNS} from property p where p.id = $1::uuid limit 1");
        let row = sqlx::query_as::<_, PropertyRow>(&query)
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
        let query = format!(
            r#"
            select {PROPERTY_COLUMNS}
            from property p
            where p.archived_at is null
              and ($1::text is null or lower(trim(coalesce(p.city, ''))) = lower(trim($1)))
              and ($2::text is null or lower(trim(coalesce(p.state_or_province, ''))) = lower(trim($2)))
              and ($3::text is null or lower(trim(coalesce(p.postal_code, ''))) = lower(trim($3)))
            order by p.updated_at desc nulls last, p.id asc
            limit 250
            "#
        );
        let rows = sqlx::query_as::<_, PropertyRow>(&query)
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
        let base = PROPERTY_COLUMNS.replace("p.id::text as id", "p.id::text as id");
        let canonical = sqlx::query_as::<_, PropertyRelationRow>(&format!(
            r#"
            select {base}, pp.relation_type, pp.relation_status
            from person_property pp
            join property p on p.id = pp.property_id
            where pp.person_id = $1::uuid and p.archived_at is null
            "#
        ))
        .bind(person_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("property.for_person.canonical", &error))?;

        let interests = sqlx::query_as::<_, PropertyRelationRow>(&format!(
            r#"
            select {base}, 'interest'::text as relation_type, pi.status as relation_status
            from property_interest pi
            join property p on p.id = pi.property_id
            where pi.person_id = $1::uuid and p.archived_at is null
            "#
        ))
        .bind(person_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("property.for_person.interest", &error))?;

        let sellers = sqlx::query_as::<_, PropertyRelationRow>(&format!(
            r#"
            select {base}, 'physical_property'::text as relation_type, null::text as relation_status
            from property p
            where p.seller_person_id = $1::uuid and p.archived_at is null
            "#
        ))
        .bind(person_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("property.for_person.seller", &error))?;

        let mut properties = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for row in canonical.into_iter().chain(interests).chain(sellers) {
            let relation = PersonPropertyRelation::try_from(row.relation_type.as_str())
                .map_err(|error| DbFailure::schema_mismatch("property.for_person", error))?;
            let key = format!("{}:{}", row.id, relation.as_str());
            if !seen.insert(key) {
                continue;
            }
            properties.push(PropertyForPerson {
                relation,
                relation_status: row.relation_status.clone(),
                property: relation_row_to_property(&row),
            });
        }

        properties.sort_by(|left, right| {
            relation_rank(&left.relation)
                .cmp(&relation_rank(&right.relation))
                .then_with(|| left.property.display_name.cmp(&right.property.display_name))
        });

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
        let query = format!(
            r#"
            update property p set name=$2, updated_at=now()
            where p.id=$1::uuid
            returning {PROPERTY_COLUMNS}
            "#
        );
        let row = sqlx::query_as::<_, PropertyRow>(&query)
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
        let query = format!(
            r#"
            update property p set status=$2, updated_at=now()
            where p.id=$1::uuid
            returning {PROPERTY_COLUMNS}
            "#
        );
        let row = sqlx::query_as::<_, PropertyRow>(&query)
            .bind(&request.property_id)
            .bind(request.status.trim())
            .fetch_optional(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("property.set_status", &error))?;
        Ok(row.map(map_property))
    }
}

async fn upsert_for_person_tx(
    tx: &mut DbTransaction,
    request: &UpsertPropertyForPersonRequest,
) -> DbResult<PropertyForPerson> {
    let mut current = match request
        .property_id
        .as_deref()
        .and_then(|value| compact(Some(value)))
    {
        Some(id) => get_on(tx.connection(), &id).await?,
        None => None,
    };

    let requested_line = request
        .address
        .as_ref()
        .and_then(|address| explicit_patch_value(&address.address_line1));

    if current.is_none() && request.property_id.is_none() {
        if let Some(line) = requested_line {
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

    let property_row = if let Some(existing) = current {
        let query = format!(
            r#"
            update property p
            set name=$2, legal_owner_name=$3, listing_identifier=$4,
                registry_entry=$5, finca_number=$6, registry_section=$7,
                address_line1=$8, city=$9, state_or_province=$10,
                neighborhood=$11, postal_code=$12, country=$13,
                iso_country_code=$14, updated_at=now()
            where p.id=$1::uuid
            returning {PROPERTY_COLUMNS}
            "#
        );
        sqlx::query_as::<_, PropertyRow>(&query)
            .bind(&existing.id)
            .bind(local_name)
            .bind(legal_owner_name)
            .bind(catastro_number)
            .bind(registry_entry)
            .bind(finca_number)
            .bind(registry_section)
            .bind(compact(address.address_line1.as_deref()))
            .bind(compact(address.city.as_deref()))
            .bind(compact(address.state_or_province.as_deref()))
            .bind(compact(address.neighborhood.as_deref()))
            .bind(compact(address.postal_code.as_deref()))
            .bind(compact(address.country.as_deref()))
            .bind(compact(address.iso_country_code.as_deref()))
            .fetch_one(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("property.upsert_for_person.update", &error))?
    } else {
        let query = format!(
            r#"
            insert into property as p (
                name, legal_owner_name, listing_identifier, registry_entry,
                finca_number, registry_section, address_line1, city,
                state_or_province, neighborhood, postal_code, country,
                iso_country_code, source_type
            )
            values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
            returning {PROPERTY_COLUMNS}
            "#
        );
        sqlx::query_as::<_, PropertyRow>(&query)
            .bind(local_name)
            .bind(legal_owner_name)
            .bind(catastro_number)
            .bind(registry_entry)
            .bind(finca_number)
            .bind(registry_section)
            .bind(compact(address.address_line1.as_deref()))
            .bind(compact(address.city.as_deref()))
            .bind(compact(address.state_or_province.as_deref()))
            .bind(compact(address.neighborhood.as_deref()))
            .bind(compact(address.postal_code.as_deref()))
            .bind(compact(address.country.as_deref()))
            .bind(compact(address.iso_country_code.as_deref()))
            .bind(
                request
                    .source_type
                    .as_deref()
                    .and_then(|value| compact(Some(value)))
                    .unwrap_or_else(|| "manual".into()),
            )
            .fetch_one(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("property.upsert_for_person.insert", &error))?
    };

    let property = map_property(property_row);
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
    .bind(
        request
            .source_type
            .as_deref()
            .and_then(|value| compact(Some(value)))
            .unwrap_or_else(|| "manual".into()),
    )
    .bind(
        request
            .source_key
            .as_deref()
            .and_then(|value| compact(Some(value))),
    )
    .execute(tx.connection())
    .await
    .map_err(|error| DbFailure::from_sqlx("property.upsert_for_person.relation", &error))?;

    Ok(PropertyForPerson {
        relation: request.relation.clone(),
        relation_status: request.relation_status.clone(),
        property,
    })
}

fn relation_rank(relation: &PersonPropertyRelation) -> u8 {
    match relation {
        PersonPropertyRelation::LegalAddress => 0,
        PersonPropertyRelation::PhysicalProperty => 1,
        PersonPropertyRelation::Address => 2,
        PersonPropertyRelation::Interest => 3,
    }
}
