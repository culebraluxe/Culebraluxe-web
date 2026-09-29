//! Moved from `media.rs` (move only): MediaRow, BeginMediaUpload, MediaUploadStatus, MediaUploadAssembly, MediaDerivativeInput, derivative_filename, MediaDao, new.

#[allow(unused_imports)]
use super::*;

#[derive(Debug, FromRow)]
pub(super) struct MediaRow {
    pub(super) property_id: String,
    pub(super) media_id: String,
    pub(super) media_type: String,
    pub(super) role: String,
    pub(super) sort_order: i32,
    pub(super) filename: Option<String>,
    pub(super) mime_type: Option<String>,
    pub(super) file_size: Option<i64>,
    pub(super) alt_text: Option<String>,
    pub(super) caption: Option<String>,
    pub(super) created_at: Option<DateTime<Utc>>,
    pub(super) mux_asset_id: Option<String>,
    pub(super) mux_playback_id: Option<String>,
    pub(super) duration_seconds: Option<String>,
    pub(super) aspect_ratio: Option<String>,
    pub(super) source_url: Option<String>,
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

/// An assembled upload: checked against what the client declared, and ready to be written.
pub struct MediaUploadAssembly {
    pub upload_id: String,
    pub property_id: String,
    pub filename: String,
    pub mime_type: String,
    pub role: String,
    pub alt_text: Option<String>,
    pub bytes: Vec<u8>,
}

/// One downscaled copy on its way into `media`. `kind` is `web` or `thumb` (migration 222's constraint is the check
/// that keeps that honest).
pub struct MediaDerivativeInput {
    pub kind: &'static str,
    pub bytes: Vec<u8>,
}

/// `ZoniBluff_1.jpg` + `web` -> `ZoniBluff_1-web.jpg`. Named so that a person looking at the storage can tell a copy
/// from the original without joining a table.
pub(super) fn derivative_filename(original: &str, kind: &str) -> String {
    match original.rsplit_once('.') {
        Some((stem, _extension)) if !stem.is_empty() => format!("{stem}-{kind}.jpg"),
        _ => format!("{original}-{kind}.jpg"),
    }
}

pub struct MediaDao {
    pub(super) db: Database,
}

impl MediaDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Photographs (originals, image type) that have no copy of this kind yet — the backfill's work list.
    pub async fn images_missing_copy(
        &self,
        kind: &str,
        limit: i64,
    ) -> DbResult<Vec<(String, Option<String>)>> {
        sqlx::query_as::<_, (String, Option<String>)>(
            r#"
            select m.id::text, m.filename
              from media m
             where m.media_type = 'image' and m.derivative_of is null and m.file_data is not null
               and not exists (select 1 from media d where d.derivative_of = m.id and d.derivative_kind = $1)
             order by m.created_at desc
             limit $2
            "#,
        )
        .bind(kind)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.images_missing_copy", &error))
    }

    /// An original photograph's own bytes.
    pub async fn original_bytes(&self, id: &str) -> DbResult<Option<Vec<u8>>> {
        sqlx::query_scalar::<_, Vec<u8>>(
            "select file_data from media where id = $1::uuid and derivative_of is null",
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.original_bytes", &error))
    }

    /// Attaches one derived copy (a JPEG) to its original.
    pub async fn insert_copy(
        &self,
        original_id: &str,
        kind: &str,
        filename: Option<&str>,
        bytes: &[u8],
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            insert into media (file_data, filename, mime_type, file_size, media_type, derivative_of, derivative_kind)
            values ($1, $2, 'image/jpeg', $3, 'image', $4::uuid, $5)
            "#,
        )
        .bind(bytes)
        .bind(filename)
        .bind(bytes.len() as i64)
        .bind(original_id)
        .bind(kind)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.insert_copy", &error))?;
        Ok(())
    }

