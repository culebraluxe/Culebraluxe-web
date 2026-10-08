//! SIG.PROVIDER — provider error mapping (TST-SIG-PROVIDER-006).
//!
//! Contract: a failure at the provider port maps onto the canonical service error codes, and never leaks the
//! provider's raw message shape as a silent success. `send`, `status`, `cancel` share
//! `SIGNATURE_PROVIDER_FAILURE`; artifact and audit-trail downloads carry their own codes
//! (`SIGNATURE_ARTIFACT_DOWNLOAD_FAILED`, `SIGNATURE_AUDIT_DOWNLOAD_FAILED`). Each mapped failure leaves the
//! store untouched.
//!
//! Level: L1 Component — the real `SignatureService` over the in-memory store, with the provider faked at the
//! `services::SignatureProvider` seam production itself uses.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sig_provider__006__provider_error_mapping

use async_trait::async_trait;
use model::{
    SignatureArtifactDownload, SignatureProviderActionResult, SignatureProviderEvent,
    SignatureProviderSendRequest, SignatureProviderSendResult, SignatureProviderStatusResult,
    SignatureRequestStatus, SignatureWebhookVerification,
};
use services::SignatureProvider;
use test_harness::providers::FakeSignatureProvider;
use test_harness::signature::{user_context, SignatureHarness};

/// A provider whose sends, statuses and cancels succeed but whose audit-trail download faults — the one failure
/// the shared `failing_calls` fake cannot produce on its own.
struct AuditFaultingProvider(FakeSignatureProvider);

#[async_trait]
impl SignatureProvider for AuditFaultingProvider {
    fn name(&self) -> &'static str {
        self.0.provider_name()
    }

    fn map_status(&self, provider_status: &str) -> SignatureRequestStatus {
        self.0.map_status(provider_status)
    }

    async fn send(
        &self,
        request: SignatureProviderSendRequest,
    ) -> Result<SignatureProviderSendResult, String> {
        self.0.send(request).await
    }

    async fn status(
        &self,
        signature_request_id: &str,
    ) -> Result<SignatureProviderStatusResult, String> {
        self.0.status(signature_request_id).await
    }

    async fn cancel(
        &self,
        signature_request_id: &str,
    ) -> Result<SignatureProviderActionResult, String> {
        self.0.cancel(signature_request_id).await
    }

    async fn verify_webhook(
        &self,
        raw_payload: &str,
        signature: &str,
    ) -> Result<SignatureWebhookVerification, String> {
        self.0.verify_webhook(raw_payload, signature).await
    }

    async fn download_signed_artifact(
        &self,
        signature_request_id: &str,
    ) -> Result<SignatureArtifactDownload, String> {
        self.0.download_signed_artifact(signature_request_id).await
    }

    async fn download_audit_trail(
        &self,
        _signature_request_id: &str,
    ) -> Result<Option<SignatureArtifactDownload>, String> {
        Err("audit vault timeout".into())
    }
}

#[tokio::test]
#[allow(non_snake_case)] // the canonical taxonomy name is part of the contract
async fn sig_provider_006__provider_error_mapping() {
    // send / status / cancel share one mapping.
    for (call, expected) in [
        ("send", "SIGNATURE_PROVIDER_FAILURE"),
        ("status", "SIGNATURE_PROVIDER_FAILURE"),
        ("cancel", "SIGNATURE_PROVIDER_FAILURE"),
    ] {
        let provider = FakeSignatureProvider::failing_calls(format!("{call} transport error"));
        let mut seeded = SignatureHarness::new(provider);
        if call != "send" {
            seeded = seeded.with_request("req", "doc", SignatureRequestStatus::Sent);
        }
        let harness = seeded;
        let service = harness.service();

        let error = match call {
            "send" => service
                .send(
                    &model::SendSignatureRequest {
                        command_id: "cmd".into(),
                        transaction_document_id: "doc".into(),
                        recipients: vec![model::SignatureRecipient {
                            role: model::SignatureRecipientRole::Signer,
                            name: "Grace Example".into(),
                            email: "grace@example.test".into(),
                            order: 1,
                            execution_role: None,
                            execution_slot_id: None,
                        }],
                        message: None,
                        created_by_user_id: None,
                        execution_role: None,
                        execution_slot_id: None,
                        slot_recipient_email: None,
                        signature_role: None,
                        completion_recipient_emails: vec![],
                    },
                    &user_context("user-ada"),
                )
                .await
                .map_err(|error| error.to_string()),
            "status" => service
                .refresh_status("req", &user_context("user-ada"))
                .await
                .map_err(|error| error.to_string()),
            _ => service
                .cancel("cmd", "req", &user_context("user-ada"))
                .await
                .map_err(|error| error.to_string()),
        }
        .expect_err("the call must fail");

        assert!(
            error.contains(expected),
            "{call}: the provider failure maps to {expected}, got: {error}"
        );
        if call != "send" {
            assert_eq!(
                harness.status("req"),
                Some(SignatureRequestStatus::Sent),
                "{call}: a mapped failure never moved the request"
            );
        }
    }

    // Artifact download has its own code.
    let provider = FakeSignatureProvider::failing_calls("artifact 404");
    let harness = SignatureHarness::new(provider).with_request(
        "req-a",
        "doc-a",
        SignatureRequestStatus::Completed,
    );
    let service = harness.service();
    let error = service
        .reconcile_completed("evt-a", "req-a", &user_context("user-ada"))
        .await
        .expect_err("the download must fail");
    assert!(
        error
            .to_string()
            .contains("SIGNATURE_ARTIFACT_DOWNLOAD_FAILED"),
        "the download fault maps to the artifact code, got: {error}"
    );
    assert!(!harness.document_signed());

    // Audit-trail failure has its own code, and a successful artifact download does not rescue it.
    let harness = SignatureHarness::new(FakeSignatureProvider::accepting()).with_request(
        "req-b",
        "doc-b",
        SignatureRequestStatus::Completed,
    );
    // The harness store is repository-real; only the provider differs, swapped at the port the service takes.
    let service = web::signature::SignatureService::new(
        harness.clone(),
        std::sync::Arc::new(AuditFaultingProvider(FakeSignatureProvider::accepting())),
        test_harness::signature::infrastructure(),
    );
    let error = service
        .reconcile_completed("evt-b", "req-b", &user_context("user-ada"))
        .await
        .expect_err("the audit download must fail");
    assert!(
        error
            .to_string()
            .contains("SIGNATURE_AUDIT_DOWNLOAD_FAILED"),
        "the audit fault maps to its own code, got: {error}"
    );
    assert!(
        !harness.document_signed(),
        "a missing audit trail rolls back the signature"
    );
}
