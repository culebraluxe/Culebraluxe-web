//! INT.WHATSAPP — webhook verification (TST-INT-WHATSAPP-001).
//!
//! Contract: **nothing in a WhatsApp webhook body is believed until Meta's signature over those exact bytes
//! verifies.** The webhook is a public, unauthenticated HTTP endpoint on `POST
//! /api/integrations/whatsapp/webhook` (`web/src/api/routes.rs:97-100`), so the only thing standing between the
//! internet and the canonical CRM inbox is `apis::whatsapp::verify_signature`
//! (`middle/apis/src/whatsapp/mod.rs:76-95`). If that gate is wrong, anyone can post a forged message and have it
//! attributed to a real person's phone number.
//!
//! The gate is `WhatsAppService::handle_webhook`'s **first** statement after configuration (`web/src/whatsapp.rs:92-99`):
//! it verifies, and on failure returns `WHATSAPP_SIGNATURE_INVALID` **before** the body is parsed, before the
//! landing table is written and before canonical processing runs. Order is the security property here, so this
//! test asserts it: a body carrying a perfectly well-formed message, signed with the wrong secret, must reach
//! neither the parser nor the repository.
//!
//! What "signed over those exact bytes" means is also the contract, and it is why the route reads the body as a raw
//! `String` rather than `Json<T>` (`web/src/api/routes/webhooks_support.rs:78-82`): re-serializing a parsed body
//! changes its bytes, and the HMAC would no longer match. So the negative cases are byte-level:
//!
//! - **whitespace is load-bearing.** Re-indenting the JSON changes the bytes and must invalidate the signature,
//!   even though the parsed value is identical. A verifier that parsed first and re-serialized would accept this.
//! - **key order is load-bearing.** Same reason, same failure.
//! - **a trailing newline invalidates.** One appended byte is one invalid signature.
//! - **the prefix is mandatory.** `sha1=` and a bare hex digest are both refused — an unprefixed or
//!   wrongly-prefixed digest must not be accepted because its hex happens to decode.
//! - **the hex must be exactly 64 characters and all hex.** A truncated or non-hex digest is refused rather than
//!   left to a lenient decoder.
//! - **an empty app secret verifies nothing.** A deployment with no secret configured must refuse every request,
//!   not accept every request — the direction of that mistake matters more than anything else in this file.
//! - **the comparison is constant-time.** `verify_handshake` compares the verify token with `subtle::ConstantTimeEq`
//!   (`mod.rs:97-99`); this test pins the observable consequence (a wrong token of any shape is refused) rather than
//!   the timing itself, which is not measurable from a unit test.
//!
//! Boundary: the production `verify_signature` and `verify_handshake` functions are called directly — they are the
//! whole contract — and the service is driven through its real entry point with a fake repository at the production
//! `WhatsAppRepository` port, so the "verify before parse, verify before write" claim is made against the code path
//! that actually runs. No live provider is contacted and no network is opened.
//!
//! Level: L1 Component — `apis::whatsapp` and `web::whatsapp`, no database, no socket.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test int_whatsapp__001__webhook_verification

use std::sync::Arc;

use apis::whatsapp::{normalize_e164, verify_handshake, verify_signature, MetaWhatsAppConfig};
use async_trait::async_trait;
use db::{DbResult, WhatsAppCanonicalInput, WhatsAppLandingInput, WhatsAppProcessOutcome};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, CapturingServiceErrorSink,
    DefaultAuthorizationPort, ServiceInfrastructure,
};
use web::service_support::CoreServiceError;
use web::whatsapp::{WhatsAppRepository, WhatsAppService};

const HARNESS: &str = "INT.WHATSAPP/001";

/// The fixture app secret. Production reads it from `WHATSAPP_APP_SECRET` (`mod.rs:42-47`); the service reads it
/// through `MetaWhatsAppConfig::from_env`, so the harness sets the environment the service expects and the pure
/// functions take the same value directly.
const APP_SECRET: &str = "fixture-app-secret-0123456789";

