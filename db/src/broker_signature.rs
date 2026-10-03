//! Lisa's standing pre-signature: the rows it needs, the authority it requires, and the image it draws.
//!
//! PORTED FROM `legacy/db/broker-signature.ts`. POLICY lives in `model::forms_broker_signature` (the allowlist, the
//! declared-signer check, the configuration); everything here is what a connection is needed for. The result is either
//! the material to draw — with the evidence the snapshot records instead of the bytes — or a REFUSAL that fails the
//! issuance, because a document about to be sent to a provider must not go out without the signature it promised.

use std::collections::BTreeMap;

use model::forms_applied_signature::{
    AppliedSignatureImageMimeType, FormAppliedSignature, BROKER_SIGNATURE_CONSENT_BASIS,
    BROKER_SIGNATURE_DATE_SEMANTIC,
};
use model::forms_broker_signature::{
    declared_signer_matches, normalized, policy_for_template, requires_execution_slot,
    BrokerSignatureConfig, DEFAULT_BROKER_SIGNATURE_PURPOSE,
};
use model::forms_execution::IssuedExecutionSlot;
use model::security::{resolve_security_level, SecurityLevel};
use model::vault::{VaultArtifactFailure, VaultCommandOutcome};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, Row as _};

use crate::error::DbFailure;

const OPERATION: &str = "vault.issue.broker_signature";

/// A configuration or policy problem: the document is not issued unsigned, and the operator is told why.
fn invalid_configuration(detail: impl AsRef<str>) -> VaultArtifactFailure {
    VaultArtifactFailure {
        outcome: VaultCommandOutcome::ValidationFailure,
        message: format!(
            "document.issue failed: broker signature configuration {}",
            detail.as_ref()
        ),
    }
}

/// The actor is neither the signature's owner nor a ROOT delegate.
fn unauthorized(detail: impl AsRef<str>) -> VaultArtifactFailure {
    VaultArtifactFailure {
        outcome: VaultCommandOutcome::Unauthorized,
        message: format!("document.issue failed: {}", detail.as_ref()),
    }
}

/// A read that failed is a precondition failure: without the rows, no signature can be shown to be authorized.
fn failed_read(error: sqlx::Error) -> VaultArtifactFailure {
    let failure = DbFailure::from_sqlx(OPERATION, &error);
    VaultArtifactFailure {
        outcome: VaultCommandOutcome::PreconditionFailure,
        message: format!("document.issue failed: {failure}"),
    }
}

/// The configured broker's rows: their app user, their canonical person, and the protected signature asset.
struct BrokerRows {
    app_user_id: String,
    media_id: String,
    person_id: Option<String>,
}

