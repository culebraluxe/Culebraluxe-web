use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Duration, Utc};
use db::{DbResult, DbTransaction, FinalizeInputs, SignerAccessRecord, SignerDao};
use hmac::{Hmac, Mac};
use model::{
    AcceptSignerConsentRequest, CompleteSignatureFieldRequest, CompleteSignerRequest,
    DeclineSignerRequest, DocumentSignRecipient, OpenSignerRequest, SignatureField,
    SignerAccessGrant, SignerActionResult, SignerRecipientState, SignerSession, SignerState,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use services::{
    AbstractService, OperationKind, ServiceCapability, ServiceContext, ServiceDescriptor,
    ServiceDispatchError, ServiceEnvelope, ServiceExecutionPolicy, ServiceInfrastructure,
    ServiceRuntime,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub const DOCSIGN_EDGE_ACTOR: &str = "document-sign-edge";
const DEFAULT_ACCESS_DAYS: i64 = 7;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccessClaims {
    access_id: String,
    recipient_id: String,
    token_version: i32,
    exp: i64,
}

#[derive(Clone)]
pub struct SignerAccessTokenCodec {
    key: Option<Arc<Vec<u8>>>,
    unavailable_reason: Option<Arc<str>>,
    site_url: Arc<str>,
}

impl SignerAccessTokenCodec {
    pub fn from_env() -> Result<Self, String> {
        let secret = std::env::var("DOCSIGN_ACCESS_SECRET")
            .ok()
            .or_else(|| std::env::var("AUTH_SECRET").ok())
            .or_else(|| std::env::var("CULEBRA_INTERNAL_API_KEY").ok())
            .map(|value| value.trim().to_owned())
            .filter(|value| value.len() >= 16)
            .ok_or_else(|| {
                "DOCSIGN_ACCESS_SECRET, AUTH_SECRET, or CULEBRA_INTERNAL_API_KEY (16+ chars) is required for signer links."
                    .to_owned()
            })?;
        let mut hasher = Sha256::new();
        hasher.update(b"culebraluxe-docsign-access:v1:");
        hasher.update(secret.as_bytes());
        let key = hasher.finalize().to_vec();
        let site_url = std::env::var("PUBLIC_SITE_URL")
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "https://culebraluxe.com".into());
        Ok(Self {
            key: Some(Arc::new(key)),
            unavailable_reason: None,
            site_url: Arc::from(site_url),
        })
    }

    pub fn for_test(secret: &str, site_url: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"culebraluxe-docsign-access:v1:");
        hasher.update(secret.as_bytes());
        Self {
            key: Some(Arc::new(hasher.finalize().to_vec())),
            unavailable_reason: None,
            site_url: Arc::from(site_url.trim_end_matches('/')),
        }
    }

    pub fn unavailable(reason: impl Into<Arc<str>>) -> Self {
        let site_url = std::env::var("PUBLIC_SITE_URL")
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "https://culebraluxe.com".into());
        Self {
            key: None,
            unavailable_reason: Some(reason.into()),
            site_url: Arc::from(site_url),
        }
    }

    fn key(&self) -> Result<&[u8], CoreServiceError> {
        self.key
            .as_deref()
            .map(|key| key.as_slice())
            .ok_or_else(|| {
                CoreServiceError::business(
                    "SIGNER_ACCESS_NOT_CONFIGURED",
                    self.unavailable_reason
                        .as_deref()
                        .unwrap_or("Signer access is not configured.")
                        .to_owned(),
                )
            })
    }

    fn mint(&self, record: &SignerAccessRecord) -> Result<String, CoreServiceError> {
        let claims = AccessClaims {
            access_id: record.id.clone(),
            recipient_id: record.recipient_id.clone(),
            token_version: record.token_version,
            exp: record.expires_at.timestamp(),
        };
        let payload = serde_json::to_vec(&claims).map_err(|error| {
            CoreServiceError::business(
                "SIGNER_ACCESS_INVALID",
                format!("Could not serialize signer access: {error}"),
            )
        })?;
        let encoded = URL_SAFE_NO_PAD.encode(payload);
        let mut mac = Hmac::<Sha256>::new_from_slice(self.key()?).map_err(|_| {
            CoreServiceError::business("SIGNER_ACCESS_INVALID", "Signer key is invalid.")
        })?;
        mac.update(encoded.as_bytes());
        let signature = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
        Ok(format!("{encoded}.{signature}"))
    }

    fn verify(&self, token: &str) -> Result<AccessClaims, CoreServiceError> {
        let (encoded, signature) = token.split_once('.').ok_or_else(|| {
            CoreServiceError::business("SIGNER_ACCESS_INVALID", "Signer link is invalid.")
        })?;
        let signature = URL_SAFE_NO_PAD.decode(signature).map_err(|_| {
            CoreServiceError::business("SIGNER_ACCESS_INVALID", "Signer link is invalid.")
        })?;
        let mut mac = Hmac::<Sha256>::new_from_slice(self.key()?).map_err(|_| {
            CoreServiceError::business("SIGNER_ACCESS_INVALID", "Signer key is invalid.")
        })?;
        mac.update(encoded.as_bytes());
        mac.verify_slice(&signature).map_err(|_| {
            CoreServiceError::business("SIGNER_ACCESS_INVALID", "Signer link is invalid.")
        })?;
        let payload = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| {
            CoreServiceError::business("SIGNER_ACCESS_INVALID", "Signer link is invalid.")
        })?;
        serde_json::from_slice(&payload).map_err(|_| {
            CoreServiceError::business("SIGNER_ACCESS_INVALID", "Signer link is invalid.")
        })
    }

    fn signing_url(&self, token: &str) -> String {
        format!("{}/sign/{token}", self.site_url)
    }
}

