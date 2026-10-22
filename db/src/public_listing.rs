//! The public site's listing copy: the one-line pitch each PUBLISHED listing carries.
//!
//! PUBLISHED MEANS THE SAME HERE AS EVERYWHERE THE SITE READS: `is_published`, an active listing, not archived, with a
//! slug — the predicate the public inventory and the public document route use. A draft's tagline is not public copy.

use crate::{Database, DbFailure, DbResult};
use model::{PublicListing, PublicListingCopy, PublicProperty, PublicPropertyMedia};

/// One row of the public inventory, with its hero already resolved by the query.
#[derive(sqlx::FromRow)]
struct ListingRow {
    id: String,
    row_key: String,
    name: String,
    property_type: Option<String>,
    status: String,
    list_price: Option<f64>,
    featured: bool,
    city: Option<String>,
    state_or_province: Option<String>,
    neighborhood: Option<String>,
    bedrooms: Option<i32>,
    bathrooms: Option<f64>,
    square_feet: Option<i32>,
    lot_size: Option<f64>,
    lot_size_units: Option<String>,
    has_ocean_view: bool,
    has_bay_view: bool,
    has_beach_view: bool,
    has_harbor_view: bool,
    has_island_view: bool,
    has_mountain_view: bool,
    has_sunrise_view: bool,
    has_sunset_view: bool,
    water_access: bool,
    beach_access: bool,
    hero_media_id: Option<String>,
    hero_alt: Option<String>,
}

impl ListingRow {
    /// The eight view flags become the words a card shows, in the same order the site has always used.
    fn into_listing(self) -> PublicListing {
        let mut views = Vec::new();
        for (flag, label) in [
            (self.has_ocean_view, "Ocean"),
            (self.has_bay_view, "Bay"),
            (self.has_beach_view, "Beach"),
            (self.has_harbor_view, "Harbor"),
            (self.has_island_view, "Island"),
            (self.has_mountain_view, "Mountain"),
            (self.has_sunrise_view, "Sunrise"),
            (self.has_sunset_view, "Sunset"),
        ] {
            if flag {
                views.push(label.to_string());
            }
        }

        PublicListing {
            key: self.row_key,
            id: self.id,
            name: self.name,
            property_type: self.property_type,
            status: self.status,
            list_price: self.list_price,
            featured: self.featured,
            city: self.city,
            state_or_province: self.state_or_province,
            neighborhood: self.neighborhood,
            bedrooms: self.bedrooms,
            bathrooms: self.bathrooms,
            square_feet: self.square_feet,
            lot_size: self.lot_size,
            lot_size_units: self.lot_size_units,
            views,
            water_access: self.water_access,
            beach_access: self.beach_access,
            hero_media_id: self.hero_media_id,
            hero_alt: self.hero_alt,
        }
    }
}

/// One copy's pointer, or `None` when the row has no object pointer (bucket/key absent). Mirrors the single-id read:
/// an absent pointer is not a location, and status/visibility fall back to the same LEGACY/private defaults.
fn location(
    bucket: Option<String>,
    key: Option<String>,
    status: Option<String>,
    visibility: Option<String>,
) -> Option<crate::MediaObjectLocation> {
    Some(crate::MediaObjectLocation {
        bucket: bucket?,
        key: key?,
        status: status.unwrap_or_else(|| "LEGACY_BLOB".to_owned()),
        visibility: visibility.unwrap_or_else(|| "private".to_owned()),
    })
}

pub struct PublicListingDao {
    db: Database,
}

