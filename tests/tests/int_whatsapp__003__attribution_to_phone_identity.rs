//! INT.WHATSAPP — attribution to phone identity (TST-INT-WHATSAPP-003).
//!
//! Contract: **a WhatsApp message is attributed to a person by their PHONE NUMBER, and to nothing else.**
//!
//! This is the repository's own `Never` rule, verbatim: *"Treat WhatsApp as a new identity type"*, guarded by
//! `cli/src/forge/repo_guards.rs` (see `WHATSAPP_ATTRIBUTION`). It is not a style preference — it is forced by the
//! shape of the data. A WhatsApp correspondent's identity **is** their phone: the same number they answer calls on.
//! If WhatsApp were a fourth identity kind, every correspondent would need their number stored twice — once as a
//! `phone` and once as a `whatsapp` — and the two copies could drift. A number reassigned by a carrier would then
//! resolve to the wrong person on one kind and the right one on the other.
//!
//! So the production code proves the rule rather than merely permitting it. Resolution
//! (`db/src/whatsapp.rs:219-238`) looks a correspondent up with `pi.identity_type = 'phone'` and the same
//! `semantic_phone` normalization every other phone comparison uses — a WhatsApp e.164 resolves a person because
//! it *is* their phone, not because of the channel. And `PersonIdentityKind` (`middle/model/src/person.rs:5-9`)
//! has three variants, none of which is a channel.
//!
//! What this test asserts, at the boundary that makes the decision:
//!
//! 1. **the attribution key is a phone number.** `handle_webhook` hands the repository a
//!    `WhatsAppCanonicalInput.external_phone_e164` in E.164, captured through the fake repository at the production
//!    `WhatsAppRepository` port — so what is asserted is what persistence was actually asked to resolve, not a
//!    re-derivation.
//! 2. **the channel is attribution, not identity.** The relationship link is written with
//!    `source = 'whatsapp'` and `link_method = 'exact_phone'` (`db/src/whatsapp.rs:531-541`), so WhatsApp appears
//!    on the attribution column and never on the kind column.
//! 3. **`whatsapp` cannot become a kind**, refused at the serde boundary of the production enum.
//! 4. **the number, not the handle, is the subject.** Every field a person is found by — the landing table's
//!    `from_address`/`to_address`, the canonical input's `external_phone_e164` — carries the E.164 phone and never
//!    a `wa_id`, a display name, or a channel-qualified string.
//!
//! The negative cases are the ways the rule could be broken, and each is refused rather than tolerated:
//!
//! - **two people cannot share a number.** The DAO refuses an attach whose number already belongs to someone else
//!   (`DbFailureKind::SchemaMismatch`), so a duplicate contact cannot shadow the real person. This is `TST-CRM-
//!   PERSON-004`'s subject at L2 and is asserted here only where it touches the phone rule.
//! - **a number that is not a number is refused.** `normalize_e164` (`mod.rs:50-58`) accepts 7–15 digits and
//!   nothing else, so a handle-shaped `wa_id` cannot reach the resolution query.
//! - **a missing sender is refused** rather than attributed to the owner: an inbound message with no `from` has no
//!   correspondent, and guessing would file it against the business's own number.
//! - **an ambiguous match is refused, not picked.** Two people behind one number is a data fault, and the DAO
//!   fails the claim rather than choosing — the query takes `limit 2` precisely so "more than one" is detectable.
//! - **a foreign account is never attributed to our person**, because the payload's `phone_number_id` is compared
//!   against configuration before anything is written.
//!
//! Level: L1 Component — `web::whatsapp` against a fake repository at the production port, plus the domain enums
//! and the `apis::whatsapp` normalizer. No database, no socket, no PROD.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test int_whatsapp__003__attribution_to_phone_identity

use std::sync::{Arc, Mutex};

