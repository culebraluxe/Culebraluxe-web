//! Lisa's standing pre-signature: the rows it needs, the authority it requires, and the image it draws.
//!
//! PORTED FROM `legacy/db/broker-signature.ts`. POLICY lives in `model::forms_broker_signature` (the allowlist, the
//! declared-signer check, the configuration); everything here is what a connection is needed for. The result is either
//! the material to draw — with the evidence the snapshot records instead of the bytes — or a REFUSAL that fails the
//! issuance, because a document about to be sent to a provider must not go out without the signature it promised.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

use model::forms_applied_signature::{
    AppliedSignatureImageMimeType, FormAppliedSignature, BROKER_SIGNATURE_CONSENT_BASIS,
    BROKER_SIGNATURE_DATE_SEMANTIC,
};
use model::forms_broker_signature::{
    declared_signer_matches, normalized, policy_for_form, requires_execution_slot,
    BrokerSignatureConfig, SignatureAuthority, DEFAULT_BROKER_SIGNATURE_PURPOSE,
};
use model::forms_execution::IssuedExecutionSlot;
use model::security::{resolve_security_level, SecurityLevel};
use model::vault::{VaultArtifactFailure, VaultCommandOutcome, VaultRenderRequest};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, Row as _};

use crate::error::DbFailure;
use crate::{DbResult, MediaObjectMetadata, VaultDao};

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
    precondition_failure(DbFailure::from_sqlx(OPERATION, &error))
}

/// The same refusal, for a read that failed before it was ever a query — a connection that could not be taken.
///
/// Shared with the preview's resolution, which acquires its own connection rather than running inside an issuance
/// transaction (`db/src/vault/bind_form_to_contract.rs`). A caller must not read this as policy: policy is
/// `ValidationFailure`/`Unauthorized`, and this is the rows being unreachable.
pub(crate) fn precondition_failure(failure: DbFailure) -> VaultArtifactFailure {
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

/// One issuance — or one preview — as `resolve_for_issuance` needs it, minus the connection it runs on.
///
/// The tail three fields are the BRANCH, not three more values: `authority` says whether the actor named by the caller
/// must be allowed to APPLY the signature (`ApplyingActor` for an issuance, which refuses rather than issue unsigned)
/// or whether this is a preview that applies nothing (`OwnerByConstruction`), `issued_at` is the deterministic date the
/// record is stamped with, and `require_execution_slot` decides whether a missing provider slot is a refusal or an
/// accepted absence. Eight positional arguments made that trio read as more input; named fields make the call site say
/// which side of the branch it is on. Borrowed, like the other pass-through inputs here: nothing outlives the statement
/// that binds it.
#[derive(Debug, Clone)]
pub struct SignatureIssuance<'a> {
    pub template_id: &'a str,
    pub field_values: &'a BTreeMap<String, String>,
    pub slots: &'a [IssuedExecutionSlot],
    pub actor_app_user_id: Option<&'a str>,
    pub authority: SignatureAuthority,
    pub issued_at: Option<&'a str>,
    pub require_execution_slot: bool,
}

