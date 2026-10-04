use crate::{Database, DbFailure, DbResult, DbTransaction};
use chrono::{DateTime, Utc};
use model::{
    DocumentSignConfig, DocumentSignEnvelopeSummary, DocumentSignRecipient, DocumentSigningMode,
    PutSignatureFieldRequest, SignatureField, SignatureFieldType, SignatureRecipientRole,
};
use serde_json::Value;
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct ConfigRow {
    signature_request_id: String,
    subject: Option<String>,
    signing_mode: String,
    expires_at: Option<DateTime<Utc>>,
    issued_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct EnvelopeSummaryRow {
    signature_request_id: String,
    transaction_document_id: String,
    subject: Option<String>,
    signing_mode: String,
    status: String,
    issued_at: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
    client_name: Option<String>,
    recipient_total: i64,
    completed_total: i64,
}

#[derive(Debug, FromRow)]
struct RecipientRow {
    id: String,
    signature_request_id: String,
    recipient_role: String,
    recipient_name: String,
    recipient_email: String,
    signer_order: i32,
    signing_step: i32,
    execution_role: Option<String>,
    execution_slot_id: Option<String>,
    state: Option<String>,
}

#[derive(Debug, FromRow)]
struct FieldRow {
    id: String,
    signature_request_id: String,
    recipient_id: String,
    field_key: String,
    field_type: String,
    page_number: i32,
    position_x: f64,
    position_y: f64,
    width: f64,
    height: f64,
    required: bool,
    label: Option<String>,
    configuration: Value,
    created_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct DocumentSignDao {
    db: Database,
}

impl DocumentSignDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn database(&self) -> Database {
        self.db.clone()
    }

    pub async fn config(&self, signature_request_id: &str) -> DbResult<Option<DocumentSignConfig>> {
        let row = sqlx::query_as::<_, ConfigRow>(
            r#"
            select signature_request_id::text as signature_request_id,
                   subject, signing_mode, expires_at, issued_at,
                   created_at, updated_at
              from document_sign_request
             where signature_request_id = $1::uuid
             limit 1
            "#,
        )
        .bind(signature_request_id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("document_sign.config", &error))?;
        row.map(map_config).transpose()
    }

    pub async fn recipients(
        &self,
        signature_request_id: &str,
    ) -> DbResult<Vec<DocumentSignRecipient>> {
        let rows = sqlx::query_as::<_, RecipientRow>(
            r#"
            select r.id::text as id,
                   r.signature_request_id::text as signature_request_id,
                   r.recipient_role, r.recipient_name, r.recipient_email,
                   r.signer_order, r.signing_step, r.execution_role, r.execution_slot_id,
                   s.state as state
              from signature_envelope_recipient r
              left join signature_recipient_state s on s.recipient_id = r.id
             where signature_request_id = $1::uuid
             order by signer_order, id
            "#,
        )
        .bind(signature_request_id)
        .fetch_all(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("document_sign.recipients", &error))?;
        rows.into_iter().map(map_recipient).collect()
    }

    pub async fn recipients_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Vec<DocumentSignRecipient>> {
        let rows = sqlx::query_as::<_, RecipientRow>(
            r#"
            select r.id::text as id,
                   r.signature_request_id::text as signature_request_id,
                   r.recipient_role, r.recipient_name, r.recipient_email,
                   r.signer_order, r.signing_step, r.execution_role, r.execution_slot_id,
                   s.state as state
              from signature_envelope_recipient r
              left join signature_recipient_state s on s.recipient_id = r.id
             where signature_request_id = $1::uuid
             order by signer_order, id
            "#,
        )
        .bind(signature_request_id)
        .fetch_all(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("document_sign.recipients_tx", &error))?;
        rows.into_iter().map(map_recipient).collect()
    }

    pub async fn fields(&self, signature_request_id: &str) -> DbResult<Vec<SignatureField>> {
        let rows = sqlx::query_as::<_, FieldRow>(
            r#"
            select id::text as id,
                   signature_request_id::text as signature_request_id,
                   recipient_id::text as recipient_id,
                   field_key, field_type, page_number,
                   position_x::float8 as position_x,
                   position_y::float8 as position_y,
                   width::float8 as width,
                   height::float8 as height,
                   required, label, configuration, created_at
              from signature_field
             where signature_request_id = $1::uuid
             order by page_number, position_y, position_x, id
            "#,
        )
        .bind(signature_request_id)
        .fetch_all(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("document_sign.fields", &error))?;
        rows.into_iter().map(map_field).collect()
    }

    pub async fn fields_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Vec<SignatureField>> {
        let rows = sqlx::query_as::<_, FieldRow>(
            r#"
            select id::text as id,
                   signature_request_id::text as signature_request_id,
                   recipient_id::text as recipient_id,
                   field_key, field_type, page_number,
                   position_x::float8 as position_x,
                   position_y::float8 as position_y,
                   width::float8 as width,
                   height::float8 as height,
                   required, label, configuration, created_at
              from signature_field
             where signature_request_id = $1::uuid
             order by page_number, position_y, position_x, id
            "#,
        )
        .bind(signature_request_id)
        .fetch_all(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("document_sign.fields_tx", &error))?;
        rows.into_iter().map(map_field).collect()
    }

    pub async fn create_config_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        subject: Option<&str>,
        signing_mode: DocumentSigningMode,
        expires_at: Option<DateTime<Utc>>,
    ) -> DbResult<DocumentSignConfig> {
        let row = sqlx::query_as::<_, ConfigRow>(
            r#"
            insert into document_sign_request (
                signature_request_id, subject, signing_mode, expires_at
            )
            values ($1::uuid, $2, $3, $4)
            on conflict (signature_request_id) do nothing
            returning signature_request_id::text as signature_request_id,
                      subject, signing_mode, expires_at, issued_at,
                      created_at, updated_at
            "#,
        )
        .bind(signature_request_id)
        .bind(subject)
        .bind(signing_mode.as_str())
        .bind(expires_at)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("document_sign.create", &error))?;

        if let Some(row) = row {
            return map_config(row);
        }

        let row = sqlx::query_as::<_, ConfigRow>(
            r#"
            select signature_request_id::text as signature_request_id,
                   subject, signing_mode, expires_at, issued_at,
                   created_at, updated_at
              from document_sign_request
             where signature_request_id = $1::uuid
             limit 1
            "#,
        )
        .bind(signature_request_id)
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("document_sign.create.existing", &error))?;
        map_config(row)
    }

    pub async fn put_field_tx(
        &self,
        tx: &mut DbTransaction,
        request: &PutSignatureFieldRequest,
    ) -> DbResult<Option<SignatureField>> {
        let row = if let Some(field_id) = request.field_id.as_deref() {
            sqlx::query_as::<_, FieldRow>(
                r#"
                update signature_field
                   set recipient_id = $3::uuid,
                       field_key = $4,
                       field_type = $5,
                       page_number = $6,
                       position_x = $7,
                       position_y = $8,
                       width = $9,
                       height = $10,
                       required = $11,
                       label = $12,
                       configuration = $13
                 where id = $2::uuid
                   and signature_request_id = $1::uuid
                returning id::text as id,
                          signature_request_id::text as signature_request_id,
                          recipient_id::text as recipient_id,
                          field_key, field_type, page_number,
                          position_x::float8 as position_x,
                          position_y::float8 as position_y,
                          width::float8 as width,
                          height::float8 as height,
                          required, label, configuration, created_at
                "#,
            )
            .bind(&request.signature_request_id)
            .bind(field_id)
            .bind(&request.recipient_id)
            .bind(request.field_key.trim())
            .bind(request.field_type.as_str())
            .bind(request.page_number)
            .bind(request.position_x)
            .bind(request.position_y)
            .bind(request.width)
            .bind(request.height)
            .bind(request.required)
            .bind(request.label.as_deref())
            .bind(&request.configuration)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("document_sign.field.update", &error))?
        } else {
            sqlx::query_as::<_, FieldRow>(
                r#"
                insert into signature_field (
                    signature_request_id, recipient_id, field_key, field_type,
                    page_number, position_x, position_y, width, height,
                    required, label, configuration
                )
                values (
                    $1::uuid, $2::uuid, $3, $4, $5,
                    $6, $7, $8, $9, $10, $11, $12
                )
                returning id::text as id,
                          signature_request_id::text as signature_request_id,
                          recipient_id::text as recipient_id,
                          field_key, field_type, page_number,
                          position_x::float8 as position_x,
                          position_y::float8 as position_y,
                          width::float8 as width,
                          height::float8 as height,
                          required, label, configuration, created_at
                "#,
            )
            .bind(&request.signature_request_id)
            .bind(&request.recipient_id)
            .bind(request.field_key.trim())
            .bind(request.field_type.as_str())
            .bind(request.page_number)
            .bind(request.position_x)
            .bind(request.position_y)
            .bind(request.width)
            .bind(request.height)
            .bind(request.required)
            .bind(request.label.as_deref())
            .bind(&request.configuration)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("document_sign.field.insert", &error))?
        };

        row.map(map_field).transpose()
    }

    pub async fn remove_field_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        field_id: &str,
    ) -> DbResult<bool> {
        let result = sqlx::query(
            r#"
            delete from signature_field
             where id = $2::uuid
               and signature_request_id = $1::uuid
            "#,
        )
        .bind(signature_request_id)
        .bind(field_id)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("document_sign.field.delete", &error))?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn lock_config_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<DocumentSignConfig>> {
        let row = sqlx::query_as::<_, ConfigRow>(
            r#"
            select signature_request_id::text as signature_request_id,
                   subject, signing_mode, expires_at, issued_at,
                   created_at, updated_at
              from document_sign_request
             where signature_request_id = $1::uuid
             for update
            "#,
        )
        .bind(signature_request_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("document_sign.lock", &error))?;
        row.map(map_config).transpose()
    }

    pub async fn mark_issued_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        expires_at: DateTime<Utc>,
    ) -> DbResult<bool> {
        let result = sqlx::query(
            r#"
            update document_sign_request
               set expires_at = $2,
                   issued_at = now()
             where signature_request_id = $1::uuid
               and issued_at is null
            "#,
        )
        .bind(signature_request_id)
        .bind(expires_at)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("document_sign.issue", &error))?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn required_field_gaps_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Vec<String>> {
        sqlx::query_scalar::<_, String>(
            r#"
            select r.id::text
              from signature_envelope_recipient r
             where r.signature_request_id = $1::uuid
               and r.recipient_role = 'signer'
               and not exists (
                    select 1
                      from signature_field f
                     where f.signature_request_id = r.signature_request_id
                       and f.recipient_id = r.id
                       and f.required
               )
             order by r.signer_order, r.id
            "#,
        )
        .bind(signature_request_id)
        .fetch_all(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("document_sign.required_fields", &error))
    }

    /// Store a derived audit document and link it to the transaction
    /// document. The completion certificate PDF and any future signed-PDF
    /// renderer both land here; the bytes decide the mime type.
    pub async fn store_audit_artifact_tx(
        &self,
        tx: &mut DbTransaction,
        transaction_document_id: &str,
        filename: &str,
        mime_type: &str,
        bytes: &[u8],
    ) -> DbResult<String> {
        let media_id: String = sqlx::query_scalar::<_, String>(
            r#"
            insert into media (file_data, filename, mime_type, file_size, media_type)
            values ($1, $2, $3, $4, 'document')
            returning id::text
            "#,
        )
        .bind(bytes)
        .bind(filename)
        .bind(mime_type)
        .bind(bytes.len() as i64)
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("document_sign.audit_media", &error))?;
        sqlx::query(
            r#"
            update transaction_document
               set signed_audit_media_id = $2::uuid,
                   updated_at = now()
             where id = $1::uuid
            "#,
        )
        .bind(transaction_document_id)
        .bind(&media_id)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("document_sign.audit_link", &error))?;
        Ok(media_id)
    }

    /// The linked audit artifact, if a previous finalize already stored one.
    /// Replay answers with the existing row instead of storing a duplicate.
    pub async fn audit_media_for_request_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<String>> {
        sqlx::query_scalar::<_, Option<String>>(
            r#"
            select td.signed_audit_media_id::text
              from transaction_document td
              join signature_request sr on sr.transaction_document_id = td.id
             where sr.id = $1::uuid
             limit 1
            "#,
        )
        .bind(signature_request_id)
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("document_sign.audit_media_read", &error))
    }

    /// Envelopes past their clock with an open canonical status. The
    /// sweep transitions each to Expired after expiring its recipients.
    pub async fn overdue_envelopes_tx(
        &self,
        tx: &mut DbTransaction,
    ) -> DbResult<Vec<String>> {
        sqlx::query_scalar::<_, String>(
            r#"
            select ds.signature_request_id::text
              from document_sign_request ds
              join signature_request sr on sr.id = ds.signature_request_id
             where ds.expires_at is not null
               and ds.expires_at <= now()
               and sr.status in ('sent', 'viewed', 'signed')
             order by ds.expires_at
            "#,
        )
        .fetch_all(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("document_sign.overdue", &error))
    }

    /// The template anchor blocks recorded on the issued document's
    /// Vault snapshot, if the issuing template declared any. Raw JSON —
    /// parsing and validation belong to the service, not the row read.
    pub async fn template_anchors_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<serde_json::Value>> {
        sqlx::query_scalar::<_, Option<serde_json::Value>>(
            r#"
            select d.source_snapshot -> 'signatureAnchors'
              from transaction_document d
              join signature_request sr on sr.transaction_document_id = d.id
             where sr.id = $1::uuid
             limit 1
            "#,
        )
        .bind(signature_request_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("document_sign.template_anchors", &error))
        .map(|row| row.flatten())
    }

    pub async fn canonical_status_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<(String, String)>> {
        sqlx::query_as::<_, (String, String)>(
            "select status, transaction_document_id::text \
             from signature_request where id = $1::uuid limit 1",
        )
        .bind(signature_request_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("document_sign.canonical_status", &error))
    }

    /// Recent native envelopes with recipient progress, newest first. The
    /// ops desk list; detail still comes from `config` + `recipients`.
    pub async fn list_envelopes(&self, limit: i64) -> DbResult<Vec<DocumentSignEnvelopeSummary>> {
        let rows = sqlx::query_as::<_, EnvelopeSummaryRow>(
            r#"
            select ds.signature_request_id::text as signature_request_id,
                   sr.transaction_document_id::text as transaction_document_id,
                   ds.subject as subject,
                   ds.signing_mode as signing_mode,
                   sr.status as status,
                   ds.issued_at as issued_at,
                   ds.expires_at as expires_at,
                   (select r2.recipient_name from signature_envelope_recipient r2
                     where r2.signature_request_id = ds.signature_request_id
                     order by r2.signer_order, r2.id limit 1) as client_name,
                   count(r.id)::bigint as recipient_total,
                   count(case when s.state = 'completed' then 1 end)::bigint as completed_total
              from document_sign_request ds
              join signature_request sr on sr.id = ds.signature_request_id
              left join signature_envelope_recipient r on r.signature_request_id = ds.signature_request_id
              left join signature_recipient_state s on s.recipient_id = r.id
             group by ds.signature_request_id, sr.transaction_document_id, ds.subject,
                      ds.signing_mode, sr.status, ds.issued_at, ds.expires_at, sr.updated_at
             order by sr.updated_at desc
             limit $1
            "#,
        )
        .bind(limit.clamp(1, 100))
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("document_sign.list", &error))?;
        rows.into_iter()
            .map(|row| {
                Ok(DocumentSignEnvelopeSummary {
                    signature_request_id: row.signature_request_id,
                    transaction_document_id: row.transaction_document_id,
                    subject: row.subject,
                    signing_mode: row.signing_mode,
                    status: row.status,
                    issued_at: row.issued_at.map(|value| value.to_rfc3339()),
                    expires_at: row.expires_at.map(|value| value.to_rfc3339()),
                    client_name: row.client_name,
                    recipient_total: row.recipient_total,
                    completed_total: row.completed_total,
                })
            })
            .collect()
    }
}

