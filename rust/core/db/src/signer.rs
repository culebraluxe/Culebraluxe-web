use crate::{Database, DbFailure, DbResult, DbTransaction};
use chrono::{DateTime, Utc};
use domain::{
    DocumentSignRecipient, SignatureField, SignatureFieldType, SignatureRecipientRole,
    SignerRecipientState, SignerState,
};
use serde_json::Value;
use sqlx::FromRow;

#[derive(Debug, Clone)]
pub struct SignerAccessRecord {
    pub id: String,
    pub recipient_id: String,
    pub token_version: i32,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub signature_request_id: String,
}

#[derive(Debug, FromRow)]
struct AccessRow {
    id: String,
    recipient_id: String,
    token_version: i32,
    expires_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
    signature_request_id: String,
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
}

#[derive(Debug, FromRow)]
struct StateRow {
    recipient_id: String,
    state: String,
    notified_at: Option<DateTime<Utc>>,
    first_viewed_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    declined_at: Option<DateTime<Utc>>,
    expired_at: Option<DateTime<Utc>>,
    last_activity_at: Option<DateTime<Utc>>,
    revision: i64,
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
pub struct SignerDao {
    db: Database,
}

impl SignerDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn database(&self) -> Database {
        self.db.clone()
    }

    pub async fn access(&self, access_id: &str) -> DbResult<Option<SignerAccessRecord>> {
        let row = sqlx::query_as::<_, AccessRow>(
            r#"
            select a.id::text as id,
                   a.recipient_id::text as recipient_id,
                   a.token_version,
                   a.expires_at,
                   a.revoked_at,
                   r.signature_request_id::text as signature_request_id
              from signature_recipient_access a
              join signature_envelope_recipient r on r.id = a.recipient_id
             where a.id = $1::uuid
             limit 1
            "#,
        )
        .bind(access_id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.access", &error))?;
        Ok(row.map(map_access))
    }

    pub async fn recipient(&self, recipient_id: &str) -> DbResult<Option<DocumentSignRecipient>> {
        let row = sqlx::query_as::<_, RecipientRow>(
            r#"
            select id::text as id,
                   signature_request_id::text as signature_request_id,
                   recipient_role, recipient_name, recipient_email,
                   signer_order, signing_step, execution_role, execution_slot_id
              from signature_envelope_recipient
             where id = $1::uuid
             limit 1
            "#,
        )
        .bind(recipient_id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.recipient", &error))?;
        row.map(map_recipient).transpose()
    }

    pub async fn state(&self, recipient_id: &str) -> DbResult<Option<SignerRecipientState>> {
        let row = sqlx::query_as::<_, StateRow>(
            r#"
            select recipient_id::text as recipient_id,
                   state, notified_at, first_viewed_at, completed_at,
                   declined_at, expired_at, last_activity_at, revision
              from signature_recipient_state
             where recipient_id = $1::uuid
             limit 1
            "#,
        )
        .bind(recipient_id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.state", &error))?;
        row.map(map_state).transpose()
    }

    pub async fn fields(&self, recipient_id: &str) -> DbResult<Vec<SignatureField>> {
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
             where recipient_id = $1::uuid
             order by page_number, position_y, position_x, id
            "#,
        )
        .bind(recipient_id)
        .fetch_all(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.fields", &error))?;
        rows.into_iter().map(map_field).collect()
    }

    pub async fn consent_exists(&self, recipient_id: &str) -> DbResult<bool> {
        sqlx::query_scalar::<_, bool>(
            r#"
            select exists(
                select 1
                  from signature_recipient_consent
                 where recipient_id = $1::uuid
            )
            "#,
        )
        .bind(recipient_id)
        .fetch_one(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.consent_exists", &error))
    }

    pub async fn is_turn(&self, recipient_id: &str) -> DbResult<bool> {
        sqlx::query_scalar::<_, bool>(
            r#"
            select case
                when ds.signing_mode = 'parallel' then true
                else not exists (
                    select 1
                      from signature_envelope_recipient earlier
                      left join signature_recipient_state earlier_state
                        on earlier_state.recipient_id = earlier.id
                     where earlier.signature_request_id = r.signature_request_id
                       and earlier.signing_step < r.signing_step
                       and coalesce(earlier_state.state, 'pending') <> 'completed'
                )
            end
              from signature_envelope_recipient r
              join document_sign_request ds
                on ds.signature_request_id = r.signature_request_id
             where r.id = $1::uuid
            "#,
        )
        .bind(recipient_id)
        .fetch_optional(&mut *self.db.connection().await?)
        .await
        .map(|value| value.unwrap_or(false))
        .map_err(|error| DbFailure::from_sqlx("signer.is_turn", &error))
    }

    pub async fn issue_access_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
        expires_at: DateTime<Utc>,
    ) -> DbResult<SignerAccessRecord> {
        let row = sqlx::query_as::<_, AccessRow>(
            r#"
            insert into signature_recipient_access (
                recipient_id, expires_at, token_version
            )
            values ($1::uuid, $2, 1)
            on conflict (recipient_id) do update
                set expires_at = excluded.expires_at,
                    revoked_at = null,
                    token_version = signature_recipient_access.token_version + 1
            returning id::text as id,
                      recipient_id::text as recipient_id,
                      token_version,
                      expires_at,
                      revoked_at,
                      (
                        select r.signature_request_id::text
                          from signature_envelope_recipient r
                         where r.id = signature_recipient_access.recipient_id
                      ) as signature_request_id
            "#,
        )
        .bind(recipient_id)
        .bind(expires_at)
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.issue_access", &error))?;
        Ok(map_access(row))
    }

    pub async fn initialize_state_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            insert into signature_recipient_state (recipient_id, state)
            values ($1::uuid, 'pending')
            on conflict (recipient_id) do nothing
            "#,
        )
        .bind(recipient_id)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.initialize_state", &error))?;
        Ok(())
    }

    pub async fn mark_notified_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            update signature_recipient_state
               set state = case when state = 'pending' then 'notified' else state end,
                   notified_at = coalesce(notified_at, now()),
                   last_activity_at = now(),
                   revision = revision + 1
             where recipient_id = $1::uuid
               and state not in ('completed','declined','expired','revoked')
            "#,
        )
        .bind(recipient_id)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.mark_notified", &error))?;
        Ok(())
    }

    pub async fn mark_open_tx(&self, tx: &mut DbTransaction, recipient_id: &str) -> DbResult<bool> {
        let result = sqlx::query(
            r#"
            update signature_recipient_state
               set state = case
                            when state in ('pending','notified') then 'viewed'
                            else state
                           end,
                   first_viewed_at = coalesce(first_viewed_at, now()),
                   last_activity_at = now(),
                   revision = revision + 1
             where recipient_id = $1::uuid
               and state not in ('completed','declined','expired','revoked')
               and first_viewed_at is null
            "#,
        )
        .bind(recipient_id)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.open", &error))?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn accept_consent_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
        consent_version: &str,
        consent_text: &str,
        consent_text_sha256: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> DbResult<bool> {
        let result = sqlx::query(
            r#"
            insert into signature_recipient_consent (
                recipient_id, consent_version, consent_text,
                consent_text_sha256, accepted_at, ip_address, user_agent
            )
            values ($1::uuid, $2, $3, $4, now(), $5::inet, $6)
            on conflict (recipient_id) do nothing
            "#,
        )
        .bind(recipient_id)
        .bind(consent_version)
        .bind(consent_text)
        .bind(consent_text_sha256)
        .bind(ip_address)
        .bind(user_agent)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.consent", &error))?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn complete_field_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
        field_id: &str,
        value: &Value,
    ) -> DbResult<bool> {
        let result = sqlx::query(
            r#"
            insert into signature_field_response (
                field_id, recipient_id, value, completed_at
            )
            select f.id, f.recipient_id, $3, now()
              from signature_field f
             where f.id = $2::uuid
               and f.recipient_id = $1::uuid
            on conflict (field_id) do update
                set value = excluded.value,
                    completed_at = now()
              where signature_field_response.recipient_id = excluded.recipient_id
            "#,
        )
        .bind(recipient_id)
        .bind(field_id)
        .bind(value)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.complete_field", &error))?;
        if result.rows_affected() != 1 {
            return Ok(false);
        }

        sqlx::query(
            r#"
            update signature_recipient_state
               set state = case
                            when state in ('pending','notified','viewed') then 'in_progress'
                            else state
                           end,
                   last_activity_at = now(),
                   revision = revision + 1
             where recipient_id = $1::uuid
               and state not in ('completed','declined','expired','revoked')
            "#,
        )
        .bind(recipient_id)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.complete_field.state", &error))?;
        Ok(true)
    }

    pub async fn required_fields_complete_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
    ) -> DbResult<bool> {
        sqlx::query_scalar::<_, bool>(
            r#"
            select not exists (
                select 1
                  from signature_field f
                 where f.recipient_id = $1::uuid
                   and f.required
                   and not exists (
                        select 1
                          from signature_field_response response
                         where response.field_id = f.id
                           and response.recipient_id = f.recipient_id
                   )
            )
            "#,
        )
        .bind(recipient_id)
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.required_fields_complete", &error))
    }

    pub async fn complete_recipient_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
    ) -> DbResult<bool> {
        let result = sqlx::query(
            r#"
            update signature_recipient_state
               set state = 'completed',
                   completed_at = coalesce(completed_at, now()),
                   last_activity_at = now(),
                   revision = revision + 1
             where recipient_id = $1::uuid
               and state not in ('completed','declined','expired','revoked')
            "#,
        )
        .bind(recipient_id)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.complete", &error))?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn decline_recipient_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
    ) -> DbResult<bool> {
        let result = sqlx::query(
            r#"
            update signature_recipient_state
               set state = 'declined',
                   declined_at = coalesce(declined_at, now()),
                   last_activity_at = now(),
                   revision = revision + 1
             where recipient_id = $1::uuid
               and state not in ('completed','declined','expired','revoked')
            "#,
        )
        .bind(recipient_id)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.decline", &error))?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn envelope_ready_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<bool> {
        sqlx::query_scalar::<_, bool>(
            r#"
            select exists (
                select 1
                  from signature_envelope_recipient
                 where signature_request_id = $1::uuid
            )
            and not exists (
                select 1
                  from signature_envelope_recipient r
                  left join signature_recipient_state s on s.recipient_id = r.id
                 where r.signature_request_id = $1::uuid
                   and coalesce(s.state, 'pending') <> 'completed'
            )
            "#,
        )
        .bind(signature_request_id)
        .fetch_one(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.envelope_ready", &error))
    }

    pub async fn revoke_request_access_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<usize> {
        let access = sqlx::query(
            r#"
            update signature_recipient_access a
               set revoked_at = coalesce(a.revoked_at, now()),
                   token_version = a.token_version + 1
              from signature_envelope_recipient r
             where r.id = a.recipient_id
               and r.signature_request_id = $1::uuid
               and a.revoked_at is null
            "#,
        )
        .bind(signature_request_id)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.revoke_request_access", &error))?;

        sqlx::query(
            r#"
            update signature_recipient_state s
               set state = case
                            when s.state in ('completed','declined','expired') then s.state
                            else 'revoked'
                           end,
                   last_activity_at = now(),
                   revision = revision + 1
              from signature_envelope_recipient r
             where r.id = s.recipient_id
               and r.signature_request_id = $1::uuid
               and s.state <> 'revoked'
            "#,
        )
        .bind(signature_request_id)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.revoke_request_state", &error))?;

        Ok(access.rows_affected() as usize)
    }

    pub async fn append_evidence_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        recipient_id: Option<&str>,
        event_type: &str,
        actor_id: Option<&str>,
        correlation_id: Option<&str>,
        causation_id: Option<&str>,
        evidence: &Value,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            insert into signature_evidence_event (
                signature_request_id, recipient_id, event_type,
                actor_id, correlation_id, causation_id, evidence
            )
            values ($1::uuid, $2::uuid, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(signature_request_id)
        .bind(recipient_id)
        .bind(event_type)
        .bind(actor_id)
        .bind(correlation_id)
        .bind(causation_id)
        .bind(evidence)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.evidence", &error))?;
        Ok(())
    }
}

