//! Moved from `property.rs` (move only): new.

#[allow(unused_imports)]
use super::*;

impl PropertyDao {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            read_cache: Arc::new(RwLock::new(None)),
        }
    }

    pub fn database(&self) -> Database {
        self.db.clone()
    }

    pub async fn warm_read_cache(&self) -> DbResult<usize> {
        let directory = sqlx::query_as::<_, PropertyAdminSummaryRow>(
            r#"
            select
                p.id::text as id,
                coalesce(nullif(trim(p.name), ''), 'Unnamed property') as name,
                p.status,
                p.slug,
                coalesce(p.location, p.address_line1, p.city) as location,
                p.list_price::text as list_price,
                p.property_type,
                p.is_active_listing,
                p.is_published,
                (p.archived_at is not null) as archived,
                (select count(*)::bigint from property_media pm join media m on m.id = pm.media_id where pm.property_id = p.id and m.media_type = 'image') as image_count,
                (select count(*)::bigint from property_media pm join media m on m.id = pm.media_id where pm.property_id = p.id and m.media_type = 'video') as video_count
            from property p
            order by
                case when p.archived_at is null then 0 else 1 end,
                case when p.is_active_listing then 0 else 1 end,
                coalesce(nullif(trim(p.name), ''), p.address_line1, p.id::text)
            "#,
        )
        .fetch_all(self.db.pool());
        let canonical = sqlx::query_as::<_, CachedPropertyRelationRow>(property_sql!(
            "select pp.person_id::text as person_id, ",
            ", pp.relation_type, pp.relation_status
             from person_property pp
             join property p on p.id = pp.property_id
             where p.archived_at is null"
        ))
        .fetch_all(self.db.pool());
        let interests = sqlx::query_as::<_, CachedPropertyRelationRow>(property_sql!(
            "select pi.person_id::text as person_id, ",
            ", 'interest'::text as relation_type, pi.status as relation_status
             from property_interest pi
             join property p on p.id = pi.property_id
             where p.archived_at is null"
        ))
        .fetch_all(self.db.pool());
        let sellers = sqlx::query_as::<_, CachedPropertyRelationRow>(property_sql!(
            "select p.seller_person_id::text as person_id, ",
            ", 'physical_property'::text as relation_type, null::text as relation_status
             from property p
             where p.seller_person_id is not null and p.archived_at is null"
        ))
        .fetch_all(self.db.pool());
        let (directory, canonical, interests, sellers) =
            tokio::try_join!(directory, canonical, interests, sellers)
                .map_err(|error| DbFailure::from_sqlx("property.warm_read_cache", &error))?;
        let directory = directory
            .into_iter()
            .map(map_admin_summary)
            .collect::<Vec<_>>();
        let mut by_person = HashMap::<String, PersonPropertyContext>::new();
        let mut seen = HashMap::<String, HashSet<String>>::new();
        for row in canonical.into_iter().chain(interests).chain(sellers) {
            let person_id = row.person_id.clone();
            let linked = map_relation(row.into_relation())?;
            let key = format!("{}:{}", linked.property.id, linked.relation.as_str());
            if seen.entry(person_id.clone()).or_default().insert(key) {
                by_person
                    .entry(person_id.clone())
                    .or_insert_with(|| PersonPropertyContext {
                        person_id,
                        properties: Vec::new(),
                    })
                    .properties
                    .push(linked);
            }
        }
        let count = directory.len().saturating_add(by_person.len());
        if let Ok(mut cache) = self.read_cache.write() {
            *cache = Some(PropertyReadCache {
                directory,
                by_person,
            });
        }
        Ok(count)
    }

    pub async fn get(&self, property_id: &str) -> DbResult<Option<Property>> {
        let row = sqlx::query_as::<_, PropertyRow>(property_sql!(
            "select ",
            " from property p where p.id = $1::uuid limit 1"
        ))
        .bind(property_id)
        .fetch_optional(&mut *self.db.connection().await?)
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
        .fetch_all(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("property.find_by_address", &error))?;

        let target = normalize(&request.address_line1);
        Ok(rows
            .into_iter()
            .find(|row| normalize(canonical_address_line(row).as_deref().unwrap_or("")) == target)
            .map(map_property))
    }

    pub async fn for_person(&self, person_id: &str) -> DbResult<PersonPropertyContext> {
        if let Ok(cache) = self.read_cache.read() {
            if let Some(cache) = cache.as_ref() {
                return Ok(cache.by_person.get(person_id).cloned().unwrap_or_else(|| {
                    PersonPropertyContext {
                        person_id: person_id.to_owned(),
                        properties: Vec::new(),
                    }
                }));
            }
        }
        self.fetch_person_context(person_id).await
    }

    pub(super) async fn fetch_person_context(
        &self,
        person_id: &str,
    ) -> DbResult<PersonPropertyContext> {
        let canonical = sqlx::query_as::<_, PropertyRelationRow>(property_sql!(
            "select ",
            ", pp.relation_type, pp.relation_status
             from person_property pp
             join property p on p.id = pp.property_id
             where pp.person_id = $1::uuid and p.archived_at is null"
        ))
        .bind(person_id)
        .fetch_all(&mut *self.db.connection().await?)
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
        .fetch_all(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("property.for_person.interest", &error))?;

        let sellers = sqlx::query_as::<_, PropertyRelationRow>(property_sql!(
            "select ",
            ", 'physical_property'::text as relation_type, null::text as relation_status
             from property p
             where p.seller_person_id = $1::uuid and p.archived_at is null"
        ))
        .bind(person_id)
        .fetch_all(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("property.for_person.seller", &error))?;

        let mut properties = Vec::new();
        let mut seen = HashSet::new();
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
                match self.fetch_person_context(&request.person_id).await {
                    Ok(context) => {
                        if let Ok(mut cache) = self.read_cache.write() {
                            if let Some(cache) = cache.as_mut() {
                                cache.by_person.insert(request.person_id.clone(), context);
                            }
                        }
                    }
                    Err(_) => {
                        if let Ok(mut cache) = self.read_cache.write() {
                            if let Some(cache) = cache.as_mut() {
                                cache.by_person.remove(&request.person_id);
                            }
                        }
                    }
                }
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
        .fetch_optional(&mut *self.db.connection().await?)
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
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("property.set_status", &error))?;
        Ok(row.map(map_property))
    }

    pub async fn set_listing_type(&self, request: &SetPropertyListingTypeRequest) -> DbResult<()> {
        sqlx::query(
            r#"
            insert into property_stellar_listing (property_id, listing_type)
            values ($1::uuid, nullif($2::text, ''))
            on conflict (property_id) do update set
                listing_type = excluded.listing_type,
                updated_at = now()
            "#,
        )
        .bind(&request.property_id)
        .bind(request.listing_type.as_deref())
        .execute(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("property.set_listing_type", &error))?;
        Ok(())
    }

    pub async fn admin_page(
        &self,
        request: &PropertyAdminPageRequest,
    ) -> DbResult<PropertyAdminPage> {
        if request.search.trim().is_empty() {
            if let Ok(cache) = self.read_cache.read() {
                if let Some(cache) = cache.as_ref() {
                    let page = request.page.max(1);
                    let page_size = request.page_size.clamp(1, 100);
                    let offset = ((page - 1) * page_size) as usize;
                    return Ok(PropertyAdminPage {
                        rows: cache
                            .directory
                            .iter()
                            .skip(offset)
                            .take(page_size as usize)
                            .cloned()
                            .collect(),
                        total: cache.directory.len() as i64,
                        page,
                        page_size,
                    });
                }
            }
        }
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
        .fetch_one(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("property.admin.count", &error))?;

        let rows = sqlx::query_as::<_, PropertyAdminSummaryRow>(
            r#"
            select
                p.id::text as id,
                coalesce(nullif(trim(p.name), ''), 'Unnamed property') as name,
                p.status,
                p.slug,
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
        .fetch_all(&mut *self.db.connection().await?)
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
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("property.admin.get", &error))?;

        Ok(row.map(map_admin_record))
    }
}