/// The row this read returns, named rather than positional: sqlx implements `FromRow` for tuples up to sixteen
/// fields and this read needs eighteen, and named fields make the mapping below readable anyway.
#[derive(sqlx::FromRow)]
struct PropertyRow {
    id: String,
    row_key: String,
    name: String,
    status: String,
    property_type: Option<String>,
    list_price: Option<f64>,
    city: Option<String>,
    state_or_province: Option<String>,
    neighborhood: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    bedrooms: Option<i32>,
    bathrooms: Option<f64>,
    bathrooms_full: Option<f64>,
    bathrooms_half: Option<f64>,
    square_feet: Option<i32>,
    lot_size: Option<f64>,
    lot_size_units: Option<String>,
    lot_size_sqft: Option<f64>,
    road_frontage_feet: Option<f64>,
    road_surface_type: Option<String>,
    lot_description: Option<String>,
    utilities_notes: Option<String>,
    year_built: Option<i32>,
    stories: Option<f64>,
    parking_spaces: Option<f64>,
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
    architecture_notes: Option<String>,
    amenities_notes: Option<String>,
    lifestyle_notes: Option<String>,
    short_description: Option<String>,
    editorial_description: Option<String>,
    listing_agent_name: Option<String>,
    listing_agent_email: Option<String>,
    listing_agent_phone: Option<String>,
    listing_office: Option<String>,
    listing_identifier: Option<String>,
    video_count: i64,
}