fn map_access(row: AccessRow) -> SignerAccessRecord {
    SignerAccessRecord {
        id: row.id,
        recipient_id: row.recipient_id,
        token_version: row.token_version,
        expires_at: row.expires_at,
        revoked_at: row.revoked_at,
        signature_request_id: row.signature_request_id,
    }
}

fn map_recipient(row: RecipientRow) -> DbResult<DocumentSignRecipient> {
    let role = match row.recipient_role.as_str() {
        "signer" => SignatureRecipientRole::Signer,
        "approver" => SignatureRecipientRole::Approver,
        other => {
            return Err(DbFailure::schema_mismatch(
                "signer.map_recipient",
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
    })
}

fn map_state(row: StateRow) -> DbResult<SignerRecipientState> {
    Ok(SignerRecipientState {
        recipient_id: row.recipient_id,
        state: SignerState::try_from(row.state.as_str())
            .map_err(|error| DbFailure::schema_mismatch("signer.map_state", error))?,
        notified_at: row.notified_at.map(|value| value.to_rfc3339()),
        first_viewed_at: row.first_viewed_at.map(|value| value.to_rfc3339()),
        completed_at: row.completed_at.map(|value| value.to_rfc3339()),
        declined_at: row.declined_at.map(|value| value.to_rfc3339()),
        expired_at: row.expired_at.map(|value| value.to_rfc3339()),
        last_activity_at: row.last_activity_at.map(|value| value.to_rfc3339()),
        revision: row.revision,
    })
}

fn map_field(row: FieldRow) -> DbResult<SignatureField> {
    Ok(SignatureField {
        id: row.id,
        signature_request_id: row.signature_request_id,
        recipient_id: row.recipient_id,
        field_key: row.field_key,
        field_type: SignatureFieldType::try_from(row.field_type.as_str())
            .map_err(|error| DbFailure::schema_mismatch("signer.map_field", error))?,
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