#[async_trait]
pub trait SignerRepository: Send + Sync {
    async fn access(&self, access_id: &str) -> DbResult<Option<SignerAccessRecord>>;
    async fn recipient(&self, recipient_id: &str) -> DbResult<Option<DocumentSignRecipient>>;
    async fn state(&self, recipient_id: &str) -> DbResult<Option<SignerRecipientState>>;
    async fn fields(&self, recipient_id: &str) -> DbResult<Vec<SignatureField>>;
    async fn consent_exists(&self, recipient_id: &str) -> DbResult<bool>;
    async fn is_turn(&self, recipient_id: &str) -> DbResult<bool>;
    async fn issue_access_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
        expires_at: DateTime<Utc>,
    ) -> DbResult<SignerAccessRecord>;
    async fn active_access_for_recipient_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
    ) -> DbResult<Option<SignerAccessRecord>>;
    async fn finalize_inputs_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<FinalizeInputs>;
    async fn expire_overdue_recipients_tx(
        &self,
        tx: &mut DbTransaction,
    ) -> DbResult<Vec<(String, String)>>;
    async fn initialize_state_tx(&self, tx: &mut DbTransaction, recipient_id: &str)
        -> DbResult<()>;
    async fn mark_notified_tx(&self, tx: &mut DbTransaction, recipient_id: &str) -> DbResult<()>;
    async fn mark_open_tx(&self, tx: &mut DbTransaction, recipient_id: &str) -> DbResult<bool>;
    async fn accept_consent_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
        consent_version: &str,
        consent_text: &str,
        consent_text_sha256: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> DbResult<bool>;
    async fn complete_field_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
        field_id: &str,
        value: &Value,
    ) -> DbResult<bool>;
    async fn required_fields_complete_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
    ) -> DbResult<bool>;
    async fn complete_recipient_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
    ) -> DbResult<bool>;
    async fn decline_recipient_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
    ) -> DbResult<bool>;
    async fn envelope_ready_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<bool>;
    async fn revoke_request_access_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<usize>;
    async fn append_evidence_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        recipient_id: Option<&str>,
        event_type: &str,
        actor_id: Option<&str>,
        correlation_id: Option<&str>,
        causation_id: Option<&str>,
        evidence: &Value,
    ) -> DbResult<()>;
}

