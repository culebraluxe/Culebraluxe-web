use crate::media::insert_media_object_with_id_tx;
use crate::{Database, DbFailure, DbResult, DbTransaction, MediaObjectMetadata};
use chrono::{DateTime, Utc};
use model::{
    LuxesignConfig, LuxesignEnvelopeSummary, LuxesignRecipient, LuxesignSigningMode,
    PutSignatureFieldRequest, SignatureField, SignatureFieldType, SignatureRecipientRole,
};
use serde_json::Value;
use sqlx::FromRow;

mod maps;
mod reads;
use self::maps::*;

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
pub struct LuxesignDao {
    db: Database,
}

impl LuxesignDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn database(&self) -> Database {
        self.db.clone()
    }

    pub async fn config(&self, signature_request_id: &str) -> DbResult<Option<LuxesignConfig>> {
        let row = sqlx::query_as::<_, ConfigRow>(
            r#"
            select signature_request_id::text as signature_request_id,
                   subject, signing_mode, expires_at, issued_at,
                   created_at, updated_at
              from luxesign_config
             where signature_request_id = $1::uuid
             limit 1
            "#,
        )
        .bind(signature_request_id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.config", &error))?;
        row.map(map_config).transpose()
    }

    pub async fn recipients(&self, signature_request_id: &str) -> DbResult<Vec<LuxesignRecipient>> {
        let rows = sqlx::query_as::<_, RecipientRow>(
            r#"
            select r.id::text as id,
                   r.signature_request_id::text as signature_request_id,
                   r.recipient_role, r.recipient_name, r.recipient_email,
                   r.signer_order, r.signing_step, r.execution_role, r.execution_slot_id,
                   s.state as state
              from luxesign_envelope_recipient r
              left join luxesign_recipient_state s on s.recipient_id = r.id
             where signature_request_id = $1::uuid
             order by signer_order, id
            "#,
        )
        .bind(signature_request_id)
        .fetch_all(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.recipients", &error))?;
        rows.into_iter().map(map_recipient).collect()
    }

    pub async fn recipients_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Vec<LuxesignRecipient>> {
        let rows = sqlx::query_as::<_, RecipientRow>(
            r#"
            select r.id::text as id,
                   r.signature_request_id::text as signature_request_id,
                   r.recipient_role, r.recipient_name, r.recipient_email,
                   r.signer_order, r.signing_step, r.execution_role, r.execution_slot_id,
                   s.state as state
              from luxesign_envelope_recipient r
              left join luxesign_recipient_state s on s.recipient_id = r.id
             where signature_request_id = $1::uuid
             order by signer_order, id
            "#,
        )
        .bind(signature_request_id)
        .fetch_all(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.recipients_tx", &error))?;
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
              from luxesign_field
             where signature_request_id = $1::uuid
             order by page_number, position_y, position_x, id
            "#,
        )
        .bind(signature_request_id)
        .fetch_all(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.fields", &error))?;
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
              from luxesign_field
             where signature_request_id = $1::uuid
             order by page_number, position_y, position_x, id
            "#,
        )
        .bind(signature_request_id)
        .fetch_all(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.fields_tx", &error))?;
        rows.into_iter().map(map_field).collect()
    }

    pub async fn create_config_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        subject: Option<&str>,
        signing_mode: LuxesignSigningMode,
        expires_at: Option<DateTime<Utc>>,
    ) -> DbResult<LuxesignConfig> {
        let row = sqlx::query_as::<_, ConfigRow>(
            r#"
            insert into luxesign_config (
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
        .map_err(|error| DbFailure::from_sqlx("luxesign.create", &error))?;

        if let Some(row) = row {
            return map_config(row);
        }

        let row = sqlx::query_as::<_, ConfigRow>(
            r#"
            select signature_request_id::text as signature_request_id,
                   subject, signing_mode, expires_at, issued_at,
                   created_at, updated_at
              from luxesign_config
             where signature_request_id = $1::uuid
             limit 1
            "#,
        )
        .bind(signature_request_id)
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.create.existing", &error))?;
        map_config(row)
    }

    /// Who is copied on the outcome, and how often a waiting signer is reminded (0 = never).
    pub async fn set_notice_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        copy_to_emails: &[String],
        reminder_every_days: i32,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            update luxesign_config
               set copy_to_emails = $2,
                   reminder_every_days = $3
             where signature_request_id = $1::uuid
            "#,
        )
        .bind(signature_request_id)
        .bind(copy_to_emails)
        .bind(reminder_every_days)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.notice.set", &error))?;
        Ok(())
    }

    pub async fn notice_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<(Vec<String>, i32)> {
        sqlx::query_as::<_, (Vec<String>, i32)>(
            r#"
            select copy_to_emails, reminder_every_days
              from luxesign_config
             where signature_request_id = $1::uuid
            "#,
        )
        .bind(signature_request_id)
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.notice.read", &error))
    }

    /// The email of the app user who prepared the envelope, when there is one and they are active.
    pub async fn operator_email_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<String>> {
        let row: Option<(Option<String>,)> = sqlx::query_as(
            r#"
            select u.email
              from luxesign_request sr
              join app_user u on u.id = sr.created_by_user_id and u.active
             where sr.id = $1::uuid
            "#,
        )
        .bind(signature_request_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.operator_email", &error))?;
        Ok(row
            .and_then(|(email,)| email)
            .map(|email| email.trim().to_owned())
            .filter(|email| !email.is_empty()))
    }

    /// The envelope's document title (what an email calls it).
    pub async fn document_title_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<String>> {
        let row: Option<(Option<String>,)> = sqlx::query_as(
            r#"
            select td.title
              from luxesign_request sr
              join transaction_document td on td.id = sr.transaction_document_id
             where sr.id = $1::uuid
            "#,
        )
        .bind(signature_request_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.document_title", &error))?;
        // The Vault keeps every issue as a version and titles it "Name vN"; the people signing see the name.
        Ok(row
            .and_then(|(title,)| title)
            .map(|title| without_vault_version(&title)))
    }

    /// Recipients who are due a reminder: it is their turn (the lowest step still open), they have not acted, the
    /// envelope is issued and live, reminders are on, fewer than `max_reminders` went out, and the last contact
    /// (reminder, else first notice, else issue) is at least the envelope's cadence ago. Reminders are counted from the
    /// durable email log by their dedupe key, so no counter is kept anywhere.
    pub async fn reminders_due_tx(
        &self,
        tx: &mut DbTransaction,
        max_reminders: i64,
    ) -> DbResult<Vec<(String, String)>> {
        sqlx::query_as::<_, (String, String)>(
            r#"
            select r.signature_request_id::text, r.id::text
              from luxesign_envelope_recipient r
              join luxesign_request sr
                on sr.id = r.signature_request_id
               and sr.status in ('requested', 'sent', 'viewed')
              join luxesign_config d
                on d.signature_request_id = sr.id
               and d.issued_at is not null
               and d.reminder_every_days > 0
               and (d.expires_at is null or d.expires_at > now())
              join luxesign_recipient_state s
                on s.recipient_id = r.id
               and s.state in ('pending', 'notified', 'viewed', 'in_progress')
              left join lateral (
                    select count(*) as sent, max(m.queued_at) as last_at
                      from email_message m
                     where m.dedupe_key like
                           'signature-reminder:' || r.signature_request_id::text || ':' || r.id::text || ':%'
              ) rem on true
             where r.signing_step = (
                       select min(r2.signing_step)
                         from luxesign_envelope_recipient r2
                         join luxesign_recipient_state s2 on s2.recipient_id = r2.id
                        where r2.signature_request_id = r.signature_request_id
                          and s2.state <> 'completed')
               and rem.sent < $1
               and coalesce(rem.last_at, s.notified_at, d.issued_at)
                   <= now() - make_interval(days => d.reminder_every_days)
             order by r.signature_request_id, r.signer_order
             limit 100
            "#,
        )
        .bind(max_reminders)
        .fetch_all(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.reminders_due", &error))
    }

    pub async fn put_field_tx(
        &self,
        tx: &mut DbTransaction,
        request: &PutSignatureFieldRequest,
    ) -> DbResult<Option<SignatureField>> {
        let row = if let Some(field_id) = request.field_id.as_deref() {
            sqlx::query_as::<_, FieldRow>(
                r#"
                update luxesign_field
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
            .map_err(|error| DbFailure::from_sqlx("luxesign.field.update", &error))?
        } else {
            sqlx::query_as::<_, FieldRow>(
                r#"
                insert into luxesign_field (
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
            .map_err(|error| DbFailure::from_sqlx("luxesign.field.insert", &error))?
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
            delete from luxesign_field
             where id = $2::uuid
               and signature_request_id = $1::uuid
            "#,
        )
        .bind(signature_request_id)
        .bind(field_id)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.field.delete", &error))?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn lock_config_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<LuxesignConfig>> {
        let row = sqlx::query_as::<_, ConfigRow>(
            r#"
            select signature_request_id::text as signature_request_id,
                   subject, signing_mode, expires_at, issued_at,
                   created_at, updated_at
              from luxesign_config
             where signature_request_id = $1::uuid
             for update
            "#,
        )
        .bind(signature_request_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.lock", &error))?;
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
            update luxesign_config
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
        .map_err(|error| DbFailure::from_sqlx("luxesign.issue", &error))?;
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
              from luxesign_envelope_recipient r
             where r.signature_request_id = $1::uuid
               and r.recipient_role = 'signer'
               and not exists (
                    select 1
                      from luxesign_field f
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
        .map_err(|error| DbFailure::from_sqlx("luxesign.required_fields", &error))
    }

    /// Store a derived audit document and link it to the transaction
    /// document. The completion certificate PDF and any future signed-PDF
    /// renderer both land here; the bytes decide the mime type.
    ///
    /// Object-first only: the caller uploads the verified object BEFORE this transaction and supplies
    /// the media id it used for `documents/{media_id}/...` plus the object metadata. No bytes are
    /// written here; there is no store-absent byte fallback.
    pub async fn store_audit_artifact_tx(
        &self,
        tx: &mut DbTransaction,
        transaction_document_id: &str,
        media_id: &str,
        filename: &str,
        mime_type: &str,
        object: &MediaObjectMetadata,
    ) -> DbResult<String> {
        insert_media_object_with_id_tx(tx, media_id, filename, mime_type, object).await?;
        sqlx::query(
            r#"
            update transaction_document
               set signed_audit_media_id = $2::uuid,
                   updated_at = now()
             where id = $1::uuid
            "#,
        )
        .bind(transaction_document_id)
        .bind(media_id)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.audit_link", &error))?;
        Ok(media_id.to_owned())
    }

    /// Store the sealed signed PDF without touching the audit link.
    /// The caller links it via [`Self::link_signed_media_tx`]; reusing
    /// [`Self::store_audit_artifact_tx`] here would overwrite
    /// `signed_audit_media_id` and make replay answer with the sealed
    /// artifact instead of the audit certificate.
    ///
    /// Object-first, like [`Self::store_audit_artifact_tx`]: the object is already uploaded under
    /// `media_id`, and only its metadata row is written here.
    pub async fn store_signed_artifact_tx(
        &self,
        tx: &mut DbTransaction,
        media_id: &str,
        filename: &str,
        mime_type: &str,
        object: &MediaObjectMetadata,
    ) -> DbResult<String> {
        insert_media_object_with_id_tx(tx, media_id, filename, mime_type, object).await
    }

    /// Link the sealed signed PDF. The renderer step owns the bytes;
    /// this only records where the sealed artifact lives.
    pub async fn link_signed_media_tx(
        &self,
        tx: &mut DbTransaction,
        transaction_document_id: &str,
        media_id: &str,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            update transaction_document
               set signed_media_id = $2::uuid,
                   signed_at = now(),
                   updated_at = now()
             where id = $1::uuid
            "#,
        )
        .bind(transaction_document_id)
        .bind(media_id)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.signed_link", &error))?;
        Ok(())
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
              join luxesign_request sr on sr.transaction_document_id = td.id
             where sr.id = $1::uuid
             limit 1
            "#,
        )
        .bind(signature_request_id)
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.audit_media_read", &error))
    }

    /// The template anchor blocks recorded on the issued document's
    /// Vault snapshot, if the issuing template declared any. Raw JSON —
    /// parsing and validation belong to the service, not the row read.
    /// The signature blocks the signers still have to fill: the issuing template's anchors, minus any block an applied
    /// signature already satisfied (Lisa's pre-signature is drawn into the PDF at issuance, so her block is done).
    ///
    /// Forms writes them under `source_snapshot.render` (`signatureAnchors`, `appliedSignatures`); an older shape kept
    /// the anchors at the top level, and is still read. `None` when the document carries no anchors at all.
    pub async fn template_anchors_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<serde_json::Value>> {
        sqlx::query_scalar::<_, Option<serde_json::Value>>(
            r#"
            with doc as (
                select coalesce(d.source_snapshot -> 'render' -> 'signatureAnchors',
                                d.source_snapshot -> 'signatureAnchors') as anchors,
                       coalesce(d.source_snapshot -> 'render' -> 'appliedSignatures', '[]'::jsonb) as applied
                  from transaction_document d
                  join luxesign_request sr on sr.transaction_document_id = d.id
                 where sr.id = $1::uuid
                 limit 1
            )
            select case when doc.anchors is null or jsonb_typeof(doc.anchors) <> 'array' then null
                        else (
                            select coalesce(jsonb_agg(a order by ord), '[]'::jsonb)
                              from jsonb_array_elements(doc.anchors) with ordinality as t(a, ord)
                             where not exists (
                                 select 1
                                   from jsonb_array_elements(doc.applied) p
                                  where p ->> 'role' = a ->> 'role'
                                    and (p ->> 'slotId') is not distinct from (a ->> 'slotId'))
                        )
                   end
              from doc
            "#,
        )
        .bind(signature_request_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.template_anchors", &error))
        .map(|row| row.flatten())
    }

    pub async fn signed_media_for_request_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<String>> {
        sqlx::query_scalar::<_, String>(
            r#"
            select td.signed_media_id::text
              from transaction_document td
              join luxesign_request sr on sr.transaction_document_id = td.id
             where sr.id = $1::uuid
             limit 1
            "#,
        )
        .bind(signature_request_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.signed_media_read", &error))
    }

    pub async fn canonical_status_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<Option<(String, String)>> {
        sqlx::query_as::<_, (String, String)>(
            "select status, transaction_document_id::text \
             from luxesign_request where id = $1::uuid limit 1",
        )
        .bind(signature_request_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.canonical_status", &error))
    }
}

#[cfg(test)]
mod tests;