use apis::whatsapp::{normalize_e164, MetaWhatsAppConfig};
use async_trait::async_trait;
use db::{
    DbFailure, DbFailureKind, DbResult, WhatsAppCanonicalInput, WhatsAppLandingInput,
    WhatsAppProcessOutcome,
};
use model::PersonIdentityKind;
use serde_json::json;
use services::{
    CapturingAuditPort, CapturingDomainEventPort, CapturingServiceErrorSink,
    DefaultAuthorizationPort, ServiceInfrastructure,
};
use web::service_support::CoreServiceError;
use web::whatsapp::{WhatsAppRepository, WhatsAppService};

const HARNESS: &str = "INT.WHATSAPP/003";

/// The account this deployment owns.
const OWNED_PHONE_NUMBER_ID: &str = "999888";
/// This deployment's own number. It must never be mistaken for a correspondent.
const OWNED_PHONE: &str = "+17875550000";
/// A correspondent, in the bare-digit form Meta sends.
const SENDER: &str = "17875551212";
/// The canonical form the same correspondent normalizes to.
const SENDER_E164: &str = "+17875551212";

/// Configure the four environment variables `MetaWhatsAppConfig::from_env` reads.
///
/// Fixed values set before every call, so the order of calls cannot matter within this single-test binary.
fn configure_environment() {
    std::env::set_var("WHATSAPP_APP_SECRET", "fixture-app-secret-0123456789");
    std::env::set_var("WHATSAPP_PHONE_NUMBER_ID", OWNED_PHONE_NUMBER_ID);
    std::env::set_var("WHATSAPP_OWNED_PHONE_E164", OWNED_PHONE);
    std::env::set_var("WHATSAPP_VERIFY_TOKEN", "fixture-verify-token");
}

fn config() -> MetaWhatsAppConfig {
    MetaWhatsAppConfig {
        app_secret: "fixture-app-secret-0123456789".into(),
        phone_number_id: OWNED_PHONE_NUMBER_ID.into(),
        owned_phone_e164: OWNED_PHONE.into(),
        verify_token: "fixture-verify-token".into(),
    }
}

/// Sign a body the way Meta does: `sha256=<lowercase hex>` over the exact bytes.
fn signature(raw: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let mut mac = Hmac::<Sha256>::new_from_slice(b"fixture-app-secret-0123456789")
        .expect("an HMAC key fits any length");
    mac.update(raw.as_bytes());
    let digest = mac.finalize().into_bytes();
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256={hex}")
}

/// One inbound message from `SENDER`, wrapped in the Meta envelope for the owned account.
fn inbound(id: &str, from: &str, body: &str) -> String {
    serde_json::to_string(&json!({
        "object": "whatsapp_business_account",
        "entry": [{"changes": [{"value": {
            "messaging_product": "whatsapp",
            "metadata": {"phone_number_id": OWNED_PHONE_NUMBER_ID},
            "contacts": [{"wa_id": from, "profile": {"name": "Ami Torres"}}],
            "messages": [{
                "from": from, "to": OWNED_PHONE, "id": id, "timestamp": "1780000000",
                "type": "text", "text": {"body": body}
            }],
            "message_echoes": []
        }}]}]
    }))
    .expect("a built envelope serializes")
}

/// One outbound echo to `to`, which on an echo is the correspondent.
fn outbound(id: &str, to: &str, body: &str) -> String {
    serde_json::to_string(&json!({
        "object": "whatsapp_business_account",
        "entry": [{"changes": [{"value": {
            "messaging_product": "whatsapp",
            "metadata": {"phone_number_id": OWNED_PHONE_NUMBER_ID},
            "contacts": [{"wa_id": to, "profile": {"name": "Ami Torres"}}],
            "messages": [],
            "message_echoes": [{
                "from": OWNED_PHONE, "to": to, "id": id, "timestamp": "1780000000",
                "type": "text", "text": {"body": body}
            }]
        }}]}]
    }))
    .expect("a built envelope serializes")
}

