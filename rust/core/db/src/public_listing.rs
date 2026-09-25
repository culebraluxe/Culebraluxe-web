//! The public site's listing copy: the one-line pitch each PUBLISHED listing carries.
//!
//! PUBLISHED MEANS THE SAME HERE AS EVERYWHERE THE SITE READS: `is_published`, an active listing, not archived, with a
//! slug — the predicate the public inventory and the public document route use. A draft's tagline is not public copy.

use crate::{Database, DbFailure, DbResult};
use domain::{PublicListing, PublicListingCopy};

pub struct PublicListingDao {
    db: Database,
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