#[async_trait]
impl SignerRepository for SignerDao {
    async fn access(&self, access_id: &str) -> DbResult<Option<SignerAccessRecord>> {
        SignerDao::access(self, access_id).await
    }
    async fn recipient(&self, recipient_id: &str) -> DbResult<Option<DocumentSignRecipient>> {
        SignerDao::recipient(self, recipient_id).await
    }
    async fn state(&self, recipient_id: &str) -> DbResult<Option<SignerRecipientState>> {
        SignerDao::state(self, recipient_id).await
    }
    async fn fields(&self, recipient_id: &str) -> DbResult<Vec<SignatureField>> {
        SignerDao::fields(self, recipient_id).await
    }
    async fn consent_exists(&self, recipient_id: &str) -> DbResult<bool> {
        SignerDao::consent_exists(self, recipient_id).await
    }
    async fn is_turn(&self, recipient_id: &str) -> DbResult<bool> {
        SignerDao::is_turn(self, recipient_id).await
    }
    async fn issue_access_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
        expires_at: DateTime<Utc>,
    ) -> DbResult<SignerAccessRecord> {
        SignerDao::issue_access_tx(self, tx, recipient_id, expires_at).await
    }
    async fn active_access_for_recipient_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
    ) -> DbResult<Option<SignerAccessRecord>> {
        SignerDao::active_access_for_recipient_tx(self, tx, recipient_id).await
    }
    async fn finalize_inputs_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<FinalizeInputs> {
        SignerDao::finalize_inputs_tx(self, tx, signature_request_id).await
    }
    async fn expire_overdue_recipients_tx(
        &self,
        tx: &mut DbTransaction,
    ) -> DbResult<Vec<(String, String)>> {
        SignerDao::expire_overdue_recipients_tx(self, tx).await
    }

    async fn initialize_state_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
    ) -> DbResult<()> {
        SignerDao::initialize_state_tx(self, tx, recipient_id).await
    }
    async fn mark_notified_tx(&self, tx: &mut DbTransaction, recipient_id: &str) -> DbResult<()> {
        SignerDao::mark_notified_tx(self, tx, recipient_id).await
    }
    async fn mark_open_tx(&self, tx: &mut DbTransaction, recipient_id: &str) -> DbResult<bool> {
        SignerDao::mark_open_tx(self, tx, recipient_id).await
    }
    async fn accept_consent_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
        consent_version: &str,
        consent_text: &str,
        consent_text_sha256: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> DbResult<bool> {
        SignerDao::accept_consent_tx(
            self,
            tx,
            recipient_id,
            consent_version,
            consent_text,
            consent_text_sha256,
            ip_address,
            user_agent,
        )
        .await
    }
    async fn complete_field_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
        field_id: &str,
        value: &Value,
    ) -> DbResult<bool> {
        SignerDao::complete_field_tx(self, tx, recipient_id, field_id, value).await
    }
    async fn required_fields_complete_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
    ) -> DbResult<bool> {
        SignerDao::required_fields_complete_tx(self, tx, recipient_id).await
    }
    async fn complete_recipient_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
    ) -> DbResult<bool> {
        SignerDao::complete_recipient_tx(self, tx, recipient_id).await
    }
    async fn decline_recipient_tx(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
    ) -> DbResult<bool> {
        SignerDao::decline_recipient_tx(self, tx, recipient_id).await
    }
    async fn envelope_ready_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<bool> {
        SignerDao::envelope_ready_tx(self, tx, signature_request_id).await
    }
    async fn revoke_request_access_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<usize> {
        SignerDao::revoke_request_access_tx(self, tx, signature_request_id).await
    }
    async fn append_evidence_tx(
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
        SignerDao::append_evidence_tx(
            self,
            tx,
            signature_request_id,
            recipient_id,
            event_type,
            actor_id,
            correlation_id,
            causation_id,
            evidence,
        )
        .await
    }
}

pub struct SignerService<R> {
    repository: R,
    codec: SignerAccessTokenCodec,
    runtime: ServiceRuntime,
}

