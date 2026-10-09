//! SIG.WEBHOOK — irrelevant / non-actionable event acknowledgement (TST-SIG-WEBHOOK-007).
//!
//! Contract: not every BoldSign callback moves the envelope. Informational events (`Viewed`,
//! `Signed` on an active envelope) are ACKNOWLEDGED — `map_webhook_event`
//! (`middle/apis/src/boldsign/mod.rs:105`) maps them to the non-terminal neutral `Viewed` event,
//! whose status (`Viewed`) is active, never terminal — while events the lifecycle cannot act on
//! (`Reassigned`, unknown types) FAIL CLOSED with an error instead of being silently acked. The
//! distinction matters downstream: `handle_webhook` reconciles artifacts only on Completed
//! (`web/src/signature/mod.rs:858`), so an informational event must never map terminal, and an
//! unmappable event must never map at all (a silent ack would lose a provider signal the envelope
//! still needs operator attention for).
//!
//! Five things must therefore hold:
//!
//! - **Informational events acknowledge as Viewed.** `Viewed` and `Signed` on an active envelope
//!   map to the neutral Viewed event — accepted, recorded, no terminal transition.
//! - **The acknowledgement is non-terminal.** `Viewed.as_status()` is an active status (it can
//!   still reach Completed) and is not Completed/Declined/Voided — reconciliation cannot fire.
//! - **Attention events fail closed.** `Reassigned` (and its siblings) return Err: the envelope
//!   needs operator attention, and a silent ack would pretend otherwise.
//! - **Unknown event types acknowledge only against a live envelope.** A type production never
//!   classified with an ACTIVE document status is acknowledged as Viewed (forward-compatible:
//!   new provider informational vocabulary does not break the webhook), but with NO status —
//!   or a non-active one — there is nothing actionable to acknowledge and it fails closed.
//!   New provider vocabulary cannot sneak through as terminal, and a meaningless event cannot
//!   sneak through at all.
//! - **Terminal mappings are untouched by this story.** Control: `Completed` still maps
//!   Completed, so the non-terminal assertions below are the mapping discriminating, not the
//!   mapping collapsing everything to Viewed.
//!
//! Level: L3 Composition — the production mapping functions, no I/O, no database.
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sig_webhook__007__irrelevant_non_actionable_event_acknowledgement_behavior

use apis::boldsign::map_webhook_event;
use model::{SignatureProviderEvent, SignatureRequestStatus};

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SIG-WEBHOOK-007).
fn sig_webhook_007__irrelevant_non_actionable_event_acknowledgement_behavior() {
    // 1. INFORMATIONAL EVENTS ACKNOWLEDGE AS VIEWED: seen or partially signed on an active
    // envelope is accepted and recorded — never terminal, never an error.
    for (event_type, document_status) in [
        ("Viewed", Some("InProgress")),
        ("Signed", Some("InProgress")),
        ("Viewed", None),
    ] {
        let event = map_webhook_event(event_type, document_status)
            .expect("an informational webhook must map, not fail");
        assert_eq!(
            event,
            SignatureProviderEvent::Viewed,
            "{event_type:?} on an active envelope must acknowledge as Viewed"
        );
    }

    // 2. THE ACKNOWLEDGEMENT IS NON-TERMINAL: Viewed carries an active status that can still
    // reach Completed, and is none of the terminal outcomes — so artifact reconciliation, which
    // fires only on Completed, cannot fire from an acknowledgement.
    let acknowledged = SignatureProviderEvent::Viewed.as_status();
    assert_eq!(
        acknowledged,
        SignatureRequestStatus::Viewed,
        "the acknowledgement must carry the Viewed status"
    );
    assert!(
        acknowledged.is_active(),
        "the acknowledgement status must stay active (the envelope is still alive)"
    );
    for terminal in [
        SignatureRequestStatus::Completed,
        SignatureRequestStatus::Declined,
        SignatureRequestStatus::Voided,
    ] {
        assert_ne!(
            acknowledged, terminal,
            "an acknowledgement must never carry the terminal status {terminal:?}"
        );
    }
    assert!(
        SignatureRequestStatus::Viewed.can_transition_to(SignatureRequestStatus::Completed),
        "control: the acknowledged envelope must still be able to complete later"
    );

    // 3. ATTENTION EVENTS FAIL CLOSED: Reassigned needs an operator, not an ack.
    let error = map_webhook_event("Reassigned", Some("InProgress"))
        .expect_err("an attention event must fail closed, not acknowledge");
    assert!(
        error.contains("operator attention"),
        "the refusal must say why the event needs a human, got: {error}"
    );

    // 4. UNKNOWN EVENT TYPES ACKNOWLEDGE ONLY AGAINST A LIVE ENVELOPE: an unclassified type
    // with an ACTIVE document status is forward-compatible informational traffic — acknowledged
    // as Viewed, never terminal — while the same type with no (or a non-active) status has
    // nothing actionable to acknowledge and fails closed.
    let forward_compatible = map_webhook_event("Frobnicate", Some("InProgress"))
        .expect("an unknown type on a live envelope must acknowledge, not break the webhook");
    assert_eq!(
        forward_compatible,
        SignatureProviderEvent::Viewed,
        "an unknown type on a live envelope acknowledges as Viewed, never terminal"
    );
    assert_ne!(
        forward_compatible.as_status(),
        SignatureRequestStatus::Completed,
        "forward-compatible acknowledgement must never complete"
    );
    let error = map_webhook_event("Frobnicate", None)
        .expect_err("an unknown type with no status must fail closed, not acknowledge");
    assert!(
        error.contains("no neutral lifecycle mapping"),
        "a meaningless event must report it has no mapping, got: {error}"
    );
    let error = map_webhook_event("Frobnicate", Some("Completed"))
        .expect_err("an unknown type on a non-active status must fail closed");
    assert!(
        error.contains("no neutral lifecycle mapping"),
        "an unknown type with nothing actionable must report no mapping, got: {error}"
    );

    // 5. CONTROL: terminal mappings are untouched — Completed still completes, so the Viewed
    // assertions above are the mapping discriminating, not everything collapsing to Viewed.
    assert_eq!(
        map_webhook_event("Completed", Some("Completed")).expect("Completed must still map"),
        SignatureProviderEvent::Completed,
        "control: genuine completion must still map Completed"
    );
}
