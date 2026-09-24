//! The public site's listing copy: the one-line pitch each PUBLISHED listing carries.
//!
//! PUBLISHED MEANS THE SAME HERE AS EVERYWHERE THE SITE READS: `is_published`, an active listing, not archived, with a
//! slug — the predicate the public inventory and the public document route use. A draft's tagline is not public copy.

use crate::{Database, DbFailure, DbResult};
use domain::PublicListingCopy;

pub struct PublicListingDao {
    db: Database,
}

impl PublicListingDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Taglines for every published listing that has one. A listing with none is simply absent.
    pub async fn listing_copy(&self) -> DbResult<Vec<PublicListingCopy>> {
        let rows = sqlx::query_as::<_, (String, String)>(
            r#"
            select p.slug, btrim(p.tagline) as tagline
            from property p
            where p.is_published = true
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