impl<R: SignerRepository> SignerService<R> {
    pub fn new(
        repository: R,
        codec: SignerAccessTokenCodec,
        infrastructure: ServiceInfrastructure,
    ) -> Self {
        Self {
            repository,
            codec,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn session(
        &self,
        access_token: &str,
        context: &ServiceContext,
    ) -> Result<SignerSession, CoreServiceError> {
        const OP: &str = "signer.session";
        let decision = authorize(
            &self.runtime,
            "signer",
            "signer.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = async {
            let access = self.resolve_access(access_token).await?;
            let recipient = self
                .repository
                .recipient(&access.recipient_id)
                .await?
                .ok_or_else(|| {
                    CoreServiceError::business(
                        "SIGNER_ACCESS_INVALID",
                        "Signer recipient not found.",
                    )
                })?;
            let state = self
                .repository
                .state(&access.recipient_id)
                .await?
                .ok_or_else(|| {
                    CoreServiceError::business("SIGNER_ACCESS_INVALID", "Signer state not found.")
                })?;
            let fields = self.repository.fields(&access.recipient_id).await?;
            let consented = self.repository.consent_exists(&access.recipient_id).await?;
            let is_turn = self.repository.is_turn(&access.recipient_id).await?;
            Ok(SignerSession {
                signature_request_id: access.signature_request_id,
                recipient,
                state,
                fields,
                consented,
                is_turn,
                expires_at: access.expires_at.to_rfc3339(),
            })
        }
        .await;
        audit_result(&self.runtime, "signer", OP, context, decision, &result).await?;
        result
    }

    pub async fn issue_access_transactional(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
        expires_at: Option<DateTime<Utc>>,
        context: &ServiceContext,
    ) -> Result<SignerAccessGrant, CoreServiceError> {
        const OP: &str = "signer.issueAccess";
        let decision = authorize(
            &self.runtime,
            "signer",
            "signer.access.issue",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            let expires_at =
                expires_at.unwrap_or_else(|| Utc::now() + Duration::days(DEFAULT_ACCESS_DAYS));
            let access = self
                .repository
                .issue_access_tx(tx, recipient_id, expires_at)
                .await?;
            self.repository
                .initialize_state_tx(tx, recipient_id)
                .await?;
            self.repository
                .append_evidence_tx(
                    tx,
                    &access.signature_request_id,
                    Some(recipient_id),
                    "recipient_access_issued",
                    context.actor.id.as_deref(),
                    Some(&context.correlation_id),
                    context.causation_id.as_deref(),
                    &json!({
                        "accessId": access.id,
                        "tokenVersion": access.token_version,
                        "expiresAt": access.expires_at.to_rfc3339(),
                    }),
                )
                .await?;
            let token = self.codec.mint(&access)?;
            Ok(SignerAccessGrant {
                access_id: access.id,
                recipient_id: access.recipient_id,
                token_version: access.token_version,
                expires_at: access.expires_at.to_rfc3339(),
                signing_url: self.codec.signing_url(&token),
                token,
            })
        }
        .await;
        audit_result(&self.runtime, "signer", OP, context, decision, &result).await?;
        result
    }

    /// The signing URL for a recipient's live grant, without rotating it.
    /// Reminders reuse the link the recipient already holds; a resend rotates
    /// through `issue_access_transactional` instead. Internal to issuance and
    /// reminder flows, which carry their own authorization and audit.
    /// Overdue recipients with their envelopes, for the sweep. Internal
    /// to the sweep, which carries the durable command's own auth.
    pub(crate) async fn expire_overdue_recipients_tx(
        &self,
        tx: &mut DbTransaction,
    ) -> DbResult<Vec<(String, String)>> {
        self.repository.expire_overdue_recipients_tx(tx).await
    }

    /// One evidence row for a swept recipient. Internal to the sweep, which
    /// carries the durable command's own auth.
    pub(crate) async fn append_sweep_evidence_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        recipient_id: &str,
        context: &ServiceContext,
    ) -> DbResult<()> {
        self.repository
            .append_evidence_tx(
                tx,
                signature_request_id,
                Some(recipient_id),
                "recipient_expired",
                context.actor.id.as_deref(),
                Some(&context.correlation_id),
                context.causation_id.as_deref(),
                &serde_json::json!({}),
            )
            .await
    }

    pub(crate) async fn active_signing_url(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
    ) -> Result<String, CoreServiceError> {
        let access = self
            .repository
            .active_access_for_recipient_tx(tx, recipient_id)
            .await?
            .ok_or_else(|| {
                CoreServiceError::business(
                    "SIGNER_ACCESS_EXPIRED",
                    "This signing link is no longer active; issue a fresh invitation.",
                )
            })?;
        let token = self.codec.mint(&access)?;
        Ok(self.codec.signing_url(&token))
    }

    /// Finalize reads for the orchestration layer: envelope readiness, the
    /// audit inputs, and the completion evidence rows. Internal to finalize,
    /// which carries the durable command's own auth.
    pub(crate) async fn envelope_ready_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<bool> {
        self.repository.envelope_ready_tx(tx, signature_request_id).await
    }

    pub(crate) async fn finalize_inputs_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
    ) -> DbResult<FinalizeInputs> {
        self.repository.finalize_inputs_tx(tx, signature_request_id).await
    }

    pub(crate) async fn append_finalize_evidence_tx(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        audit_media_id: &str,
        context: &ServiceContext,
    ) -> DbResult<()> {
        for (event_type, evidence) in [
            (
                "audit_artifact_created",
                json!({ "auditMediaId": audit_media_id }),
            ),
            ("document_completed", json!({})),
        ] {
            self.repository
                .append_evidence_tx(
                    tx,
                    signature_request_id,
                    None,
                    event_type,
                    context.actor.id.as_deref(),
                    Some(&context.correlation_id),
                    context.causation_id.as_deref(),
                    &evidence,
                )
                .await?;
        }
        Ok(())
    }

    pub async fn mark_notified_transactional(
        &self,
        tx: &mut DbTransaction,
        recipient_id: &str,
        signature_request_id: &str,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        self.repository.mark_notified_tx(tx, recipient_id).await?;
        self.repository
            .append_evidence_tx(
                tx,
                signature_request_id,
                Some(recipient_id),
                "invitation_queued",
                context.actor.id.as_deref(),
                Some(&context.correlation_id),
                context.causation_id.as_deref(),
                &json!({}),
            )
            .await?;
        Ok(())
    }

