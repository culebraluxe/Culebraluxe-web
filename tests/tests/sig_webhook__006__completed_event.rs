//! SIG.WEBHOOK — completed event (TST-SIG-WEBHOOK-006).
//!
//! Contract: a BoldSign `Completed` webhook for a completed envelope is the ONE provider event
//! that proves the envelope finished — `map_webhook_event` (`middle/apis/src/boldsign/mod.rs:105`)
//! maps `("Completed", Some("Completed"))` to the neutral `SignatureProviderEvent::Completed`, and
//! `as_status` (`middle/model/src/signature.rs:277`) carries it to `SignatureRequestStatus::Completed`.
//! Downstream, `SignatureService::handle_webhook` reconciles signed artifacts ONLY on this status
//! (`web/src/signature/mod.rs:858`), so the mapping is the load-bearing decision: over-map and an
//! in-progress envelope mints signed artifacts; under-map and a finished envelope never does.
//!
//! Four things must therefore hold:
//!
//! - **Genuine completion maps to Completed.** `("Completed", Some("Completed"))` is the neutral
//!   Completed event and the Completed request status — the full chain, event to status.
//! - **A recipient signature is NOT envelope completion.** `("Signed", Some("InProgress"))`
//!   maps to Viewed, never Completed: one signer finishing while the envelope is still in
//!   progress must not trigger artifact reconciliation.
//! - **The terminal event type is authoritative.** Event type `Completed` maps to Completed
//!   without consulting the accompanying document status — the provider's own classification is
//!   the proof — while informational types (`Viewed`/`Signed`) consult it. The decision table is
//!   pinned exactly as production implements it, so a future edit that starts cross-checking
//!   (or stops trusting) the terminal type fails here deliberately.
//! - **The Completed status is terminal-shaped.** It cannot transition back to Sent (the model
//!   forbids it), so a completed request that later receives an older event cannot regress —
//!   the replay story (TST-SIG-WEBHOOK-009) rests on this.
//!
//! Level: L3 Composition — the production mapping functions, no I/O, no database.
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sig_webhook__006__completed_event

use apis::boldsign::map_webhook_event;
use model::{SignatureProviderEvent, SignatureRequestStatus};

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SIG-WEBHOOK-006).
fn sig_webhook_006__completed_event() {
    // 1. GENUINE COMPLETION maps end to end: provider event type + provider document status in,
    // neutral Completed event out, Completed request status out.
    let event = map_webhook_event("Completed", Some("Completed"))
        .expect("a completed envelope's Completed webhook must map");
    assert_eq!(
        event,
        SignatureProviderEvent::Completed,
        "genuine completion must be the neutral Completed event"
    );
    assert_eq!(
        event.as_status(),
        SignatureRequestStatus::Completed,
        "the neutral Completed event must carry the Completed request status"
    );

    // 2. A RECIPIENT SIGNATURE is not envelope completion: one signer done while the envelope is
    // still InProgress is Viewed — acknowledged, never reconciled as finished.
    let partial = map_webhook_event("Signed", Some("InProgress"))
        .expect("a recipient-signed webhook must still map");
    assert_eq!(
        partial,
        SignatureProviderEvent::Viewed,
        "a recipient signature on an in-progress envelope must be Viewed, never Completed"
    );
    assert_ne!(
        partial.as_status(),
        SignatureRequestStatus::Completed,
        "the partial signature must not carry the Completed status or reconciliation would fire early"
    );

    // 3. THE TERMINAL EVENT TYPE IS AUTHORITATIVE: production trusts the provider's own
    // classification for terminal types — event type `Completed` maps to Completed however the
    // accompanying document status reads — while informational types consult it (assertion 2).
    // This pins the decision table exactly: a future edit that starts cross-checking the
    // terminal type, or stops trusting it, fails here deliberately.
    let authoritative = map_webhook_event("Completed", Some("InProgress"))
        .expect("a Completed event type must still map");
    assert_eq!(
        authoritative,
        SignatureProviderEvent::Completed,
        "event type Completed is authoritative: the provider's classification is the proof"
    );
    let authoritative_none = map_webhook_event("Completed", None)
        .expect("a Completed event type without document status must still map");
    assert_eq!(
        authoritative_none,
        SignatureProviderEvent::Completed,
        "event type Completed needs no accompanying status to complete"
    );

    // 4. COMPLETED IS TERMINAL-SHAPED: the model forbids Completed -> Sent, so a terminal request
    // cannot regress when an older event is replayed through `apply_status`.
    assert!(
        !SignatureRequestStatus::Completed.can_transition_to(SignatureRequestStatus::Sent),
        "Completed must not transition back to Sent"
    );
    assert!(
        SignatureRequestStatus::Sent.can_transition_to(SignatureRequestStatus::Completed),
        "control: Sent must still reach Completed, or the terminal assertion proves nothing"
    );
}