/// Resolve the material, or nothing at all when this document's broker role is not the configured signer's.
///
/// `authority` decides whether the actor named by the caller must be allowed to APPLY the signature: `ApplyingActor`
/// for an issuance (a refusal fails the document), `OwnerByConstruction` for a preview that only shows what issuance
/// would apply (`model::forms_broker_signature::SignatureAuthority`).
/// The one implementation of the policy, the authority rule, the slot mapping and the asset load.
///
/// `reader` is `Some` when the caller owns an object store: then the protected asset MUST be a verified object and
/// `media.file_data` is never read. It is `None` only when a deployment has no store (tests/dev, legacy issuance),
/// which is the one case the legacy blob fallback is allowed.
async fn resolve_for_issuance_from(
    connection: &mut PgConnection,
    issuance: &SignatureIssuance<'_>,
    reader: &dyn ProtectedAssetReader,
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
    let Some(policy) = policy_for_form(issuance.template_id, issuance.field_values) else {
        return Ok(Vec::new());
    };
    // Lisa is not occupying this document's broker role: no signature is applied, and that is not a failure.
    if !declared_signer_matches(policy, issuance.field_values, &config) {
        return Ok(Vec::new());
    }
    let Some(issued_at) = issuance.issued_at.filter(|value| !value.trim().is_empty()) else {
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

    // WHO is applying it. An issuance has to prove the actor may apply her signature — her, or a ROOT delegate — and
    // refuse rather than issue unsigned. A preview applies nothing: it shows the document, so it is not asked for that
    // authority, and it is not the person who will sign.
    if issuance.authority == SignatureAuthority::ApplyingActor {
        let Some(actor) = issuance
            .actor_app_user_id
            .filter(|value| !value.trim().is_empty())
        else {
            return Err(unauthorized(
                "an authenticated application user is required to apply the broker pre-signature.",
            ));
        };
        if !actor_may_apply(connection, actor, &rows.app_user_id).await? {
            return Err(unauthorized(
                "the authenticated actor is neither the configured broker signature owner nor a ROOT delegate.",
            ));
        }
    }

    let role_slots: Vec<&IssuedExecutionSlot> = issuance
        .slots
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
    if issuance.require_execution_slot
        && requires_execution_slot(issuance.template_id)
        && slot.is_none()
    {
        return Err(invalid_configuration(format!(
            "could not resolve {} to the required {} execution slot for {}.",
            config.signer_name, policy.role, issuance.template_id
        )));
    }

    let asset = load_protected_asset(connection, &rows.media_id, reader).await?;
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

/// One verified object pointer read from the `media` row. The bytes are NOT read here: db owns rows and the caller
/// owns the store, so the store is lent in through [`ProtectedAssetReader`].
struct ProtectedAssetPointer<'a> {
    bucket: &'a str,
    key: &'a str,
    sha256: Option<&'a str>,
    size: i64,
    status: &'a str,
    visibility: &'a str,
}

/// The non-byte half of a protected asset row: enough to validate whatever bytes are loaded against the record.
struct ProtectedAssetRecord<'a> {
    mime_type: &'a str,
    alt_text: &'a str,
    caption: &'a str,
    /// `Some` on the object-first path, `None` on the legacy blob path.
    object: Option<ProtectedAssetPointer<'a>>,
}

/// The object-store port this module needs, stated in the db crate so `db` keeps no storage dependency.
///
/// The web composition's adapter (`web/src/vault/applied_signatures.rs`) implements it over the real store; the
/// unit tests below implement it deterministically. db never names `services` or a provider SDK.
trait ProtectedAssetReader: Send + Sync {
    fn read<'a>(
        &'a self,
        bucket: &'a str,
        key: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, String>> + Send + 'a>>;
}

/// Adapts a caller's closure to the port. The closure owns the store handle; db only asks it for bytes.
struct ClosureAssetReader<F> {
    load: F,
}

impl<F, Fut> ProtectedAssetReader for ClosureAssetReader<F>
where
    F: Fn(String, String) -> Fut + Send + Sync,
    Fut: Future<Output = Result<Vec<u8>, String>> + Send + 'static,
{
    fn read<'a>(
        &'a self,
        bucket: &'a str,
        key: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, String>> + Send + 'a>> {
        Box::pin((self.load)(bucket.to_owned(), key.to_owned()))
    }
}

fn checksum_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn recorded_caption_checksum(caption: &str) -> Option<String> {
    caption
        .trim()
        .strip_prefix("sha256:")
        .map(|value| value.trim().to_lowercase())
}

