//! SIG.PROVIDER — refresh (TST-SIG-PROVIDER-002).
//!
//! Contract: `refresh` re-observes the provider and converges the canonical request to the observed status —
//! exactly one `status` call against the provider, and the store follows the provider's word, not the other way
//! around. A provider fault must surface as an error and leave the stored status untouched; an impossible
//! observation (the provider claiming the request bounced back to `sent` from `completed`) must be rejected, not
//! applied.
//!
//! Level: L1 Component — the real `SignatureService` over the in-memory store, with the provider faked at the
//! `services::SignatureProvider` seam production itself uses.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sig_provider__002__refresh

use model::SignatureRequestStatus;
use test_harness::providers::FakeSignatureProvider;
use test_harness::signature::{user_context, SignatureHarness};

#[tokio::test]
#[allow(non_snake_case)] // the canonical taxonomy name is part of the contract
async fn sig_provider_002__refresh() {
    // Provider observes `viewed` for the request: refresh must converge the store to that.
    let provider = FakeSignatureProvider::accepting().with_status(SignatureRequestStatus::Viewed);
    let harness = SignatureHarness::new(provider).with_request(
        "req-1",
        "doc-1",
        SignatureRequestStatus::Sent,
    );
    let service = harness.service();

    let command = service
        .refresh_status("req-1", &user_context("user-ada"))
        .await
        .expect("refresh answers");

    assert!(!command.replayed);
    assert_eq!(
        harness.status("req-1"),
        Some(SignatureRequestStatus::Viewed),
        "the stored status follows the provider's observation"
    );

    // A second refresh to the same observation is a no-op transition, not an error.
    let again = service
        .refresh_status("req-1", &user_context("user-ada"))
        .await
        .expect("a repeated observation is fine");
    assert!(
        again.outcome == model::SignatureCommandOutcome::Success,
        "same-state refresh still succeeds"
    );

    let calls = harness.calls();
    assert!(
        calls.iter().filter(|call| *call == "apply_status").count() == 2,
        "each refresh applies the observed status exactly once: {calls:?}"
    );
}

#[tokio::test]
async fn refresh_is_refused_when_the_provider_faults_and_changes_nothing() {
    let provider = FakeSignatureProvider::failing_calls("provider unreachable");
    let harness = SignatureHarness::new(provider).with_request(
        "req-2",
        "doc-2",
        SignatureRequestStatus::Sent,
    );
    let service = harness.service();

    let error = service
        .refresh_status("req-2", &user_context("user-ada"))
        .await
        .expect_err("a provider fault is an error");

    assert!(
        error.to_string().contains("SIGNATURE_PROVIDER_FAILURE"),
        "the fault maps to the provider-failure code, got: {error}"
    );
    assert_eq!(
        harness.status("req-2"),
        Some(SignatureRequestStatus::Sent),
        "a failed refresh must not move the stored status"
    );
    assert!(
        !harness.calls().iter().any(|call| call == "apply_status"),
        "no status write is attempted on a provider fault"
    );
}

#[tokio::test]
async fn refresh_rejects_an_impossible_observation() {
    let provider = FakeSignatureProvider::accepting().with_status(SignatureRequestStatus::Sent);
    let harness = SignatureHarness::new(provider).with_request(
        "req-3",
        "doc-3",
        SignatureRequestStatus::Completed,
    );
    let service = harness.service();

    let result = service
        .refresh_status("req-3", &user_context("user-ada"))
        .await;

    assert!(
        result.is_err()
            || matches!(
                result.as_ref().map(|command| command.outcome),
                Ok(model::SignatureCommandOutcome::ValidationFailure)
            ),
        "a completed request observed as sent is rejected, not applied: {result:?}"
    );
    assert_eq!(
        harness.status("req-3"),
        Some(SignatureRequestStatus::Completed),
        "the stored status survives the refused observation"
    );
}
