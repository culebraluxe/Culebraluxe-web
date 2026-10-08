//! PROP.PROPERTY_BASED — status transitions (TST-PROP-PROPERTY-BASED-002).
//!
//! Contract: document and signature statuses move through a closed transition
//! table. Terminal states (`Voided`, `Superseded`, and the five signature
//! terminals) never transition out; a signature never moves backwards; every
//! state round-trips through its wire string.
//!
//! Level: L0 Pure — the executable boundary is `model::{vault, signature}`,
//! `const fn` tables, no I/O.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test prop_property_based__002__status_transitions

use model::{SignatureRequestStatus, TransactionDocumentState};
use proptest::prelude::*;

fn document_state() -> impl Strategy<Value = TransactionDocumentState> {
    use TransactionDocumentState as S;
    prop_oneof![
        Just(S::Draft),
        Just(S::Ready),
        Just(S::Sent),
        Just(S::Signed),
        Just(S::Voided),
        Just(S::Superseded),
    ]
}

fn signature_state() -> impl Strategy<Value = SignatureRequestStatus> {
    use SignatureRequestStatus as S;
    prop_oneof![
        Just(S::Requested),
        Just(S::Sent),
        Just(S::Viewed),
        Just(S::Signed),
        Just(S::Completed),
        Just(S::Declined),
        Just(S::Voided),
        Just(S::Expired),
        Just(S::Error),
    ]
}

/// The document table, restated as data so the property checks the code's
/// table against an independent reading of the requirement.
fn document_allowed(from: TransactionDocumentState) -> &'static [TransactionDocumentState] {
    use TransactionDocumentState as S;
    match from {
        S::Draft => &[S::Ready, S::Voided, S::Superseded],
        S::Ready => &[S::Sent, S::Voided, S::Superseded],
        S::Sent => &[S::Signed, S::Voided],
        S::Signed => &[S::Voided, S::Superseded],
        S::Voided | S::Superseded => &[],
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// For every pair of states: the transition answer matches the closed
    /// table, terminals never leave, and wire strings round-trip.
    #[test]
    #[allow(non_snake_case)]
    fn prop_property_based_002__status_transitions(
        from in document_state(),
        to in document_state(),
        sfrom in signature_state(),
        sto in signature_state(),
    ) {
        // Fixed positives: the documented happy paths transition.
        prop_assert!(TransactionDocumentState::Draft.can_transition_to(TransactionDocumentState::Ready));
        prop_assert!(TransactionDocumentState::Ready.can_transition_to(TransactionDocumentState::Sent));
        prop_assert!(TransactionDocumentState::Sent.can_transition_to(TransactionDocumentState::Signed));
        prop_assert!(SignatureRequestStatus::Requested.can_transition_to(SignatureRequestStatus::Sent));
        prop_assert!(SignatureRequestStatus::Viewed.can_transition_to(SignatureRequestStatus::Signed));
        prop_assert!(SignatureRequestStatus::Signed.can_transition_to(SignatureRequestStatus::Completed));
        // Fixed negatives: terminals and backwards moves refuse.
        prop_assert!(!TransactionDocumentState::Voided.can_transition_to(TransactionDocumentState::Draft));
        prop_assert!(!TransactionDocumentState::Superseded.can_transition_to(TransactionDocumentState::Ready));
        prop_assert!(!SignatureRequestStatus::Completed.can_transition_to(SignatureRequestStatus::Sent));
        prop_assert!(!SignatureRequestStatus::Declined.can_transition_to(SignatureRequestStatus::Viewed));
        prop_assert!(!SignatureRequestStatus::Voided.can_transition_to(SignatureRequestStatus::Requested));
        prop_assert!(!SignatureRequestStatus::Signed.can_transition_to(SignatureRequestStatus::Requested));

        // Property: the document table is exactly the closed table — every
        // listed target is allowed, everything else (including self) refuses.
        let allowed = document_allowed(from);
        prop_assert_eq!(
            from.can_transition_to(to),
            allowed.contains(&to),
            "{:?} -> {:?} disagrees with the closed table",
            from,
            to
        );
        // Property: document terminals are sinks.
        if matches!(from, TransactionDocumentState::Voided | TransactionDocumentState::Superseded) {
            prop_assert!(!from.can_transition_to(to), "{:?} is terminal and must not leave", from);
        }
        // Property: signature terminals are sinks.
        if matches!(
            sfrom,
            SignatureRequestStatus::Completed
                | SignatureRequestStatus::Declined
                | SignatureRequestStatus::Voided
                | SignatureRequestStatus::Expired
                | SignatureRequestStatus::Error
        ) {
            prop_assert!(!sfrom.can_transition_to(sto), "{:?} is terminal and must not leave", sfrom);
        }
        // Property: beyond the requested funnel, a signature never returns to
        // `Requested`.
        if !matches!(sfrom, SignatureRequestStatus::Requested) {
            prop_assert!(
                !sfrom.can_transition_to(SignatureRequestStatus::Requested),
                "{:?} must never move backwards to Requested",
                sfrom
            );
        }
        // Property: wire strings round-trip through `TryFrom`.
        let document_round = TransactionDocumentState::try_from(from.as_str());
        prop_assert_eq!(
            document_round.as_ref(),
            Ok(&from),
            "document state wire string must round-trip"
        );
        let signature_round = SignatureRequestStatus::try_from(sfrom.as_str());
        prop_assert_eq!(
            signature_round.as_ref(),
            Ok(&sfrom),
            "signature state wire string must round-trip"
        );
        // Property: unknown wire strings refuse on both tables.
        prop_assert!(TransactionDocumentState::try_from("archived").is_err());
        prop_assert!(SignatureRequestStatus::try_from("archived").is_err());
    }
}
