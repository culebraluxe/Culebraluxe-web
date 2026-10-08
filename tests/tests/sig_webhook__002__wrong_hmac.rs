//! SIG.WEBHOOK — wrong HMAC (TST-SIG-WEBHOOK-002).
//!
//! Contract: a webhook whose HMAC does not match the configured secret is forged —
//! `verify_webhook_signature` (`middle/apis/src/boldsign/mod.rs:180`) must reject it, and the
//! rejection must name the mismatch. This is the negative twin of TST-SIG-WEBHOOK-001 (which pins
//! that the right signature over the right body verifies): together they prove the gate has both a
//! pass path and a refuse path, so a verifier that accepted everything — or nothing — fails one
//! of the two stories.
//!
//! Four things must therefore hold:
//!
//! - **A foreign secret is refused.** A header minted with any other secret is an HMAC mismatch.
//! - **A corrupted signature is refused.** Flipping one hex digit of a valid signature keeps the
//!   header well-formed but breaks the MAC — still a mismatch, never a pass.
//! - **A non-signature is refused.** A 64-hex-digit string that was never a MAC over this body
//!   (all zeros) is a mismatch: the verifier compares, it does not pattern-match.
//! - **A malformed signature degrades to mismatch, not acceptance.** A signature part that is
//!   not 32 hex bytes matches nothing — the verifier skips undecodable parts and reports
//!   mismatch rather than erroring open.
//!
//! Plus the positive control without which the negatives prove nothing: the uncorrupted header
//! verifies, so a test run where verification always fails cannot go green here.
//!
//! Level: L3 Composition — the production verifier function, no I/O, no database.
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sig_webhook__002__wrong_hmac

use apis::boldsign::verify_webhook_signature;
use hmac::{Hmac, Mac};
use sha2::Sha256;

const SECRET: &str = "sig-webhook-002-test-secret";
const TOLERANCE_SECONDS: i64 = 300;

fn mint_header(raw_body: &str, secret: &str, timestamp: i64) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC takes any key");
    mac.update(format!("{timestamp}.{raw_body}").as_bytes());
    let hex: String = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("t={timestamp}, s0={hex}")
}

fn now_seconds() -> i64 {
    chrono::Utc::now().timestamp()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SIG-WEBHOOK-002).
fn sig_webhook_002__wrong_hmac() {
    let body = r#"{"event":{"id":"evt-002","eventType":"Completed"},"data":{"documentId":"env-002","status":"Completed"}}"#;
    let now = now_seconds();
    let valid = mint_header(body, SECRET, now);

    // Positive control: the uncorrupted header verifies — without this, every rejection below
    // could be "the verifier rejects everything" and the test would prove nothing.
    assert!(
        verify_webhook_signature(body, &valid, SECRET, now, TOLERANCE_SECONDS).is_ok(),
        "the control header must verify, or the refusal assertions prove nothing"
    );

    // 1. A FOREIGN SECRET is refused: minted elsewhere, presented here.
    let foreign = mint_header(body, "some-other-webhook-secret", now);
    assert_ne!(foreign, valid, "distinct secrets must mint distinct headers");
    let error = verify_webhook_signature(body, &foreign, SECRET, now, TOLERANCE_SECONDS)
        .expect_err("a foreign-secret HMAC must be refused");
    assert!(
        error.contains("HMAC mismatch"),
        "a foreign secret must report mismatch, got: {error}"
    );

    // 2. A CORRUPTED signature is refused: well-formed header, broken MAC. Flip the final hex
    // digit to a different valid hex digit so the header still parses.
    let mut corrupted = valid.clone();
    let last = corrupted.pop().expect("the header ends with a hex digit");
    let flipped = if last == '0' { '1' } else { '0' };
    corrupted.push(flipped);
    assert!(
        verify_webhook_signature(body, &corrupted, SECRET, now, TOLERANCE_SECONDS).is_err(),
        "a one-digit-corrupted signature must be refused"
    );

    // 3. A NON-SIGNATURE is refused: 64 hex digits that were never a MAC over this body.
    let zeros = format!("t={now}, s0={}", "0".repeat(64));
    let error = verify_webhook_signature(body, &zeros, SECRET, now, TOLERANCE_SECONDS)
        .expect_err("an all-zero signature must be refused");
    assert!(
        error.contains("HMAC mismatch"),
        "a non-signature must report mismatch, got: {error}"
    );

    // 4. An UNDECODABLE signature degrades to mismatch, not acceptance: the verifier skips parts
    // that are not 32 hex bytes and, with nothing left to match, reports mismatch.
    let undecodable = format!("t={now}, s0=not-hex-at-all");
    let error = verify_webhook_signature(body, &undecodable, SECRET, now, TOLERANCE_SECONDS)
        .expect_err("an undecodable signature must be refused");
    assert!(
        error.contains("HMAC mismatch"),
        "an undecodable part must degrade to mismatch, got: {error}"
    );
}