    pub async fn media_bytes(&self, id: &str) -> DbResult<Option<(String, Vec<u8>)>> {
        sqlx::query_as::<_, (String, Vec<u8>)>(
            r#"
            select coalesce(copy.mime_type, m.mime_type),
                   coalesce(copy.file_data, m.file_data)
              from media m
              join media root on root.id = coalesce(m.derivative_of, m.id)
              left join lateral (
                  select d.file_data, d.mime_type
                    from media d
                   where d.derivative_of = root.id
                     and d.derivative_kind = 'web'
                   limit 1
              ) copy on true
             where m.id = $1::uuid
             limit 1
            "#,
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.bytes", &error))
    }

    pub async fn upload_standalone(
        &self,
        filename: &str,
        mime_type: &str,
        bytes: &[u8],
    ) -> DbResult<(String, String, String, i64)> {
        sqlx::query_as::<_, (String, String, String, i64)>(
            r#"
            insert into media (file_data, filename, mime_type, file_size, media_type)
            values ($1, $2, $3, $4, 'image')
            returning id::text, filename, mime_type, file_size
            "#,
        )
        .bind(bytes)
        .bind(filename)
        .bind(mime_type)
        .bind(bytes.len() as i64)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.upload_standalone", &error))
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

    /// Takes a photograph off a property. The photograph itself (and, by cascade, its web and thumbnail copies) goes
    /// too unless another property still shows it. `false` when the photograph was not on this property.
    pub async fn remove_property_media(&self, property_id: &str, media_id: &str) -> DbResult<bool> {
        let mut tx = self.db.begin("media.remove").await?;
        let removed = sqlx::query(
            "delete from property_media where property_id = $1::uuid and media_id = $2::uuid",
        )
        .bind(property_id)
        .bind(media_id)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.remove.unlink", &error))?
        .rows_affected();
        if removed == 0 {
            return Ok(false);
        }
        sqlx::query(
            "delete from media where id = $1::uuid and not exists (select 1 from property_media where media_id = $1::uuid)",
        )
        .bind(media_id)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.remove.delete", &error))?;
        tx.commit().await?;
        Ok(true)
    }

    /// Whether this property already shows this file (same name, same size): a folder chosen again skips it.
    pub async fn property_has_photo(
        &self,
        property_id: &str,
        filename: &str,
        byte_size: i64,
    ) -> DbResult<bool> {
        sqlx::query_scalar::<_, bool>(
            r#"
            select exists (
                select 1 from property_media pm join media m on m.id = pm.media_id
                 where pm.property_id = $1::uuid and m.filename = $2 and m.file_size = $3 and m.derivative_of is null
            )
            "#,
        )
        .bind(property_id)
        .bind(filename)
        .bind(byte_size)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.upload.already_stored", &error))
    }

    /// An unfinished upload of this same file to this property, to resume: its id, status, and the chunks it has.
    pub async fn resumable_media_upload(
        &self,
        property_id: &str,
        filename: &str,
        byte_size: i64,
        chunk_size: i32,
    ) -> DbResult<Option<(String, String, Vec<i32>)>> {
        let found = sqlx::query_as::<_, (String, String)>(
            r#"
            select upload_id::text, status from media_upload
             where property_id = $1::uuid and filename = $2 and byte_size = $3 and chunk_size = $4
             order by created_at desc
             limit 1
            "#,
        )
        .bind(property_id)
        .bind(filename)
        .bind(byte_size)
        .bind(chunk_size)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.upload.resumable", &error))?;
        let Some((upload_id, status)) = found else {
            return Ok(None);
        };
        let received = sqlx::query_scalar::<_, i32>(
            "select chunk_index from media_upload_chunk where upload_id = $1::uuid order by chunk_index",
        )
        .bind(&upload_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.upload.received", &error))?;
        Ok(Some((upload_id, status, received)))
    }

    /// Claims an upload for finishing: `uploading` (or a `failed` one being retried) becomes `complete`, atomically, so
    /// two requests can never both assemble and store the same photograph. `false` when nothing was claimable.
    pub async fn claim_media_upload(&self, upload_id: &str) -> DbResult<bool> {
        let claimed = sqlx::query_scalar::<_, String>(
            "update media_upload set status = 'complete' where upload_id = $1::uuid and status in ('uploading', 'failed') returning status",
        )
        .bind(upload_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.upload.claim", &error))?;
        Ok(claimed.is_some())
    }

    /// Marks a claimed upload as failed; its chunks stay, so finishing can be tried again.
    pub async fn fail_media_upload(&self, upload_id: &str) -> DbResult<()> {
        sqlx::query("update media_upload set status = 'failed' where upload_id = $1::uuid and status = 'complete'")
            .bind(upload_id)
            .execute(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("media.upload.fail", &error))?;
        Ok(())
    }

    /// The upload's status, or `None` once it is gone (a finished upload's manifest is deleted with its chunks).
    pub async fn media_upload_state(&self, upload_id: &str) -> DbResult<Option<String>> {
        sqlx::query_scalar::<_, String>(
            "select status from media_upload where upload_id = $1::uuid",
        )
        .bind(upload_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.upload.state", &error))
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
}
