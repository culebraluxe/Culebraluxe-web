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
/// One chunked upload, as declared by the client before any bytes arrive.
///
/// The declared size and count are a PROMISE, and storing them is only worth anything because completion checks
/// them: a truncated photograph must not be storable as a photograph, and a client must not be able to send far more
/// than it declared by understating the count.
pub struct BeginMediaUpload {
    pub upload_id: String,
    pub property_id: String,
    pub filename: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub chunk_count: i32,
    pub chunk_size: i32,
    pub sha256: Option<String>,
    pub role: String,
    pub alt_text: Option<String>,
}

/// How far along an upload is: what was declared, against what has actually landed.
pub struct MediaUploadStatus {
    pub upload_id: String,
    pub byte_size: i64,
    pub chunk_count: i32,
    pub received_bytes: i64,
    pub received_chunks: i32,
    pub status: String,
}

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
            with existing as (
                select id
                from media
                where mux_asset_id = $2
                order by created_at asc
                limit 1
            ),
            inserted as (
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
                select
                    null,
                    'mux-' || $2,
                    'application/vnd.apple.mpegurl',
                    null,
                    null,
                    $1,
                    'video',
                    $2,
                    $3,
                    $4::numeric,
                    $5,
                    'https://stream.mux.com/' || $3 || '.m3u8'
                where not exists (select 1 from existing)
                returning id
            )
            select id::text from existing
            union all
            select id::text from inserted
            limit 1
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
            on conflict (property_id, media_id)
            do update set role = excluded.role
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

    /// Opens an upload. Idempotent: opening the same upload twice returns the manifest that already exists, so a
    /// client that retried `init` — a dropped response, a refreshed tab — does not wipe bytes it already sent.
    pub async fn begin_media_upload(
        &self,
        request: &BeginMediaUpload,
    ) -> DbResult<MediaUploadStatus> {
        sqlx::query(
            r#"
            insert into media_upload (
                upload_id, property_id, filename, mime_type, byte_size, chunk_count, chunk_size, sha256, role, alt_text
            )
            values ($1::uuid, $2::uuid, $3, $4, $5, $6, $7, $8, $9, $10)
            on conflict (upload_id) do nothing
            "#,
        )
        .bind(&request.upload_id)
        .bind(&request.property_id)
        .bind(&request.filename)
        .bind(&request.mime_type)
        .bind(request.byte_size)
        .bind(request.chunk_count)
        .bind(request.chunk_size)
        .bind(request.sha256.as_deref())
        .bind(&request.role)
        .bind(request.alt_text.as_deref())
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.upload.begin", &error))?;

        self.media_upload_status(&request.upload_id).await
    }

    /// Where an upload stands: bytes and chunks received, against what was declared.
    pub async fn media_upload_status(&self, upload_id: &str) -> DbResult<MediaUploadStatus> {
        let row = sqlx::query_as::<_, (String, i64, i32, i64, i32, String)>(
            r#"
            select m.upload_id::text,
                   m.byte_size,
                   m.chunk_count,
                   coalesce(sum(length(c.bytes))::bigint, 0) as received_bytes,
                   count(c.chunk_index)::int as received_chunks,
                   m.status
              from media_upload m
              left join media_upload_chunk c on c.upload_id = m.upload_id
             where m.upload_id = $1::uuid
             group by m.upload_id, m.byte_size, m.chunk_count, m.status
            "#,
        )
        .bind(upload_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.upload.status", &error))?;

        let Some((upload_id, byte_size, chunk_count, received_bytes, received_chunks, status)) =
            row
        else {
            return Err(DbFailure::configuration(
                "media.upload.status",
                "the upload is unknown or has already been completed",
            ));
        };

        Ok(MediaUploadStatus {
            upload_id,
            byte_size,
            chunk_count,
            received_bytes,
            received_chunks,
            status,
        })
    }

    /// Stores one chunk. Idempotent by primary key: re-sending a chunk REPLACES it rather than duplicating it, which
    /// is what makes a retry safe — a client that never saw a response can send the same index again.
    ///
    /// The guards live here rather than in the caller because every caller's mistake would otherwise become a stored
    /// defect: a chunk for an unknown upload, an index outside the declared count, or bytes that would push the
    /// upload past the size it declared are all refused before anything is written.
    pub async fn stage_media_chunk(
        &self,
        upload_id: &str,
        chunk_index: i32,
        bytes: &[u8],
    ) -> DbResult<MediaUploadStatus> {
        let manifest = sqlx::query_as::<_, (String, i64, i32)>(
            r#"
            select status, byte_size, chunk_count
              from media_upload
             where upload_id = $1::uuid
            "#,
        )
        .bind(upload_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.upload.chunk_manifest", &error))?;

        let Some((status, byte_size, chunk_count)) = manifest else {
            return Err(DbFailure::configuration(
                "media.upload.chunk",
                "the upload is unknown or has already been completed",
            ));
        };
        if status != "uploading" {
            return Err(DbFailure::configuration(
                "media.upload.chunk",
                format!("this upload is {status} and no longer accepts chunks"),
            ));
        }
        if chunk_index < 0 || chunk_index >= chunk_count {
            return Err(DbFailure::configuration(
                "media.upload.chunk",
                format!("chunk {chunk_index} is outside the declared {chunk_count} chunks"),
            ));
        }

        sqlx::query(
            r#"
            insert into media_upload_chunk (upload_id, chunk_index, bytes)
            values ($1::uuid, $2, $3)
            on conflict (upload_id, chunk_index) do update set bytes = excluded.bytes
            "#,
        )
        .bind(upload_id)
        .bind(chunk_index)
        .bind(bytes)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.upload.chunk", &error))?;

        let progress = self.media_upload_status(upload_id).await?;
        if progress.received_bytes > byte_size {
            return Err(DbFailure::configuration(
                "media.upload.chunk",
                format!(
                    "these chunks add up to {} bytes but the upload declared {byte_size}",
                    progress.received_bytes
                ),
            ));
        }
        Ok(progress)
    }

    /// Assembles the chunks and writes the photograph — through the same `insert into media (...)` every other upload
    /// path uses, so nothing downstream can tell that the bytes arrived in pieces.
    ///
    /// EVERY DECLARED CHUNK MUST BE PRESENT AND THE TOTAL MUST MATCH. That is what the manifest is for: a missing
    /// chunk would otherwise be stored as a shorter photograph, and a partial upload would look complete to every
    /// reader downstream.
    pub async fn complete_media_upload(
        &self,
        upload_id: &str,
    ) -> DbResult<UploadPropertyMediaResult> {
        let status = self.media_upload_status(upload_id).await?;
        if status.status != "uploading" {
            return Err(DbFailure::configuration(
                "media.upload.complete",
                format!("this upload is {}", status.status),
            ));
        }
        if status.received_chunks != status.chunk_count {
            return Err(DbFailure::configuration(
                "media.upload.complete",
                format!(
                    "{} of {} chunks arrived; the upload is incomplete",
                    status.received_chunks, status.chunk_count
                ),
            ));
        }

        let (property_id, filename, mime_type, role, alt_text) =
            sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
                r#"
                select property_id::text, filename, mime_type, role, alt_text
                  from media_upload
                 where upload_id = $1::uuid
                "#,
            )
            .bind(upload_id)
            .fetch_one(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("media.upload.complete_manifest", &error))?;

        let chunks = sqlx::query_as::<_, (i32, Vec<u8>)>(
            r#"
            select chunk_index, bytes
              from media_upload_chunk
             where upload_id = $1::uuid
             order by chunk_index
            "#,
        )
        .bind(upload_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.upload.complete_chunks", &error))?;

        let mut assembled: Vec<u8> = Vec::with_capacity(status.byte_size.max(0) as usize);
        for (_, bytes) in chunks {
            assembled.extend_from_slice(&bytes);
        }
        if assembled.len() as i64 != status.byte_size {
            return Err(DbFailure::configuration(
                "media.upload.complete",
                format!(
                    "the assembled upload is {} bytes but {} was declared",
                    assembled.len(),
                    status.byte_size
                ),
            ));
        }

        let mut tx = self.db.begin("media.upload.complete").await?;

        let media_id = sqlx::query_scalar::<_, String>(
            r#"
            insert into media (
                file_data, filename, mime_type, file_size, alt_text, media_type
            )
            values ($1, $2, $3, $4, $5, 'image')
            returning id::text
            "#,
        )
        .bind(&assembled)
        .bind(&filename)
        .bind(&mime_type)
        .bind(assembled.len() as i64)
        .bind(alt_text.as_deref())
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.upload.complete_insert", &error))?;

        if role == "hero" {
            sqlx::query(
                r#"
                update property_media
                set role = 'gallery'
                where property_id = $1::uuid and role = 'hero'
                "#,
            )
            .bind(&property_id)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("media.upload.complete_demote", &error))?;
        }

        sqlx::query(
            r#"
            insert into property_media (property_id, media_id, role, sort_order)
            values ($1::uuid, $2::uuid, $3, 0)
            "#,
        )
        .bind(&property_id)
        .bind(&media_id)
        .bind(&role)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.upload.complete_attach", &error))?;

        // The staged bytes go as soon as the photograph exists: the `media` row is the record now, and keeping a
        // second copy would grow Neon with every upload for no reader's benefit.
        sqlx::query("delete from media_upload_chunk where upload_id = $1::uuid")
            .bind(upload_id)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("media.upload.complete_drop_chunks", &error))?;
        sqlx::query("delete from media_upload where upload_id = $1::uuid")
            .bind(upload_id)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("media.upload.complete_drop_manifest", &error))?;

        tx.commit().await?;

        Ok(UploadPropertyMediaResult {
            ok: true,
            media_id,
            property_id,
            role,
        })
    }

    /// Drops uploads nobody finished — a browser closed at chunk 3 of 5, a laptop that slept. Without this, staged
    /// bytes accumulate in Neon forever and nothing notices, because every row involved is individually valid.
    pub async fn sweep_stale_media_uploads(&self, older_than_hours: i32) -> DbResult<u64> {
        let mut tx = self.db.begin("media.upload.sweep").await?;

        let chunk_rows = sqlx::query(
            r#"
            delete from media_upload_chunk c
             using media_upload m
             where c.upload_id = m.upload_id
               and m.status = 'uploading'
               and m.created_at < now() - make_interval(hours => $1)
            "#,
        )
        .bind(older_than_hours)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.upload.sweep_chunks", &error))?;

        let manifest_rows = sqlx::query(
            r#"
            delete from media_upload
             where status = 'uploading'
               and created_at < now() - make_interval(hours => $1)
            "#,
        )
        .bind(older_than_hours)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.upload.sweep_manifests", &error))?;

        tx.commit().await?;
        Ok(chunk_rows.rows_affected() + manifest_rows.rows_affected())
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
