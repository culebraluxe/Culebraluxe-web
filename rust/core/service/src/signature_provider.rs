use async_trait::async_trait;
use domain::{
    SignatureArtifactDownload, SignatureProviderActionResult, SignatureProviderSendRequest,
    SignatureProviderSendResult, SignatureProviderStatusResult, SignatureRequestStatus,
    SignatureWebhookVerification,
};

/// Provider-neutral port implemented by external signature adapters.
///
/// Canonical services own lifecycle truth. Providers own delivery, observation,
/// revocation, webhook trust, and artifact downloads.
#[async_trait]
pub trait SignatureProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn map_status(&self, provider_status: &str) -> SignatureRequestStatus;

    async fn send(
        &self,
        request: SignatureProviderSendRequest,
    ) -> Result<SignatureProviderSendResult, String>;

    async fn status(
        &self,
        signature_request_id: &str,
    ) -> Result<SignatureProviderStatusResult, String>;

    async fn cancel(
        &self,
        signature_request_id: &str,
    ) -> Result<SignatureProviderActionResult, String>;

    async fn verify_webhook(
        &self,
        raw_payload: &str,
        signature: &str,
    ) -> Result<SignatureWebhookVerification, String>;

    async fn download_signed_artifact(
        &self,
        signature_request_id: &str,
    ) -> Result<SignatureArtifactDownload, String>;

    async fn download_audit_trail(
        &self,
        signature_request_id: &str,
    ) -> Result<Option<SignatureArtifactDownload>, String>;
}
