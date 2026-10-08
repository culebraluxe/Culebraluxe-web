//! Fake external-provider adapters.
//!
//! An external provider is the one collaborator a test must never reach for real: a live send is a legal event, a
//! live signature costs money, and a live webhook is someone else's system. The harness therefore ships *fakes only* —
//! adapters that implement the production port, record what they were asked, and answer from a script. Nothing in
//! this module opens a socket, reads a credential, or names a provider endpoint, and each fake answers
//! [`SignatureProvider::name`] with a `fake-` prefix so a log makes plain which side answered.
//!
//! The fakes implement `services::SignatureProvider`, the same trait the BoldSign adapter implements, so a service
//! test is wired to the real boundary and substituted at the adapter seam production itself uses.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use model::{
    SignatureArtifactDownload, SignatureProviderActionResult, SignatureProviderEvent,
    SignatureProviderSendRequest, SignatureProviderSendResult, SignatureProviderStatusResult,
    SignatureRequestStatus, SignatureWebhookVerification,
};
use services::SignatureProvider;

/// One call the code under test made against the fake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderCall {
    Send(SignatureProviderSendRequest),
    Status(String),
    Cancel(String),
    VerifyWebhook {
        raw_payload: String,
        signature: String,
    },
    DownloadArtifact(String),
    DownloadAuditTrail(String),
}

/// A deterministic implementation of the signature provider port.
///
/// It never performs I/O. `send` answers the configured result, `status` the configured status, and every call is
/// recorded so a test can assert on the interaction as well as the outcome.
pub struct FakeSignatureProvider {
    name: &'static str,
    send: SignatureProviderSendResult,
    status: SignatureRequestStatus,
    cancel_ok: bool,
    artifact: SignatureArtifactDownload,
    webhook: SignatureWebhookVerification,
    fail: Option<String>,
    calls: Arc<Mutex<Vec<ProviderCall>>>,
    count: Arc<AtomicU64>,
}