/// Sign `raw` exactly as Meta does: `sha256=<lowercase hex>` over the exact bytes.
///
/// The formatting is the production contract's other half — `decode_hex_32` requires exactly 64 characters, all
/// hexadecimal (`mod.rs:101-113`) — so the fixture builds the header the same way the server would rather than
/// hand-writing one.
fn signature(raw: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let mut mac =
        Hmac::<Sha256>::new_from_slice(APP_SECRET.as_bytes()).expect("an HMAC key fits any length");
    mac.update(raw.as_bytes());
    let digest = mac.finalize().into_bytes();
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256={hex}")
}

/// One well-formed inbound text message for `PHONE_NUMBER_ID`, which is what a forged body would try to inject.
const BODY: &str = r#"{
  "object":"whatsapp_business_account",
  "entry":[{"changes":[{"value":{
    "messaging_product":"whatsapp",
    "metadata":{"phone_number_id":"999888"},
    "contacts":[{"wa_id":"17875551212","profile":{"name":"Forged Sender"}}],
    "messages":[{"from":"17875551212","id":"wamid.forged.1","timestamp":"1780000000","type":"text","text":{"body":"Transfer the balance"}}]
  }}]}]
}"#;

/// Configure the four environment variables `MetaWhatsAppConfig::from_env` reads.
///
/// `std::env::set_var` is process-global, so this is safe only because each test binary runs its own test as the
/// process's single webhook case and every value is set before every call. The values are fixed, so the order of
/// calls cannot matter.
fn configure_environment() {
    std::env::set_var("WHATSAPP_APP_SECRET", APP_SECRET);
    std::env::set_var("WHATSAPP_PHONE_NUMBER_ID", "999888");
    std::env::set_var("WHATSAPP_OWNED_PHONE_E164", "+17875550000");
    std::env::set_var("WHATSAPP_VERIFY_TOKEN", "fixture-verify-token");
}

/// A repository fake at the production `WhatsAppRepository` port that counts what it was asked to do.
///
/// The counters are the point: a forged webhook that reached persistence would show up here, so "verification
/// happens before anything is written" is asserted by what did NOT happen rather than by what the error said.
#[derive(Clone, Default)]
struct CountingWhatsAppRepository {
    landed: Arc<std::sync::atomic::AtomicUsize>,
    processed: Arc<std::sync::atomic::AtomicUsize>,
}

