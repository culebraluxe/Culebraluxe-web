//! Media rows: the DAO, its object-first writers and readers, and the property media link.
//!
//! The chunk-staging protocol (`media_upload` / `media_upload_chunk`) is retired: its manifest, status and
//! assembly types and every DAO method that wrote a staged piece are gone. The two remaining legacy shapes are
//! marked `LEGACY` below — the blob reader `media_bytes` (still reachable through `MEDIA_OBJECT_STORAGE_READS`)
//! and the blob writer `upload_standalone` (the signed-PDF door) — and are the only reasons `media.file_data`
//! still exists.

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

pub struct MediaDao {
    pub(super) db: Database,
}

impl MediaDao {
    pub(super) async fn insert_object_row(
        tx: &mut crate::DbTransaction,
        row: &MediaObjectRow,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            insert into media (
                id, filename, mime_type, file_size, alt_text, media_type,
                derivative_of, derivative_kind, object_bucket, object_key, object_etag,
                object_sha256, object_size, object_migrated_at, storage_status, visibility
            )
            values (
                $1::uuid, $2, $3, $4, $5, $6, $7::uuid, $8, $9, $10, $11,
                $12, $13, now(), $14, $15
            )
            "#,
        )
        .bind(&row.id)
        .bind(&row.filename)
        .bind(&row.mime_type)
        .bind(row.object.size)
        .bind(row.alt_text.as_deref())
        .bind(&row.media_type)
        .bind(row.derivative_of.as_deref())
        .bind(row.derivative_kind.as_deref())
        .bind(&row.object.bucket)
        .bind(&row.object.key)
        .bind(row.object.etag.as_deref())
        .bind(row.object.sha256.as_deref())
        .bind(row.object.size)
        .bind(&row.object.status)
        .bind(row.object.visibility.as_deref())
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.insert_object_row", &error))?;
        Ok(())
    }

    /// Object-first standalone image: one row, no property link.
    pub async fn upload_standalone_object(&self, row: &MediaObjectRow) -> DbResult<()> {
        let mut tx = self.db.begin("media.upload_standalone_object").await?;
        Self::insert_object_row(&mut tx, row).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Bounded, restartable selection of legacy media for object-store migration.
    ///
    /// The ordering is by id so an interrupted run can resume with `after_id`. `is_public` mirrors
    /// the public listing predicate and applies to the root media, so an unpublished property's
    /// derivative is private as well. Documents are never public here; Vault stays private.
    pub async fn list_media_for_migration(
        &self,
        only_id: Option<&str>,
        after_id: Option<&str>,
        limit: i64,
    ) -> DbResult<Vec<MediaMigrationRow>> {
        let rows = sqlx::query_as::<_, MediaMigrationRow>(
            r#"
            select
                m.id::text as id,
                m.media_type,
                m.mime_type,
                m.filename,
                m.file_data,
                m.derivative_of::text as derivative_of,
                m.derivative_kind,
                (select pm.property_id::text
                   from property_media pm
                  where pm.media_id = coalesce(m.derivative_of, m.id)
                  order by pm.property_id
                  limit 1) as property_id,
                (m.media_type = 'image' and exists (
                    select 1
                      from property_media pm
                      join property p on p.id = pm.property_id
                     where pm.media_id = coalesce(m.derivative_of, m.id)
                       and p.archived_at is null
                       and p.is_published
                       and p.slug is not null
                       and p.status in ('active', 'under_contract', 'sold')
                )) as is_public,
                m.storage_status
            from media m
            where m.file_data is not null
              and m.media_type in ('image', 'document')
              -- Bulk runs never re-read a VERIFIED row; --only still reports it as a skip.
              and ($1::text is not null or m.storage_status not in ('VERIFIED', 'OBJECT_PRIMARY'))
              and ($1::text is null or m.id = $1::uuid)
              and ($2::text is null or m.id > $2::uuid)
            order by m.id
            limit $3
            "#,
        )
        .bind(only_id)
        .bind(after_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.list_media_for_migration", &error))?;
        Ok(rows)
    }

    /// Read only the storage pointer, never `file_data`.
    ///
    /// The SERVABLE copy is what a reader gets, not the record copy: for a photograph that is the `web`
    /// derivative when one exists (the same rule `media_bytes` applies to the legacy blob), otherwise the
    /// row's own object. Object-first uploads give every row its own pointer, so resolving here keeps the
    /// read path working for callers that only know the media id they attached to the property.
    pub async fn media_object_location(
        &self,
        media_id: &str,
    ) -> DbResult<Option<MediaObjectLocation>> {
        let row = sqlx::query_as::<
            _,
            (
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
            ),
        >(
            r#"
            select
                case when copy.id is not null then copy.object_bucket else m.object_bucket end,
                case when copy.id is not null then copy.object_key else m.object_key end,
                case when copy.id is not null then copy.storage_status else m.storage_status end,
                case when copy.id is not null then copy.visibility else m.visibility end
              from media m
              join media root on root.id = coalesce(m.derivative_of, m.id)
              left join lateral (
                  select d.id, d.object_bucket, d.object_key, d.storage_status, d.visibility
                    from media d
                   where d.derivative_of = root.id
                     and d.derivative_kind = 'web'
                   limit 1
              ) copy on true
             where m.id = $1::uuid
             limit 1
            "#,
        )
        .bind(media_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.media_object_location", &error))?;
        Ok(row.and_then(|(bucket, key, status, visibility)| {
            Some(MediaObjectLocation {
                bucket: bucket?,
                key: key?,
                status: status.unwrap_or_else(|| "LEGACY_BLOB".to_owned()),
                visibility: visibility.unwrap_or_else(|| "private".to_owned()),
            })
        }))
    }

    /// The id of the photograph this property already shows under this name and size, if any.
    ///
    /// The full-file upload path uses this as its retry guard: a request resent after a lost response must
    /// answer with the row that already exists instead of attaching the same file twice. The rule is the
    /// same one `property_has_photo` uses for the chunked protocol.
    pub async fn property_photo_id(
        &self,
        property_id: &str,
        filename: &str,
        byte_size: i64,
    ) -> DbResult<Option<String>> {
        sqlx::query_scalar::<_, String>(
            r#"
            select m.id::text
              from property_media pm
              join media m on m.id = pm.media_id
             where pm.property_id = $1::uuid
               and m.filename = $2
               and m.file_size = $3
               and m.derivative_of is null
             order by pm.created_at asc
             limit 1
            "#,
        )
        .bind(property_id)
        .bind(filename)
        .bind(byte_size)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.property_photo_id", &error))
    }

    /// The same publication rule the public listing uses. New uploads on a published property may
    /// go to the public bucket; everything else shadow-writes to the private bucket.
    pub async fn property_is_public(&self, property_id: &str) -> DbResult<bool> {
        let row = sqlx::query_as::<_, (bool,)>(
            "select exists (
                select 1 from property
                 where id = $1::uuid
                   and archived_at is null
                   and is_published
                   and slug is not null
                   and status in ('active', 'under_contract', 'sold')
            )",
        )
        .bind(property_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.property_is_public", &error))?;
        Ok(row.0)
    }

    /// Persist the outcome of a shadow object write. The legacy `file_data` blob is never touched here;
    /// the object is only an additional copy until a later, verified cutover.
    pub async fn set_media_object(
        &self,
        media_id: &str,
        metadata: &MediaObjectMetadata,
    ) -> DbResult<bool> {
        let result = sqlx::query(
            "update media
                set object_bucket = $2,
                    object_key = $3,
                    object_etag = $4,
                    object_sha256 = $5,
                    object_size = $6,
                    storage_status = $7,
                    visibility = coalesce($8, visibility),
                    object_migrated_at = case when $7 = 'VERIFIED' then now() else object_migrated_at end,
                    updated_at = now()
              where id = $1::uuid",
        )
        .bind(media_id)
        .bind(&metadata.bucket)
        .bind(&metadata.key)
        .bind(metadata.etag.as_deref())
        .bind(metadata.sha256.as_deref())
        .bind(metadata.size)
        .bind(&metadata.status)
        .bind(metadata.visibility.as_deref())
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.set_media_object", &error))?;
        Ok(result.rows_affected() > 0)
    }

    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Photographs (originals, image type) that have no copy of this kind yet — the backfill's work list.
    ///
    /// An original counts when its bytes are readable from EITHER place they can live: the legacy `file_data`
    /// blob or an OBJECT (`object_key`). The backfill used to require `file_data is not null`, which silently
    /// skipped every object-first original — exactly the rows that need a card copy once the blob is gone.
    pub async fn images_missing_copy(
        &self,
        kind: &str,
        limit: i64,
    ) -> DbResult<Vec<(String, Option<String>)>> {
        sqlx::query_as::<_, (String, Option<String>)>(
            r#"
            select m.id::text, m.filename
              from media m
             where m.media_type = 'image' and m.derivative_of is null
               and (m.file_data is not null or m.object_key is not null)
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

    /// The object pointer of the ORIGINAL row (never a derivative): what an image-copy backfill must read.
    ///
    /// `media_object_location` resolves the servable `web` copy, which is not what a backfill that derives a new
    /// copy wants; this one is the record copy itself.
    pub async fn original_object_location(
        &self,
        media_id: &str,
    ) -> DbResult<Option<MediaObjectLocation>> {
        let row = sqlx::query_as::<_, (Option<String>, Option<String>, String, String)>(
            "select object_bucket, object_key, storage_status, visibility
               from media
              where id = $1::uuid and derivative_of is null",
        )
        .bind(media_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.original_object_location", &error))?;
        Ok(row.and_then(|(bucket, key, status, visibility)| {
            Some(MediaObjectLocation {
                bucket: bucket?,
                key: key?,
                status,
                visibility,
            })
        }))
    }

    /// The object-first mirror of `insert_copy`: record one derivative's object metadata, no bytes.
    ///
    /// The caller has already uploaded and verified the object; this is the metadata half, so a backfill can add
    /// a `card` copy without writing `file_data`.
    pub async fn insert_copy_object(
        &self,
        original_id: &str,
        kind: &str,
        filename: Option<&str>,
        object: &MediaObjectMetadata,
    ) -> DbResult<String> {
        sqlx::query_scalar::<_, String>(
            r#"
            insert into media (
                filename, mime_type, file_size, media_type, derivative_of, derivative_kind,
                object_bucket, object_key, object_etag, object_sha256, object_size,
                object_migrated_at, storage_status, visibility
            )
            values (
                $1, 'image/jpeg', $2, 'image', $3::uuid, $4,
                $5, $6, $7, $8, $9, now(), 'VERIFIED', $10
            )
            returning id::text
            "#,
        )
        .bind(filename)
        .bind(object.size)
        .bind(original_id)
        .bind(kind)
        .bind(&object.bucket)
        .bind(&object.key)
        .bind(object.etag.as_deref())
        .bind(object.sha256.as_deref())
        .bind(object.size)
        .bind(object.visibility.as_deref())
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.insert_copy_object", &error))
    }

    /// LEGACY BLOB READ. Retained only for `MEDIA_OBJECT_STORAGE_READS`-off reads; removed with the
    /// `media.file_data` drop. Every object-first reader goes through `media_object_location`.
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

    /// LEGACY BLOB WRITE. The standalone media door (signed-contract PDFs) still stores bytes in
    /// `media.file_data`; its object-first replacement is `upload_standalone_object`. It is the remaining
    /// writer in this module blocking the blob-column drop.
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
}
