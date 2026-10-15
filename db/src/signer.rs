use crate::{Database, DbFailure, DbResult, DbTransaction};
use chrono::{DateTime, Utc};
use model::{
    LuxesignRecipient, SignatureField, SignatureFieldType, SignatureRecipientRole,
    SignerRecipientState, SignerState,
};
use serde_json::Value;
use sqlx::FromRow;

mod reads;

#[derive(Debug, Clone)]
pub struct SignerAccessRecord {
    pub id: String,
    pub recipient_id: String,
    pub token_version: i32,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub signature_request_id: String,
}

/// A recipient's consent as `accept_consent_tx` binds it (table `luxesign_recipient_consent`).
///
/// Seven positional arguments were seven chances to hand `consent_version` where `consent_text_sha256` belongs: both
/// are short strings on the same row, and the statement casts neither, so the transposition compiles and the row keeps
/// a version it never accepted. Named fields make the call site say which is which, and they name the pair the table
/// exists for: the text that was shown, and the hash that proves which text that was. Borrowed, like the other
/// pass-through inputs here: nothing outlives the statement that binds it.
#[derive(Debug, Clone)]
pub struct NewRecipientConsent<'a> {
    pub recipient_id: &'a str,
    pub consent_version: &'a str,
    pub consent_text: &'a str,
    pub consent_text_sha256: &'a str,
    pub ip_address: Option<&'a str>,
    pub user_agent: Option<&'a str>,
}

/// One evidence event as `append_evidence_tx` binds it (table `luxesign_evidence_event`).
///
/// `recipient_id`, `actor_id`, `correlation_id` and `causation_id` are four optional ids in a row, and only the first
/// casts to `uuid`: swapping `actor_id` with `correlation_id` compiles and writes a trail that names the wrong party.
/// Named fields make the call site say which id is which — the subject, the actor, and the two the trail is
/// reconstructed from.
#[derive(Debug, Clone)]
pub struct NewEvidenceEvent<'a> {
    pub signature_request_id: &'a str,
    pub recipient_id: Option<&'a str>,
    pub event_type: &'a str,
    pub actor_id: Option<&'a str>,
    pub correlation_id: Option<&'a str>,
    pub causation_id: Option<&'a str>,
    pub evidence: &'a Value,
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