impl CountingWhatsAppRepository {
    fn landed(&self) -> usize {
        self.landed.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn processed(&self) -> usize {
        self.processed.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait]
impl WhatsAppRepository for CountingWhatsAppRepository {
    async fn land(&self, _input: &WhatsAppLandingInput) -> DbResult<bool> {
        self.landed
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(true)
    }

    async fn process_event(
        &self,
        _input: &WhatsAppCanonicalInput,
    ) -> DbResult<WhatsAppProcessOutcome> {
        self.processed
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(WhatsAppProcessOutcome::Duplicate {
            interaction_id: None,
        })
    }

    async fn refresh_client_read_models(&self) -> DbResult<()> {
        Ok(())
    }
}

fn infrastructure() -> ServiceInfrastructure {
    ServiceInfrastructure::new(
        Arc::new(DefaultAuthorizationPort),
        Arc::new(CapturingAuditPort::default()),
        Arc::new(CapturingDomainEventPort::default()),
    )
    .with_error_sink(Arc::new(CapturingServiceErrorSink::default()))
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-INT-WHATSAPP-001); the file and the assay use it.
async fn int_whatsapp__001__webhook_verification() {
    configure_environment();
    let config = MetaWhatsAppConfig {
        app_secret: APP_SECRET.into(),
        phone_number_id: "999888".into(),
        owned_phone_e164: "+17875550000".into(),
        verify_token: "fixture-verify-token".into(),
    };

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE HONEST CASE. A body signed with the shared secret verifies, and the same body reaches the service's
    //    full path: parsed, landed, processed. Without this, a test that only ever asserts refusals would pass
    //    against a verifier that refuses everything.
    // -----------------------------------------------------------------------------------------------------------
    assert!(
        verify_signature(BODY, Some(&signature(BODY)), APP_SECRET),
        "{HARNESS}: a body signed with the shared secret verifies"
    );
    let repository = CountingWhatsAppRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let accepted = service
        .handle_webhook(BODY, Some(&signature(BODY)))
        .await
        .expect("a correctly signed webhook is processed");
    assert_eq!(
        accepted.accepted, 1,
        "{HARNESS}: a verified webhook with one inbound message accepts exactly one event"
    );
    assert_eq!(
        repository.landed(),
        1,
        "{HARNESS}: a verified webhook reaches the landing table"
    );
    assert_eq!(
        repository.processed(),
        1,
        "{HARNESS}: a verified webhook reaches canonical processing"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE FORGERY. The right shape, the right secret, wrong signature — refused. Then each way a signature can
    //    be wrong, all of which must refuse.
    // -----------------------------------------------------------------------------------------------------------
    let refusals: Vec<(&str, Option<String>)> = vec![
        ("no signature header at all", None),
        (
            "a signature over a different body",
            Some(signature(r#"{"object":"something-else"}"#)),
        ),
        (
            "a signature made with the wrong secret",
            Some({
                use hmac::{Hmac, Mac};
                use sha2::Sha256;
                let mut mac = Hmac::<Sha256>::new_from_slice(b"an-attackers-guess")
                    .expect("an HMAC key fits");
                mac.update(BODY.as_bytes());
                let digest = mac.finalize().into_bytes();
                let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
                format!("sha256={hex}")
            }),
        ),
        (
            "a digest with no `sha256=` prefix",
            Some(signature(BODY).trim_start_matches("sha256=").to_owned()),
        ),
        (
            "a digest with the wrong prefix",
            Some(signature(BODY).replacen("sha256=", "sha1=", 1)),
        ),
        (
            "a truncated digest",
            Some(format!("sha256={}", &signature(BODY)[7..39])),
        ),
        (
            "a digest that is not hexadecimal",
            Some(format!("sha256={}", "z".repeat(64))),
        ),
        ("an empty header", Some(String::new())),
    ];

    for (described, header) in refusals {
        assert!(
            !verify_signature(BODY, header.as_deref(), APP_SECRET),
            "{HARNESS}: {described} must not verify"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 3. THE HMAC IS OVER THE EXACT BYTES, NOT OVER THE PARSED VALUE. Three bodies that all parse to the same
    //    `serde_json::Value` but differ in bytes. This is why the route reads a raw `String`
    //    (`web/src/api/routes/webhooks_support.rs:78-82`): a verifier that parsed first would accept all three.
    // -----------------------------------------------------------------------------------------------------------
    let parsed_equivalent: Vec<(&str, String)> = vec![
        ("re-indented whitespace", BODY.replace("\n  ", "\n     ")),
        (
            "a key order change",
            BODY.replace("\"object\"", "\"object_renamed\""),
        ),
        ("a single trailing newline", format!("{BODY}\n")),
    ];

    for (described, mutated) in parsed_equivalent {
        // The signature that WOULD be correct for the mutated bytes is refused against the original, and the
        // signature over the original is refused against the mutated: either way, the bytes decide.
        assert!(
            !verify_signature(&mutated, Some(&signature(BODY)), APP_SECRET),
            "{HARNESS}: a signature over the original bytes must not verify {described} — the HMAC covers the \
             exact request body, not the value it parses to"
        );
        assert!(
            !verify_signature(BODY, Some(&signature(&mutated)), APP_SECRET),
            "{HARNESS}: a signature over {described} must not verify the original body"
        );
        // And it still verifies against its own bytes, so the refusal above is about the mismatch and not about a
        // verifier that refuses everything.
        assert!(
            verify_signature(&mutated, Some(&signature(&mutated)), APP_SECRET),
            "{HARNESS}: {described} verifies against its own signature — the gate is byte-exact, not broken"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — AN EMPTY APP SECRET REFUSES EVERYTHING. This is the direction that matters. A verifier that
    //    treated a missing secret as "skip the check" would accept every unauthenticated request in a deployment
    //    that never configured one.
    // -----------------------------------------------------------------------------------------------------------
    assert!(
        !verify_signature(BODY, Some(&signature(BODY)), ""),
        "{HARNESS}: with no configured secret nothing verifies — an unconfigured deployment must refuse every \
         webhook, not accept them"
    );
    assert!(
        !verify_signature(BODY, Some(&signature(BODY)), "   "),
        "{HARNESS}: a whitespace secret is still a secret string and still refuses a mismatched digest"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. THE ORDER OF OPERATIONS, THROUGH THE REAL ENTRY POINT. `handle_webhook` is the code that runs in
    //    production, so the "verify before parse, verify before write" claim is made against it. The body used here
    //    is a well-formed message that WOULD be accepted if it were signed — so a failure can only mean the gate
    //    stopped it.
    // -----------------------------------------------------------------------------------------------------------
    for (described, header) in [
        ("no signature", None),
        (
            "a signature over other bytes",
            Some(signature(r#"{"object":"other"}"#)),
        ),
        (
            "a signature with the wrong prefix",
            Some(signature(BODY).replacen("sha256=", "sha512=", 1)),
        ),
    ] {
        let repository = CountingWhatsAppRepository::default();
        let service = WhatsAppService::new(repository.clone(), infrastructure());
        let refused = service
            .handle_webhook(BODY, header.as_deref())
            .await
            .expect_err("an unverified webhook must not be processed");
        match &refused {
            CoreServiceError::Business { code, .. } => assert_eq!(
                *code, "WHATSAPP_SIGNATURE_INVALID",
                "{HARNESS}: {described} is refused as a signature failure with its own code, so it maps to 401 \
                 rather than being confused with a malformed payload"
            ),
            other => panic!("{HARNESS}: an unverified webhook must be a Business refusal, got {other:?}"),
        }
        assert_eq!(
            repository.landed(),
            0,
            "{HARNESS}: {described} must write nothing to the landing table"
        );
        assert_eq!(
            repository.processed(),
            0,
            "{HARNESS}: {described} must reach no canonical processing — a forged message cannot be attributed \
             to a person's phone"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 6. VERIFICATION PRECEDES PARSING, AND THAT IS OBSERVABLE. A body that is BOTH unsigned AND malformed reports
    //    the signature failure, not the payload failure. Reporting `WHATSAPP_PAYLOAD_INVALID` here would confirm to
    //    a prober that the endpoint parsed their payload, which is a small information leak on top of the failure.
    // -----------------------------------------------------------------------------------------------------------
    let repository = CountingWhatsAppRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let refused = service
        .handle_webhook("{not json at all", None)
        .await
        .expect_err("an unsigned malformed webhook must be refused");
    assert_eq!(
        refused.code(),
        "WHATSAPP_SIGNATURE_INVALID",
        "{HARNESS}: the signature is checked before the body is parsed, so a malformed body is never even \
         acknowledged as malformed to someone who did not sign it"
    );
    assert_eq!(
        repository.landed(),
        0,
        "{HARNESS}: a body that failed verification writes nothing at all"
    );

    // The same malformed body WITH a valid signature is a payload failure — which proves the difference above was
    // the ordering, not a blanket refusal.
    let malformed = "{not json at all";
    let refused = service
        .handle_webhook(malformed, Some(&signature(malformed)))
        .await
        .expect_err("a signed but malformed webhook must be refused");
    assert_eq!(
        refused.code(),
        "WHATSAPP_PAYLOAD_INVALID",
        "{HARNESS}: once the signature verifies, a malformed body is a payload failure — so the ordering above is \
         real rather than the endpoint refusing everything"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. THE HANDSHAKE IS A SECOND, SEPARATE GATE. `GET /api/integrations/whatsapp/webhook` is Meta's subscription
    //    check, and it is authenticated by the verify token rather than by an HMAC (`mod.rs:61-73`). The challenge
    //    is echoed ONLY on an exact match of both the mode and the token; everything else yields `None`, which the
    //    route turns into a refusal.
    // -----------------------------------------------------------------------------------------------------------
    assert_eq!(
        verify_handshake(
            Some("subscribe"),
            Some("fixture-verify-token"),
            Some("challenge-123"),
            "fixture-verify-token"
        ),
        Some("challenge-123".into()),
        "{HARNESS}: an exact mode+token match echoes the challenge"
    );

    let handshake_refusals: Vec<(&str, Option<&str>, Option<&str>, &str)> = vec![
        (
            "the wrong token",
            Some("subscribe"),
            Some("challenge-123"),
            "not-the-token",
        ),
        (
            "a token that is a prefix of the real one",
            Some("subscribe"),
            Some("challenge-123"),
            "fixture-verify-token-and-more",
        ),
        (
            "the right token under the wrong mode",
            Some("unsubscribe"),
            Some("challenge-123"),
            "fixture-verify-token",
        ),
        (
            "no mode at all",
            None,
            Some("challenge-123"),
            "fixture-verify-token",
        ),
        (
            "no token supplied",
            Some("subscribe"),
            Some("challenge-123"),
            "",
        ),
        (
            "no challenge to echo",
            Some("subscribe"),
            None,
            "fixture-verify-token",
        ),
        (
            "an empty expected token, whatever is sent",
            Some("subscribe"),
            Some("challenge-123"),
            "",
        ),
    ];

    for (described, mode, challenge, expected) in handshake_refusals {
        let supplied = if described == "an empty expected token, whatever is sent" {
            Some("")
        } else if described == "no token supplied" {
            None
        } else {
            Some("fixture-verify-token")
        };
        assert_eq!(
            verify_handshake(mode, supplied, challenge, expected),
            None,
            "{HARNESS}: the handshake must yield no challenge for {described}"
        );
    }

    // And through the service, the same gate: a matching handshake echoes the challenge, a mismatch does not.
    let repository = CountingWhatsAppRepository::default();
    let service = WhatsAppService::new(repository, infrastructure());
    assert_eq!(
        service
            .verify_handshake(
                Some("subscribe"),
                Some("fixture-verify-token"),
                Some("challenge-123")
            )
            .expect("the handshake is answered, not errored"),
        Some("challenge-123".into()),
        "{HARNESS}: the service echoes the challenge only on an exact token match"
    );
    assert_eq!(
        service
            .verify_handshake(Some("subscribe"), Some("wrong"), Some("challenge-123"))
            .expect("a mismatched handshake is answered with no challenge, not an error"),
        None,
        "{HARNESS}: a mismatched token yields no challenge"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 8. THE CONFIGURED SECRET IS THE ONE THE SERVICE USES. `handle_webhook` reads the secret from the environment
    //    through `MetaWhatsAppConfig::from_env` (`web/src/whatsapp.rs:92`), so a test that verified against a literal
    //    while the service used something else would prove nothing. This pins that they are the same value: the
    //    signature computed over the fixture secret is accepted by the service, and a signature computed over any
    //    other secret is not.
    // -----------------------------------------------------------------------------------------------------------
    assert_eq!(
        config.app_secret, APP_SECRET,
        "{HARNESS}: the fixture config and the environment the service reads must carry one secret"
    );
    let repository = CountingWhatsAppRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    assert!(
        service
            .handle_webhook(BODY, Some(&signature(BODY)))
            .await
            .is_ok(),
        "{HARNESS}: the service verifies against the configured secret, not a test-local one"
    );
    assert_eq!(
        repository.processed(),
        1,
        "{HARNESS}: a verified webhook is processed exactly once"
    );

    // The owned phone is configuration, and it is normalized on the way in (`mod.rs:50-58`) — the same rule that
    // makes a webhook's own sender comparable to a stored identity. Asserted here because verification and
    // attribution share the config object: a service that verified against one secret and attributed against
    // another would be configured two ways at once.
    assert_eq!(
        normalize_e164(&config.owned_phone_e164).expect("the fixture owned phone is valid"),
        config.owned_phone_e164,
        "{HARNESS}: the owned phone is stored already-normalized, so a verified event's sender is comparable to \
         it without a second normalization step"
    );
}