/// Resolve the material, or nothing at all when this document's broker role is not the configured signer's.
pub async fn resolve_for_issuance(
    connection: &mut PgConnection,
    template_id: &str,
    field_values: &BTreeMap<String, String>,
    slots: &[IssuedExecutionSlot],
    actor_app_user_id: Option<&str>,
    issued_at: Option<&str>,
    require_execution_slot: bool,
) -> Result<Vec<FormAppliedSignature>, VaultArtifactFailure> {
    let config = BrokerSignatureConfig::from_env();
    if !config.enabled {
        return Ok(Vec::new());
    }
    if !config.configured {
        return Err(invalid_configuration(
            "is incomplete; signer name and license are required.",
        ));
    }
    let Some(policy) = policy_for_template(template_id) else {
        return Ok(Vec::new());
    };
    // Lisa is not occupying this document's broker role: no signature is applied, and that is not a failure.
    if !declared_signer_matches(policy, field_values, &config) {
        return Ok(Vec::new());
    }
    let Some(actor) = actor_app_user_id.filter(|value| !value.trim().is_empty()) else {
        return Err(unauthorized(
            "an authenticated application user is required to apply the broker pre-signature.",
        ));
    };
    let Some(issued_at) = issued_at.filter(|value| !value.trim().is_empty()) else {
        return Err(invalid_configuration(
            "requires the command requestedAt timestamp as its deterministic issuance date.",
        ));
    };
    let applied_at = chrono::DateTime::parse_from_rfc3339(issued_at.trim())
        .map_err(|_| {
            invalid_configuration(
                "requires the command requestedAt timestamp as its deterministic issuance date.",
            )
        })?
        .with_timezone(&chrono::Utc)
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    let rows = resolve_broker_rows(connection, &config).await?;
    if !actor_may_apply(connection, actor, &rows.app_user_id).await? {
        return Err(unauthorized(
            "the authenticated actor is neither the configured broker signature owner nor a ROOT delegate.",
        ));
    }

    let role_slots: Vec<&IssuedExecutionSlot> = slots
        .iter()
        .filter(|slot| {
            slot.role == policy.role && normalized(&slot.name) == normalized(&config.signer_name)
        })
        .collect();
    if role_slots.len() > 1 {
        return Err(invalid_configuration(format!(
            "resolved more than one {} participant for {}.",
            policy.role, config.signer_name
        )));
    }
    let slot = role_slots.first().copied();
    if let (Some(slot_person), Some(broker_person)) = (
        slot.and_then(|slot| slot.person_id.clone()),
        rows.person_id.clone(),
    ) {
        if slot_person != broker_person {
            return Err(unauthorized(
                "the broker participant does not match the canonical signature owner.",
            ));
        }
    }
    // On the templates that use one ordered envelope, the slot IS the thing the provider fills: without it, Lisa would
    // be signed locally on a document that has no line for her.
    if require_execution_slot && requires_execution_slot(template_id) && slot.is_none() {
        return Err(invalid_configuration(format!(
            "could not resolve {} to the required {} execution slot for {template_id}.",
            config.signer_name, policy.role
        )));
    }

    let asset = load_protected_asset(connection, &rows.media_id).await?;
    Ok(vec![FormAppliedSignature {
        role: policy.role.to_string(),
        slot_id: slot.map(|slot| slot.slot_id.clone()),
        signer_name: config.signer_name.clone(),
        credential_line: config.credential_line(),
        signer_app_user_id: rows.app_user_id,
        image_bytes: asset.bytes,
        image_mime_type: asset.mime_type,
        asset_media_id: rows.media_id,
        asset_checksum_sha256: asset.checksum_sha256,
        applied_at,
        consent_basis: BROKER_SIGNATURE_CONSENT_BASIS.to_string(),
        date_semantic: BROKER_SIGNATURE_DATE_SEMANTIC.to_string(),
    }])
}

/// The broker's rows. Identity comes from DURABLE database identity unless an operator pinned a UUID, so PROD and DEV
/// each find their own rows and nobody has to remember one.
async fn resolve_broker_rows(
    connection: &mut PgConnection,
    config: &BrokerSignatureConfig,
) -> Result<BrokerRows, VaultArtifactFailure> {
    let user_rows = match config.app_user_id.as_deref() {
        Some(app_user_id) => {
            sqlx::query(
                r#"
                select id::text as id, display_name, person_id::text as person_id, active
                from app_user
                where id = $1::uuid
                limit 2
                "#,
            )
            .bind(app_user_id)
            .fetch_all(&mut *connection)
            .await
        }
        None => {
            sqlx::query(
                r#"
                select id::text as id, display_name, person_id::text as person_id, active
                from app_user
                where active = true
                  and lower(display_name) = lower($1)
                order by id
                limit 2
                "#,
            )
            .bind(&config.signer_name)
            .fetch_all(&mut *connection)
            .await
        }
    }
    .map_err(failed_read)?;
    if user_rows.len() != 1 {
        return Err(invalid_configuration(
            "must resolve exactly one active Lisa Penfield app user.",
        ));
    }
    let user = &user_rows[0];
    let active: bool = user.try_get("active").unwrap_or(false);
    let display_name: String = user
        .try_get::<Option<String>, _>("display_name")
        .ok()
        .flatten()
        .unwrap_or_default();
    if !active || normalized(&display_name) != normalized(&config.signer_name) {
        return Err(invalid_configuration(
            "resolved broker identity does not match the active Lisa Penfield principal.",
        ));
    }
    let app_user_id: String = user
        .try_get::<Option<String>, _>("id")
        .ok()
        .flatten()
        .unwrap_or_default();
    let person_id: Option<String> = user.try_get("person_id").ok().flatten();

    let media_rows = match config.media_id.as_deref() {
        Some(media_id) => {
            sqlx::query(
                r#"
                select id::text as id
                from media
                where id = $1::uuid
                  and media_type = 'image'
                  and mime_type in ('image/png','image/jpeg')
                limit 2
                "#,
            )
            .bind(media_id)
            .fetch_all(&mut *connection)
            .await
        }
        None => {
            sqlx::query(
                r#"
                select id::text as id
                from media
                where alt_text = $1
                  and media_type = 'image'
                  and mime_type in ('image/png','image/jpeg')
                order by created_at desc
                limit 2
                "#,
            )
            .bind(DEFAULT_BROKER_SIGNATURE_PURPOSE)
            .fetch_all(&mut *connection)
            .await
        }
    }
    .map_err(failed_read)?;
    if media_rows.len() != 1 {
        return Err(invalid_configuration(
            "must resolve exactly one protected Lisa Penfield signature image.",
        ));
    }
    Ok(BrokerRows {
        app_user_id,
        media_id: media_rows[0]
            .try_get::<Option<String>, _>("id")
            .ok()
            .flatten()
            .unwrap_or_default(),
        person_id,
    })
}

