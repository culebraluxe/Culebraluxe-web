use crate::{Database, DbFailure, DbResult};
use chrono::{DateTime, Utc};
use domain::MediaAsset;
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct MediaRow {
    property_id: String,
    media_id: String,
    media_type: String,
    role: String,
    sort_order: i32,
    filename: Option<String>,
    mime_type: Option<String>,
    file_size: Option<i64>,
    alt_text: Option<String>,
    caption: Option<String>,
    created_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct MediaDao {
    db: Database,
}

impl MediaDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn for_property(&self, property_id: &str) -> DbResult<Vec<MediaAsset>> {
        let rows = sqlx::query_as::<_, MediaRow>(
            r#"
            select pm.property_id::text as property_id,
                   m.id::text as media_id,
                   m.media_type,
                   pm.role,
                   pm.sort_order,
                   m.filename,
                   m.mime_type,
                   m.file_size,
                   m.alt_text,
                   m.caption,
                   pm.created_at
            from property_media pm
            join media m on m.id = pm.media_id
            where pm.property_id = $1::uuid
            order by pm.sort_order asc nulls last, pm.created_at asc, m.id asc
            "#,
        )
        .bind(property_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.for_property", &error))?;

        Ok(rows
            .into_iter()
            .map(|row| MediaAsset {
                id: row.media_id.clone(),
                property_id: row.property_id,
                media_type: row.media_type,
                role: row.role,
                sort_order: row.sort_order,
                filename: row.filename,
                mime_type: row.mime_type,
                file_size: row.file_size,
                alt_text: row.alt_text,
                caption: row.caption,
                created_at: row.created_at.map(|value| value.to_rfc3339()),
                url: format!("/api/media/{}", row.media_id),
            })
            .collect())
    }
}
