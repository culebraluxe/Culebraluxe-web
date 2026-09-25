//! The public site's listing copy: the one-line pitch each PUBLISHED listing carries.
//!
//! PUBLISHED MEANS THE SAME HERE AS EVERYWHERE THE SITE READS: `is_published`, an active listing, not archived, with a
//! slug — the predicate the public inventory and the public document route use. A draft's tagline is not public copy.

use crate::{Database, DbFailure, DbResult};
use domain::{PublicListing, PublicListingCopy, PublicProperty};

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

    /// The taglines of published listings, for the anonymous public site: the same guest door the public document route
    /// uses (no identity headers), authorized as the published `property.public.read`.
    pub async fn listings(&self) -> DbResult<Vec<PublicListing>> {
        // THE VISIBILITY RULE, AND NOTHING ELSE. No slug requirement (the row's key is its slug or its id, and the
        // public resolver accepts the name too), no price requirement (zero is a price), no hero requirement (the
        // first photograph is the hero). Every extra condition here is a way for a listing that exists to be missing
        // from the site, which is the failure this read exists to end.
        let rows = sqlx::query_as::<_, (String, String, Option<String>, String, Option<f64>, bool)>(
            r#"
            select
                coalesce(p.slug, p.id::text) as row_key,
                p.name,
                nullif(btrim(coalesce(p.property_type, '')), ''),
                p.status,
                p.list_price::float8,
                coalesce(p.featured, false)
            from property p
            where p.archived_at is null
              and p.status in ('active', 'under_contract', 'sold')
              and p.name is not null
            order by coalesce(p.featured, false) desc, p.name asc
            "#,
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("public_listing.listings", &error))?;

        Ok(rows
            .into_iter()
            .map(
                |(key, name, property_type, status, list_price, featured)| PublicListing {
                    key,
                    name,
                    property_type,
                    status,
                    list_price,
                    featured,
                },
            )
            .collect())
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

        // The marked hero sorts first, then gallery order — so `first()` IS the hero whether or not one was flagged.
        let media = sqlx::query_as::<_, (String, String)>(
            r#"
            select m.id::text, pm.role
              from property_media pm
              join media m on m.id = pm.media_id
             where pm.property_id = $1::uuid
               and m.media_type = 'image'
             order by case when pm.role = 'hero' then 0 else 1 end, pm.sort_order, m.created_at
            "#,
        )
        .bind(&row.id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("public_listing.property_media", &error))?;

        let hero_media_id = media.first().map(|(media_id, _)| media_id.clone());
        let gallery_media_ids = media.iter().map(|(media_id, _)| media_id.clone()).collect();

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
            gallery_media_ids,
            video_count: row.video_count,
        }))
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
              and p.is_active_listing = true
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
