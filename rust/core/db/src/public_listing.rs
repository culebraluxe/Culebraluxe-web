//! The public site's listing copy: the one-line pitch each PUBLISHED listing carries.
//!
//! PUBLISHED MEANS THE SAME HERE AS EVERYWHERE THE SITE READS: `is_published`, an active listing, not archived, with a
//! slug — the predicate the public inventory and the public document route use. A draft's tagline is not public copy.

use crate::{Database, DbFailure, DbResult};
use domain::{PublicListing, PublicListingCopy, PublicProperty, PublicPropertyMedia};

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
            beach_access: self.beach_access,
            hero_media_id: self.hero_media_id,
            hero_alt: self.hero_alt,
        }
    }
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
    bedrooms: Option<i32>,
    bathrooms: Option<f64>,
    square_feet: Option<i32>,
    lot_size: Option<f64>,
    lot_size_units: Option<String>,
    year_built: Option<i32>,
    architecture_notes: Option<String>,
    short_description: Option<String>,
    editorial_description: Option<String>,
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
                p.lot_size_units,
                coalesce(p.has_ocean_view, false) as has_ocean_view,
                coalesce(p.has_bay_view, false) as has_bay_view,
                coalesce(p.has_beach_view, false) as has_beach_view,
                coalesce(p.has_harbor_view, false) as has_harbor_view,
                coalesce(p.has_island_view, false) as has_island_view,
                coalesce(p.has_mountain_view, false) as has_mountain_view,
                coalesce(p.has_sunrise_view, false) as has_sunrise_view,
                coalesce(p.has_sunset_view, false) as has_sunset_view,
                coalesce(p.has_beach_access, false) as beach_access,
                hero.media_id as hero_media_id,
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
                p.bedrooms::int as bedrooms,
                p.bathrooms::float8 as bathrooms,
                p.square_feet::int as square_feet,
                coalesce(p.lot_size_acres, p.lot_size)::float8 as lot_size,
                p.lot_size_units,
                (p.year_built::text)::int as year_built,
                p.architecture_notes,
                p.short_description,
                p.editorial_description,
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
        let media_rows = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<f64>, i32, Option<String>, Option<String>, Option<f64>)>(
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

        Ok(Some(PublicProperty {
            key: row.row_key,
            name: row.name,
            status: row.status,
            property_type: row.property_type,
            list_price: row.list_price,
            city: row.city,
            state_or_province: row.state_or_province,
            neighborhood: row.neighborhood,
            bedrooms: row.bedrooms,
            bathrooms: row.bathrooms,
            square_feet: row.square_feet,
            lot_size: row.lot_size,
            lot_size_units: row.lot_size_units,
            year_built: row.year_built,
            architecture_notes: row.architecture_notes,
            short_description: row.short_description,
            editorial_description: row.editorial_description,
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
                p.lot_size_units,
                coalesce(p.has_ocean_view, false) as has_ocean_view,
                coalesce(p.has_bay_view, false) as has_bay_view,
                coalesce(p.has_beach_view, false) as has_beach_view,
                coalesce(p.has_harbor_view, false) as has_harbor_view,
                coalesce(p.has_island_view, false) as has_island_view,
                coalesce(p.has_mountain_view, false) as has_mountain_view,
                coalesce(p.has_sunrise_view, false) as has_sunrise_view,
                coalesce(p.has_sunset_view, false) as has_sunset_view,
                coalesce(p.has_beach_access, false) as beach_access,
                hero.media_id as hero_media_id,
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

    /// The servable bytes for one photograph: the web copy when there is one, the original otherwise.
    ///
    /// THE GATE IS EVALUATED ON THE ORIGINAL. A copy has no `property_media` link of its own, so asking whether the
    /// COPY is published would answer "no links, therefore fine" and expose every copy of every unpublished Property.
    /// Media with no Property link at all stays reachable — it is not listing media.
    ///
    /// `None` covers both "no such media" and "not published": an anonymous visitor must not be able to tell those
    /// apart, which is why this returns one thing rather than an error.
    pub async fn media_bytes(&self, id: &str) -> DbResult<Option<(String, Vec<u8>)>> {
        let row = sqlx::query_as::<_, (String, Vec<u8>)>(
            r#"
            select
                coalesce(copy.mime_type, m.mime_type),
                coalesce(copy.file_data, m.file_data)
            from media m
            join media root on root.id = coalesce(m.derivative_of, m.id)
            left join lateral (
                select d.file_data, d.mime_type
                  from media d
                 where d.derivative_of = root.id and d.derivative_kind = 'web'
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
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("public_listing.media_bytes", &error))?;

        Ok(row)
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
