use crate::{Database, DbFailure, DbResult};
use domain::{PublishingListing, PublishingSnapshot};
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct PublishingRow {
    property_id: String,
    name: String,
    status: String,
    slug: Option<String>,
    location: Option<String>,
    is_active_listing: bool,
    is_published: bool,
    list_price: Option<String>,
    property_type: Option<String>,
    public_remarks: Option<String>,
    short_description: Option<String>,
    seo_title: Option<String>,
    seo_description: Option<String>,
    catastro_number: Option<String>,
    legal_owner_name: Option<String>,
    image_count: i64,
    video_count: i64,
    hero_count: i64,
    listing_type: Option<String>,
    agent_mls_id: Option<String>,
}

#[derive(Clone)]
pub struct PublishingDao {
    db: Database,
}

impl PublishingDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn snapshot(&self) -> DbResult<PublishingSnapshot> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, PublishingRow>(
                r#"
                select
                  p.id::text as property_id,
                  coalesce(nullif(trim(p.name), ''), 'Unnamed property') as name,
                  p.status,
                  p.slug,
                  coalesce(p.location, p.address_line1, p.city) as location,
                  p.is_active_listing,
                  p.is_published,
                  p.list_price::text as list_price,
                  p.property_type,
                  p.public_remarks,
                  p.short_description,
                  p.seo_title,
                  p.seo_description,
                  p.catastro_number,
                  p.legal_owner_name,
                  (select count(*)::bigint
                     from property_media pm join media m on m.id=pm.media_id
                    where pm.property_id=p.id and m.media_type='image') as image_count,
                  (select count(*)::bigint
                     from property_media pm join media m on m.id=pm.media_id
                    where pm.property_id=p.id and m.media_type='video') as video_count,
                  (select count(*)::bigint
                     from property_media pm join media m on m.id=pm.media_id
                    where pm.property_id=p.id and m.media_type='image' and pm.role='hero') as hero_count,
                  s.listing_type,
                  s.agent_mls_id
                from property p
                left join property_stellar_listing s on s.property_id=p.id
                where p.archived_at is null
                  and (p.is_active_listing or p.status in ('active','coming_soon','under_contract'))
                order by
                  case when p.is_published then 0 else 1 end,
                  case when p.is_active_listing then 0 else 1 end,
                  p.updated_at desc,
                  p.name
                "#,
            )
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("publishing.snapshot", &error))
        })?;

        let listings: Vec<PublishingListing> = rows.into_iter().map(project).collect();
        Ok(PublishingSnapshot {
            ready_count: listings
                .iter()
                .filter(|listing| listing.stellar_package_ready && listing.website_ready)
                .count() as i64,
            live_count: listings
                .iter()
                .filter(|listing| listing.is_published)
                .count() as i64,
            listings,
        })
    }
}

fn present(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

fn project(row: PublishingRow) -> PublishingListing {
    let copy_ready = present(row.public_remarks.as_deref())
        && present(row.short_description.as_deref())
        && present(row.seo_title.as_deref())
        && present(row.seo_description.as_deref());
    let media_ready = row.image_count > 0 && row.hero_count > 0;
    let website_ready = present(row.slug.as_deref()) && copy_ready && media_ready;
    let facebook_ready = present(row.public_remarks.as_deref())
        && row.image_count > 0
        && present(row.list_price.as_deref());
    let stellar_package_ready = present(row.list_price.as_deref())
        && present(row.property_type.as_deref())
        && present(row.catastro_number.as_deref())
        && present(row.legal_owner_name.as_deref())
        && present(row.listing_type.as_deref())
        && present(row.agent_mls_id.as_deref())
        && media_ready;

    let mut missing = Vec::new();
    for (ok, label) in [
        (present(row.list_price.as_deref()), "List price"),
        (present(row.property_type.as_deref()), "Property type"),
        (present(row.catastro_number.as_deref()), "Catastro"),
        (present(row.legal_owner_name.as_deref()), "Legal owner"),
        (present(row.public_remarks.as_deref()), "Public remarks"),
        (
            present(row.short_description.as_deref()),
            "Short description",
        ),
        (present(row.seo_title.as_deref()), "SEO title"),
        (present(row.seo_description.as_deref()), "SEO description"),
        (row.hero_count > 0, "Hero image"),
        (row.image_count > 0, "Listing photos"),
        (present(row.listing_type.as_deref()), "MLS listing type"),
        (present(row.agent_mls_id.as_deref()), "Agent MLS ID"),
    ] {
        if !ok {
            missing.push(label.to_owned());
        }
    }

    PublishingListing {
        property_id: row.property_id,
        name: row.name,
        status: row.status,
        slug: row.slug,
        location: row.location,
        is_active_listing: row.is_active_listing,
        is_published: row.is_published,
        list_price: row.list_price,
        property_type: row.property_type,
        image_count: row.image_count,
        video_count: row.video_count,
        has_hero: row.hero_count > 0,
        copy_ready,
        media_ready,
        website_ready,
        facebook_ready,
        stellar_package_ready,
        listing_type: row.listing_type,
        agent_mls_id: row.agent_mls_id,
        missing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_names_missing_inputs_instead_of_guessing() {
        let listing = project(PublishingRow {
            property_id: "p".into(),
            name: "Casa".into(),
            status: "active".into(),
            slug: Some("casa".into()),
            location: None,
            is_active_listing: true,
            is_published: false,
            list_price: Some("1200000".into()),
            property_type: Some("residential".into()),
            public_remarks: Some("Public".into()),
            short_description: Some("Short".into()),
            seo_title: Some("Casa".into()),
            seo_description: Some("Sea view".into()),
            catastro_number: None,
            legal_owner_name: Some("Owner".into()),
            image_count: 4,
            video_count: 0,
            hero_count: 1,
            listing_type: Some("Exclusive Right".into()),
            agent_mls_id: Some("MLS1".into()),
        });
        assert!(listing.website_ready);
        assert!(!listing.stellar_package_ready);
        assert_eq!(listing.missing, vec!["Catastro"]);
    }
}
