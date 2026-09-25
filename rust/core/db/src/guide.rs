//! PostgreSQL repository for the public Island Guide.

use crate::{Database, DbFailure, DbResult};
use domain::GuideItem;

pub struct GuideDao {
    db: Database,
}

impl GuideDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn items(&self) -> DbResult<Vec<GuideItem>> {
        let rows = sqlx::query_as::<_, (
            String, String, String, Option<String>, Option<String>, Option<String>, String,
            Option<String>, Option<String>, Option<String>, Option<String>, Option<f64>, Option<f64>,
            i32, Option<String>, Option<String>
        )>(r#"
            select
                gi.slug,
                gi.section,
                gi.name,
                gi.eyebrow,
                gi.subtitle,
                gi.area,
                gi.description,
                gi.note,
                gi.address,
                gi.phone,
                gi.website_url,
                gi.latitude::float8,
                gi.longitude::float8,
                gi.sort_order,
                card.media_id::text,
                card.alt_text
            from guide_item gi
            left join lateral (
                select m.id as media_id, m.alt_text
                from guide_item_media gim
                join media m on m.id = gim.media_id
                where gim.guide_item_id = gi.id
                  and gim.role = 'card'
                  and m.media_type = 'image'
                order by gim.sort_order asc, gim.created_at asc
                limit 1
            ) card on true
            where gi.is_active = true
            order by
                case gi.section
                    when 'beaches' then 1
                    when 'water' then 2
                    when 'wildlife-land' then 3
                    when 'coffee-casual' then 4
                    when 'dining' then 5
                    when 'getting-here' then 6
                    when 'getting-around' then 7
                    when 'essentials' then 8
                    when 'island-story' then 9
                    else 99
                end,
                gi.sort_order asc,
                gi.name asc
        "#)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("guide.items", &error))?;

        Ok(rows.into_iter().map(|row| GuideItem {
            slug: row.0,
            section: row.1,
            name: row.2,
            eyebrow: row.3,
            subtitle: row.4,
            area: row.5,
            description: row.6,
            note: row.7,
            address: row.8,
            phone: row.9,
            website_url: row.10,
            latitude: row.11,
            longitude: row.12,
            sort_order: row.13,
            image_path: row.14.map(|id| format!("/api/media/{id}")),
            image_alt: row.15,
        }).collect())
    }
}