impl PublicListingDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Every listing the public site shows: the visibility rule, and nothing else required.
    ///
    /// The view LABELS are spelled here rather than exposed as eight booleans, because every surface wants the same
    /// words and none of them wants the storage shape. The hero comes with the row: a card without a picture is a card
    /// nobody clicks, and the marked hero or the first photograph is always one of them.
    pub async fn listings(&self) -> DbResult<Vec<PublicListing>> {
        let rows = sqlx::query_as::<_, ListingRow>(
            r#"
            select
                p.id::text as id,
                coalesce(p.slug, p.id::text) as row_key,
                p.name,
                nullif(btrim(coalesce(p.property_type, '')), '') as property_type,
                p.status,
                p.list_price::float8 as list_price,
                coalesce(p.featured, false) as featured,
                p.city,
                p.state_or_province,
                p.neighborhood,
                p.bedrooms::int as bedrooms,
                p.bathrooms::float8 as bathrooms,
                p.square_feet::int as square_feet,
                coalesce(p.lot_size_acres, p.lot_size)::float8 as lot_size,
                -- The unit of the number chosen above: Lot acres (the Records field) is acres whatever the legacy
                -- lot_size_units says, which describes lot_size only. Taking it alone printed acres as "SF".
                case when p.lot_size_acres is not null then 'acres' else p.lot_size_units end as lot_size_units,
                coalesce(p.has_ocean_view, false) as has_ocean_view,
                coalesce(p.has_bay_view, false) as has_bay_view,
                coalesce(p.has_beach_view, false) as has_beach_view,
                coalesce(p.has_harbor_view, false) as has_harbor_view,
                coalesce(p.has_island_view, false) as has_island_view,
                coalesce(p.has_mountain_view, false) as has_mountain_view,
                coalesce(p.has_sunrise_view, false) as has_sunrise_view,
                coalesce(p.has_sunset_view, false) as has_sunset_view,
                coalesce(p.has_water_access, false) as water_access,
                coalesce(p.has_beach_access, false) as beach_access,
                hero.media_id::text as hero_media_id,
                hero.alt_text as hero_alt
            from property p
            left join lateral (
                select m.id as media_id, m.alt_text
                  from property_media pm
                  join media m on m.id = pm.media_id
                 where pm.property_id = p.id
                   and pm.role in ('hero', 'gallery')
                   and m.media_type = 'image'
                 order by case when pm.role = 'hero' then 0 else 1 end asc,
                          pm.sort_order asc,
                          pm.created_at asc
                 limit 1
            ) hero on true
            where p.archived_at is null
              and p.status in ('active', 'under_contract', 'sold')
              and p.name is not null
            order by coalesce(p.featured, false) desc, p.name asc
            "#,
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("public_listing.listings", &error))?;

        Ok(rows.into_iter().map(ListingRow::into_listing).collect())
    }

    /// One Property, resolved by whatever identifies it: its slug, its name (any case, spaces or dashes), or its id.
    ///
    /// The public site used to resolve this in TypeScript against the legacy SQL module. It lives here now, so the
    /// property page and the buyers grid read the same way — through the service, in one language, with one rule.
    ///
    /// The hero is the marked photograph or the first one: a listing whose hero was never flagged still has a picture.
    pub async fn property(&self, key: &str) -> DbResult<Option<PublicProperty>> {
        let row = sqlx::query_as::<_, PropertyRow>(
            r#"
            select
                p.id::text as id,
                coalesce(p.slug, p.id::text) as row_key,
                p.name,
                p.status,
                nullif(btrim(coalesce(p.property_type, '')), '') as property_type,
                p.list_price::float8 as list_price,
                p.city,
                p.state_or_province,
                p.neighborhood,
                p.latitude::float8 as latitude,
                p.longitude::float8 as longitude,
                p.bedrooms::int as bedrooms,
                p.bathrooms::float8 as bathrooms,
                p.bathrooms_full::float8 as bathrooms_full,
                p.bathrooms_half::float8 as bathrooms_half,
                p.square_feet::int as square_feet,
                coalesce(p.lot_size_acres, p.lot_size)::float8 as lot_size,
                -- The unit of the number chosen above: Lot acres (the Records field) is acres whatever the legacy
                -- lot_size_units says, which describes lot_size only. Taking it alone printed acres as "SF".
                case when p.lot_size_acres is not null then 'acres' else p.lot_size_units end as lot_size_units,
                p.lot_size_sqft::float8 as lot_size_sqft,
                p.road_frontage_feet::float8 as road_frontage_feet,
                p.road_surface_type,
                p.lot_description,
                p.utilities_notes,
                (p.year_built::text)::int as year_built,
                p.stories::float8 as stories,
                p.parking_spaces::float8 as parking_spaces,
                coalesce(p.has_ocean_view, false) as has_ocean_view,
                coalesce(p.has_bay_view, false) as has_bay_view,
                coalesce(p.has_beach_view, false) as has_beach_view,
                coalesce(p.has_harbor_view, false) as has_harbor_view,
                coalesce(p.has_island_view, false) as has_island_view,
                coalesce(p.has_mountain_view, false) as has_mountain_view,
                coalesce(p.has_sunrise_view, false) as has_sunrise_view,
                coalesce(p.has_sunset_view, false) as has_sunset_view,
                coalesce(p.has_water_access, false) as has_water_access,
                coalesce(p.has_beach_access, false) as has_beach_access,
                coalesce(p.has_pool, false) as has_pool,
                coalesce(p.has_generator, false) as has_generator,
                coalesce(p.has_solar, false) as has_solar,
                coalesce(p.is_furnished, false) as is_furnished,
                coalesce(p.is_gated, false) as is_gated,
                p.architecture_notes,
                p.amenities_notes,
                p.lifestyle_notes,
                p.short_description,
                p.editorial_description,
                p.listing_agent_name,
                p.listing_agent_email,
                p.listing_agent_phone,
                p.listing_office,
                p.listing_identifier,
                -- Videos are Mux assets attached as media: either the media says video, or the role does.
                (
                    select count(*)::bigint
                      from property_media vpm
                      join media vm on vm.id = vpm.media_id
                     where vpm.property_id = p.id
                       and (vm.media_type = 'video' or vpm.role in ('video', 'short'))
                ) as video_count
            from property p
            where (
                    p.slug = $1
                    or lower(p.name) = lower($1)
                    or lower(replace($1, '-', ' ')) = lower(p.name)
                    or p.id::text = $1
                  )
              and p.archived_at is null
              and p.status in ('active', 'under_contract', 'sold')
            limit 1
            "#,
        )
        .bind(key)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("public_listing.property", &error))?;

        let Some(row) = row else {
            return Ok(None);
        };

        // THE FULL MEDIA LIST, not just the hero. Photographs, Mux videos and documents are all `media` rows, and the
        // surface is what sorts them into a gallery, a video strip and a document list. The hero is resolved here —
        // the marked one first, otherwise the first photograph — so no surface has to remember that rule.
        let media_rows = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<f64>,
                i32,
                Option<String>,
                Option<String>,
                Option<f64>,
            ),
        >(
            r#"
            select
                m.id::text,
                pm.role,
                m.media_type,
                m.alt_text,
                m.caption,
                m.filename,
                m.mime_type,
                m.file_size::float8,
                pm.sort_order,
                m.mux_playback_id,
                m.aspect_ratio,
                m.duration_seconds::float8
              from property_media pm
              join media m on m.id = pm.media_id
             where pm.property_id = $1::uuid
             order by case when pm.role = 'hero' then 0 else 1 end, pm.sort_order, m.created_at
            "#,
        )
        .bind(&row.id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("public_listing.property_media", &error))?;

        let media: Vec<PublicPropertyMedia> = media_rows
            .into_iter()
            .map(
                |(
                    id,
                    role,
                    media_type,
                    alt_text,
                    caption,
                    filename,
                    mime_type,
                    file_size,
                    sort_order,
                    mux_playback_id,
                    aspect_ratio,
                    duration_seconds,
                )| PublicPropertyMedia {
                    id,
                    role,
                    media_type,
                    alt_text,
                    caption,
                    filename,
                    mime_type,
                    file_size,
                    sort_order,
                    mux_playback_id,
                    aspect_ratio,
                    duration_seconds,
                },
            )
            .collect();

        let hero_media_id = media
            .iter()
            .find(|item| item.media_type == "image" && item.role == "hero")
            .or_else(|| media.iter().find(|item| item.media_type == "image"))
            .map(|item| item.id.clone());

        let mut view_type = Vec::new();
        for (flag, label) in [
            (row.has_ocean_view, "Ocean"),
            (row.has_bay_view, "Bay"),
            (row.has_beach_view, "Beach"),
            (row.has_harbor_view, "Harbor"),
            (row.has_island_view, "Island"),
            (row.has_mountain_view, "Mountain"),
            (row.has_sunrise_view, "Sunrise"),
            (row.has_sunset_view, "Sunset"),
        ] {
            if flag {
                view_type.push(label.to_owned());
            }
        }

        let mut amenities = Vec::new();
        for (flag, label) in [
            (row.has_pool, "Pool"),
            (row.has_generator, "Whole-Home Generator"),
            (row.has_solar, "Solar Power"),
            (row.is_furnished, "Furnished"),
            (row.is_gated, "Gated"),
            (row.has_water_access, "Water Access"),
            (row.has_beach_access, "Beach Access"),
        ] {
            if flag {
                amenities.push(label.to_owned());
            }
        }
        if let Some(notes) = row
            .amenities_notes
            .clone()
            .filter(|value| !value.trim().is_empty())
        {
            amenities.push(notes);
        }

        let mut lifestyle_tags = Vec::new();
        for (flag, label) in [
            (row.has_ocean_view, "Ocean View"),
            (row.has_sunset_view, "Sunset View"),
            (row.has_sunrise_view, "Sunrise View"),
            (row.has_beach_access, "Beach Access"),
            (row.has_water_access, "Water Access"),
            (row.has_pool, "Pool"),
            (row.is_gated, "Private Estate"),
        ] {
            if flag {
                lifestyle_tags.push(label.to_owned());
            }
        }
        if let Some(notes) = row
            .lifestyle_notes
            .clone()
            .filter(|value| !value.trim().is_empty())
        {
            lifestyle_tags.push(notes);
        }

        Ok(Some(PublicProperty {
            id: row.id,
            key: row.row_key,
            name: row.name,
            status: row.status,
            property_type: row.property_type,
            list_price: row.list_price,
            city: row.city,
            state_or_province: row.state_or_province,
            neighborhood: row.neighborhood,
            latitude: row.latitude,
            longitude: row.longitude,
            bedrooms: row.bedrooms,
            bathrooms: row.bathrooms,
            bathrooms_full: row.bathrooms_full,
            bathrooms_half: row.bathrooms_half,
            square_feet: row.square_feet,
            lot_size: row.lot_size,
            lot_size_units: row.lot_size_units,
            lot_size_sqft: row.lot_size_sqft,
            road_frontage_feet: row.road_frontage_feet,
            road_surface_type: row.road_surface_type,
            lot_description: row.lot_description,
            utilities_notes: row.utilities_notes,
            year_built: row.year_built,
            stories: row.stories,
            parking_spaces: row.parking_spaces,
            view_type,
            water_access: row.has_water_access,
            beach_access: row.has_beach_access,
            amenities,
            architecture_notes: row.architecture_notes,
            lifestyle_tags,
            short_description: row.short_description,
            editorial_description: row.editorial_description,
            listing_agent_name: row.listing_agent_name,
            listing_agent_email: row.listing_agent_email,
            listing_agent_phone: row.listing_agent_phone,
            listing_office: row.listing_office,
            listing_identifier: row.listing_identifier,
            hero_media_id,
            media,
            video_count: row.video_count,
        }))
    }

    /// Listings like this one: same kind of Property, visible, not itself.
    ///
    /// "Similar" is deliberately simple — same `property_type`, most expensive first — because the previous definition
    /// lived in TypeScript with its own filters and drifted from the inventory rule. What matters is that it uses the
    /// SAME visibility rule as everything else: a similar listing is a listing the site is showing.
    pub async fn similar(&self, key: &str, limit: i64) -> DbResult<Vec<PublicListing>> {
        let rows = sqlx::query_as::<_, ListingRow>(
            r#"
            with subject as (
                select p.id, nullif(btrim(coalesce(p.property_type, '')), '') as property_type
                  from property p
                 where (
                         p.slug = $1
                         or lower(p.name) = lower($1)
                         or lower(replace($1, '-', ' ')) = lower(p.name)
                         or p.id::text = $1
                       )
                   and p.archived_at is null
                   and p.status in ('active', 'under_contract', 'sold')
                 limit 1
            )
            select
                p.id::text as id,
                coalesce(p.slug, p.id::text) as row_key,
                p.name,
                nullif(btrim(coalesce(p.property_type, '')), '') as property_type,
                p.status,
                p.list_price::float8 as list_price,
                coalesce(p.featured, false) as featured,
                p.city,
                p.state_or_province,
                p.neighborhood,
                p.bedrooms::int as bedrooms,
                p.bathrooms::float8 as bathrooms,
                p.square_feet::int as square_feet,
                coalesce(p.lot_size_acres, p.lot_size)::float8 as lot_size,
                -- The unit of the number chosen above: Lot acres (the Records field) is acres whatever the legacy
                -- lot_size_units says, which describes lot_size only. Taking it alone printed acres as "SF".
                case when p.lot_size_acres is not null then 'acres' else p.lot_size_units end as lot_size_units,
                coalesce(p.has_ocean_view, false) as has_ocean_view,
                coalesce(p.has_bay_view, false) as has_bay_view,
                coalesce(p.has_beach_view, false) as has_beach_view,
                coalesce(p.has_harbor_view, false) as has_harbor_view,
                coalesce(p.has_island_view, false) as has_island_view,
                coalesce(p.has_mountain_view, false) as has_mountain_view,
                coalesce(p.has_sunrise_view, false) as has_sunrise_view,
                coalesce(p.has_sunset_view, false) as has_sunset_view,
                coalesce(p.has_water_access, false) as water_access,
                coalesce(p.has_beach_access, false) as beach_access,
                hero.media_id::text as hero_media_id,
                hero.alt_text as hero_alt
            from property p
            join subject on true
            left join lateral (
                select m.id as media_id, m.alt_text
                  from property_media pm
                  join media m on m.id = pm.media_id
                 where pm.property_id = p.id
                   and pm.role in ('hero', 'gallery')
                   and m.media_type = 'image'
                 order by case when pm.role = 'hero' then 0 else 1 end asc,
                          pm.sort_order asc,
                          pm.created_at asc
                 limit 1
            ) hero on true
            where p.archived_at is null
              and p.status in ('active', 'under_contract', 'sold')
              and p.id <> subject.id
              and p.name is not null
              and (subject.property_type is null or p.property_type = subject.property_type)
            order by p.list_price desc nulls last, p.name asc
            limit $2
            "#,
        )
        .bind(key)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("public_listing.similar", &error))?;

        Ok(rows.into_iter().map(ListingRow::into_listing).collect())
    }

    /// Every slug the public site can serve, for the sitemap. Rows without a slug are absent rather than null: a
    /// sitemap entry with no URL is not an entry.
    pub async fn slugs(&self) -> DbResult<Vec<String>> {
        let rows = sqlx::query_scalar::<_, String>(
            r#"
            select p.slug
              from property p
             where p.archived_at is null
               and p.status in ('active', 'under_contract', 'sold')
               and p.slug is not null
             order by p.slug
            "#,
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("public_listing.slugs", &error))?;
        Ok(rows)
    }

    /// The storage pointer of the public copy to serve, selected by `size`. No bytes are read here.
    pub async fn media_object_location(
        &self,
        id: &str,
        size: &str,
    ) -> DbResult<Option<crate::MediaObjectLocation>> {
        let size = if matches!(size, "card" | "thumb") {
            size
        } else {
            "web"
        };
        let row = sqlx::query_as::<
            _,
            (
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
            ),
        >(
            r#"
            select
                copy.id::text,
                case when copy.id is not null then copy.object_bucket else m.object_bucket end,
                case when copy.id is not null then copy.object_key else m.object_key end,
                case when copy.id is not null then copy.storage_status else m.storage_status end,
                case when copy.id is not null then copy.visibility else m.visibility end
            from media m
            join media root on root.id = coalesce(m.derivative_of, m.id)
            left join lateral (
                select d.id, d.object_bucket, d.object_key, d.storage_status, d.visibility
                  from media d
                 where d.derivative_of = root.id
                   and d.derivative_kind in ('web', 'card', 'thumb')
                 -- Never fall through to the original when a smaller servable copy exists. The requested
                 -- size wins; then web, then card; thumbnails prefer card over the full web copy.
                 order by case
                     when d.derivative_kind = $2 then 0
                     when $2 = 'thumb' and d.derivative_kind = 'card' then 1
                     when $2 = 'thumb' and d.derivative_kind = 'web' then 2
                     when d.derivative_kind = 'web' then 1
                     when d.derivative_kind = 'card' then 2
                     else 3
                 end
                 limit 1
            ) as copy on true
            where m.id = $1::uuid
              and not exists (
                    select 1
                      from property_media pm
                      join property p on p.id = pm.property_id
                     where pm.media_id = root.id
                       and not (
                             p.archived_at is null
                             and p.status in ('active', 'under_contract', 'sold')
                           )
                  )
            limit 1
            "#,
        )
        .bind(id)
        .bind(size)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("public_listing.media_object_location", &error))?;
        Ok(row.and_then(|(_copy_id, bucket, key, status, visibility)| {
            Some(crate::MediaObjectLocation {
                bucket: bucket?,
                key: key?,
                status: status.unwrap_or_else(|| "LEGACY_BLOB".to_owned()),
                visibility: visibility.unwrap_or_else(|| "private".to_owned()),
            })
        }))
    }

    /// The object pointers for the three servable copies (card, web, thumb) of MANY media ids, in ONE query.
    ///
    /// WHY ONE QUERY: a public page asks for every photograph on it at once. The single-id resolver repeated three
    /// times per photograph was three round trips and three statements per image (93 for a 31-photo page). This
    /// lateral-join form returns every requested photograph's three chosen copies in a single round trip, with the
    /// SAME derivative selection and property-visibility rule the single-id read uses:
    ///
    /// - card: card, then web, then thumb;
    /// - web: web, then card, then thumb;
    /// - thumb: thumb, then card, then web;
    /// - no derivative at all falls back to the ORIGINAL row's pointer, exactly as the single read does.
    ///
    /// Returns one row per id that the public gate knows; unknown ids and photographs of unpublished Properties are
    /// simply absent (the caller answers `None` for them). The returned tuples are
    /// `(media id, card, web, thumb)`; each location is `None` when that row has no object pointer yet.
    pub async fn media_object_locations(
        &self,
        ids: &[String],
    ) -> DbResult<
        Vec<(
            String,
            Option<crate::MediaObjectLocation>,
            Option<crate::MediaObjectLocation>,
            Option<crate::MediaObjectLocation>,
        )>,
    > {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query_as::<
            _,
            (
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
            ),
        >(
            r#"
            select
                m.id::text,
                -- card: the requested copy, then web, then thumb; the original only when no copy exists.
                case when card.id is not null then card.object_bucket else m.object_bucket end,
                case when card.id is not null then card.object_key else m.object_key end,
                case when card.id is not null then card.storage_status else m.storage_status end,
                case when card.id is not null then card.visibility else m.visibility end,
                -- web: the web copy, then card, then thumb.
                case when web.id is not null then web.object_bucket else m.object_bucket end,
                case when web.id is not null then web.object_key else m.object_key end,
                case when web.id is not null then web.storage_status else m.storage_status end,
                case when web.id is not null then web.visibility else m.visibility end,
                -- thumb: the thumb copy, then card, then web (a tile must never pull the 3600px copy).
                case when thumb.id is not null then thumb.object_bucket else m.object_bucket end,
                case when thumb.id is not null then thumb.object_key else m.object_key end,
                case when thumb.id is not null then thumb.storage_status else m.storage_status end,
                case when thumb.id is not null then thumb.visibility else m.visibility end
            from media m
            join media root on root.id = coalesce(m.derivative_of, m.id)
            left join lateral (
                select d.id, d.object_bucket, d.object_key, d.storage_status, d.visibility
                  from media d
                 where d.derivative_of = root.id
                   and d.derivative_kind in ('card', 'web', 'thumb')
                 order by case d.derivative_kind
                            when 'card' then 0 when 'web' then 1 else 2 end
                 limit 1
            ) as card on true
            left join lateral (
                select d.id, d.object_bucket, d.object_key, d.storage_status, d.visibility
                  from media d
                 where d.derivative_of = root.id
                   and d.derivative_kind in ('card', 'web', 'thumb')
                 order by case d.derivative_kind
                            when 'web' then 0 when 'card' then 1 else 2 end
                 limit 1
            ) as web on true
            left join lateral (
                select d.id, d.object_bucket, d.object_key, d.storage_status, d.visibility
                  from media d
                 where d.derivative_of = root.id
                   and d.derivative_kind in ('card', 'web', 'thumb')
                 order by case d.derivative_kind
                            when 'thumb' then 0 when 'card' then 1 when 'web' then 2 else 3 end
                 limit 1
            ) as thumb on true
            where m.id = any(array(select unnest($1::text[])::uuid))
              and not exists (
                    select 1
                      from property_media pm
                      join property p on p.id = pm.property_id
                     where pm.media_id = root.id
                       and not (
                             p.archived_at is null
                             and p.status in ('active', 'under_contract', 'sold')
                           )
                  )
            "#,
        )
        .bind(ids)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("public_listing.media_object_locations", &error))?;

        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    row.0,
                    location(row.1, row.2, row.3, row.4),
                    location(row.5, row.6, row.7, row.8),
                    location(row.9, row.10, row.11, row.12),
                )
            })
            .collect())
    }

    /// Taglines for every published listing that has one. A listing with none is simply absent.
    pub async fn listing_copy(&self) -> DbResult<Vec<PublicListingCopy>> {
        let rows = sqlx::query_as::<_, (String, String)>(
            r#"
            select p.slug, btrim(p.tagline) as tagline
            from property p
            where p.status in ('active', 'under_contract', 'sold')
              and p.archived_at is null
              and p.slug is not null
              and nullif(btrim(p.tagline), '') is not null
            order by p.slug
            "#,
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("public_listing.listing_copy", &error))?;
        Ok(rows
            .into_iter()
            .map(|(slug, tagline)| PublicListingCopy { slug, tagline })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn same(
        left: &Option<crate::MediaObjectLocation>,
        right: &Option<crate::MediaObjectLocation>,
    ) -> bool {
        match (left, right) {
            (None, None) => true,
            (Some(left), Some(right)) => {
                left.bucket == right.bucket
                    && left.key == right.key
                    && left.status == right.status
                    && left.visibility == right.visibility
            }
            _ => false,
        }
    }

    /// THE BATCH MUST EQUAL THE SINGLE READS IT REPLACES. Read-only against DEV: pick existing photographs (some with
    /// derivatives, some without) and compare the one batched query's card/web/thumb pointers with the three
    /// single-id lookups the page used to make. Also proves unknown ids are absent and no write happens.
    ///
    /// Ignored by default; run with DATABASE_URL_DEV present.
    #[tokio::test]
    #[ignore = "needs DATABASE_URL_DEV; read-only batch/single equivalence"]
    async fn the_batch_pointer_read_matches_the_single_lookups() {
        let database = Database::connect_target(crate::DbTarget::Dev)
            .await
            .expect("DATABASE_URL_DEV must reach the disposable DEV branch");
        let dao = PublicListingDao::new(database);

        // Photographs that DO have at least one derivative (the interesting selection cases).
        let mut ids: Vec<String> = sqlx::query_scalar::<_, String>(
            "select distinct d.derivative_of::text
               from media d
              where d.derivative_of is not null
              limit 5",
        )
        .fetch_all(dao.db.pool())
        .await
        .expect("the derivative parents are readable");
        // One photograph with NO derivatives (the original-only path).
        if let Some(plain) = sqlx::query_scalar::<_, String>(
            "select m.id::text
               from media m
              where m.derivative_of is null
                and not exists (select 1 from media d where d.derivative_of = m.id)
              limit 1",
        )
        .fetch_optional(dao.db.pool())
        .await
        .expect("the derivative-free photograph is readable")
        {
            ids.push(plain);
        }
        assert!(!ids.is_empty(), "DEV must hold at least one media row");

        let batch = dao
            .media_object_locations(&ids)
            .await
            .expect("the batch query runs");
        assert!(
            !batch.is_empty(),
            "at least one requested photograph must be publicly gated"
        );

        for id in &ids {
            // A row the public gate refuses (unpublished Property) is absent from the batch; the single lookups
            // must agree by returning no pointer for every size.
            let row = batch.iter().find(|(batch_id, ..)| batch_id == id);
            for (size_index, size) in ["card", "web", "thumb"].iter().enumerate() {
                let single = dao
                    .media_object_location(id, size)
                    .await
                    .expect("the single lookup runs");
                let batch_location = row.and_then(|(_, card, web, thumb)| match size_index {
                    0 => Some(card),
                    1 => Some(web),
                    _ => Some(thumb),
                });
                assert!(
                    same(batch_location.unwrap_or(&None), &single),
                    "batch {size} for {id} must equal the single lookup: {:?} vs {single:?}",
                    batch_location
                );
            }
        }

        let unknown = dao
            .media_object_locations(&["00000000-0000-4000-8000-000000000000".to_owned()])
            .await
            .expect("an unknown id is a miss, not an error");
        assert!(
            unknown.is_empty(),
            "an unknown id must be absent from the batch"
        );
        let empty = dao.media_object_locations(&[]).await.expect("no ids");
        assert!(empty.is_empty(), "an empty id list issues no rows");
        eprintln!(
            "proof: one batch query matched 3 single lookups for each of {} media ids",
            ids.len()
        );
    }
}
