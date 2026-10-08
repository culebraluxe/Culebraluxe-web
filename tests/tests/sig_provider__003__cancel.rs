//! SIG.PROVIDER — cancel (TST-SIG-PROVIDER-003).
//!
//! Contract: `cancel` revokes with the provider FIRST; only a proven revocation moves the canonical request to
//! `voided`. A provider refusal or fault must end the command without touching the store — the request stays
//! active. Once voided, the local store agrees (the provider is told before the database is written, never
//! after).
//!
//! Level: L1 Component — the real `SignatureService` over the in-memory store, with the provider faked at the
//! `services::SignatureProvider` seam production itself uses.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sig_provider__003__cancel

use model::{SignatureCommandOutcome, SignatureRequestStatus};
use test_harness::providers::FakeSignatureProvider;
use test_harness::signature::{user_context, SignatureHarness};

#[tokio::test]
#[allow(non_snake_case)] // the canonical taxonomy name is part of the contract
async fn sig_provider_003__cancel() {
    let provider = FakeSignatureProvider::accepting();
    let harness = SignatureHarness::new(provider).with_request(
        "req-1",
        "doc-1",
        SignatureRequestStatus::Sent,
    );
    let service = harness.service();

    let command = service
        .cancel("cmd-1", "req-1", &user_context("user-ada"))
        .await
        .expect("cancel answers");

    assert_eq!(command.outcome, SignatureCommandOutcome::Success);
    assert_eq!(
        harness.status("req-1"),
        Some(SignatureRequestStatus::Voided),
        "a proven provider revocation voids the request"
    );
    assert_eq!(
        harness
            .calls()
            .iter()
            .filter(|call| *call == "cancel")
            .count(),
        1,
        "the store is written exactly once per proven revocation"
    );
}

#[tokio::test]
async fn cancel_refused_by_the_provider_leaves_the_request_active() {
    let provider = FakeSignatureProvider::accepting().with_cancel_ok(false);
    let harness = SignatureHarness::new(provider).with_request(
        "req-2",
        "doc-2",
        SignatureRequestStatus::Sent,
    );
    let service = harness.service();

    let command = service
        .cancel("cmd-2", "req-2", &user_context("user-ada"))
        .await
        .expect("a refusal is a conflict outcome, not a panic");

    assert_eq!(command.outcome, SignatureCommandOutcome::Conflict);
    assert_eq!(
        harness.status("req-2"),
        Some(SignatureRequestStatus::Sent),
        "the request stays active when revocation cannot be proven"
    );
    assert!(
        !harness.calls().iter().any(|call| call == "cancel"),
        "the store is never written on a provider refusal"
    );
}

#[tokio::test]
async fn cancel_maps_a_provider_fault_and_changes_nothing() {
    let provider = FakeSignatureProvider::failing_calls("webhook provider down");
    let harness = SignatureHarness::new(provider).with_request(
        "req-3",
        "doc-3",
        SignatureRequestStatus::Viewed,
    );
    let service = harness.service();

    let error = service
        .cancel("cmd-3", "req-3", &user_context("user-ada"))
        .await
        .expect_err("a provider fault is an error");

    assert!(
        error.to_string().contains("SIGNATURE_PROVIDER_FAILURE"),
        "the fault maps to the provider-failure code, got: {error}"
    );
    assert_eq!(
        harness.status("req-3"),
        Some(SignatureRequestStatus::Viewed)
    );
    assert!(!harness.calls().iter().any(|call| call == "cancel"));
}
