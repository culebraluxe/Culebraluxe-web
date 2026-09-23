use crate::{Database, DbFailure, DbResult};
use chrono::{DateTime, Utc};
use domain::{
    AttachPropertyVideoRequest, AttachPropertyVideoResult, MediaAsset, UploadPropertyMediaRequest,
    UploadPropertyMediaResult,
};
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
    mux_asset_id: Option<String>,
    mux_playback_id: Option<String>,
    duration_seconds: Option<String>,
    aspect_ratio: Option<String>,
    source_url: Option<String>,
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
                   pm.created_at,
                   m.mux_asset_id,
                   m.mux_playback_id,
                   m.duration_seconds::text as duration_seconds,
                   m.aspect_ratio,
                   m.source_url
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
                mux_asset_id: row.mux_asset_id,
                mux_playback_id: row.mux_playback_id,
                duration_seconds: row.duration_seconds,
                aspect_ratio: row.aspect_ratio,
                source_url: row.source_url,
                url: format!("/api/media/{}", row.media_id),
            })
            .collect())
    }

    pub async fn attach_property_video(
        &self,
        request: &AttachPropertyVideoRequest,
    ) -> DbResult<AttachPropertyVideoResult> {
        let mut tx = self.db.begin("media.attach_property_video").await?;

        let media_id = sqlx::query_scalar::<_, String>(
            r#"
            insert into media (
                file_data,
                filename,
                mime_type,
                file_size,
                alt_text,
                caption,
                media_type,
                mux_asset_id,
                mux_playback_id,
                duration_seconds,
                aspect_ratio,
                source_url
            )
            values (
                null,
                null,
                null,
                null,
                null,
                $1,
                'video',
                $2,
                $3,
                $4::numeric,
                $5,
                null
            )
            returning id::text
            "#,
        )
        .bind(&request.caption)
        .bind(&request.mux_asset_id)
        .bind(&request.mux_playback_id)
        .bind(&request.duration_seconds)
        .bind(&request.aspect_ratio)
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.video.insert_media", &error))?;

        sqlx::query(
            r#"
            insert into property_media (property_id, media_id, role, sort_order)
            values (
                $1::uuid,
                $2::uuid,
                $3,
                coalesce(
                    (
                        select max(sort_order) + 1
                        from property_media
                        where property_id = $1::uuid
                          and role in ('video', 'short')
                    ),
                    0
                )
            )
            "#,
        )
        .bind(&request.property_id)
        .bind(&media_id)
        .bind(&request.role)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.video.attach_property", &error))?;

        tx.commit().await?;

        Ok(AttachPropertyVideoResult {
            ok: true,
            media_id,
            property_id: request.property_id.clone(),
            role: request.role.clone(),
            mux_asset_id: request.mux_asset_id.clone(),
            mux_playback_id: request.mux_playback_id.clone(),
        })
    }

    pub async fn upload_property_media(
        &self,
        request: &UploadPropertyMediaRequest,
    ) -> DbResult<UploadPropertyMediaResult> {
        let mut tx = self.db.begin("media.upload_property_media").await?;

        let media_id = sqlx::query_scalar::<_, String>(
            r#"
            insert into media (
                file_data, filename, mime_type, file_size, alt_text, media_type
            )
            values ($1, $2, $3, $4, $5, 'image')
            returning id::text
            "#,
        )
        .bind(&request.bytes)
        .bind(&request.filename)
        .bind(&request.mime_type)
        .bind(request.bytes.len() as i64)
        .bind(&request.alt_text)
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.upload.insert_media", &error))?;

        if request.role == "hero" {
            sqlx::query(
                r#"
                update property_media
                set role = 'gallery'
                where property_id = $1::uuid and role = 'hero'
                "#,
            )
            .bind(&request.property_id)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("media.upload.demote_hero", &error))?;
        }

        sqlx::query(
            r#"
            insert into property_media (property_id, media_id, role, sort_order)
            values ($1::uuid, $2::uuid, $3, 0)
            "#,
        )
        .bind(&request.property_id)
        .bind(&media_id)
        .bind(&request.role)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.upload.attach_property", &error))?;

        tx.commit().await?;

        Ok(UploadPropertyMediaResult {
            ok: true,
            media_id,
            property_id: request.property_id.clone(),
            role: request.role.clone(),
        })
    }
}
