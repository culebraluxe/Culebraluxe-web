use crate::{Database, DbFailure, DbResult, DbTransaction};
use chrono::{DateTime, Utc};
use domain::{
    normalize_signature_email, ApplySignatureStatusRequest, SendSignatureRequest,
    SignatureCommandOutcome, SignatureCommandResult, SignatureRecipient, SignatureRequest,
    SignatureRequestResult, SignatureRequestStatus, SignatureStatusResult,
};
use serde_json::{json, Value};
use sqlx::FromRow;
use std::collections::BTreeSet;

#[derive(Debug, FromRow)]
struct SignatureRow {
    id: String,
    transaction_document_id: String,
    status: String,
    message: Option<String>,
    execution_role: Option<String>,
    execution_slot_id: Option<String>,
    created_by_user_id: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct DocumentSnapshotRow {
    source_snapshot: Option<Value>,
}

#[derive(Debug, FromRow)]
struct ReceiptRow {
    outcome: String,
    aggregate_id: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Clone)]
struct IssuedSlot {
    slot_id: String,
    role: String,
    email: Option<String>,
}

fn map_signature(row: SignatureRow) -> DbResult<SignatureRequest> {
    let status = SignatureRequestStatus::try_from(row.status.as_str())
        .map_err(|error| DbFailure::schema_mismatch("signature.map", error))?;
    Ok(SignatureRequest {
        id: row.id,
        transaction_document_id: row.transaction_document_id,
        status,
        message: row.message,
        execution_role: row.execution_role,
        execution_slot_id: row.execution_slot_id,
        created_by_user_id: row.created_by_user_id,
        created_at: row.created_at.to_rfc3339(),
        updated_at: row.updated_at.to_rfc3339(),
    })
}