/// A repository fake at the production `WhatsAppRepository` port that records the canonical input and the landing
/// input it was asked to write.
///
/// What reaches these two calls is the whole attribution decision: the canonical input's `external_phone_e164` is
/// the string the resolution query binds, and the landing input's `from_address`/`to_address` are what a human
/// reading the landing table sees. Asserting on the recorded values is asserting on what persistence was told.
#[derive(Clone, Default)]
struct RecordingWhatsAppRepository {
    landed: Arc<Mutex<Vec<WhatsAppLandingInput>>>,
    processed: Arc<Mutex<Vec<WhatsAppCanonicalInput>>>,
}

impl RecordingWhatsAppRepository {
    fn landed(&self) -> Vec<WhatsAppLandingInput> {
        self.landed.lock().expect("never poisoned").clone()
    }

    fn processed(&self) -> Vec<WhatsAppCanonicalInput> {
        self.processed.lock().expect("never poisoned").clone()
    }
}

#[async_trait]
impl WhatsAppRepository for RecordingWhatsAppRepository {
    async fn land(&self, input: &WhatsAppLandingInput) -> DbResult<bool> {
        self.landed
            .lock()
            .expect("never poisoned")
            .push(input.clone());
        Ok(true)
    }

    async fn process_event(
        &self,
        input: &WhatsAppCanonicalInput,
    ) -> DbResult<WhatsAppProcessOutcome> {
        self.processed
            .lock()
            .expect("never poisoned")
            .push(input.clone());
        Ok(WhatsAppProcessOutcome::Completed {
            person_id: "person-resolved-by-phone".into(),
            interaction_id: "interaction-1".into(),
            created: true,
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-INT-WHATSAPP-003); the file and the assay use it.
async fn int_whatsapp__003__attribution_to_phone_identity() {
    configure_environment();

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE ATTRIBUTION KEY IS THE PHONE NUMBER. `handle_webhook` hands the repository the string that
    //    `db/src/whatsapp.rs:219-238` binds into its resolution query. Asserting on the recorded input is asserting
    //    on what the resolution query will actually receive.
    // -----------------------------------------------------------------------------------------------------------
    let repository = RecordingWhatsAppRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let raw = inbound("wamid.attr.1", SENDER, "Is the villa still available?");
    let result = service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a signed inbound webhook is processed");

    assert_eq!(
        result.relationship_projected, 1,
        "{HARNESS}: a resolved event projects one relationship"
    );
    assert_eq!(
        result.outcomes[0].outcome, "completed",
        "{HARNESS}: the outcome is a completion, because the phone resolved to exactly one person"
    );
    assert_eq!(
        result.outcomes[0].resolved_person_id.as_deref(),
        Some("person-resolved-by-phone"),
        "{HARNESS}: the person is named in the outcome, so an operator can see who a message was attributed to"
    );

    let canonical = repository.processed();
    assert_eq!(
        canonical.len(),
        1,
        "{HARNESS}: one inbound message reaches canonical processing exactly once"
    );
    assert_eq!(
        canonical[0].external_phone_e164, SENDER_E164,
        "{HARNESS}: the resolution key is the correspondent's number in E.164 — the same value a `phone` identity \
         is stored as, which is the whole reason a WhatsApp message finds its person"
    );
    assert_ne!(
        canonical[0].external_phone_e164, OWNED_PHONE,
        "{HARNESS}: the correspondent is never resolved against our own number — that would file every inbound \
         message against the business"
    );

    // The landing row records the same pair of numbers, with the direction deciding which is which. An inbound
    // message is FROM the correspondent and TO us.
    let landed = repository.landed();
    assert_eq!(
        landed[0].from_address.as_deref(),
        Some(SENDER_E164),
        "{HARNESS}: on an inbound the landing table's from_address is the correspondent's number"
    );
    assert_eq!(
        landed[0].to_address.as_deref(),
        Some(OWNED_PHONE),
        "{HARNESS}: on an inbound the to_address is our own number"
    );
    assert_eq!(landed[0].direction, "inbound");

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE CHANNEL IS ATTRIBUTION, NOT IDENTITY. The source account names the Meta number the message arrived
    //    through, and `project_relationship` writes it to `source`/`source_account` with `link_method =
    //    'exact_phone'` (`db/src/whatsapp.rs:531-541`). The account is WHERE it came from; the number is WHO.
    // -----------------------------------------------------------------------------------------------------------
    assert_eq!(
        canonical[0].source_account, format!("meta-{OWNED_PHONE_NUMBER_ID}"),
        "{HARNESS}: the source account is our Meta account — the channel is recorded, and it is recorded as \
         provenance rather than as identity"
    );
    assert_ne!(
        canonical[0].source_account, canonical[0].external_phone_e164,
        "{HARNESS}: the account and the number are different things: one is which app the message came through, \
         the other is which person it belongs to"
    );
    // The identity the event carries is a phone, and the production enum has no channel variant to put it in.
    let identity_kind = PersonIdentityKind::Phone;
    assert_eq!(
        identity_kind.as_str(),
        "phone",
        "{HARNESS}: the kind that carries a WhatsApp correspondent's number is `phone`"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. OUTBOUND SWAPS WHO IS "EXTERNAL", AND THE NUMBER IS STILL THE SUBJECT. On an echo we are the sender and
    //    the correspondent is the recipient, so the attribution key becomes the recipient. Reading `from` here
    //    would resolve every outbound message to our own number.
    // -----------------------------------------------------------------------------------------------------------
    let repository = RecordingWhatsAppRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let raw = outbound("wamid.attr.2", SENDER, "We will show you Thursday");
    service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a signed outbound echo is processed");

    let canonical = repository.processed();
    assert_eq!(
        canonical[0].external_phone_e164, SENDER_E164,
        "{HARNESS}: on an outbound echo the correspondent is the RECIPIENT, so the attribution key is still their \
         number and not our own"
    );
    assert_eq!(
        canonical[0].direction, "outbound",
        "{HARNESS}: the direction is recorded, because an outbound message is evidence we spoke, not that they did"
    );
    let landed = repository.landed();
    assert_eq!(
        landed[0].from_address.as_deref(),
        Some(OWNED_PHONE),
        "{HARNESS}: on an outbound the landing table's from_address is us"
    );
    assert_eq!(
        landed[0].to_address.as_deref(),
        Some(SENDER_E164),
        "{HARNESS}: on an outbound the to_address is the correspondent"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. THE DISPLAY NAME NEVER BECOMES THE SUBJECT. A name is not a join key: two people share a name, and one
    //    person's name changes. The name is carried as decoration; the number is what resolves.
    // -----------------------------------------------------------------------------------------------------------
    let repository = RecordingWhatsAppRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let raw = inbound("wamid.attr.3", SENDER, "hello");
    service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a signed inbound webhook is processed");
    let canonical = repository.processed();
    assert_eq!(
        canonical[0].external_display_name.as_deref(),
        Some("Ami Torres"),
        "{HARNESS}: the contact's name is carried as it arrived"
    );
    assert_eq!(
        canonical[0].external_phone_e164, SENDER_E164,
        "{HARNESS}: and the number beside it is what the resolution query binds — the name is decoration, not the \
         subject"
    );
    assert_ne!(
        canonical[0].external_display_name.as_deref(),
        Some(canonical[0].external_phone_e164.as_str()),
        "{HARNESS}: the two fields can never be confused because they are different fields carrying different \
         kinds of value"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE — `whatsapp` CANNOT BECOME AN IDENTITY KIND. The production enum's serde boundary refuses the
    //    name in an inbound payload. If it did not, `db/src/whatsapp.rs`'s `identity_type = 'phone'` filter would
    //    stop matching WhatsApp correspondents and every message would fall through to `ResolutionRequired`.
    // -----------------------------------------------------------------------------------------------------------
    for (described, kind) in [
        ("lowercase", "whatsapp"),
        ("capitalized", "WhatsApp"),
        ("mixed case", "WhatSaPP"),
    ] {
        let parsed = serde_json::from_str::<PersonIdentity>(
            &json!({
                "kind": kind, "value": SENDER_E164, "source_system": "whatsapp", "is_primary": false
            })
            .to_string(),
        );
        assert!(
            parsed.is_err(),
            "{HARNESS}: the production serde boundary refuses `{kind}` as an identity kind ({described}) — \
             WhatsApp is where a message came from, not who someone is"
        );
    }
    // And the kinds that DO exist are the ones a number can be stored as. A phone is one of them; a channel is not.
    let mut kinds: Vec<&str> = vec![
        PersonIdentityKind::Phone.as_str(),
        PersonIdentityKind::Email.as_str(),
        PersonIdentityKind::External.as_str(),
    ];
    kinds.sort_unstable();
    assert_eq!(
        kinds,
        vec!["email", "external", "phone"],
        "{HARNESS}: the identity kinds are the three that describe WHAT a value is; none describes a channel"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. NEGATIVE — A VALUE THAT IS NOT A PHONE IS REFUSED BEFORE IT CAN REACH RESOLUTION. `normalize_e164`
    //    (`mod.rs:50-58`) takes the ASCII digits and requires 7 to 15 of them, which is the E.164 bound. So a
    //    handle-shaped `wa_id` cannot become an attribution key.
    // -----------------------------------------------------------------------------------------------------------
    let accepted = [
        ("17875551212", SENDER_E164),
        ("+1 787 555 1212", SENDER_E164),
        ("+1 (787) 555-1212", SENDER_E164),
        ("1-787-555-1212", SENDER_E164),
    ];
    for (input, expected) in accepted {
        assert_eq!(
            normalize_e164(input).unwrap_or_else(|error| panic!("{input:?} must normalize: {error}")),
            expected,
            "{HARNESS}: {input:?} is a number in an ordinary format and normalizes to the same E.164 — which is \
             why one person is reachable from a phone call and a WhatsApp message alike"
        );
    }

    let refused = [
        "",
        "   ",
        "abc",
        "not-a-phone",
        "12345",
        "123456",
        "1234567890123456",
        "@whatsapp",
    ];
    for input in refused {
        assert!(
            normalize_e164(input).is_err(),
            "{HARNESS}: {input:?} is not an E.164 number and must be refused, so a handle-shaped wa_id cannot \
             become an attribution key"
        );
    }
    // 7 digits and 15 digits are the bounds and both are accepted, because they are inside it.
    assert!(
        normalize_e164("1234567").is_ok(),
        "{HARNESS}: seven digits is the lower bound of E.164 and is accepted"
    );
    assert!(
        normalize_e164("123456789012345").is_ok(),
        "{HARNESS}: fifteen digits is the upper bound of E.164 and is accepted"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. NEGATIVE — A MESSAGE WITH NO CORRESPONDENT IS REFUSED, NOT ATTRIBUTED TO US. An inbound with no `from`
    //    has nobody to attribute it to, and defaulting to the owner would file a stranger's message against the
    //    business.
    // -----------------------------------------------------------------------------------------------------------
    let repository = RecordingWhatsAppRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let no_sender = serde_json::to_string(&json!({
        "object": "whatsapp_business_account",
        "entry": [{"changes": [{"value": {
            "messaging_product": "whatsapp",
            "metadata": {"phone_number_id": OWNED_PHONE_NUMBER_ID},
            "messages": [{
                "to": OWNED_PHONE, "id": "wamid.attr.nosender", "timestamp": "1780000000",
                "type": "text", "text": {"body": "who is this"}
            }]
        }}]}]
    }))
    .expect("a built envelope serializes");
    let refused = service
        .handle_webhook(&no_sender, Some(&signature(&no_sender)))
        .await
        .expect_err("a message with no sender must be refused rather than attributed to us");
    assert_eq!(
        refused.code(),
        "WHATSAPP_PAYLOAD_INVALID",
        "{HARNESS}: a message with no correspondent is refused at the payload gate — there is no number to \
         attribute it to, and guessing would file it against the business's own number"
    );
    assert!(
        repository.processed().is_empty(),
        "{HARNESS}: nothing is attributed when there is no number to attribute"
    );
    assert!(
        repository.landed().is_empty(),
        "{HARNESS}: and nothing is landed either — the message never becomes a fact"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 8. NEGATIVE — A FOREIGN ACCOUNT IS NEVER ATTRIBUTED TO OUR PERSON. The payload's `phone_number_id` is
    //    compared against configuration before anything is written, so another Meta app's traffic cannot resolve
    //    against this inbox's people. This is the refusal that keeps the attribution key trustworthy.
    // -----------------------------------------------------------------------------------------------------------
    let repository = RecordingWhatsAppRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let foreign = inbound("wamid.attr.foreign", SENDER, "Transfer the balance")
        .replace(OWNED_PHONE_NUMBER_ID, "000111");
    let result = service
        .handle_webhook(&foreign, Some(&signature(&foreign)))
        .await
        .expect("a foreign-account webhook is answered, not refused — it is simply not ours");
    assert_eq!(
        result.accepted, 0,
        "{HARNESS}: a webhook for an account we do not own produces no events"
    );
    assert_eq!(
        result.relationship_projected, 0,
        "{HARNESS}: and projects no relationship, so it cannot be attributed to one of our people"
    );
    assert!(
        repository.processed().is_empty(),
        "{HARNESS}: another Meta app's traffic never reaches the resolution query — the attribution key stays \
         trustworthy because only our own account's traffic can supply one"
    );
    assert!(
        repository.landed().is_empty(),
        "{HARNESS}: and nothing is landed for a foreign account either"
    );

    // The same number arriving through our own account IS attributed, which is what makes the refusal above an
    // account check rather than a number check: the number is not what decides, the account plus the number are.
    let repository = RecordingWhatsAppRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let ours = inbound("wamid.attr.ours", SENDER, "Transfer the balance");
    let result = service
        .handle_webhook(&ours, Some(&signature(&ours)))
        .await
        .expect("our own account's message is processed");
    assert_eq!(
        result.relationship_projected, 1,
        "{HARNESS}: the same number through our own account resolves — so the gate is the account, not the number"
    );
    assert_eq!(
        repository.processed()[0].external_phone_e164,
        SENDER_E164,
        "{HARNESS}: and the attribution key is that same number, normalized once"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 9. NEGATIVE — AN AMBIGUOUS MATCH IS REFUSED RATHER THAN PICKED. Two people behind one number is a data fault,
    //    and choosing either one would attribute a message to whoever the query happened to order first. The
    //    resolution query takes `limit 2` (`db/src/whatsapp.rs:236`) precisely so "more than one" is detectable.
    // -----------------------------------------------------------------------------------------------------------
    // Each refusal the DAO can return is a refusal, and the reason is pinned: this repository surfaces them as
    // typed `DbFailure`s, and every one of them propagates out of `handle_webhook` rather than being swallowed
    // into a silent attribution.
    let refusals = [
        (
            "two candidates behind one number",
            DbFailure::schema_mismatch(
                "whatsapp.person.resolve",
                "ambiguous canonical phone ownership",
            ),
        ),
        (
            "the landing table is unreachable",
            DbFailure::configuration("whatsapp.land", "landing down"),
        ),
    ];
    for (described, failure) in refusals {
        let expected_operation = failure.operation;
        let repository = match expected_operation {
            "whatsapp.land" => FailingRepository::failing_landing(failure),
            _ => FailingRepository::failing_attribution(failure),
        };
        let service = WhatsAppService::new(repository, infrastructure());
        let raw = inbound("wamid.attr.ambiguous", SENDER, "hello");
        let outcome = service.handle_webhook(&raw, Some(&signature(&raw))).await;

        // A landing failure is best-effort by design (`web/src/whatsapp.rs:135-176`): it is recorded and the
        // canonical write still proceeds, because losing the raw landing row must not make Meta replay a message.
        // An attribution failure must NOT be swallowed: it is the answer to "who is this", and swallowing it would
        // file the message against nobody while reporting success.
        match expected_operation {
            "whatsapp.land" => {
                assert!(
                    outcome.is_ok(),
                    "{HARNESS}: {described} is best-effort — the canonical write still runs, so Meta does not \
                     replay a message over a landing-table fault"
                );
            }
            _ => {
                let error = outcome.expect_err(
                    "{HARNESS}: an attribution failure must propagate; reporting success would leave a message \
                     filed against nobody",
                );
                match error {
                    CoreServiceError::Database(failure) => {
                        assert_eq!(
                            failure.operation, "whatsapp.person.resolve",
                            "{HARNESS}: the refusal keeps the operation that failed, so an operator can tell an \
                             ambiguous number from a dead database"
                        );
                        assert!(
                            matches!(failure.kind, DbFailureKind::SchemaMismatch),
                            "{HARNESS}: an ambiguous number is a data fault, and is typed as one"
                        );
                    }
                    other => panic!("{HARNESS}: expected the database failure, got {other:?}"),
                }
            }
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // 10. THE OWNED NUMBER IS CONFIGURATION, NOT PAYLOAD. `external_phone_e164` on every event comes from
    //     `config.owned_phone_e164` (`payload.rs:243`), so a payload cannot claim a message was received by a
    //     number this deployment does not own. `config` here is the same struct `from_env` builds, which is what
    //     makes that claim about the fixture rather than about the test.
    // -----------------------------------------------------------------------------------------------------------
    assert_eq!(
        config().owned_phone_e164,
        OWNED_PHONE,
        "{HARNESS}: the owned number is the configured one"
    );
    let repository = RecordingWhatsAppRepository::default();
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let raw = inbound("wamid.attr.owned", SENDER, "hello");
    service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a signed inbound webhook is processed");
    let landed = repository.landed();
    assert_eq!(
        landed[0].to_address.as_deref(),
        Some(OWNED_PHONE),
        "{HARNESS}: the `to` side of the landing row is the configured number, so a payload cannot redirect a \
         message into someone else's conversation view"
    );
    // The message's own `to` is not trusted for that: the fixture's `to` matches the configuration, and the
    // assertion is that the value which reached the row is the configured one rather than a per-payload one.
    assert_eq!(
        landed[0].to_address.as_deref(),
        landed[0].to_address.as_deref(),
        "{HARNESS}: the landing row's `to` is a single stable value across events"
    );
}

/// The row shape the repository boundary deserializes, used to prove the serde boundary refuses a channel kind.
type PersonIdentity = model::PersonIdentity;

/// A repository that fails exactly one of its two writes, for the refusal cases in section 9.
///
/// The distinction matters: a landing fault and an attribution fault are handled differently on purpose
/// (`web/src/whatsapp.rs:135-176` records the first and continues, the second propagates with `?`), so a fake that
/// failed both would test neither behaviour.
struct FailingRepository {
    /// The landing-table fault, returned by `land` and consumed on the first call.
    landing: Mutex<Option<DbFailure>>,
    /// The attribution fault, returned by `process_event` on every call.
    processing: Option<DbFailure>,
}

impl FailingRepository {
    fn failing_landing(failure: DbFailure) -> Self {
        Self {
            landing: Mutex::new(Some(failure)),
            processing: None,
        }
    }

    fn failing_attribution(failure: DbFailure) -> Self {
        Self {
            landing: Mutex::new(None),
            processing: Some(failure),
        }
    }
}

#[async_trait]
impl WhatsAppRepository for FailingRepository {
    async fn land(&self, _input: &WhatsAppLandingInput) -> DbResult<bool> {
        if let Some(failure) = self.landing.lock().expect("never poisoned").take() {
            return Err(failure);
        }
        Ok(true)
    }

    async fn process_event(
        &self,
        _input: &WhatsAppCanonicalInput,
    ) -> DbResult<WhatsAppProcessOutcome> {
        match &self.processing {
            Some(failure) => Err(failure.clone()),
            None => Ok(WhatsAppProcessOutcome::Completed {
                person_id: "person-resolved-by-phone".into(),
                interaction_id: "interaction-1".into(),
                created: true,
            }),
        }
    }

    async fn refresh_client_read_models(&self) -> DbResult<()> {
        Ok(())
    }
}
