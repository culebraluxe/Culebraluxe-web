//! Shared object-first insert for PDFs/documents.
//!
//! The caller uploads and verifies the object first, then supplies its metadata. No bytes cross this
//! boundary, so the eventual `media.file_data` drop does not require another writer refactor.

use crate::{DbFailure, DbResult, DbTransaction, MediaObjectMetadata};
use sqlx::PgConnection;

async fn insert(
    executor: &mut PgConnection,
    id: &str,
    filename: &str,
    mime_type: &str,
    media_type: &str,
    object: &MediaObjectMetadata,
) -> DbResult<()> {
    sqlx::query(
        r#"
        insert into media (
            id, filename, mime_type, file_size, media_type,
            object_bucket, object_key, object_etag, object_sha256, object_size,
            object_migrated_at, storage_status, visibility
        )
        values ($1::uuid, $2, $3, $4, $5, $6, $7, $8, $9, $10, now(), 'VERIFIED', $11)
        "#,
    )
    .bind(id)
    .bind(filename)
    .bind(mime_type)
    .bind(object.size)
    .bind(media_type)
    .bind(&object.bucket)
    .bind(&object.key)
    .bind(object.etag.as_deref())
    .bind(object.sha256.as_deref())
    .bind(object.size)
    .bind(object.visibility.as_deref())
    .execute(executor)
    .await
    .map_err(|error| DbFailure::from_sqlx("media.insert_object_media", &error))?;
    Ok(())
}

/// Insert a document object row inside an existing transaction. Returns the new media id.
pub async fn insert_media_object_tx(
    tx: &mut DbTransaction,
    filename: &str,
    mime_type: &str,
    object: &MediaObjectMetadata,
) -> DbResult<String> {
    let id = uuid::Uuid::new_v4().to_string();
    insert(
        tx.connection(),
        &id,
        filename,
        mime_type,
        "document",
        object,
    )
    .await?;
    Ok(id)
}

/// Insert a document object row on a raw connection (completion/reconciler paths).
pub async fn insert_media_object_conn(
    connection: &mut PgConnection,
    filename: &str,
    mime_type: &str,
    object: &MediaObjectMetadata,
) -> DbResult<String> {
    let id = uuid::Uuid::new_v4().to_string();
    insert(connection, &id, filename, mime_type, "document", object).await?;
    Ok(id)
}