#[derive(Debug, Clone)]
pub struct FinalizeRecipient {
    pub id: String,
    pub name: String,
    pub email: String,
    pub role: String,
    pub signer_order: i32,
    pub signing_step: i32,
    pub state: String,
    pub consent_version: Option<String>,
    pub consent_sha256: Option<String>,
    pub consent_accepted_at: Option<String>,
    /// When this recipient finished (their state row), for the certificate.
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FinalizeField {
    pub field_key: String,
    pub field_type: String,
    pub label: Option<String>,
    pub page_number: i32,
    pub recipient_id: String,
    pub required: bool,
    pub position_x: f64,
    pub position_y: f64,
    pub width: f64,
    pub height: f64,
    pub value: Option<Value>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FinalizeEvent {
    pub event_type: String,
    pub occurred_at: String,
    pub actor_id: Option<String>,
    /// The recipient the event is about, when it is about one.
    pub recipient_id: Option<String>,
    pub evidence: Value,
}

#[derive(Debug, Clone, Default)]
pub struct FinalizeInputs {
    pub recipients: Vec<FinalizeRecipient>,
    pub fields: Vec<FinalizeField>,
    pub events: Vec<FinalizeEvent>,
}

#[derive(Debug, FromRow)]
struct FinalizeRecipientRow {
    id: String,
    name: String,
    email: String,
    role: String,
    signer_order: i32,
    signing_step: i32,
    state: String,
    consent_version: Option<String>,
    consent_sha256: Option<String>,
    consent_accepted_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct FinalizeFieldRow {
    field_key: String,
    field_type: String,
    label: Option<String>,
    page_number: i32,
    recipient_id: String,
    required: bool,
    position_x: f64,
    position_y: f64,
    width: f64,
    height: f64,
    value: Option<Value>,
    completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct FinalizeEventRow {
    event_type: String,
    occurred_at: DateTime<Utc>,
    actor_id: Option<String>,
    recipient_id: Option<String>,
    evidence: Value,
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

    pub async fn issue_access_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
        expires_at: DateTime<Utc>,
    ) -> DbResult<SignerAccessRecord> {
        let row = sqlx::query_as::<_, AccessRow>(
            r#"
            insert into luxesign_recipient_access (
                recipient_id, expires_at, token_version
            )
            values ($1::uuid, $2, 1)
            on conflict (recipient_id) do update
                set expires_at = excluded.expires_at,
                    revoked_at = null,
                    token_version = luxesign_recipient_access.token_version + 1
            returning id::text as id,
                      recipient_id::text as recipient_id,
                      token_version,
                      expires_at,
                      revoked_at,
                      (
                        select r.signature_request_id::text
                          from luxesign_envelope_recipient r
                         where r.id = luxesign_recipient_access.recipient_id
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
            insert into luxesign_recipient_state (recipient_id, state)
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
            update luxesign_recipient_state
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
            update luxesign_recipient_state
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
        consent: &NewRecipientConsent<'_>,
    ) -> DbResult<bool> {
        let result = sqlx::query(
            r#"
            insert into luxesign_recipient_consent (
                recipient_id, consent_version, consent_text,
                consent_text_sha256, accepted_at, ip_address, user_agent
            )
            values ($1::uuid, $2, $3, $4, now(), $5::inet, $6)
            on conflict (recipient_id) do nothing
            "#,
        )
        .bind(consent.recipient_id)
        .bind(consent.consent_version)
        .bind(consent.consent_text)
        .bind(consent.consent_text_sha256)
        .bind(consent.ip_address)
        .bind(consent.user_agent)
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
            insert into luxesign_field_response (
                field_id, recipient_id, value, completed_at
            )
            select f.id, f.recipient_id, $3, now()
              from luxesign_field f
             where f.id = $2::uuid
               and f.recipient_id = $1::uuid
            on conflict (field_id) do update
                set value = excluded.value,
                    completed_at = now()
              where luxesign_field_response.recipient_id = excluded.recipient_id
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
            update luxesign_recipient_state
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
                  from luxesign_field f
                 where f.recipient_id = $1::uuid
                   and f.required
                   and not exists (
                        select 1
                          from luxesign_field_response response
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
            update luxesign_recipient_state
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
            update luxesign_recipient_state
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
                  from luxesign_envelope_recipient
                 where signature_request_id = $1::uuid
            )
            and not exists (
                select 1
                  from luxesign_envelope_recipient r
                  left join luxesign_recipient_state s on s.recipient_id = r.id
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
            update luxesign_recipient_access a
               set revoked_at = coalesce(a.revoked_at, now()),
                   token_version = a.token_version + 1
              from luxesign_envelope_recipient r
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
            update luxesign_recipient_state s
               set state = case
                            when s.state in ('completed','declined','expired') then s.state
                            else 'revoked'
                           end,
                   last_activity_at = now(),
                   revision = revision + 1
              from luxesign_envelope_recipient r
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
        event: &NewEvidenceEvent<'_>,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            insert into luxesign_evidence_event (
                signature_request_id, recipient_id, event_type,
                actor_id, correlation_id, causation_id, evidence
            )
            values ($1::uuid, $2::uuid, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(event.signature_request_id)
        .bind(event.recipient_id)
        .bind(event.event_type)
        .bind(event.actor_id)
        .bind(event.correlation_id)
        .bind(event.causation_id)
        .bind(event.evidence)
        .execute(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.evidence", &error))?;
        Ok(())
    }

    /// Expire overdue recipients: non-terminal states whose access grant
    /// lapsed, or whose envelope clock passed. Returns the expired ids so
    /// the caller can record evidence per recipient. Terminal states
    /// (completed, declined, expired, revoked) never reopen.
    pub async fn expire_overdue_recipients_tx(
        &self,
        tx: &mut DbTransaction,
    ) -> DbResult<Vec<(String, String)>> {
        sqlx::query_as::<_, (String, String)>(
            r#"
            update luxesign_recipient_state s
               set state = 'expired',
                   expired_at = coalesce(s.expired_at, now()),
                   last_activity_at = now(),
                   revision = revision + 1
              from luxesign_envelope_recipient r
              left join luxesign_recipient_access a on a.recipient_id = r.id
              left join luxesign_config ds on ds.signature_request_id = r.signature_request_id
             where s.recipient_id = r.id
               and s.state not in ('completed', 'declined', 'expired', 'revoked')
               and (
                    (a.recipient_id is not null and a.revoked_at is null and a.expires_at <= now())
                 or (ds.expires_at is not null and ds.expires_at <= now())
               )
             returning s.recipient_id::text, r.signature_request_id::text
            "#,
        )
        .fetch_all(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.expire_overdue", &error))
    }

    /// Everything the completion audit artifact is built from, read in one
    /// place: recipients with runtime state and consent evidence, fields
    /// with their responses, and the append-only event ledger in order.
    pub async fn finalize_inputs_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<FinalizeInputs> {
        let recipients = sqlx::query_as::<_, FinalizeRecipientRow>(
            r#"
            select r.id::text as id,
                   r.recipient_name as name,
                   r.recipient_email as email,
                   r.recipient_role as role,
                   r.signer_order as signer_order,
                   r.signing_step as signing_step,
                   coalesce(s.state, 'pending') as state,
                   c.consent_version as consent_version,
                   c.consent_text_sha256 as consent_sha256,
                   c.accepted_at as consent_accepted_at,
                   s.completed_at as completed_at
              from luxesign_envelope_recipient r
              left join luxesign_recipient_state s on s.recipient_id = r.id
              left join luxesign_recipient_consent c on c.recipient_id = r.id
             where r.signature_request_id = $1::uuid
             order by r.signer_order, r.id
            "#,
        )
        .bind(signature_request_id)
        .fetch_all(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.finalize_recipients", &error))?;
        let fields = sqlx::query_as::<_, FinalizeFieldRow>(
            r#"
            select f.field_key as field_key,
                   f.field_type as field_type,
                   f.label as label,
                   f.page_number as page_number,
                   f.recipient_id::text as recipient_id,
                   f.required as required,
                   f.position_x::float8 as position_x,
                   f.position_y::float8 as position_y,
                   f.width::float8 as width,
                   f.height::float8 as height,
                   resp.value as value,
                   resp.completed_at as completed_at
              from luxesign_field f
              left join luxesign_field_response resp on resp.field_id = f.id
             where f.signature_request_id = $1::uuid
             order by f.page_number, f.position_y, f.position_x, f.id
            "#,
        )
        .bind(signature_request_id)
        .fetch_all(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.finalize_fields", &error))?;
        let events = sqlx::query_as::<_, FinalizeEventRow>(
            r#"
            select event_type,
                   occurred_at,
                   actor_id,
                   recipient_id::text as recipient_id,
                   evidence
              from luxesign_evidence_event
             where signature_request_id = $1::uuid
             order by occurred_at, id
            "#,
        )
        .bind(signature_request_id)
        .fetch_all(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.finalize_events", &error))?;
        Ok(FinalizeInputs {
            recipients: recipients
                .into_iter()
                .map(|row| FinalizeRecipient {
                    id: row.id,
                    name: row.name,
                    email: row.email,
                    role: row.role,
                    signer_order: row.signer_order,
                    signing_step: row.signing_step,
                    state: row.state,
                    consent_version: row.consent_version,
                    consent_sha256: row.consent_sha256,
                    consent_accepted_at: row.consent_accepted_at.map(|value| value.to_rfc3339()),
                    completed_at: row.completed_at.map(|value| value.to_rfc3339()),
                })
                .collect(),
            fields: fields
                .into_iter()
                .map(|row| FinalizeField {
                    field_key: row.field_key,
                    field_type: row.field_type,
                    label: row.label,
                    page_number: row.page_number,
                    recipient_id: row.recipient_id,
                    required: row.required,
                    position_x: row.position_x,
                    position_y: row.position_y,
                    width: row.width,
                    height: row.height,
                    value: row.value,
                    completed_at: row.completed_at.map(|value| value.to_rfc3339()),
                })
                .collect(),
            events: events
                .into_iter()
                .map(|row| FinalizeEvent {
                    event_type: row.event_type,
                    occurred_at: row.occurred_at.to_rfc3339(),
                    actor_id: row.actor_id,
                    recipient_id: row.recipient_id,
                    evidence: row.evidence,
                })
                .collect(),
        })
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

fn map_recipient(row: RecipientRow) -> DbResult<LuxesignRecipient> {
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
    Ok(LuxesignRecipient {
        id: row.id,
        signature_request_id: row.signature_request_id,
        role,
        name: row.recipient_name,
        email: row.recipient_email,
        signer_order: row.signer_order,
        signing_step: row.signing_step,
        execution_role: row.execution_role,
        execution_slot_id: row.execution_slot_id,
        // Session reads carry liveness separately; this snapshot stays state-free.
        state: None,
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