    pub async fn open_transactional(
        &self,
        tx: &mut DbTransaction,
        request: &OpenSignerRequest,
        context: &ServiceContext,
    ) -> Result<SignerActionResult, CoreServiceError> {
        const OP: &str = "signer.open";
        let decision = authorize(
            &self.runtime,
            "signer",
            "signer.act",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            let access = self.resolve_access(&request.access_token).await?;
            validate_recipient_binding(&request.recipient_id, &access.recipient_id)?;
            validate_actor_binding(context, &access.recipient_id)?;
            let state = self.current_state(&access.recipient_id).await?;
            if state.state.is_terminal() {
                return Err(CoreServiceError::business(
                    "SIGNER_ALREADY_TERMINAL",
                    "This signer can no longer open an active signing session.",
                ));
            }
            let first_open = self
                .repository
                .mark_open_tx(tx, &access.recipient_id)
                .await?;
            if first_open {
                self.repository
                    .append_evidence_tx(
                        tx,
                        &access.signature_request_id,
                        Some(&access.recipient_id),
                        "recipient_opened",
                        context.actor.id.as_deref(),
                        Some(&context.correlation_id),
                        context.causation_id.as_deref(),
                        &json!({}),
                    )
                    .await?;
            }
            let state = self.current_state(&access.recipient_id).await?;
            Ok(SignerActionResult {
                signature_request_id: access.signature_request_id,
                recipient_id: access.recipient_id,
                state: state.state,
                envelope_ready_to_finalize: false,
            })
        }
        .await;
        audit_result(&self.runtime, "signer", OP, context, decision, &result).await?;
        result
    }

    pub async fn accept_consent_transactional(
        &self,
        tx: &mut DbTransaction,
        request: &AcceptSignerConsentRequest,
        context: &ServiceContext,
    ) -> Result<SignerActionResult, CoreServiceError> {
        const OP: &str = "signer.acceptConsent";
        let decision = authorize(
            &self.runtime,
            "signer",
            "signer.act",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            let access = self.resolve_access(&request.access_token).await?;
            validate_recipient_binding(&request.recipient_id, &access.recipient_id)?;
            validate_actor_binding(context, &access.recipient_id)?;
            if request.consent_version.trim().is_empty()
                || request.consent_text.trim().is_empty()
                || request.consent_text_sha256.len() != 64
                || !request
                    .consent_text_sha256
                    .chars()
                    .all(|value| value.is_ascii_hexdigit() && !value.is_ascii_uppercase())
            {
                return Err(CoreServiceError::business(
                    "SIGNER_CONSENT_INVALID",
                    "Consent version, exact text and lower-case SHA-256 are required.",
                ));
            }
            let inserted = self
                .repository
                .accept_consent_tx(
                    tx,
                    &access.recipient_id,
                    request.consent_version.trim(),
                    request.consent_text.trim(),
                    &request.consent_text_sha256,
                    request.ip_address.as_deref(),
                    request.user_agent.as_deref(),
                )
                .await?;
            if inserted {
                self.repository
                    .append_evidence_tx(
                        tx,
                        &access.signature_request_id,
                        Some(&access.recipient_id),
                        "consent_accepted",
                        context.actor.id.as_deref(),
                        Some(&context.correlation_id),
                        context.causation_id.as_deref(),
                        &json!({
                            "consentVersion": request.consent_version,
                            "consentTextSha256": request.consent_text_sha256,
                        }),
                    )
                    .await?;
            }
            let state = self.current_state(&access.recipient_id).await?;
            Ok(SignerActionResult {
                signature_request_id: access.signature_request_id,
                recipient_id: access.recipient_id,
                state: state.state,
                envelope_ready_to_finalize: false,
            })
        }
        .await;
        audit_result(&self.runtime, "signer", OP, context, decision, &result).await?;
        result
    }

    pub async fn complete_field_transactional(
        &self,
        tx: &mut DbTransaction,
        request: &CompleteSignatureFieldRequest,
        context: &ServiceContext,
    ) -> Result<SignerActionResult, CoreServiceError> {
        const OP: &str = "signer.completeField";
        let decision = authorize(
            &self.runtime,
            "signer",
            "signer.act",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            let access = self.resolve_access(&request.access_token).await?;
            validate_recipient_binding(&request.recipient_id, &access.recipient_id)?;
            validate_actor_binding(context, &access.recipient_id)?;
            let state = self.current_state(&access.recipient_id).await?;
            if state.state.is_terminal() {
                return Err(CoreServiceError::business(
                    "SIGNER_ALREADY_TERMINAL",
                    "This signer can no longer change fields.",
                ));
            }
            if !self.repository.consent_exists(&access.recipient_id).await? {
                return Err(CoreServiceError::business(
                    "SIGNER_CONSENT_REQUIRED",
                    "Consent must be accepted before completing fields.",
                ));
            }
            if !self.repository.is_turn(&access.recipient_id).await? {
                return Err(CoreServiceError::business(
                    "SIGNER_NOT_YOUR_TURN",
                    "An earlier signing step must complete first.",
                ));
            }
            if !self
                .repository
                .complete_field_tx(tx, &access.recipient_id, &request.field_id, &request.value)
                .await?
            {
                return Err(CoreServiceError::business(
                    "SIGNER_FIELD_NOT_OWNED",
                    "The field does not belong to this signer.",
                ));
            }
            self.repository
                .append_evidence_tx(
                    tx,
                    &access.signature_request_id,
                    Some(&access.recipient_id),
                    "field_completed",
                    context.actor.id.as_deref(),
                    Some(&context.correlation_id),
                    context.causation_id.as_deref(),
                    &json!({ "fieldId": request.field_id }),
                )
                .await?;
            Ok(SignerActionResult {
                signature_request_id: access.signature_request_id,
                recipient_id: access.recipient_id,
                state: state.state,
                envelope_ready_to_finalize: false,
            })
        }
        .await;
        audit_result(&self.runtime, "signer", OP, context, decision, &result).await?;
        result
    }