fn parse_slots(snapshot: Option<&Value>) -> Result<Vec<IssuedSlot>, String> {
    let raw = snapshot
        .and_then(|value| value.get("issuedParticipants"))
        .and_then(Value::as_array)
        .ok_or_else(|| "issuedParticipants is missing or empty.".to_owned())?;
    if raw.is_empty() {
        return Err("issuedParticipants is missing or empty.".into());
    }

    let mut seen = BTreeSet::new();
    let mut slots = Vec::with_capacity(raw.len());
    for (index, value) in raw.iter().enumerate() {
        let object = value
            .as_object()
            .ok_or_else(|| format!("issuedParticipants[{index}] is not an object."))?;
        let slot_id = object
            .get("slotId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned();
        let role = object
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned();
        let email = object
            .get("email")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let person_id = object
            .get("personId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let required = object
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(true);

        if slot_id.is_empty() {
            return Err(format!("issuedParticipants[{index}] is missing slotId."));
        }
        if role.is_empty() {
            return Err(format!("issuedParticipants[{index}] is missing role."));
        }
        if !seen.insert(slot_id.clone()) {
            return Err(format!("duplicate slotId '{slot_id}'."));
        }
        if !slot_id.starts_with(&format!("{role}:")) {
            return Err(format!(
                "role '{role}' is inconsistent with slotId '{slot_id}'."
            ));
        }
        if required && person_id.is_none() && email.is_none() {
            return Err(format!(
                "required slot '{slot_id}' has no usable participant identity anchor."
            ));
        }

        slots.push(IssuedSlot {
            slot_id,
            role,
            email,
        });
    }
    Ok(slots)
}

async fn claim_receipt(
    tx: &mut DbTransaction,
    command_id: &str,
    actor_app_user_id: Option<&str>,
) -> DbResult<bool> {
    let claimed = sqlx::query_scalar::<_, String>(
        r#"
        insert into workflow_command_receipt (
            command_id, outcome, aggregate_id, message, actor_app_user_id
        )
        values ($1, 'pending', null, null, $2::uuid)
        on conflict (command_id) do nothing
        returning command_id
        "#,
    )
    .bind(command_id)
    .bind(actor_app_user_id)
    .fetch_optional(tx.connection())
    .await
    .map_err(|error| DbFailure::from_sqlx("signature.receipt.claim", &error))?;
    Ok(claimed.is_some())
}

async fn read_receipt(tx: &mut DbTransaction, command_id: &str) -> DbResult<Option<ReceiptRow>> {
    sqlx::query_as::<_, ReceiptRow>(
        r#"
        select outcome, aggregate_id::text as aggregate_id, message
        from workflow_command_receipt
        where command_id = $1
        limit 1
        "#,
    )
    .bind(command_id)
    .fetch_optional(tx.connection())
    .await
    .map_err(|error| DbFailure::from_sqlx("signature.receipt.read", &error))
}

async fn finalize_receipt(
    tx: &mut DbTransaction,
    command_id: &str,
    outcome: SignatureCommandOutcome,
    aggregate_id: Option<&str>,
    message: Option<&str>,
    actor_app_user_id: Option<&str>,
) -> DbResult<()> {
    sqlx::query(
        r#"
        update workflow_command_receipt
        set outcome = $2,
            aggregate_id = $3::uuid,
            message = $4,
            actor_app_user_id = $5::uuid
        where command_id = $1
        "#,
    )
    .bind(command_id)
    .bind(outcome.as_str())
    .bind(aggregate_id)
    .bind(message)
    .bind(actor_app_user_id)
    .execute(tx.connection())
    .await
    .map_err(|error| DbFailure::from_sqlx("signature.receipt.finalize", &error))?;
    Ok(())
}

fn replay_result(command_id: &str, receipt: Option<ReceiptRow>) -> SignatureCommandResult {
    let Some(receipt) = receipt else {
        return SignatureCommandResult {
            command_id: command_id.into(),
            outcome: SignatureCommandOutcome::Conflict,
            aggregate_id: None,
            message: Some("Command has no receipt; treat as in-flight.".into()),
            replayed: true,
            value: None,
        };
    };
    if receipt.outcome == "pending" {
        return SignatureCommandResult {
            command_id: command_id.into(),
            outcome: SignatureCommandOutcome::Conflict,
            aggregate_id: receipt.aggregate_id,
            message: Some("Command claim is in-flight (pending receipt); retry later.".into()),
            replayed: true,
            value: None,
        };
    }
    SignatureCommandResult {
        command_id: command_id.into(),
        outcome: SignatureCommandOutcome::try_from(receipt.outcome.as_str())
            .unwrap_or(SignatureCommandOutcome::Conflict),
        aggregate_id: receipt.aggregate_id,
        message: receipt.message,
        replayed: true,
        value: None,
    }
}

fn command_result(
    command_id: &str,
    outcome: SignatureCommandOutcome,
    aggregate_id: Option<String>,
    message: Option<String>,
    value: Option<Value>,
) -> SignatureCommandResult {
    SignatureCommandResult {
        command_id: command_id.into(),
        outcome,
        aggregate_id,
        message,
        replayed: false,
        value,
    }
}

fn validate_bound_recipients(
    slots: &[IssuedSlot],
    recipients: &[SignatureRecipient],
) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for recipient in recipients
        .iter()
        .filter(|recipient| recipient.execution_slot_id.is_some())
    {
        let slot_id = recipient.execution_slot_id.as_deref().unwrap_or_default();
        let slot = slots
            .iter()
            .find(|slot| slot.slot_id == slot_id)
            .ok_or_else(|| format!("Execution slot '{slot_id}' does not exist in document."))?;

        if !seen.insert(slot_id.to_owned()) {
            return Err(format!(
                "Execution slot '{slot_id}' is duplicated in the signature envelope."
            ));
        }
        if let Some(role) = recipient.execution_role.as_deref() {
            if role != slot.role {
                return Err(format!(
                    "Execution role '{role}' does not match slot '{slot_id}'."
                ));
            }
        }
        if slot.email.as_deref().is_none_or(|email| {
            normalize_signature_email(email) != normalize_signature_email(&recipient.email)
        }) {
            return Err(format!(
                "Recipient for slot '{slot_id}' does not match the immutable issued participant."
            ));
        }
    }
    Ok(())
}

#[derive(Clone)]
pub struct SignatureDao {
    db: Database,
}

impl SignatureDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get(&self, id: &str) -> DbResult<Option<SignatureRequest>> {
        let row = sqlx::query_as::<_, SignatureRow>(
            r#"
            select id::text as id,
                   transaction_document_id::text as transaction_document_id,
                   status, message, execution_role, execution_slot_id,
                   created_by_user_id::text as created_by_user_id,
                   created_at, updated_at
            from signature_request
            where id = $1::uuid
            limit 1
            "#,
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("signature.get", &error))?;
        row.map(map_signature).transpose()
    }

    pub async fn active_for_document(
        &self,
        transaction_document_id: &str,
    ) -> DbResult<Option<SignatureRequest>> {
        let row = sqlx::query_as::<_, SignatureRow>(
            r#"
            select id::text as id,
                   transaction_document_id::text as transaction_document_id,
                   status, message, execution_role, execution_slot_id,
                   created_by_user_id::text as created_by_user_id,
                   created_at, updated_at
            from signature_request
            where transaction_document_id = $1::uuid
              and status in ('requested', 'sent', 'viewed', 'signed')
            order by created_at asc, id
            limit 1
            "#,
        )
        .bind(transaction_document_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("signature.active_for_document", &error))?;
        row.map(map_signature).transpose()
    }

    pub async fn list_by_document(
        &self,
        transaction_document_id: &str,
    ) -> DbResult<Vec<SignatureRequest>> {
        let rows = sqlx::query_as::<_, SignatureRow>(
            r#"
            select id::text as id,
                   transaction_document_id::text as transaction_document_id,
                   status, message, execution_role, execution_slot_id,
                   created_by_user_id::text as created_by_user_id,
                   created_at, updated_at
            from signature_request
            where transaction_document_id = $1::uuid
            order by created_at asc, id
            "#,
        )
        .bind(transaction_document_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("signature.list_by_document", &error))?;
        rows.into_iter().map(map_signature).collect()
    }

    pub async fn send(
        &self,
        request: &SendSignatureRequest,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        let mut tx = self.db.begin("signature.send").await?;
        let result = async {
            if !claim_receipt(&mut tx, &request.command_id, actor_app_user_id).await? {
                let receipt = read_receipt(&mut tx, &request.command_id).await?;
                return Ok(replay_result(&request.command_id, receipt));
            }

            let requires_snapshot = request.execution_slot_id.is_some()
                || request
                    .recipients
                    .iter()
                    .any(|recipient| recipient.execution_slot_id.is_some());

            let document = sqlx::query_as::<_, DocumentSnapshotRow>(
                r#"
                select source_snapshot
                from transaction_document
                where id = $1::uuid
                limit 1
                "#,
            )
            .bind(&request.transaction_document_id)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("signature.send.document", &error))?;

            let Some(document) = document else {
                let outcome = SignatureCommandOutcome::NotFound;
                let message = "Transaction document not found.".to_owned();
                finalize_receipt(
                    &mut tx,
                    &request.command_id,
                    outcome,
                    None,
                    Some(&message),
                    actor_app_user_id,
                )
                .await?;
                return Ok(command_result(
                    &request.command_id,
                    outcome,
                    None,
                    Some(message),
                    None,
                ));
            };

            let mut execution_role = request.execution_role.clone();
            if requires_snapshot {
                let slots = match parse_slots(document.source_snapshot.as_ref()) {
                    Ok(slots) => slots,
                    Err(error) => {
                        let outcome = SignatureCommandOutcome::ValidationFailure;
                        let message = format!("Invalid issued-participant snapshot: {error}");
                        finalize_receipt(
                            &mut tx,
                            &request.command_id,
                            outcome,
                            None,
                            Some(&message),
                            actor_app_user_id,
                        )
                        .await?;
                        return Ok(command_result(
                            &request.command_id,
                            outcome,
                            None,
                            Some(message),
                            None,
                        ));
                    }
                };

                if let Some(slot_id) = request.execution_slot_id.as_deref() {
                    let Some(slot) = slots.iter().find(|slot| slot.slot_id == slot_id) else {
                        let outcome = SignatureCommandOutcome::ValidationFailure;
                        let message = format!(
                            "Execution slot '{slot_id}' does not exist in document {}.",
                            request.transaction_document_id
                        );
                        finalize_receipt(
                            &mut tx,
                            &request.command_id,
                            outcome,
                            None,
                            Some(&message),
                            actor_app_user_id,
                        )
                        .await?;
                        return Ok(command_result(
                            &request.command_id,
                            outcome,
                            None,
                            Some(message),
                            None,
                        ));
                    };

                    if request
                        .execution_role
                        .as_deref()
                        .is_some_and(|role| role != slot.role)
                    {
                        let outcome = SignatureCommandOutcome::ValidationFailure;
                        let message = format!(
                            "Execution role '{}' does not match slot '{slot_id}'.",
                            request.execution_role.as_deref().unwrap_or_default()
                        );
                        finalize_receipt(
                            &mut tx,
                            &request.command_id,
                            outcome,
                            None,
                            Some(&message),
                            actor_app_user_id,
                        )
                        .await?;
                        return Ok(command_result(
                            &request.command_id,
                            outcome,
                            None,
                            Some(message),
                            None,
                        ));
                    }

                    if let Some(expected) = request.slot_recipient_email.as_deref() {
                        if slot.email.as_deref().is_none_or(|email| {
                            normalize_signature_email(email)
                                != normalize_signature_email(expected)
                        }) {
                            let outcome = SignatureCommandOutcome::ValidationFailure;
                            let message =
                                "Recipient does not match the immutable execution slot.".to_owned();
                            finalize_receipt(
                                &mut tx,
                                &request.command_id,
                                outcome,
                                None,
                                Some(&message),
                                actor_app_user_id,
                            )
                            .await?;
                            return Ok(command_result(
                                &request.command_id,
                                outcome,
                                None,
                                Some(message),
                                None,
                            ));
                        }
                    }
                    execution_role = Some(slot.role.clone());
                }

                if let Err(message) = validate_bound_recipients(&slots, &request.recipients) {
                    let outcome = SignatureCommandOutcome::ValidationFailure;
                    finalize_receipt(
                        &mut tx,
                        &request.command_id,
                        outcome,
                        None,
                        Some(&message),
                        actor_app_user_id,
                    )
                    .await?;
                    return Ok(command_result(
                        &request.command_id,
                        outcome,
                        None,
                        Some(message),
                        None,
                    ));
                }
            }

            let inserted = sqlx::query_as::<_, SignatureRow>(
                r#"
                insert into signature_request (
                    transaction_document_id, status, message, created_by_user_id,
                    execution_role, execution_slot_id
                )
                values ($1::uuid,'requested',$2,$3::uuid,$4,$5)
                on conflict (transaction_document_id)
                  where status in ('requested', 'sent', 'viewed', 'signed')
                  do nothing
                returning id::text as id,
                          transaction_document_id::text as transaction_document_id,
                          status, message, execution_role, execution_slot_id,
                          created_by_user_id::text as created_by_user_id,
                          created_at, updated_at
                "#,
            )
            .bind(&request.transaction_document_id)
            .bind(request.message.as_deref())
            .bind(request.created_by_user_id.as_deref())
            .bind(execution_role.as_deref())
            .bind(request.execution_slot_id.as_deref())
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("signature.send.insert", &error))?;

            let (signature, existing) = if let Some(row) = inserted {
                for recipient in &request.recipients {
                    sqlx::query(
                        r#"
                        insert into signature_envelope_recipient (
                            signature_request_id, execution_role, execution_slot_id,
                            recipient_name, recipient_email, signer_order
                        )
                        values ($1::uuid,$2,$3,$4,$5,$6)
                        "#,
                    )
                    .bind(&row.id)
                    .bind(recipient.execution_role.as_deref())
                    .bind(recipient.execution_slot_id.as_deref())
                    .bind(recipient.name.trim())
                    .bind(recipient.email.trim())
                    .bind(recipient.order)
                    .execute(tx.connection())
                    .await
                    .map_err(|error| {
                        DbFailure::from_sqlx("signature.send.insert_recipient", &error)
                    })?;
                }
                (map_signature(row)?, false)
            } else {
                let row = sqlx::query_as::<_, SignatureRow>(
                    r#"
                    select id::text as id,
                           transaction_document_id::text as transaction_document_id,
                           status, message, execution_role, execution_slot_id,
                           created_by_user_id::text as created_by_user_id,
                           created_at, updated_at
                    from signature_request
                    where transaction_document_id = $1::uuid
                      and status in ('requested', 'sent', 'viewed', 'signed')
                    order by created_at asc, id
                    limit 1
                    "#,
                )
                .bind(&request.transaction_document_id)
                .fetch_optional(tx.connection())
                .await
                .map_err(|error| DbFailure::from_sqlx("signature.send.active", &error))?;

                let Some(row) = row else {
                    let outcome = SignatureCommandOutcome::Conflict;
                    let message =
                        "Could not record the signature request (active-request conflict)."
                            .to_owned();
                    finalize_receipt(
                        &mut tx,
                        &request.command_id,
                        outcome,
                        None,
                        Some(&message),
                        actor_app_user_id,
                    )
                    .await?;
                    return Ok(command_result(
                        &request.command_id,
                        outcome,
                        None,
                        Some(message),
                        None,
                    ));
                };
                if row.execution_slot_id.as_deref() != request.execution_slot_id.as_deref() {
                    let outcome = SignatureCommandOutcome::Conflict;
                    let message = "Another signature request is active for this document; complete or void it before sending a different execution slot.".to_owned();
                    finalize_receipt(
                        &mut tx,
                        &request.command_id,
                        outcome,
                        Some(&row.id),
                        Some(&message),
                        actor_app_user_id,
                    )
                    .await?;
                    return Ok(command_result(
                        &request.command_id,
                        outcome,
                        Some(row.id),
                        Some(message),
                        None,
                    ));
                }
                (map_signature(row)?, true)
            };

            let aggregate_id = signature.id.clone();
            let value = serde_json::to_value(SignatureRequestResult {
                signature_request: signature,
                existing,
            })
            .map_err(|error| {
                DbFailure::schema_mismatch("signature.send.serialize", error.to_string())
            })?;
            let outcome = SignatureCommandOutcome::Success;
            finalize_receipt(
                &mut tx,
                &request.command_id,
                outcome,
                Some(&aggregate_id),
                None,
                actor_app_user_id,
            )
            .await?;
            Ok(command_result(
                &request.command_id,
                outcome,
                Some(aggregate_id),
                None,
                Some(value),
            ))
        }
        .await;

        match result {
            Ok(value) => {
                tx.commit().await?;
                Ok(value)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }

    pub async fn apply_status(
        &self,
        request: &ApplySignatureStatusRequest,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        self.transition(request, request.target_status, actor_app_user_id)
            .await
    }

    pub async fn cancel(
        &self,
        command_id: &str,
        signature_request_id: &str,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        self.transition(
            &ApplySignatureStatusRequest {
                command_id: command_id.into(),
                signature_request_id: signature_request_id.into(),
                target_status: Some(SignatureRequestStatus::Voided),
            },
            Some(SignatureRequestStatus::Voided),
            actor_app_user_id,
        )
        .await
    }

    pub async fn decline(
        &self,
        command_id: &str,
        signature_request_id: &str,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        self.transition(
            &ApplySignatureStatusRequest {
                command_id: command_id.into(),
                signature_request_id: signature_request_id.into(),
                target_status: Some(SignatureRequestStatus::Declined),
            },
            Some(SignatureRequestStatus::Declined),
            actor_app_user_id,
        )
        .await
    }

    async fn transition(
        &self,
        request: &ApplySignatureStatusRequest,
        target: Option<SignatureRequestStatus>,
        actor_app_user_id: Option<&str>,
    ) -> DbResult<SignatureCommandResult> {
        let mut tx = self.db.begin("signature.transition").await?;
        let result = async {
            if !claim_receipt(&mut tx, &request.command_id, actor_app_user_id).await? {
                let receipt = read_receipt(&mut tx, &request.command_id).await?;
                return Ok(replay_result(&request.command_id, receipt));
            }

            let current = sqlx::query_as::<_, SignatureRow>(
                r#"
                select id::text as id,
                       transaction_document_id::text as transaction_document_id,
                       status, message, execution_role, execution_slot_id,
                       created_by_user_id::text as created_by_user_id,
                       created_at, updated_at
                from signature_request
                where id = $1::uuid
                limit 1
                "#,
            )
            .bind(&request.signature_request_id)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("signature.transition.read", &error))?;

            let Some(current) = current else {
                let outcome = SignatureCommandOutcome::NotFound;
                let message = "Signature request not found.".to_owned();
                finalize_receipt(
                    &mut tx,
                    &request.command_id,
                    outcome,
                    None,
                    Some(&message),
                    actor_app_user_id,
                )
                .await?;
                return Ok(command_result(
                    &request.command_id,
                    outcome,
                    None,
                    Some(message),
                    None,
                ));
            };

            let from = SignatureRequestStatus::try_from(current.status.as_str())
                .map_err(|error| DbFailure::schema_mismatch("signature.transition", error))?;
            let mapped_current = map_signature(current)?;

            let (signature, transitioned) = match target {
                None => (mapped_current, false),
                Some(target) if target == from => (mapped_current, false),
                Some(target) if !from.can_transition_to(target) => {
                    let outcome = SignatureCommandOutcome::ValidationFailure;
                    let message = format!(
                        "Transition {} -> {} is not allowed.",
                        from.as_str(),
                        target.as_str()
                    );
                    finalize_receipt(
                        &mut tx,
                        &request.command_id,
                        outcome,
                        Some(&mapped_current.id),
                        Some(&message),
                        actor_app_user_id,
                    )
                    .await?;
                    return Ok(command_result(
                        &request.command_id,
                        outcome,
                        Some(mapped_current.id),
                        Some(message),
                        None,
                    ));
                }
                Some(target) => {
                    let updated = sqlx::query_as::<_, SignatureRow>(
                        r#"
                        update signature_request
                        set status = $2, updated_at = now()
                        where id = $1::uuid and status = $3
                        returning id::text as id,
                                  transaction_document_id::text as transaction_document_id,
                                  status, message, execution_role, execution_slot_id,
                                  created_by_user_id::text as created_by_user_id,
                                  created_at, updated_at
                        "#,
                    )
                    .bind(&request.signature_request_id)
                    .bind(target.as_str())
                    .bind(from.as_str())
                    .fetch_optional(tx.connection())
                    .await
                    .map_err(|error| DbFailure::from_sqlx("signature.transition.update", &error))?;

                    let Some(updated) = updated else {
                        let outcome = SignatureCommandOutcome::Conflict;
                        let message = format!(
                            "Signature request {} changed concurrently.",
                            request.signature_request_id
                        );
                        finalize_receipt(
                            &mut tx,
                            &request.command_id,
                            outcome,
                            Some(&request.signature_request_id),
                            Some(&message),
                            actor_app_user_id,
                        )
                        .await?;
                        return Ok(command_result(
                            &request.command_id,
                            outcome,
                            Some(request.signature_request_id.clone()),
                            Some(message),
                            None,
                        ));
                    };
                    (map_signature(updated)?, true)
                }
            };

            let aggregate_id = signature.id.clone();
            let value = serde_json::to_value(SignatureStatusResult {
                signature_request: signature,
                transitioned,
            })
            .map_err(|error| {
                DbFailure::schema_mismatch("signature.transition.serialize", error.to_string())
            })?;
            let outcome = SignatureCommandOutcome::Success;
            finalize_receipt(
                &mut tx,
                &request.command_id,
                outcome,
                Some(&aggregate_id),
                None,
                actor_app_user_id,
            )
            .await?;
            Ok(command_result(
                &request.command_id,
                outcome,
                Some(aggregate_id),
                None,
                Some(value),
            ))
        }
        .await;

        match result {
            Ok(value) => {
                tx.commit().await?;
                Ok(value)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_slot_parser_rejects_role_mismatch() {
        let snapshot = json!({
            "issuedParticipants": [{
                "slotId": "BUYER:1",
                "role": "SELLER",
                "email": "person@example.test",
                "required": true
            }]
        });
        assert!(parse_slots(Some(&snapshot)).is_err());
    }

    #[test]
    fn strict_slot_parser_rejects_unanchored_required_slot() {
        let snapshot = json!({
            "issuedParticipants": [{
                "slotId": "BUYER:1",
                "role": "BUYER",
                "required": true
            }]
        });
        assert!(parse_slots(Some(&snapshot)).is_err());
    }
}
