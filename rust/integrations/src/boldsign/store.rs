use db::{Database, DbFailure, DbResult};
use serde_json::Value;
use sqlx::FromRow;

#[derive(Debug, Clone)]
pub struct BoldSignDocumentPdf {
    pub file_data: Option<Vec<u8>>,
    pub filename: String,
    pub mime_type: String,
    pub source_snapshot: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct BoldSignRequest {
    pub signature_request_id: String,
    pub envelope_id: Option<String>,
    pub document_ids: Vec<String>,
    pub status: String,
    pub last_error: Option<String>,
    pub error_retryable: Option<bool>,
}

#[derive(Debug, FromRow)]
struct BoldSignDocumentPdfRow {
    file_data: Option<Vec<u8>>,
    filename: String,
    mime_type: String,
    source_snapshot: Option<Value>,
}

impl From<BoldSignDocumentPdfRow> for BoldSignDocumentPdf {
    fn from(row: BoldSignDocumentPdfRow) -> Self {
        Self {
            file_data: row.file_data,
            filename: row.filename,
            mime_type: row.mime_type,
            source_snapshot: row.source_snapshot,
        }
    }
}

#[derive(Debug, FromRow)]
struct BoldSignRequestRow {
    signature_request_id: String,
    envelope_id: Option<String>,
    document_ids: Value,
    status: String,
    last_error: Option<String>,
    error_retryable: Option<bool>,
}

impl From<BoldSignRequestRow> for BoldSignRequest {
    fn from(row: BoldSignRequestRow) -> Self {
        let document_ids = row
            .document_ids
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        Self {
            signature_request_id: row.signature_request_id,
            envelope_id: row.envelope_id,
            document_ids,
            status: row.status,
            last_error: row.last_error,
            error_retryable: row.error_retryable,
        }
    }
}

#[derive(Clone)]
pub struct BoldSignStore {
    db: Database,
}

impl BoldSignStore {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn database(&self) -> Database {
        self.db.clone()
    }

    pub async fn load_document_pdf(
        &self,
        transaction_document_id: &str,
    ) -> DbResult<Option<BoldSignDocumentPdf>> {
        sqlx::query_as::<_, BoldSignDocumentPdfRow>(
            r#"
            select m.file_data,
                   m.filename,
                   m.mime_type,
                   d.source_snapshot
            from transaction_document d
            join media m on m.id = d.media_id
            where d.id = $1::uuid
            limit 1
            "#,
        )
        .bind(transaction_document_id)
        .fetch_optional(self.db.pool())
        .await
        .map(|row| row.map(Into::into))
        .map_err(|error| DbFailure::from_sqlx("boldsign.store.load_pdf", &error))
    }

    pub async fn get_by_signature_request(
        &self,
        signature_request_id: &str,
    ) -> DbResult<Option<BoldSignRequest>> {
        sqlx::query_as::<_, BoldSignRequestRow>(
            r#"
            select signature_request_id::text as signature_request_id,
                   envelope_id,
                   document_ids,
                   status,
                   last_error,
                   error_retryable
            from bold_sign_request
            where signature_request_id = $1::uuid
            limit 1
            "#,
        )
        .bind(signature_request_id)
        .fetch_optional(self.db.pool())
        .await
        .map(|row| row.map(Into::into))
        .map_err(|error| DbFailure::from_sqlx("boldsign.store.get_by_request", &error))
    }

    pub async fn get_by_envelope(&self, envelope_id: &str) -> DbResult<Option<BoldSignRequest>> {
        sqlx::query_as::<_, BoldSignRequestRow>(
            r#"
            select signature_request_id::text as signature_request_id,
                   envelope_id,
                   document_ids,
                   status,
                   last_error,
                   error_retryable
            from bold_sign_request
            where envelope_id = $1
            limit 1
            "#,
        )
        .bind(envelope_id)
        .fetch_optional(self.db.pool())
        .await
        .map(|row| row.map(Into::into))
        .map_err(|error| DbFailure::from_sqlx("boldsign.store.get_by_envelope", &error))
    }

    pub async fn create(
        &self,
        signature_request_id: &str,
        envelope_id: &str,
        status: &str,
    ) -> DbResult<Option<BoldSignRequest>> {
        sqlx::query_as::<_, BoldSignRequestRow>(
            r#"
            insert into bold_sign_request (
                signature_request_id, envelope_id, document_ids, status
            )
            values ($1::uuid,$2,'[]'::jsonb,$3)
            on conflict (envelope_id) where envelope_id is not null do nothing
            returning signature_request_id::text as signature_request_id,
                      envelope_id,
                      document_ids,
                      status,
                      last_error,
                      error_retryable
            "#,
        )
        .bind(signature_request_id)
        .bind(envelope_id)
        .bind(status)
        .fetch_optional(self.db.pool())
        .await
        .map(|row| row.map(Into::into))
        .map_err(|error| DbFailure::from_sqlx("boldsign.store.create", &error))
    }

    pub async fn update_status(
        &self,
        signature_request_id: &str,
        status: &str,
        document_ids: &[String],
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            update bold_sign_request
            set status = $2,
                document_ids = $3,
                last_error = null,
                error_retryable = null,
                updated_at = now()
            where signature_request_id = $1::uuid
            "#,
        )
        .bind(signature_request_id)
        .bind(status)
        .bind(serde_json::json!(document_ids))
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("boldsign.store.update_status", &error))?;
        Ok(())
    }

    pub async fn record_error(
        &self,
        signature_request_id: &str,
        error_message: &str,
        retryable: bool,
        status: &str,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            insert into bold_sign_request (
                signature_request_id, envelope_id, document_ids, status,
                last_error, error_retryable
            )
            values ($1::uuid,null,'[]'::jsonb,$2,$3,$4)
            on conflict (signature_request_id)
            do update set status = excluded.status,
                          last_error = excluded.last_error,
                          error_retryable = excluded.error_retryable,
                          updated_at = now()
            "#,
        )
        .bind(signature_request_id)
        .bind(status)
        .bind(error_message)
        .bind(retryable)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("boldsign.store.record_error", &error))?;
        Ok(())
    }

    pub async fn record_webhook(
        &self,
        provider_event_id: &str,
        envelope_id: &str,
        signature_request_id: &str,
        provider_event_type: &str,
        neutral_event: &str,
        payload: &Value,
    ) -> DbResult<bool> {
        let inserted = sqlx::query_scalar::<_, String>(
            r#"
            insert into bold_sign_webhook_event (
                provider_event_id, envelope_id, signature_request_id,
                provider_event_type, neutral_event, payload
            )
            values ($1,$2,$3::uuid,$4,$5,$6)
            on conflict (provider_event_id) do nothing
            returning id::text
            "#,
        )
        .bind(provider_event_id)
        .bind(envelope_id)
        .bind(signature_request_id)
        .bind(provider_event_type)
        .bind(neutral_event)
        .bind(payload)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("boldsign.store.record_webhook", &error))?;
        Ok(inserted.is_some())
    }
}