    pub async fn complete_transactional(
        &self,
        tx: &mut DbTransaction,
        request: &CompleteSignerRequest,
        context: &ServiceContext,
    ) -> Result<SignerActionResult, CoreServiceError> {
        const OP: &str = "signer.complete";
        let decision = authorize(
            &self.runtime,
            "signer",
            "signer.act",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            let access = self.resolve_access(&request.access_token).await?;
            validate_recipient_binding(&request.recipient_id, &access.recipient_id)?;
            validate_actor_binding(context, &access.recipient_id)?;
            let state = self.current_state(&access.recipient_id).await?;
            if state.state == SignerState::Completed {
                let envelope_ready_to_finalize = self
                    .repository
                    .envelope_ready_tx(tx, &access.signature_request_id)
                    .await?;
                return Ok(SignerActionResult {
                    signature_request_id: access.signature_request_id,
                    recipient_id: access.recipient_id,
                    state: SignerState::Completed,
                    envelope_ready_to_finalize,
                });
            }
            if state.state.is_terminal() {
                return Err(CoreServiceError::business(
                    "SIGNER_ALREADY_TERMINAL",
                    "This signer can no longer complete the document.",
                ));
            }
            if !self.repository.consent_exists(&access.recipient_id).await? {
                return Err(CoreServiceError::business(
                    "SIGNER_CONSENT_REQUIRED",
                    "Consent must be accepted before signing.",
                ));
            }
            if !self.repository.is_turn(&access.recipient_id).await? {
                return Err(CoreServiceError::business(
                    "SIGNER_NOT_YOUR_TURN",
                    "An earlier signing step must complete first.",
                ));
            }
            if !self
                .repository
                .required_fields_complete_tx(tx, &access.recipient_id)
                .await?
            {
                return Err(CoreServiceError::business(
                    "SIGNER_REQUIRED_FIELDS_INCOMPLETE",
                    "All required fields must be completed before signing.",
                ));
            }
            if !self
                .repository
                .complete_recipient_tx(tx, &access.recipient_id)
                .await?
            {
                return Err(CoreServiceError::business(
                    "SIGNER_ALREADY_TERMINAL",
                    "This signer can no longer complete the document.",
                ));
            }
            self.repository
                .append_evidence_tx(
                    tx,
                    &access.signature_request_id,
                    Some(&access.recipient_id),
                    "recipient_completed",
                    context.actor.id.as_deref(),
                    Some(&context.correlation_id),
                    context.causation_id.as_deref(),
                    &json!({}),
                )
                .await?;
            let ready = self
                .repository
                .envelope_ready_tx(tx, &access.signature_request_id)
                .await?;
            Ok(SignerActionResult {
                signature_request_id: access.signature_request_id,
                recipient_id: access.recipient_id,
                state: SignerState::Completed,
                envelope_ready_to_finalize: ready,
            })
        }
        .await;
        audit_result(&self.runtime, "signer", OP, context, decision, &result).await?;
        result
    }