impl FakeSignatureProvider {
    /// A fake that accepts everything: `send` and `cancel` succeed, status is `Sent`.
    pub fn accepting() -> Self {
        Self {
            name: "fake-signature",
            send: SignatureProviderSendResult {
                ok: true,
                provider_status: "sent".into(),
                error: None,
            },
            status: SignatureRequestStatus::Sent,
            cancel_ok: true,
            artifact: SignatureArtifactDownload {
                bytes: b"fake signed artifact".to_vec(),
                filename: "signed.pdf".into(),
                mime_type: "application/pdf".into(),
            },
            webhook: SignatureWebhookVerification {
                event: SignatureProviderEvent::Sent,
                signature_request_id: "fake-request".into(),
            },
            fail: None,
            calls: Arc::new(Mutex::new(Vec::new())),
            count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Whether this fake's `cancel` succeeds.
    pub fn with_cancel_ok(mut self, cancel_ok: bool) -> Self {
        self.cancel_ok = cancel_ok;
        self
    }

    /// A provider whose configured artifact payload is `bytes`/`filename`/`mime_type`.
    pub fn with_artifact(mut self, bytes: Vec<u8>, filename: &str, mime_type: &str) -> Self {
        self.artifact = SignatureArtifactDownload {
            bytes,
            filename: filename.into(),
            mime_type: mime_type.into(),
        };
        self
    }

    /// The status this fake answers to `status` calls.
    pub fn with_status(mut self, status: SignatureRequestStatus) -> Self {
        self.status = status;
        self
    }

    /// Every call then fails at the port boundary with `error` — the provider-side fault mapping under test needs
    /// the `Err` arm, not the `ok: false` arm.
    pub fn failing_calls(error: impl Into<String>) -> Self {
        let mut fake = Self::accepting();
        fake.fail = Some(error.into());
        fake
    }

    /// The event and request id this fake answers to `verify_webhook` calls.
    pub fn with_webhook(
        mut self,
        event: SignatureProviderEvent,
        signature_request_id: &str,
    ) -> Self {
        self.webhook = SignatureWebhookVerification {
            event,
            signature_request_id: signature_request_id.to_owned(),
        };
        self
    }

    /// A fake whose `send` fails with `error`.
    pub fn failing(error: impl Into<String>) -> Self {
        let mut fake = Self::accepting();
        fake.send = SignatureProviderSendResult {
            ok: false,
            provider_status: "error".into(),
            error: Some(error.into()),
        };
        fake
    }

    /// The name this fake reports (always `fake-…`).
    pub fn provider_name(&self) -> &'static str {
        self.name
    }

    /// Whether this adapter touches a live provider. Always false for a fake; the method exists so a production
    /// wiring check can be written against the same question.
    pub const fn is_fake(&self) -> bool {
        true
    }

    /// Every call recorded so far.
    pub fn calls(&self) -> Vec<ProviderCall> {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// How many calls were recorded.
    pub fn call_count(&self) -> u64 {
        self.count.load(Ordering::SeqCst)
    }

    /// The configured status result.
    pub fn status_result(&self) -> SignatureRequestStatus {
        self.status
    }

    fn record(&self, call: ProviderCall) {
        self.count.fetch_add(1, Ordering::SeqCst);
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(call);
    }
}

impl Default for FakeSignatureProvider {
    fn default() -> Self {
        Self::accepting()
    }
}

#[async_trait]
impl SignatureProvider for FakeSignatureProvider {
    fn name(&self) -> &'static str {
        self.name
    }

    fn map_status(&self, provider_status: &str) -> SignatureRequestStatus {
        match provider_status.trim().to_ascii_lowercase().as_str() {
            "sent" => SignatureRequestStatus::Sent,
            "viewed" => SignatureRequestStatus::Viewed,
            "signed" => SignatureRequestStatus::Signed,
            "completed" => SignatureRequestStatus::Completed,
            "declined" => SignatureRequestStatus::Declined,
            "voided" => SignatureRequestStatus::Voided,
            "expired" => SignatureRequestStatus::Expired,
            _ => SignatureRequestStatus::Error,
        }
    }

    async fn send(
        &self,
        request: SignatureProviderSendRequest,
    ) -> Result<SignatureProviderSendResult, String> {
        self.record(ProviderCall::Send(request));
        if let Some(error) = &self.fail {
            return Err(error.clone());
        }
        Ok(self.send.clone())
    }

    async fn status(
        &self,
        signature_request_id: &str,
    ) -> Result<SignatureProviderStatusResult, String> {
        self.record(ProviderCall::Status(signature_request_id.to_owned()));
        if let Some(error) = &self.fail {
            return Err(error.clone());
        }
        Ok(SignatureProviderStatusResult {
            status: self.status,
        })
    }

    async fn cancel(
        &self,
        signature_request_id: &str,
    ) -> Result<SignatureProviderActionResult, String> {
        self.record(ProviderCall::Cancel(signature_request_id.to_owned()));
        if let Some(error) = &self.fail {
            return Err(error.clone());
        }
        Ok(SignatureProviderActionResult {
            ok: self.cancel_ok,
            error: (!self.cancel_ok).then(|| "fake cancel refusal".into()),
        })
    }

    async fn verify_webhook(
        &self,
        raw_payload: &str,
        signature: &str,
    ) -> Result<SignatureWebhookVerification, String> {
        self.record(ProviderCall::VerifyWebhook {
            raw_payload: raw_payload.to_owned(),
            signature: signature.to_owned(),
        });
        if let Some(error) = &self.fail {
            return Err(error.clone());
        }
        Ok(self.webhook.clone())
    }

    async fn download_signed_artifact(
        &self,
        signature_request_id: &str,
    ) -> Result<SignatureArtifactDownload, String> {
        self.record(ProviderCall::DownloadArtifact(
            signature_request_id.to_owned(),
        ));
        if let Some(error) = &self.fail {
            return Err(error.clone());
        }
        Ok(self.artifact.clone())
    }

    async fn download_audit_trail(
        &self,
        signature_request_id: &str,
    ) -> Result<Option<SignatureArtifactDownload>, String> {
        self.record(ProviderCall::DownloadAuditTrail(
            signature_request_id.to_owned(),
        ));
        if let Some(error) = &self.fail {
            return Err(error.clone());
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> SignatureProviderSendRequest {
        SignatureProviderSendRequest {
            signature_request_id: "req-1".into(),
            transaction_document_id: "doc-1".into(),
            recipients: Vec::new(),
            message: None,
            signature_role: None,
            signature_slot_id: None,
            completion_recipient_emails: Vec::new(),
        }
    }

    #[tokio::test]
    async fn a_fake_is_never_live_and_always_prefixed() {
        let fake = FakeSignatureProvider::accepting();
        assert!(fake.is_fake());
        assert!(fake.provider_name().starts_with("fake-"));
        assert_eq!(fake.call_count(), 0, "constructing a fake makes no call");
    }

    #[tokio::test]
    async fn calls_are_recorded_and_answered_from_the_configuration() {
        let fake = FakeSignatureProvider::accepting();
        let result = fake.send(request()).await.unwrap();
        assert!(result.ok);
        assert_eq!(
            fake.status("req-1").await.unwrap().status,
            SignatureRequestStatus::Sent
        );
        assert_eq!(fake.call_count(), 2);
        assert_eq!(fake.calls().len(), 2);
        assert!(matches!(fake.calls()[0], ProviderCall::Send(_)));
        assert_eq!(fake.calls()[1], ProviderCall::Status("req-1".into()));
    }

    #[tokio::test]
    async fn a_failing_fake_reports_its_error_without_a_network() {
        let fake = FakeSignatureProvider::failing("provider said no");
        let result = fake.send(request()).await.unwrap();
        assert!(!result.ok);
        assert_eq!(result.error.as_deref(), Some("provider said no"));
    }

    #[test]
    fn status_mapping_is_total() {
        let fake = FakeSignatureProvider::accepting();
        assert_eq!(
            fake.map_status("completed"),
            SignatureRequestStatus::Completed
        );
        assert_eq!(fake.map_status("nonsense"), SignatureRequestStatus::Error);
    }
}
