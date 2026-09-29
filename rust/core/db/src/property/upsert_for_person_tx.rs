//! Moved from `property.rs` (move only): upsert_for_person_tx.

#[allow(unused_imports)]
use super::*;

pub(super) async fn upsert_for_person_tx(
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