    pub async fn decline_transactional(
        &self,
        tx: &mut DbTransaction,
        request: &DeclineSignerRequest,
        context: &ServiceContext,
    ) -> Result<SignerActionResult, CoreServiceError> {
        const OP: &str = "signer.decline";
        let decision = authorize(
            &self.runtime,
            "signer",
            "signer.act",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            let access = self.resolve_access(&request.access_token).await?;
            validate_recipient_binding(&request.recipient_id, &access.recipient_id)?;
            validate_actor_binding(context, &access.recipient_id)?;
            if !self
                .repository
                .decline_recipient_tx(tx, &access.recipient_id)
                .await?
            {
                return Err(CoreServiceError::business(
                    "SIGNER_ALREADY_TERMINAL",
                    "This signer can no longer decline the document.",
                ));
            }
            self.repository
                .append_evidence_tx(
                    tx,
                    &access.signature_request_id,
                    Some(&access.recipient_id),
                    "recipient_declined",
                    context.actor.id.as_deref(),
                    Some(&context.correlation_id),
                    context.causation_id.as_deref(),
                    &json!({ "reason": request.reason }),
                )
                .await?;
            Ok(SignerActionResult {
                signature_request_id: access.signature_request_id,
                recipient_id: access.recipient_id,
                state: SignerState::Declined,
                envelope_ready_to_finalize: false,
            })
        }
        .await;
        audit_result(&self.runtime, "signer", OP, context, decision, &result).await?;
        result
    }

    pub(crate) async fn revoke_request_access_transactional(
        &self,
        tx: &mut DbTransaction,
        signature_request_id: &str,
        context: &ServiceContext,
    ) -> Result<usize, CoreServiceError> {
        let revoked = self
            .repository
            .revoke_request_access_tx(tx, signature_request_id)
            .await?;
        if revoked > 0 {
            self.repository
                .append_evidence_tx(
                    tx,
                    signature_request_id,
                    None,
                    "recipient_access_revoked",
                    context.actor.id.as_deref(),
                    Some(&context.correlation_id),
                    context.causation_id.as_deref(),
                    &json!({ "revokedCount": revoked }),
                )
                .await?;
        }
        Ok(revoked)
    }

    async fn resolve_access(&self, token: &str) -> Result<SignerAccessRecord, CoreServiceError> {
        let claims = self.codec.verify(token)?;
        if claims.exp <= Utc::now().timestamp() {
            return Err(CoreServiceError::business(
                "SIGNER_ACCESS_EXPIRED",
                "This signing link has expired.",
            ));
        }
        let access = self
            .repository
            .access(&claims.access_id)
            .await?
            .ok_or_else(|| {
                CoreServiceError::business("SIGNER_ACCESS_INVALID", "Signer link is invalid.")
            })?;
        if access.recipient_id != claims.recipient_id
            || access.token_version != claims.token_version
            || access.expires_at.timestamp() != claims.exp
        {
            return Err(CoreServiceError::business(
                "SIGNER_ACCESS_INVALID",
                "Signer link is no longer current.",
            ));
        }
        if access.revoked_at.is_some() {
            return Err(CoreServiceError::business(
                "SIGNER_ACCESS_REVOKED",
                "This signing link has been revoked.",
            ));
        }
        if access.expires_at <= Utc::now() {
            return Err(CoreServiceError::business(
                "SIGNER_ACCESS_EXPIRED",
                "This signing link has expired.",
            ));
        }
        Ok(access)
    }

    pub(crate) async fn current_state(
        &self,
        recipient_id: &str,
    ) -> Result<SignerRecipientState, CoreServiceError> {        self.repository.state(recipient_id).await?.ok_or_else(|| {
            CoreServiceError::business("SIGNER_ACCESS_INVALID", "Signer state not found.")
        })
    }
}

fn validate_recipient_binding(
    requested_recipient_id: &str,
    token_recipient_id: &str,
) -> Result<(), CoreServiceError> {
    if requested_recipient_id == token_recipient_id {
        Ok(())
    } else {
        Err(CoreServiceError::business(
            "SIGNER_ACCESS_INVALID",
            "The requested recipient does not match the signing capability.",
        ))
    }
}

fn validate_actor_binding(
    context: &ServiceContext,
    recipient_id: &str,
) -> Result<(), CoreServiceError> {
    if context.actor.id.as_deref() == Some(DOCSIGN_EDGE_ACTOR) || context.principal.is_some() {
        return Ok(());
    }
    let expected = format!("signature-recipient:{recipient_id}");
    if context.actor.id.as_deref() == Some(expected.as_str()) {
        Ok(())
    } else {
        Err(CoreServiceError::business(
            "SIGNER_ACCESS_INVALID",
            "Signer actor does not match the signing capability.",
        ))
    }
}

fn capability(
    name: &str,
    kind: OperationKind,
    description: &str,
    authorization: &str,
    execution: ServiceExecutionPolicy,
) -> ServiceCapability {
    ServiceCapability {
        name: name.into(),
        kind,
        description: description.into(),
        authorization: authorization.into(),
        idempotent: true,
        execution,
    }
}

