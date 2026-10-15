//! The row mappers, moved out of `luxesign.rs` (2026-10-14).
//!
//! Text-exact except for `pub(super)`, which the parent needs to keep its `use self::maps::*;` re-export and
//! therefore keep every existing call site spelled the same.

use super::*;

pub(super) fn map_config(row: ConfigRow) -> DbResult<LuxesignConfig> {
    Ok(LuxesignConfig {
        signature_request_id: row.signature_request_id,
        subject: row.subject,
        signing_mode: LuxesignSigningMode::try_from(row.signing_mode.as_str())
            .map_err(|error| DbFailure::schema_mismatch("luxesign.map_config", error))?,
        expires_at: row.expires_at.map(|value| value.to_rfc3339()),
        issued_at: row.issued_at.map(|value| value.to_rfc3339()),
        created_at: row.created_at.to_rfc3339(),
        updated_at: row.updated_at.to_rfc3339(),
    })
}

pub(super) fn map_recipient(row: RecipientRow) -> DbResult<LuxesignRecipient> {
    let role = match row.recipient_role.as_str() {
        "signer" => SignatureRecipientRole::Signer,
        "approver" => SignatureRecipientRole::Approver,
        other => {
            return Err(DbFailure::schema_mismatch(
                "luxesign.map_recipient",
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
        state: row.state,
    })
}

pub(super) fn map_field(row: FieldRow) -> DbResult<SignatureField> {
    Ok(SignatureField {
        id: row.id,
        signature_request_id: row.signature_request_id,
        recipient_id: row.recipient_id,
        field_key: row.field_key,
        field_type: SignatureFieldType::try_from(row.field_type.as_str())
            .map_err(|error| DbFailure::schema_mismatch("luxesign.map_field", error))?,
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

/// `"Listing Agreement v9"` becomes `"Listing Agreement"`. Only a trailing ` v<digits>` is removed, so a title that
/// merely contains a v is left alone.
pub(super) fn without_vault_version(title: &str) -> String {
    match title.rsplit_once(" v") {
        Some((name, number))
            if !name.trim().is_empty()
                && !number.is_empty()
                && number.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            name.to_owned()
        }
        _ => title.to_owned(),
    }
}
