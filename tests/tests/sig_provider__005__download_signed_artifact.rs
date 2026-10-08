//! SIG.PROVIDER — download signed artifact (TST-SIG-PROVIDER-005).
//!
//! Contract: reconciliation downloads the signed artifact from the provider and hands the exact bytes to the
//! canonical store — the store, never the provider nor the service, owns what persists. A download failure maps
//! to `SIGNATURE_ARTIFACT_DOWNLOAD_FAILED` and leaves no trace: the document is not signed and the store is
//! never asked to reconcile.
//!
//! Level: L1 Component — the real `SignatureService` over the in-memory store, with the provider faked at the
//! `services::SignatureProvider` seam production itself uses.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sig_provider__005__download_signed_artifact

use model::SignatureRequestStatus;
use test_harness::providers::FakeSignatureProvider;
use test_harness::signature::{user_context, SignatureHarness};

#[tokio::test]
#[allow(non_snake_case)] // the canonical taxonomy name is part of the contract
async fn sig_provider_005__download_signed_artifact() {
    let provider = FakeSignatureProvider::accepting();
    let harness = SignatureHarness::new(provider).with_request(
        "req-1",
        "doc-1",
        SignatureRequestStatus::Completed,
    );
    let service = harness.service();

    let command = service
        .reconcile_completed("evt-1", "req-1", &user_context("user-ada"))
        .await
        .expect("the artifact downloads and reconciles");

    assert_eq!(command.outcome, model::SignatureCommandOutcome::Success);
    assert!(harness.document_signed(), "the document is marked signed");
    assert_eq!(
        harness.reconciled_bytes(),
        vec![b"fake signed artifact".to_vec()],
        "the exact provider bytes reach the store"
    );
}

#[tokio::test]
async fn a_failed_download_leaves_the_document_unsigned() {
    let provider = FakeSignatureProvider::failing_calls("artifact store timeout");
    let harness = SignatureHarness::new(provider).with_request(
        "req-2",
        "doc-2",
        SignatureRequestStatus::Completed,
    );
    let service = harness.service();

    let error = service
        .reconcile_completed("evt-2", "req-2", &user_context("user-ada"))
        .await
        .expect_err("a download failure is an error");

    assert!(
        error
            .to_string()
            .contains("SIGNATURE_ARTIFACT_DOWNLOAD_FAILED"),
        "the fault maps to the artifact-download code, got: {error}"
    );
    assert!(!harness.document_signed());
    assert!(
        harness.reconciled_bytes().is_empty(),
        "nothing reaches the store when the download failed"
    );
    assert!(
        !harness
            .calls()
            .iter()
            .any(|call| call == "reconcile_completed"),
        "the store is never asked to reconcile a failed download"
    );
}