#[async_trait]
impl<R: SignerRepository + 'static> AbstractService for SignerService<R> {
    fn descriptor(&self) -> ServiceDescriptor {
        ServiceDescriptor {
            domain: "signer".into(),
            version: "1".into(),
            description: "Recipient signing execution, consent and evidence service".into(),
            capabilities: vec![
                capability(
                    "signer.session",
                    OperationKind::Query,
                    "Resolve one signer-scoped signing session.",
                    "signer.read",
                    ServiceExecutionPolicy::inline(),
                ),
                capability(
                    "signer.open",
                    OperationKind::Command,
                    "Record the first signer open as durable evidence.",
                    "signer.act",
                    ServiceExecutionPolicy::ordered("recipientId"),
                ),
                capability(
                    "signer.acceptConsent",
                    OperationKind::Command,
                    "Persist exact write-once signer consent.",
                    "signer.act",
                    ServiceExecutionPolicy::ordered("recipientId"),
                ),
                capability(
                    "signer.completeField",
                    OperationKind::Command,
                    "Complete one recipient-owned signing field.",
                    "signer.act",
                    ServiceExecutionPolicy::ordered("recipientId"),
                ),
                capability(
                    "signer.complete",
                    OperationKind::Command,
                    "Complete one recipient's required signing actions.",
                    "signer.act",
                    ServiceExecutionPolicy::ordered("recipientId"),
                ),
                capability(
                    "signer.decline",
                    OperationKind::Command,
                    "Decline one signing request as the bound recipient.",
                    "signer.act",
                    ServiceExecutionPolicy::ordered("recipientId"),
                ),
            ],
            dependencies: vec!["signature".into()],
            invariants: vec![
                "A signer may mutate only fields owned by its immutable recipient id.".into(),
                "Consent is write-once exact evidence.".into(),
                "Sequential signing steps are enforced server-side.".into(),
                "Terminal recipient states never reopen.".into(),
            ],
        }
    }

    async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        context: &ServiceContext,
    ) -> Result<Value, ServiceDispatchError> {
        match envelope.operation.as_str() {
            "signer.session" => {
                let token = envelope
                    .payload
                    .get("accessToken")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| ServiceDispatchError::InvalidPayload {
                        domain: envelope.domain.clone(),
                        operation: envelope.operation.clone(),
                        message: "accessToken is required.".into(),
                    })?;
                serde_json::to_value(self.session(token, context).await.map_err(service_error)?)
                    .map_err(serialization_error)
            }
            "signer.open"
            | "signer.acceptConsent"
            | "signer.completeField"
            | "signer.complete"
            | "signer.decline" => Err(ServiceDispatchError::business(
                "DURABLE_COMMAND_REQUIRED",
                format!(
                    "{} must enter through the durable command dispatcher.",
                    envelope.operation
                ),
                false,
            )),
            operation => Err(ServiceDispatchError::UnknownOperation {
                domain: "signer".into(),
                operation: operation.into(),
            }),
        }
    }
}

fn service_error(error: CoreServiceError) -> ServiceDispatchError {
    match error {
        CoreServiceError::Business { code, message } => {
            ServiceDispatchError::business(code, message, false)
        }
        CoreServiceError::Database(error) => {
            ServiceDispatchError::infrastructure("DATABASE", error.to_string(), error.retryable)
        }
        CoreServiceError::Runtime(error) => {
            ServiceDispatchError::infrastructure("SERVICE_RUNTIME", error.to_string(), true)
        }
    }
}

fn serialization_error(error: serde_json::Error) -> ServiceDispatchError {
    ServiceDispatchError::infrastructure("SERVICE_SERIALIZATION_FAILED", error.to_string(), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signer_access_tokens_are_tamper_evident() {
        let codec = SignerAccessTokenCodec::for_test(
            "0123456789abcdef0123456789abcdef",
            "https://example.test",
        );
        let record = SignerAccessRecord {
            id: "11111111-1111-4111-8111-111111111111".into(),
            recipient_id: "22222222-2222-4222-8222-222222222222".into(),
            token_version: 3,
            expires_at: Utc::now() + Duration::hours(1),
            revoked_at: None,
            signature_request_id: "33333333-3333-4333-8333-333333333333".into(),
        };
        let token = codec.mint(&record).unwrap();
        let claims = codec.verify(&token).unwrap();
        assert_eq!(claims.recipient_id, record.recipient_id);
        assert!(codec.signing_url(&token).contains("/sign/"));

        let mut tampered = token.clone();
        tampered.push('x');
        assert!(codec.verify(&tampered).is_err());
    }

    #[test]
    fn request_recipient_must_match_the_signed_capability() {
        assert!(validate_recipient_binding("recipient-1", "recipient-1").is_ok());
        let error = validate_recipient_binding("recipient-2", "recipient-1").unwrap_err();
        assert_eq!(error.code(), "SIGNER_ACCESS_INVALID");
    }
}
