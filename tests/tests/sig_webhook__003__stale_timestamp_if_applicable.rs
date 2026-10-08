//! SIG.WEBHOOK — stale timestamp, if applicable (TST-SIG-WEBHOOK-003).
//!
//! Contract: the BoldSign signature header binds the HMAC to a moment —
//! `verify_webhook_signature` (`middle/apis/src/boldsign/mod.rs:180`) rejects a header whose
//! timestamp skew exceeds the configured tolerance, in EITHER direction. A captured valid webhook
//! replayed an hour later must not authenticate: without the timestamp check, HMAC validity alone
//! would make every intercepted delivery replayable forever. The "if applicable" in the title is
//! answered here: the check IS applicable — production enforces a tolerance window — so this story
//! pins it.
//!
//! Five things must therefore hold:
//!
//! - **A fresh signature verifies.** Timestamp equal to now is inside any window (control).
//! - **A stale signature is refused.** Skew beyond tolerance into the past fails closed with a
//!   tolerance error, not a mismatch: the operator can tell replay from forgery.
//! - **A future signature is refused too.** Skew is absolute — a timestamp from the future is
//!   just as replayable, so it is refused the same way.
//! - **The boundary is exact.** Skew of exactly the tolerance still verifies; one second past
//!   it does not. A window checked with `>` rather than `>=` (or vice versa) fails here.
//! - **The tolerance parameter is honored.** The same stale header verifies under a wider
//!   tolerance and fails under a narrower one — the window is the caller's, not a hardcoded
//!   constant smuggled past configuration.
//!
//! Level: L3 Composition — the production verifier function, no I/O, no database.
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sig_webhook__003__stale_timestamp_if_applicable

use apis::boldsign::verify_webhook_signature;
use hmac::{Hmac, Mac};
use sha2::Sha256;

const SECRET: &str = "sig-webhook-003-test-secret";
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SIG-WEBHOOK-003).
fn sig_webhook_003__stale_timestamp_if_applicable() {
    let body = r#"{"event":{"id":"evt-003","eventType":"Completed"},"data":{"documentId":"env-003","status":"Completed"}}"#;
    let now = now_seconds();

    // 1 + 4. FRESH verifies, and the BOUNDARY is exact: skew of exactly the tolerance is inside
    // the window, one second past it is outside. Each timestamp needs its own header because the
    // HMAC covers `{timestamp}.{body}` — reusing one header across timestamps would confound the
    // timestamp verdict with an HMAC mismatch.
    let edge = mint_header(body, SECRET, now - TOLERANCE_SECONDS);
    assert!(
        verify_webhook_signature(body, &edge, SECRET, now, TOLERANCE_SECONDS).is_ok(),
        "skew of exactly the tolerance must still verify (the window is `skew > tolerance`)"
    );
    let past = mint_header(body, SECRET, now - TOLERANCE_SECONDS - 1);

    // 2. A STALE signature is refused — and says tolerance, not mismatch.
    let error = verify_webhook_signature(body, &past, SECRET, now, TOLERANCE_SECONDS)
        .expect_err("a signature one second past tolerance must be refused");
    assert!(
        error.contains("tolerance"),
        "a stale signature must report the tolerance window, got: {error}"
    );

    // 3. A FUTURE signature is refused the same way: skew is absolute, so a timestamp from the
    // future is replayable either way and must not authenticate.
    let future = mint_header(body, SECRET, now + TOLERANCE_SECONDS + 1);
    let error = verify_webhook_signature(body, &future, SECRET, now, TOLERANCE_SECONDS)
        .expect_err("a future-dated signature must be refused");
    assert!(
        error.contains("tolerance"),
        "a future signature must report the tolerance window, got: {error}"
    );

    // 5. THE TOLERANCE PARAMETER IS HONORED: the stale header that failed above verifies under a
    // wider window — the window is the caller's configuration (production passes
    // `BOLDSIGN_WEBHOOK_TOLERANCE_SECONDS`, default 300), not a constant.
    assert!(
        verify_webhook_signature(body, &past, SECRET, now, TOLERANCE_SECONDS + 1).is_ok(),
        "the same header must verify when the caller's tolerance covers its skew"
    );
    assert!(
        verify_webhook_signature(body, &past, SECRET, now, 1).is_err(),
        "the same header must fail under a narrower tolerance"
    );
}
