//! The read side of [`SignerDao`]: one round trip per question, no state, no writes.
//!
//! Moved out of `signer.rs` on 2026-10-14 by the boundedness split (move-only): the file crossed the
//! 800-line ceiling and the read methods are the half that answers with a query and returns. They stay
//! inherent to `SignerDao`, so every call site resolves exactly as it did before the move.

use super::*;

impl SignerDao {
    pub async fn access(&self, access_id: &str) -> DbResult<Option<SignerAccessRecord>> {
        let row = sqlx::query_as::<_, AccessRow>(
            r#"
            select a.id::text as id,
                   a.recipient_id::text as recipient_id,
                   a.token_version,
                   a.expires_at,
                   a.revoked_at,
                   r.signature_request_id::text as signature_request_id
              from luxesign_recipient_access a
              join luxesign_envelope_recipient r on r.id = a.recipient_id
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

    /// The live grant for one recipient, if it has neither expired nor been
    /// revoked. Reminders reuse this link; a resend rotates it instead.
    pub async fn active_access_for_recipient_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
    ) -> DbResult<Option<SignerAccessRecord>> {
        let row = sqlx::query_as::<_, AccessRow>(
            r#"
            select a.id::text as id,
                   a.recipient_id::text as recipient_id,
                   a.token_version,
                   a.expires_at,
                   a.revoked_at,
                   r.signature_request_id::text as signature_request_id
              from luxesign_recipient_access a
              join luxesign_envelope_recipient r on r.id = a.recipient_id
             where a.recipient_id = $1::uuid
               and a.revoked_at is null
               and a.expires_at > now()
             limit 1
            "#,
        )
        .bind(recipient_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.active_access", &error))?;
        Ok(row.map(map_access))
    }

    pub async fn recipient(&self, recipient_id: &str) -> DbResult<Option<LuxesignRecipient>> {
        let row = sqlx::query_as::<_, RecipientRow>(
            r#"
            select id::text as id,
                   signature_request_id::text as signature_request_id,
                   recipient_role, recipient_name, recipient_email,
                   signer_order, signing_step, execution_role, execution_slot_id
              from luxesign_envelope_recipient
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
              from luxesign_recipient_state
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
              from luxesign_field
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
                  from luxesign_recipient_consent
                 where recipient_id = $1::uuid
            )
            "#,
        )
        .bind(recipient_id)
        .fetch_one(&mut *self.db.connection().await?)
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.consent_exists", &error))
    }

    /// The envelope-level facts a signer's page shows: status, title, the sender's note, which of this recipient's
    /// fields are answered, and everyone's progress (names and states only).
    pub async fn session_context(
        &self,
        recipient_id: &str,
    ) -> DbResult<model::SignerSessionContext> {
        let mut connection = self.db.connection().await?;
        let head: Option<(String, Option<String>, Option<String>, Option<String>)> =
            sqlx::query_as(
                r#"
            select sr.status, td.title, d.subject, sr.message
              from luxesign_envelope_recipient r
              join luxesign_request sr on sr.id = r.signature_request_id
              join transaction_document td on td.id = sr.transaction_document_id
              left join luxesign_config d on d.signature_request_id = sr.id
             where r.id = $1::uuid
            "#,
            )
            .bind(recipient_id)
            .fetch_optional(&mut *connection)
            .await
            .map_err(|error| DbFailure::from_sqlx("signer.session_context.head", &error))?;
        let answered: Vec<String> = sqlx::query_scalar(
            r#"
            select f.id::text
              from luxesign_field f
              join luxesign_field_response resp on resp.field_id = f.id
             where f.recipient_id = $1::uuid
               and resp.recipient_id = $1::uuid
            "#,
        )
        .bind(recipient_id)
        .fetch_all(&mut *connection)
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.session_context.answered", &error))?;
        let parties: Vec<(String, String, String, bool)> = sqlx::query_as(
            r#"
            select r2.recipient_name, r2.recipient_role, s.state, r2.id = r1.id
              from luxesign_envelope_recipient r1
              join luxesign_envelope_recipient r2
                on r2.signature_request_id = r1.signature_request_id
              join luxesign_recipient_state s on s.recipient_id = r2.id
             where r1.id = $1::uuid
             order by r2.signing_step, r2.signer_order
            "#,
        )
        .bind(recipient_id)
        .fetch_all(&mut *connection)
        .await
        .map_err(|error| DbFailure::from_sqlx("signer.session_context.parties", &error))?;
        let (envelope_status, document_title, subject, message) = head.unwrap_or_default();
        Ok(model::SignerSessionContext {
            envelope_status,
            document_title,
            subject,
            message,
            answered_field_ids: answered,
            parties: parties
                .into_iter()
                .map(|(name, role, state, is_you)| model::SignerParty {
                    name,
                    role,
                    state,
                    is_you,
                })
                .collect(),
        })
    }

    pub async fn is_turn(&self, recipient_id: &str) -> DbResult<bool> {
        sqlx::query_scalar::<_, bool>(
            r#"
            select case
                when ds.signing_mode = 'parallel' then true
                else not exists (
                    select 1
                      from luxesign_envelope_recipient earlier
                      left join luxesign_recipient_state earlier_state
                        on earlier_state.recipient_id = earlier.id
                     where earlier.signature_request_id = r.signature_request_id
                       and earlier.signing_step < r.signing_step
                       and coalesce(earlier_state.state, 'pending') <> 'completed'
                )
            end
              from luxesign_envelope_recipient r
              join luxesign_config ds
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
}
