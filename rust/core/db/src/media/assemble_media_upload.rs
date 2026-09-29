//! Moved from `media.rs` (move only): assemble_media_upload.

#[allow(unused_imports)]
use super::*;

impl MediaDao {
    /// Reads and checks the staged chunks, and returns the assembled photograph — WITHOUT writing anything.
    ///
    /// Split from the write deliberately. Decoding a 13 MB photograph and building its derivatives takes real time,
    /// and a database transaction held open across that work is a transaction every other upload waits behind. Check
    /// here; write in `commit_media_upload`.
    ///
    /// EVERY DECLARED CHUNK MUST BE PRESENT AND THE TOTAL MUST MATCH. That is what the manifest is for: a missing
    /// chunk would otherwise be stored as a shorter photograph, and a partial upload would look complete to every
    /// reader downstream.
    pub async fn assemble_media_upload(&self, upload_id: &str) -> DbResult<MediaUploadAssembly> {
        let status = self.media_upload_status(upload_id).await?;
        // `complete` is the claim `claim_media_upload` took: exactly one finisher assembles an upload.
        if status.status != "complete" {
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

        Ok(MediaUploadAssembly {
            upload_id: upload_id.to_string(),
            property_id,
            filename,
            mime_type,
            role,
            alt_text,
            bytes: assembled,
        })
    }

    /// Writes the photograph and its derivatives, links it, and drops the staged bytes — in ONE transaction.
    ///
    /// The copies are written here rather than by their own call so that a photograph can never exist without the copy
    /// that makes it servable: a reader either sees the photo and its copies, or sees nothing.
    pub async fn commit_media_upload(
        &self,
        upload_id: &str,
        assembly: &MediaUploadAssembly,
        derivatives: &[MediaDerivativeInput],
    ) -> DbResult<UploadPropertyMediaResult> {
        let property_id = assembly.property_id.clone();
        let filename = assembly.filename.clone();
        let mime_type = assembly.mime_type.clone();
        let alt_text = assembly.alt_text.clone();
        let role = assembly.role.clone();
        let assembled = &assembly.bytes;

        let mut tx = self.db.begin("media.upload.commit").await?;

        let media_id = sqlx::query_scalar::<_, String>(
            r#"
            insert into media (
                file_data, filename, mime_type, file_size, alt_text, media_type
            )
            values ($1, $2, $3, $4, $5, 'image')
            returning id::text
            "#,
        )
        .bind(assembled)
        .bind(&filename)
        .bind(&mime_type)
        .bind(assembled.len() as i64)
        .bind(alt_text.as_deref())
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.upload.complete_insert", &error))?;

        for derivative in derivatives {
            sqlx::query(
                r#"
                insert into media (
                    file_data, filename, mime_type, file_size, media_type, derivative_of, derivative_kind
                )
                values ($1, $2, 'image/jpeg', $3, 'image', $4::uuid, $5)
                "#,
            )
            .bind(&derivative.bytes)
            .bind(derivative_filename(&filename, derivative.kind))
            .bind(derivative.bytes.len() as i64)
            .bind(&media_id)
            .bind(derivative.kind)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("media.upload.commit_derivative", &error))?;
        }

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
    /// Make one of a property's photographs its hero: the previous hero goes back to the gallery, in one transaction.
    /// `false` when the photograph is not this property's.
    pub async fn set_property_hero(&self, property_id: &str, media_id: &str) -> DbResult<bool> {
        let mut tx = self.db.begin("media.set_hero").await?;
        let linked = sqlx::query_scalar::<_, i64>(
            "select count(*)::bigint from property_media where property_id = $1::uuid and media_id = $2::uuid",
        )
        .bind(property_id)
        .bind(media_id)
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.set_hero.check", &error))?;
        if linked == 0 {
            return Ok(false);
        }
        sqlx::query("update property_media set role = 'gallery' where property_id = $1::uuid and role = 'hero'")
            .bind(property_id)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("media.set_hero.demote", &error))?;
        sqlx::query("update property_media set role = 'hero' where property_id = $1::uuid and media_id = $2::uuid")
            .bind(property_id)
            .bind(media_id)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("media.set_hero.promote", &error))?;
        tx.commit().await?;
        Ok(true)
    }

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
