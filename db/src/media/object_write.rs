//! The property media link: hero promotion and the object-first upload writer.
//!
//! HISTORY: this module was `assemble_media_upload.rs` and held the chunk assembly/commit and the blob property
//! upload. All three are retired with the staged protocol and the blob writer — the file was renamed because nothing
//! here assembles anything any more. `save_property_media_object_set` is the ONLY writer that creates media rows and
//! the property link for an upload now, and it writes metadata only (no `file_data`).

#[allow(unused_imports)]
use super::*;

impl MediaDao {
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

    /// Object-first property image: insert the row and attach it. Lives here, with the assembler,
    /// because the hero demotion/promotion is this file's single-owner responsibility.
    pub async fn save_property_media_object(
        &self,
        property_id: &str,
        role: &str,
        row: &MediaObjectRow,
    ) -> DbResult<UploadPropertyMediaResult> {
        let mut tx = self.db.begin("media.save_property_media_object").await?;
        MediaDao::insert_object_row(&mut tx, row).await?;
        if role == "hero" {
            sqlx::query(
                "update property_media set role = 'gallery' where property_id = $1::uuid and role = 'hero'",
            )
            .bind(property_id)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("media.object.demote_hero", &error))?;
        }
        sqlx::query(
            "insert into property_media (property_id, media_id, role, sort_order) values ($1::uuid, $2::uuid, $3, 0)",
        )
        .bind(property_id)
        .bind(&row.id)
        .bind(role)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.object.attach_property", &error))?;
        tx.commit().await?;
        Ok(UploadPropertyMediaResult {
            ok: true,
            media_id: row.id.clone(),
            property_id: property_id.to_owned(),
            role: role.to_owned(),
        })
    }

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

    /// Object-first upload: every row the file produced, and the property link, in ONE transaction.
    ///
    /// This is the only row-creating writer for uploaded property media now (whole-file route and direct-to-object
    /// finalize both land here). The objects are already uploaded and verified by the service before this is called,
    /// so the rows are metadata only and `file_data` stays NULL. There is no staging table to write, which is why
    /// `media_upload` / `media_upload_chunk` could be dropped.
    ///
    /// `rows[0]` is the original; every other row is a derivative of it. A failure anywhere rolls the
    /// whole set back, and the caller deletes the objects it uploaded (the DB cannot roll those back).
    pub async fn save_property_media_object_set(
        &self,
        property_id: &str,
        role: &str,
        rows: &[MediaObjectRow],
    ) -> DbResult<UploadPropertyMediaResult> {
        let Some(original) = rows.first() else {
            return Err(DbFailure::configuration(
                "media.save_property_media_object_set",
                "at least the original row is required",
            ));
        };
        let mut tx = self
            .db
            .begin("media.save_property_media_object_set")
            .await?;
        for row in rows {
            MediaDao::insert_object_row(&mut tx, row).await?;
        }
        if role == "hero" {
            sqlx::query(
                "update property_media set role = 'gallery' where property_id = $1::uuid and role = 'hero'",
            )
            .bind(property_id)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("media.object_set.demote_hero", &error))?;
        }
        sqlx::query(
            "insert into property_media (property_id, media_id, role, sort_order) values ($1::uuid, $2::uuid, $3, 0)",
        )
        .bind(property_id)
        .bind(&original.id)
        .bind(role)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("media.object_set.attach_property", &error))?;
        tx.commit().await?;
        Ok(UploadPropertyMediaResult {
            ok: true,
            media_id: original.id.clone(),
            property_id: property_id.to_owned(),
            role: role.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DbTarget;

    fn object_row(
        id: &str,
        filename: &str,
        derivative_of: Option<&str>,
        derivative_kind: Option<&str>,
    ) -> MediaObjectRow {
        MediaObjectRow {
            id: id.to_owned(),
            filename: filename.to_owned(),
            mime_type: "image/jpeg".to_owned(),
            alt_text: None,
            media_type: "image".to_owned(),
            derivative_of: derivative_of.map(str::to_owned),
            derivative_kind: derivative_kind.map(str::to_owned),
            object: MediaObjectMetadata {
                bucket: "property_media_private".to_owned(),
                key: format!("media/{id}/original.jpg"),
                etag: Some("etag".to_owned()),
                sha256: Some("a".repeat(64)),
                size: 1_234,
                status: "VERIFIED".to_owned(),
                visibility: Some("private".to_owned()),
            },
        }
    }

    async fn open_dev() -> Database {
        Database::connect_target(DbTarget::Dev)
            .await
            .expect("DATABASE_URL_DEV must reach the disposable DEV branch")
    }

    /// The object-first writer is the only row-creating path left after the staged upload was retired. This proves on
    /// a real database that it records the photograph, its copies and the property link, and that the object adapters
    /// a blob backfill needs (`original_object_location`, `insert_copy_object`) work on the same rows.
    ///
    /// Ignored by default; run with DATABASE_URL_DEV present. It cleans up every row it creates.
    #[tokio::test]
    #[ignore = "needs DATABASE_URL_DEV; writes and removes its own rows on the DEV branch"]
    async fn the_object_set_writer_writes_the_rows_and_the_link() {
        let database = open_dev().await;
        let dao = MediaDao::new(database.clone());
        let property_id = sqlx::query_scalar::<_, String>(
            "select id::text from property where archived_at is null order by id limit 1",
        )
        .fetch_one(database.pool())
        .await
        .expect("DEV has at least one property");

        let original_id = uuid::Uuid::new_v4().to_string();
        let derivative_id = uuid::Uuid::new_v4().to_string();
        let rows = vec![
            object_row(&original_id, "whole-file.jpg", None, None),
            object_row(
                &derivative_id,
                "whole-file-web.jpg",
                Some(&original_id),
                Some("web"),
            ),
        ];

        let outcome = dao
            .save_property_media_object_set(&property_id, "gallery", &rows)
            .await;

        // The backfill work list must FIND an original whose bytes live only in object storage: this row has no
        // `file_data` and no card copy yet, so `images_missing_copy` is exactly the query that used to skip it.
        let missing_card = dao.images_missing_copy("card", 500).await;

        // The object-first adapters a blob backfill needs: the ORIGINAL's pointer, and a copy recorded from
        // metadata alone (no `file_data`).
        let original_location = dao.original_object_location(&original_id).await;
        let card_id = dao
            .insert_copy_object(
                &original_id,
                "card",
                Some("whole-file-card.jpg"),
                &MediaObjectMetadata {
                    bucket: "property_media_private".to_owned(),
                    key: format!("media/{original_id}/card.jpg"),
                    etag: Some("etag".to_owned()),
                    sha256: Some("b".repeat(64)),
                    size: 900,
                    status: "VERIFIED".to_owned(),
                    visibility: Some("private".to_owned()),
                },
            )
            .await;

        let media_rows = sqlx::query_scalar::<_, i64>(
            "select count(*) from media where id = $1::uuid or id = $2::uuid or id = $3::uuid",
        )
        .bind(&original_id)
        .bind(&derivative_id)
        .bind(card_id.as_deref().unwrap_or(""))
        .fetch_one(database.pool())
        .await
        .expect("the media rows are readable");
        let linked = sqlx::query_scalar::<_, bool>(
            "select exists (select 1 from property_media where property_id = $1::uuid and media_id = $2::uuid)",
        )
        .bind(&property_id)
        .bind(&original_id)
        .fetch_one(database.pool())
        .await
        .expect("the property link is readable");
        // Clean up before asserting: a failed assertion must not leave rows in the DEV branch.
        let _ = sqlx::query("delete from property_media where media_id = $1::uuid")
            .bind(&original_id)
            .execute(database.pool())
            .await;
        let _ =
            sqlx::query("delete from media where id = $1::uuid or id = $2::uuid or id = $3::uuid")
                .bind(&original_id)
                .bind(&derivative_id)
                .bind(card_id.as_deref().unwrap_or(""))
                .execute(database.pool())
                .await;

        let stored = outcome.expect("the object set is written and attached");
        assert_eq!(stored.media_id, original_id);
        assert_eq!(stored.property_id, property_id);
        assert_eq!(
            media_rows, 3,
            "the original, its copy, and the metadata-only card copy are recorded"
        );
        let missing_card = missing_card.expect("the backfill work list is readable");
        assert!(
            missing_card.iter().any(|(id, _)| id == &original_id),
            "an object-pointer original without a card copy must be on the backfill work list; got {:?}",
            missing_card.iter().map(|(id, _)| id).collect::<Vec<_>>()
        );
        let location = original_location
            .expect("the original pointer is readable")
            .expect("the original has an object pointer");
        assert_eq!(location.key, format!("media/{original_id}/original.jpg"));
        assert_eq!(location.status, "VERIFIED");
        let card_id = card_id.expect("the metadata-only copy is recorded");
        assert!(!card_id.is_empty());
        assert!(
            linked,
            "the property link is written in the same transaction"
        );
    }
}
