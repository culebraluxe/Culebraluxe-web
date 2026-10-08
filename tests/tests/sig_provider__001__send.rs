//! SIG.PROVIDER — send (TST-SIG-PROVIDER-001).
//!
//! Contract: the FakeSignatureProvider correctly implements the SignatureProvider interface
//! and can be used to test signature send operations.
//!
//! This test verifies that the fake provider correctly records calls and returns configured responses.
//!
//! Level: L1 Component — the fake provider against deterministic infrastructure.
//!
//! The negative case is the one that matters: if the fake provider doesn't correctly record calls
//! or return configured responses, tests using it would be unreliable.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sig_provider__001__send

use test_harness::providers::{FakeSignatureProvider, ProviderCall};
use model::{SignatureProviderSendRequest, SignatureProviderSendResult, SignatureRequestStatus, SignatureArtifactDownload, SignatureProviderEvent};
use services::SignatureProvider;

const HARNESS: &str = "FakeSignatureProvider/L1 Component";

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SIG-PROVIDER-001)
async fn sig_provider_001__send() {
    // 1. A fake that accepts everything: send and cancel succeed, status is Sent.
    let fake = FakeSignatureProvider::accepting();
    assert!(fake.is_fake(), "{HARNESS}: fake provider must report is_fake() = true");
    assert!(fake.provider_name().starts_with("fake-"), "{HARNESS}: fake provider name must start with 'fake-'");
    assert_eq!(fake.call_count(), 0, "{HARNESS}: constructing a fake makes no call");

    // 2. Send succeeds and is recorded.
    let result = fake.send(request()).await.expect("send must succeed");
    assert!(result.ok, "{HARNESS}: accepting fake must return ok=true");
    assert_eq!(result.provider_status, "sent", "{HARNESS}: accepting fake must return provider_status 'sent'");
    assert_eq!(fake.call_count(), 1, "{HARNESS}: send must record one call");
    assert_eq!(fake.calls().len(), 1, "{HARNESS}: calls() must have one entry");
    assert!(matches!(fake.calls()[0], ProviderCall::Send(_)), "{HARNESS}: first call must be Send");

    // 3. Status returns configured status.
    let status_result = fake.status("req-1").await.expect("status must succeed");
    assert_eq!(status_result.status, SignatureRequestStatus::Sent, "{HARNESS}: status must be Sent");
    assert_eq!(fake.call_count(), 2, "{HARNESS}: status must record one call");
    assert_eq!(fake.calls()[1], ProviderCall::Status("req-1".into()), "{HARNESS}: second call must be Status");

    // 4. Cancel succeeds.
    let cancel_result = fake.cancel("req-1").await.expect("cancel must succeed");
    assert!(cancel_result.ok, "{HARNESS}: cancel must succeed");
    assert_eq!(fake.call_count(), 3, "{HARNESS}: cancel must record one call");

    // 5. A failing fake returns configured error.
    let failing_fake = FakeSignatureProvider::failing("provider said no");
    let result = failing_fake.send(request()).await.expect("send must succeed (operation succeeds, provider fails)");
    assert!(!result.ok, "{HARNESS}: failing fake must return ok=false");
    assert_eq!(result.error.as_deref(), Some("provider said no"), "{HARNESS}: failing fake must return configured error");
    assert_eq!(result.provider_status, "error", "{HARNESS}: failing fake must return provider_status 'error'");

    // 6. Status mapping is total.
    let fake2 = FakeSignatureProvider::accepting();
    assert_eq!(fake2.map_status("completed"), SignatureRequestStatus::Completed, "{HARNESS}: completed maps to Completed");
    assert_eq!(fake2.map_status("nonsense"), SignatureRequestStatus::Error, "{HARNESS}: unknown status maps to Error");

    // 7. Artifact download returns configured artifact.
    let artifact = fake.download_signed_artifact("req-1").await.expect("download must succeed");
    assert_eq!(artifact.filename, "signed.pdf", "{HARNESS}: artifact filename must match");
    assert_eq!(artifact.mime_type, "application/pdf", "{HARNESS}: artifact mime_type must match");
    assert_eq!(artifact.bytes, b"fake signed artifact", "{HARNESS}: artifact bytes must match");

    // 8. Audit trail download returns None for fake.
    let audit = fake.download_audit_trail("req-1").await.expect("download_audit_trail must succeed");
    assert!(audit.is_none(), "{HARNESS}: fake provider must return None for audit trail");

    // 9. Webhook verification returns configured event.
    let webhook = fake.verify_webhook("payload", "signature").await.expect("verify_webhook must succeed");
    assert_eq!(webhook.event, SignatureProviderEvent::Sent, "{HARNESS}: webhook must return Sent event");
    assert_eq!(webhook.signature_request_id, "fake-request", "{HARNESS}: webhook must return fake request id");

    // 10. All calls are recorded.
    assert_eq!(fake.call_count(), 6, "{HARNESS}: total calls must be 6 (send, status, cancel, download, audit, webhook)");
    assert_eq!(fake.calls().len(), 6, "{HARNESS}: calls() must have 6 entries");
}