/// ROOT is the deliberate break-glass level in the four-level model: a ROOT actor may execute a broker-owned operation on
/// Lisa's behalf. The command receipt still records the REAL actor; the applied evidence still records Lisa as signer.
async fn actor_may_apply(
    connection: &mut PgConnection,
    actor_app_user_id: &str,
    broker_app_user_id: &str,
) -> Result<bool, VaultArtifactFailure> {
    if actor_app_user_id == broker_app_user_id {
        return Ok(true);
    }
    let rows = sqlx::query(
        r#"
        select r.code as code
        from app_user u
        join app_user_role aur on aur.app_user_id = u.id
        join security_role r on r.id = aur.role_id and r.active = true
        where u.id = $1::uuid
          and u.active = true
        order by r.code
        "#,
    )
    .bind(actor_app_user_id)
    .fetch_all(&mut *connection)
    .await
    .map_err(failed_read)?;
    let codes: Vec<String> = rows
        .iter()
        .filter_map(|row| row.try_get::<Option<String>, _>("code").ok().flatten())
        .map(|code| code.trim().to_string())
        .filter(|code| !code.is_empty())
        .collect();
    Ok(resolve_security_level(&codes) == SecurityLevel::Root)
}

struct ProtectedAsset {
    bytes: Vec<u8>,
    mime_type: AppliedSignatureImageMimeType,
    checksum_sha256: String,
}

/// Load the protected signature image, and prove it is the asset it claims to be.
///
/// The `caption` may carry `sha256:<hex>`; when it does, a mismatch REFUSES the signature rather than drawing an image
/// whose integrity cannot be shown.
async fn load_protected_asset(
    connection: &mut PgConnection,
    media_id: &str,
) -> Result<ProtectedAsset, VaultArtifactFailure> {
    let row = sqlx::query(
        r#"
        select file_data, mime_type, alt_text, caption
        from media
        where id = $1::uuid
          and media_type = 'image'
        limit 1
        "#,
    )
    .bind(media_id)
    .fetch_optional(&mut *connection)
    .await
    .map_err(failed_read)?;
    let Some(row) = row else {
        return Err(invalid_configuration(
            "asset is missing from the protected media store or is not an image.",
        ));
    };
    let mime: String = row
        .try_get::<Option<String>, _>("mime_type")
        .ok()
        .flatten()
        .unwrap_or_default();
    let Some(mime_type) = AppliedSignatureImageMimeType::parse(&mime) else {
        return Err(invalid_configuration(
            "asset must be an image/png or image/jpeg file.",
        ));
    };
    let alt_text: String = row
        .try_get::<Option<String>, _>("alt_text")
        .ok()
        .flatten()
        .unwrap_or_default();
    if alt_text != DEFAULT_BROKER_SIGNATURE_PURPOSE {
        return Err(invalid_configuration(
            "asset does not carry the canonical Lisa Penfield signature purpose.",
        ));
    }
    let bytes: Vec<u8> = row
        .try_get::<Option<Vec<u8>>, _>("file_data")
        .ok()
        .flatten()
        .unwrap_or_default();
    if bytes.is_empty() {
        return Err(invalid_configuration("asset contains no image bytes."));
    }
    let checksum_sha256 = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let caption: String = row
        .try_get::<Option<String>, _>("caption")
        .ok()
        .flatten()
        .unwrap_or_default();
    let recorded = caption
        .trim()
        .strip_prefix("sha256:")
        .map(|value| value.trim().to_lowercase());
    if let Some(recorded) = recorded {
        if recorded != checksum_sha256 {
            return Err(invalid_configuration(
                "asset checksum does not match the protected media record.",
            ));
        }
    }
    Ok(ProtectedAsset {
        bytes,
        mime_type,
        checksum_sha256,
    })
}