/// Validate loaded bytes against the row that describes them.
///
/// Shared by both paths so the object path cannot quietly drop a check the legacy path made (or the other way
/// round). On the object path the row's recorded SHA-256 and size are BOTH required to match; a null SHA-256 is a
/// refusal, because a protected signature whose integrity cannot be shown must not be drawn.
fn verify_protected_asset(
    record: &ProtectedAssetRecord<'_>,
    bytes: Vec<u8>,
) -> Result<ProtectedAsset, VaultArtifactFailure> {
    let Some(mime_type) = AppliedSignatureImageMimeType::parse(record.mime_type) else {
        return Err(invalid_configuration(
            "asset must be an image/png or image/jpeg file.",
        ));
    };
    if record.alt_text != DEFAULT_BROKER_SIGNATURE_PURPOSE {
        return Err(invalid_configuration(
            "asset does not carry the canonical Lisa Penfield signature purpose.",
        ));
    }
    if bytes.is_empty() {
        return Err(invalid_configuration("asset contains no image bytes."));
    }
    let checksum_sha256 = checksum_hex(&bytes);
    if let Some(object) = record.object.as_ref() {
        if object.status != "VERIFIED" {
            return Err(invalid_configuration(
                "asset object is not marked VERIFIED in the protected store.",
            ));
        }
        // A protected signature image lives in a PRIVATE storage domain. Both existing private semantics are
        // accepted because the legacy backfill (`web/src/bin/media_migrate.rs`) maps an image with no property
        // link to `property_media_private`/`private`, while a Vault-domain writer uses `vault`/`vault`; neither is
        // public, and a public placement is refused outright.
        let private_domain = matches!(
            (object.bucket, object.visibility),
            ("vault", "vault") | ("property_media_private", "private")
        );
        if !private_domain {
            return Err(invalid_configuration(
                "asset object must be in a private storage domain (vault/vault or property_media_private/private); a protected signature image must never be public.",
            ));
        }
        if object.size <= 0 || bytes.len() as i64 != object.size {
            return Err(invalid_configuration(
                "asset object size does not match the protected media record.",
            ));
        }
        let Some(recorded) = object.sha256.filter(|value| !value.trim().is_empty()) else {
            return Err(invalid_configuration(
                "asset object has no recorded SHA-256, so its integrity cannot be shown.",
            ));
        };
        if !recorded.trim().eq_ignore_ascii_case(&checksum_sha256) {
            return Err(invalid_configuration(
                "asset object checksum does not match the protected media record.",
            ));
        }
    }
    if let Some(recorded) = recorded_caption_checksum(record.caption) {
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

fn row_text(row: &sqlx::postgres::PgRow, column: &str) -> String {
    row.try_get::<Option<String>, _>(column)
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// Load the protected signature image: object-first when the caller owns a store, legacy blob only when it does not.
async fn load_protected_asset(
    connection: &mut PgConnection,
    media_id: &str,
    reader: &dyn ProtectedAssetReader,
) -> Result<ProtectedAsset, VaultArtifactFailure> {
    load_protected_asset_object(connection, media_id, reader).await
}

/// Object-first read: fetch the row's verified pointer (metadata only), then let the caller's store supply the
/// bytes, which are checked against the recorded size and SHA-256.
///
/// A store-backed deployment NEVER falls back to `media.file_data`: a row without a verified object is a refusal
/// that names the cutover step, never a silent blob. The object must be in a private storage domain (see
/// [`verify_protected_asset`]); a public placement is refused.
async fn load_protected_asset_object(
    connection: &mut PgConnection,
    media_id: &str,
    reader: &dyn ProtectedAssetReader,
) -> Result<ProtectedAsset, VaultArtifactFailure> {
    let row = sqlx::query(
        r#"
        select object_bucket, object_key, object_sha256, object_size, storage_status, visibility,
               mime_type, alt_text, caption
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
    let bucket = row_text(&row, "object_bucket");
    let key = row_text(&row, "object_key");
    if bucket.is_empty() || key.is_empty() {
        return Err(invalid_configuration(
            "asset has no verified object in the protected store; backfill the protected signature object before object reads are enabled.",
        ));
    }
    let sha256: Option<String> = row.try_get("object_sha256").ok().flatten();
    let size: i64 = row.try_get("object_size").unwrap_or(0);
    let status = row_text(&row, "storage_status");
    let visibility = row_text(&row, "visibility");
    let bytes = reader.read(&bucket, &key).await.map_err(|detail| {
        VaultArtifactFailure {
            outcome: VaultCommandOutcome::PreconditionFailure,
            message: format!(
                "document.issue failed: the protected signature asset object could not be read: {detail}"
            ),
        }
    })?;
    verify_protected_asset(
        &ProtectedAssetRecord {
            mime_type: &row_text(&row, "mime_type"),
            alt_text: &row_text(&row, "alt_text"),
            caption: &row_text(&row, "caption"),
            object: Some(ProtectedAssetPointer {
                bucket: &bucket,
                key: &key,
                sha256: sha256.as_deref(),
                size,
                status: &status,
                visibility: &visibility,
            }),
        },
        bytes,
    )
}

/// Validate an object-first protected-signature write before it reaches SQL.
fn validate_protected_signature_object(
    mime_type: &str,
    object: &MediaObjectMetadata,
) -> DbResult<()> {
    if AppliedSignatureImageMimeType::parse(mime_type).is_none() {
        return Err(DbFailure::configuration(
            "broker_signature.insert_protected_signature_object",
            "the protected signature object must be an image/png or image/jpeg file",
        ));
    }
    if object.status != "VERIFIED" {
        return Err(DbFailure::configuration(
            "broker_signature.insert_protected_signature_object",
            "the protected signature object must be VERIFIED before its row is written",
        ));
    }
    let private_domain = matches!(
        (object.bucket.as_str(), object.visibility.as_deref()),
        ("vault", Some("vault")) | ("property_media_private", Some("private"))
    );
    if !private_domain {
        return Err(DbFailure::configuration(
            "broker_signature.insert_protected_signature_object",
            "the protected signature object must live in a private storage domain (vault/vault or property_media_private/private) and never be public",
        ));
    }
    if object.size <= 0 {
        return Err(DbFailure::configuration(
            "broker_signature.insert_protected_signature_object",
            "the protected signature object must have a positive size",
        ));
    }
    if object
        .sha256
        .as_deref()
        .map_or(true, |value| value.trim().is_empty())
    {
        return Err(DbFailure::configuration(
            "broker_signature.insert_protected_signature_object",
            "the protected signature object must carry its SHA-256",
        ));
    }
    Ok(())
}

impl VaultDao {
    /// The OBJECT-FIRST resolution the browser preview uses.
    ///
    /// `read_object` receives the row's `(bucket, key)` and must return the stored bytes; any error is a
    /// precondition failure (the rows were reachable, the bytes were not). The row must be a verified Vault object:
    /// there is no blob fallback here, because a caller that has a store has already opted into object reads.
    ///
    /// Authorization is unchanged: the caller still resolves under `vault.read`, and `authority` is the same
    /// `ApplyingActor`/`OwnerByConstruction` choice issuance uses.
    pub async fn resolve_applied_signatures_object_first<F, Fut>(
        &self,
        request: &VaultRenderRequest,
        authority: SignatureAuthority,
        read_object: F,
    ) -> Result<Vec<FormAppliedSignature>, VaultArtifactFailure>
    where
        F: Fn(String, String) -> Fut + Send + Sync,
        Fut: Future<Output = Result<Vec<u8>, String>> + Send + 'static,
    {
        let slots = model::forms_execution::slots_from_signers(&request.participants);
        let issuance = SignatureIssuance {
            template_id: &request.template_id,
            field_values: &request.field_values,
            slots: &slots,
            actor_app_user_id: request.actor_app_user_id.as_deref(),
            authority,
            issued_at: request.issued_at.as_deref(),
            require_execution_slot: false,
        };
        let reader = ClosureAssetReader { load: read_object };
        let mut connection = self.database().connection().await.map_err(precondition_failure)?;
        resolve_for_issuance_from(&mut connection, &issuance, &reader).await
    }

    /// Write the protected signature asset as an OBJECT ROW — no `media.file_data` column is named.
    ///
    /// The caller has already uploaded and verified the object (the db crate owns no store); this records its
    /// metadata under the caller-chosen media id, with the canonical purpose in `alt_text` and a PRIVATE storage
    /// domain (`vault`/`vault` or `property_media_private`/`private`, never public). `caption` may carry
    /// `sha256:<hex>` exactly as the reader checks it.
    ///
    /// A legacy row with the same `alt_text` should be retired as part of the backfill; this writer refuses to guess
    /// (it never updates an existing row) so a duplicate purpose cannot silently become the resolved asset.
    pub async fn insert_protected_signature_object(
        &self,
        id: &str,
        filename: &str,
        mime_type: &str,
        caption: Option<&str>,
        object: &MediaObjectMetadata,
    ) -> DbResult<String> {
        validate_protected_signature_object(mime_type, object)?;
        let mut connection = self.database().connection().await?;
        sqlx::query(
            r#"
            insert into media (
                id, filename, mime_type, file_size, media_type, alt_text, caption,
                object_bucket, object_key, object_etag, object_sha256, object_size,
                object_migrated_at, storage_status, visibility
            )
            values (
                $1::uuid, $2, $3, $4, 'image', $5, $6,
                $7, $8, $9, $10, $11,
                now(), 'VERIFIED', 'vault'
            )
            "#,
        )
        .bind(id)
        .bind(filename)
        .bind(mime_type)
        .bind(object.size)
        .bind(DEFAULT_BROKER_SIGNATURE_PURPOSE)
        .bind(caption)
        .bind(&object.bucket)
        .bind(&object.key)
        .bind(object.etag.as_deref())
        .bind(object.sha256.as_deref())
        .bind(object.size)
        .execute(&mut *connection)
        .await
        .map_err(|error| {
            DbFailure::from_sqlx("broker_signature.insert_protected_signature_object", &error)
        })?;
        Ok(id.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pointer<'a>(
        bucket: &'a str,
        key: &'a str,
        sha256: Option<&'a str>,
        size: i64,
        status: &'a str,
        visibility: &'a str,
    ) -> ProtectedAssetPointer<'a> {
        ProtectedAssetPointer {
            bucket,
            key,
            sha256,
            size,
            status,
            visibility,
        }
    }

    fn record<'a>(
        object: Option<ProtectedAssetPointer<'a>>,
        mime_type: &'a str,
        alt_text: &'a str,
        caption: &'a str,
    ) -> ProtectedAssetRecord<'a> {
        ProtectedAssetRecord {
            mime_type,
            alt_text,
            caption,
            object,
        }
    }

    const PIXEL: &[u8] = b"\x89PNG\r\n\x1a\nprotected-pixel";

    fn object_metadata(sha256: &str) -> MediaObjectMetadata {
        MediaObjectMetadata {
            bucket: "vault".to_owned(),
            key: "media/media-1/original.png".to_owned(),
            etag: Some("\"etag\"".to_owned()),
            sha256: Some(sha256.to_owned()),
            size: PIXEL.len() as i64,
            status: "VERIFIED".to_owned(),
            visibility: Some("vault".to_owned()),
        }
    }

    #[test]
    fn a_verified_vault_object_passes_and_reports_its_checksum() {
        let sha = checksum_hex(PIXEL);
        let asset = verify_protected_asset(
            &record(
                Some(pointer(
                    "vault",
                    "media/media-1/original.png",
                    Some(&sha),
                    PIXEL.len() as i64,
                    "VERIFIED",
                    "vault",
                )),
                "image/png",
                DEFAULT_BROKER_SIGNATURE_PURPOSE,
                "",
            ),
            PIXEL.to_vec(),
        )
        .expect("a verified object with matching bytes is the asset");
        assert_eq!(asset.bytes, PIXEL);
        assert_eq!(asset.checksum_sha256, sha);
    }

    #[test]
    fn a_tampered_object_is_refused() {
        let recorded = "0".repeat(64);
        let refused = verify_protected_asset(
            &record(
                Some(pointer(
                    "vault",
                    "media/media-1/original.png",
                    Some(&recorded),
                    PIXEL.len() as i64,
                    "VERIFIED",
                    "vault",
                )),
                "image/png",
                DEFAULT_BROKER_SIGNATURE_PURPOSE,
                "",
            ),
            PIXEL.to_vec(),
        );
        assert!(refused.is_err(), "bytes that do not hash to the record are refused");
    }

    #[test]
    fn an_object_without_a_recorded_sha256_is_refused() {
        let refused = verify_protected_asset(
            &record(
                Some(pointer(
                    "vault",
                    "media/media-1/original.png",
                    None,
                    PIXEL.len() as i64,
                    "VERIFIED",
                    "vault",
                )),
                "image/png",
                DEFAULT_BROKER_SIGNATURE_PURPOSE,
                "",
            ),
            PIXEL.to_vec(),
        );
        assert!(refused.is_err(), "integrity that cannot be shown is not accepted");
    }

    #[test]
    fn a_size_mismatch_is_refused() {
        let sha = checksum_hex(PIXEL);
        let refused = verify_protected_asset(
            &record(
                Some(pointer(
                    "vault",
                    "media/media-1/original.png",
                    Some(&sha),
                    PIXEL.len() as i64 + 1,
                    "VERIFIED",
                    "vault",
                )),
                "image/png",
                DEFAULT_BROKER_SIGNATURE_PURPOSE,
                "",
            ),
            PIXEL.to_vec(),
        );
        assert!(refused.is_err());
    }

    #[test]
    fn the_private_property_media_domain_is_also_accepted() {
        let sha = checksum_hex(PIXEL);
        let asset = verify_protected_asset(
            &record(
                Some(pointer(
                    "property_media_private",
                    "media/media-1/original.png",
                    Some(&sha),
                    PIXEL.len() as i64,
                    "VERIFIED",
                    "private",
                )),
                "image/png",
                DEFAULT_BROKER_SIGNATURE_PURPOSE,
                "",
            ),
            PIXEL.to_vec(),
        )
        .expect("the legacy backfill's private image domain is a valid protected placement");
        assert_eq!(asset.bytes, PIXEL);
    }

    #[test]
    fn an_unverified_or_public_object_is_refused() {
        let sha = checksum_hex(PIXEL);
        for (status, visibility, bucket) in [
            ("PENDING", "vault", "vault"),
            ("VERIFIED", "public", "property_media_public"),
            ("VERIFIED", "vault", "property_media_private"),
            ("VERIFIED", "private", "vault"),
        ] {
            let refused = verify_protected_asset(
                &record(
                    Some(pointer(
                        bucket,
                        "media/media-1/original.png",
                        Some(&sha),
                        PIXEL.len() as i64,
                        status,
                        visibility,
                    )),
                    "image/png",
                    DEFAULT_BROKER_SIGNATURE_PURPOSE,
                    "",
                ),
                PIXEL.to_vec(),
            );
            assert!(refused.is_err(), "{status}/{visibility}/{bucket} must be refused");
        }
    }

    #[test]
    fn a_wrong_purpose_or_mime_is_refused() {
        let sha = checksum_hex(PIXEL);
        let wrong_alt = verify_protected_asset(
            &record(None, "image/png", "some-other-purpose", ""),
            PIXEL.to_vec(),
        );
        assert!(wrong_alt.is_err());
        let wrong_mime =
            verify_protected_asset(&record(None, "image/gif", DEFAULT_BROKER_SIGNATURE_PURPOSE, ""), PIXEL.to_vec());
        assert!(wrong_mime.is_err());
        let _ = sha;
    }

    #[test]
    fn a_caption_checksum_must_match_on_both_paths() {
        let wrong = format!("sha256:{}", "0".repeat(64));
        let object_path = verify_protected_asset(
            &record(
                Some(pointer(
                    "vault",
                    "media/media-1/original.png",
                    Some(&checksum_hex(PIXEL)),
                    PIXEL.len() as i64,
                    "VERIFIED",
                    "vault",
                )),
                "image/png",
                DEFAULT_BROKER_SIGNATURE_PURPOSE,
                &wrong,
            ),
            PIXEL.to_vec(),
        );
        assert!(object_path.is_err(), "a caption mismatch is refused on the object path");
        let legacy = verify_protected_asset(
            &record(None, "image/png", DEFAULT_BROKER_SIGNATURE_PURPOSE, &wrong),
            PIXEL.to_vec(),
        );
        assert!(legacy.is_err(), "a caption mismatch is refused on the legacy path");
    }

    #[test]
    fn an_empty_asset_is_refused() {
        let refused = verify_protected_asset(
            &record(None, "image/png", DEFAULT_BROKER_SIGNATURE_PURPOSE, ""),
            Vec::new(),
        );
        assert!(refused.is_err());
    }

    #[test]
    fn the_object_write_refuses_a_public_or_unverified_object() {
        let sha = checksum_hex(PIXEL);
        assert!(validate_protected_signature_object("image/png", &object_metadata(&sha)).is_ok());

        let mut private_image = object_metadata(&sha);
        private_image.bucket = "property_media_private".to_owned();
        private_image.visibility = Some("private".to_owned());
        assert!(
            validate_protected_signature_object("image/png", &private_image).is_ok(),
            "the existing private image domain is a valid protected placement"
        );

        let mut public = object_metadata(&sha);
        public.bucket = "property_media_public".to_owned();
        public.visibility = Some("public".to_owned());
        assert!(validate_protected_signature_object("image/png", &public).is_err());

        let mut unverified = object_metadata(&sha);
        unverified.status = "PENDING".to_owned();
        assert!(validate_protected_signature_object("image/png", &unverified).is_err());

        let mut no_sha = object_metadata(&sha);
        no_sha.sha256 = None;
        assert!(validate_protected_signature_object("image/png", &no_sha).is_err());

        assert!(validate_protected_signature_object("application/pdf", &object_metadata(&sha)).is_err());
    }
}