fn map_config(row: ConfigRow) -> DbResult<DocumentSignConfig> {
    Ok(DocumentSignConfig {
        signature_request_id: row.signature_request_id,
        subject: row.subject,
        signing_mode: DocumentSigningMode::try_from(row.signing_mode.as_str())
            .map_err(|error| DbFailure::schema_mismatch("document_sign.map_config", error))?,
        expires_at: row.expires_at.map(|value| value.to_rfc3339()),
        issued_at: row.issued_at.map(|value| value.to_rfc3339()),
        created_at: row.created_at.to_rfc3339(),
        updated_at: row.updated_at.to_rfc3339(),
    })
}

fn map_recipient(row: RecipientRow) -> DbResult<DocumentSignRecipient> {
    let role = match row.recipient_role.as_str() {
        "signer" => SignatureRecipientRole::Signer,
        "approver" => SignatureRecipientRole::Approver,
        other => {
            return Err(DbFailure::schema_mismatch(
                "document_sign.map_recipient",
                format!("unknown recipient role: {other}"),
            ))
        }
    };
    Ok(DocumentSignRecipient {
        id: row.id,
        signature_request_id: row.signature_request_id,
        role,
        name: row.recipient_name,
        email: row.recipient_email,
        signer_order: row.signer_order,
        signing_step: row.signing_step,
        execution_role: row.execution_role,
        execution_slot_id: row.execution_slot_id,
        state: row.state,
    })
}

fn map_field(row: FieldRow) -> DbResult<SignatureField> {
    Ok(SignatureField {
        id: row.id,
        signature_request_id: row.signature_request_id,
        recipient_id: row.recipient_id,
        field_key: row.field_key,
        field_type: SignatureFieldType::try_from(row.field_type.as_str())
            .map_err(|error| DbFailure::schema_mismatch("document_sign.map_field", error))?,
        page_number: row.page_number,
        position_x: row.position_x,
        position_y: row.position_y,
        width: row.width,
        height: row.height,
        required: row.required,
        label: row.label,
        configuration: row.configuration,
        created_at: row.created_at.to_rfc3339(),
    })
}
