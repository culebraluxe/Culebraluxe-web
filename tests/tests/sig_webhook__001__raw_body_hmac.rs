//! SIG.WEBHOOK — raw-body HMAC (TST-SIG-WEBHOOK-001).
//!
//! Contract: BoldSign authenticates its callback by HMAC-SHA256 over the RAW request body —
//! `verify_webhook_signature` (`middle/apis/src/boldsign/mod.rs:180`) recomputes the MAC over
//! `{timestamp}.{raw_body}` with the configured webhook secret and accepts only an exact match.
//! The timestamp binds the signature to a moment (staleness is TST-SIG-WEBHOOK-003); this story
//! pins the body half: the MAC covers the raw bytes production received, not a re-serialization.
//!
//! Four things must therefore hold:
//!
//! - **A signature minted over the exact raw body verifies.** The test mints its own
//!   `t=<ts>, s0=<hmac>` header with an independent HMAC-SHA256 oracle (the `hmac`/`sha2`
//!   crates, test-only) and the production verifier accepts it.
//! - **Any byte change to the body voids the signature.** The same header over the body with
//!   one byte flipped — and over a semantically identical but byte-different re-serialization
//!   (extra whitespace) — is rejected. A verifier that parsed first and re-serialized would
//!   accept the latter; production must not.
//! - **The secret is load-bearing.** A header minted with a different secret is rejected, so a
//!   signature from another integration (or no secret at all) cannot authenticate.
//! - **The header shape is enforced.** A missing `s0` part is malformed, not a mismatch —
//!   fail closed with a distinct error, never an accept.
//!
//! Level: L3 Composition — the production verifier function, no I/O, no database.
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sig_webhook__001__raw_body_hmac

use apis::boldsign::verify_webhook_signature;
use hmac::{Hmac, Mac};
use sha2::Sha256;

const SECRET: &str = "sig-webhook-001-test-secret";
const TOLERANCE_SECONDS: i64 = 300;

/// Independent oracle: mint the `t=<ts>, s0=<hex-hmac>` header the BoldSign contract specifies.
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SIG-WEBHOOK-001).
fn sig_webhook_001__raw_body_hmac() {
    let body = r#"{"event":{"id":"evt-001","eventType":"Completed"},"data":{"documentId":"env-001","status":"Completed"}}"#;
    let now = now_seconds();

    // 1. A signature minted over the EXACT raw body verifies.
    let header = mint_header(body, SECRET, now);
    assert!(
        verify_webhook_signature(body, &header, SECRET, now, TOLERANCE_SECONDS).is_ok(),
        "a signature over the exact raw body must verify"
    );

    // 2. ANY byte change voids the signature. One flipped byte inside a string value ...
    let mut tampered = body.to_owned();
    tampered.replace_range(60..61, "X");
    assert_ne!(tampered, body, "the tamper must actually change a byte");
    assert!(
        verify_webhook_signature(&tampered, &header, SECRET, now, TOLERANCE_SECONDS).is_err(),
        "the same header over a one-byte-different body must be rejected: the MAC covers raw bytes"
    );

    // ... and a byte-identical-meaning re-serialization (extra whitespace) is a DIFFERENT body.
    let reparsed: serde_json::Value =
        serde_json::from_str(body).expect("the fixture body is valid JSON");
    let reserialized = serde_json::to_string_pretty(&reparsed).expect("re-serialization works");
    assert_ne!(
        reserialized, body,
        "the re-serialization must differ byte-wise"
    );
    assert!(
        verify_webhook_signature(&reserialized, &header, SECRET, now, TOLERANCE_SECONDS).is_err(),
        "a verifier that parsed-then-reserialized would accept this; production must verify raw bytes"
    );
    // Control: the re-serialization verifies under ITS OWN header, so the rejection above is the
    // body binding, not an inability to verify pretty JSON at all.
    let reserialized_header = mint_header(&reserialized, SECRET, now);
    assert!(
        verify_webhook_signature(
            &reserialized,
            &reserialized_header,
            SECRET,
            now,
            TOLERANCE_SECONDS
        )
        .is_ok(),
        "the control header must verify, or the tamper assertions prove nothing"
    );

    // 3. The secret is load-bearing: another secret's header is rejected.
    let foreign_header = mint_header(body, "a-different-integration-secret", now);
    let error = verify_webhook_signature(body, &foreign_header, SECRET, now, TOLERANCE_SECONDS)
        .expect_err("a foreign-secret signature must be rejected");
    assert!(
        error.contains("HMAC mismatch"),
        "a wrong-secret rejection must say mismatch, got: {error}"
    );

    // 4. The header shape is enforced: no signature part is malformed, not a pass.
    let shapeless = format!("t={now}");
    let error = verify_webhook_signature(body, &shapeless, SECRET, now, TOLERANCE_SECONDS)
        .expect_err("a header with no signature part must be rejected");
    assert!(
        error.contains("malformed"),
        "a shapeless header must fail closed as malformed, got: {error}"
    );
}
