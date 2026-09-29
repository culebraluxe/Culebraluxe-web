//! Moved from `property.rs` (move only): merge_parcel_record.

#[allow(unused_imports)]
use super::*;

impl PropertyDao {
    /// FIND BY CATASTRO: THE OTHER RECORD FOR THIS PARCEL IS MERGED INTO THIS ONE. The Regrid load gave every parcel its
    /// own record; a listing entered by hand is a second record for the same land. The listing (`target_id`) keeps
    /// every value it has; each field it lacks (null or empty) is filled from the other record, the parcel link moves
    /// to it, whatever hangs off the other record (media, people, deals, MLS details, …) is moved onto it, and the
    /// other record is deleted. Runs inside the caller's transaction: all of it happens, or none.
    ///
    /// `Ok(None)` when no other live record carries that catastro number; otherwise the merged record's name.
    pub async fn merge_parcel_record(&self, target_id: &str, catastro: &str) -> DbResult<Option<String>> {
        let digits: String = catastro.chars().filter(char::is_ascii_digit).collect();
        if digits.is_empty() {
            return Ok(None);
        }
        let mut connection = self.db.connection().await?;
        let sources = sqlx::query_as::<_, (String, String)>(
            r#"
            select id::text, name from property
             where id <> $1::uuid and archived_at is null
               and regexp_replace(coalesce(catastro_number, ''), '[^0-9]', '', 'g') = $2
             for update
            "#,
        )
        .bind(target_id)
        .bind(&digits)
        .fetch_all(&mut *connection)
        .await
        .map_err(|error| DbFailure::from_sqlx("property.merge.find", &error))?;
        let (source_id, source_name) = match sources.as_slice() {
            [] => return Ok(None),
            [one] => one.clone(),
            _ => {
                return Err(DbFailure::configuration(
                    "property.merge.find",
                    format!("{} records carry catastro {catastro}; merge them one at a time", sources.len()),
                ))
            }
        };

        // The other record, kept aside, so it can be deleted before its values are copied (its unique values —
        // the parcel link, the slug — would otherwise collide with themselves on the listing).
        sqlx::query("create temporary table property_merge_source on commit drop as select * from property where id = $1::uuid")
            .bind(&source_id)
            .execute(&mut *connection)
            .await
            .map_err(|error| DbFailure::from_sqlx("property.merge.keep", &error))?;

        // Everything that hangs off the other record, onto the listing — read from the catalog, so a new table is
        // included without anyone remembering it.
        let references = sqlx::query_as::<_, (String, String)>(
            r#"
            select c.conrelid::regclass::text, a.attname::text
              from pg_constraint c
              join pg_attribute a on a.attrelid = c.conrelid and a.attnum = any(c.conkey)
             where c.contype = 'f' and c.confrelid = 'property'::regclass
            "#,
        )
        .fetch_all(&mut *connection)
        .await
        .map_err(|error| DbFailure::from_sqlx("property.merge.references", &error))?;
        let quote = |name: &str| format!("\"{}\"", name.replace('"', "\"\""));
        for (table, column) in &references {
            let table = table.split('.').map(|part| quote(part.trim_matches('"'))).collect::<Vec<_>>().join(".");
            let column = quote(column);
            // Identifiers from the catalog, quoted — no value from a request reaches this text.
            sqlx::query(sqlx::AssertSqlSafe(format!("update {table} set {column} = $1::uuid where {column} = $2::uuid")))
                .bind(target_id)
                .bind(&source_id)
                .execute(&mut *connection)
                .await
                .map_err(|error| DbFailure::from_sqlx("property.merge.move", &error))?;
        }

        sqlx::query("delete from property where id = $1::uuid")
            .bind(&source_id)
            .execute(&mut *connection)
            .await
            .map_err(|error| DbFailure::from_sqlx("property.merge.delete", &error))?;

        // Each field the listing lacks, from the other record: text counts as missing when empty.
        let columns = sqlx::query_as::<_, (String, String)>(
            r#"
            select column_name::text, data_type::text from information_schema.columns
             where table_schema = 'public' and table_name = 'property' and is_generated = 'NEVER'
               and column_name not in ('id', 'created_at', 'updated_at')
             order by ordinal_position
            "#,
        )
        .fetch_all(&mut *connection)
        .await
        .map_err(|error| DbFailure::from_sqlx("property.merge.columns", &error))?;
        let assignments = columns
            .iter()
            .map(|(name, kind)| {
                let column = quote(name);
                if kind == "text" || kind == "character varying" {
                    format!("{column} = coalesce(nullif(t.{column}, ''), s.{column})")
                } else {
                    format!("{column} = coalesce(t.{column}, s.{column})")
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        // Column names from the catalog, quoted — no value from a request reaches this text.
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "update property t set {assignments}, updated_at = now() from property_merge_source s where t.id = $1::uuid"
        )))
        .bind(target_id)
        .execute(&mut *connection)
        .await
        .map_err(|error| DbFailure::from_sqlx("property.merge.fill", &error))?;

        Ok(Some(source_name))
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
        .execute(&mut *self.db.connection().await?)
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
