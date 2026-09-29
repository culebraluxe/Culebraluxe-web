//! Moved from `signature.rs` (move only): SignatureRow, DocumentSnapshotRow, ReceiptRow, ReconcileRow, IssuedSlot, map_signature, parse_slots, claim_receipt, read_receipt, finalize_receipt, replay_result, command_result, validate_bound_recipients, SignatureDao.

#[allow(unused_imports)]
use super::*;

#[derive(Debug, FromRow)]
pub(super) struct SignatureRow {
    pub(super) id: String,
    pub(super) transaction_document_id: String,
    pub(super) status: String,
    pub(super) message: Option<String>,
    pub(super) execution_role: Option<String>,
    pub(super) execution_slot_id: Option<String>,
    pub(super) created_by_user_id: Option<String>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub(super) struct DocumentSnapshotRow {
    pub(super) source_snapshot: Option<Value>,
}

#[derive(Debug, FromRow)]
pub(super) struct ReceiptRow {
    pub(super) outcome: String,
    pub(super) aggregate_id: Option<String>,
    pub(super) message: Option<String>,
}

#[derive(Debug, FromRow)]
pub(super) struct ReconcileRow {
    pub(super) document_id: String,
    pub(super) state: String,
    pub(super) signed_media_id: Option<String>,
    pub(super) signed_audit_media_id: Option<String>,
    pub(super) signed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub(super) struct IssuedSlot {
    pub(super) slot_id: String,
    pub(super) role: String,
    pub(super) email: Option<String>,
}

pub(super) fn map_signature(row: SignatureRow) -> DbResult<SignatureRequest> {
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

pub(super) fn parse_slots(snapshot: Option<&Value>) -> Result<Vec<IssuedSlot>, String> {
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

pub(super) async fn claim_receipt(
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

pub(super) async fn read_receipt(
    tx: &mut DbTransaction,
    command_id: &str,
) -> DbResult<Option<ReceiptRow>> {
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

pub(super) async fn finalize_receipt(
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

pub(super) fn replay_result(
    command_id: &str,
    receipt: Option<ReceiptRow>,
) -> SignatureCommandResult {
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

pub(super) fn command_result(
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

pub(super) fn validate_bound_recipients(
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
    pub(super) db: Database,
}
