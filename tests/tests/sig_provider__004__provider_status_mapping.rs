//! SIG.PROVIDER — provider status mapping (TST-SIG-PROVIDER-004).
//!
//! Contract: the provider's vocabulary maps onto the canonical lifecycle exactly once, and nothing else. BoldSign's
//! `"InProgress"` is `sent`, `"Revoked"` is `voided`, `"Draft"` is `requested`; any string outside that map is
//! `error` — never a silent neutral guess, never a panic. This function is what `SignatureService::send` and
//! `refresh_status` use to translate the provider's answer, so a regression here re-colors every request.
//!
//! Level: L1 Component — the production mapping the `BoldSignSignatureProvider::map_status` trait method
//! delegates to (`middle/apis/src/boldsign/provider.rs:108`).
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sig_provider__004__provider_status_mapping

use model::SignatureRequestStatus;

#[test]
#[allow(non_snake_case)] // the canonical taxonomy name is part of the contract
fn sig_provider_004__provider_status_mapping() {
    use apis::boldsign::map_status;

    // The complete provider vocabulary, mapped exactly once.
    assert_eq!(map_status("InProgress"), SignatureRequestStatus::Sent);
    assert_eq!(map_status("Completed"), SignatureRequestStatus::Completed);
    assert_eq!(map_status("Declined"), SignatureRequestStatus::Declined);
    assert_eq!(map_status("Expired"), SignatureRequestStatus::Expired);
    assert_eq!(map_status("Revoked"), SignatureRequestStatus::Voided);
    assert_eq!(map_status("Draft"), SignatureRequestStatus::Requested);
    assert_eq!(map_status("Scheduled"), SignatureRequestStatus::Requested);

    // An unknown provider status is an explicit error state, never a neutral or a success.
    assert_eq!(map_status("SomethingNew"), SignatureRequestStatus::Error);
    assert_eq!(map_status(""), SignatureRequestStatus::Error);
    assert_eq!(map_status("completed"), SignatureRequestStatus::Error);

    // The error arm is terminal for activity: no active request ever maps out of this table silently. The mapped
    // variants are all real lifecycle states.
    for provider_status in [
        "InProgress",
        "Completed",
        "Declined",
        "Expired",
        "Revoked",
        "Draft",
        "Scheduled",
    ] {
        let mapped = map_status(provider_status);
        assert_ne!(
            mapped,
            SignatureRequestStatus::Error,
            "a documented provider status must never degrade to the catch-all error state"
        );
    }
}
