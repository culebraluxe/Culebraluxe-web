//! INT.WHATSAPP — duplicate event (TST-INT-WHATSAPP-004).
//!
//! Contract: **the same WhatsApp message delivered twice is one message.** Meta retries, and it retries a lot —
//! any 2xx that took longer than its timeout comes back. If a retry were processed as new work it would create a
//! second interaction, double a counter, and — worst — look to a person like the correspondent sent the same
//! message twice.
//!
//! There are THREE independent idempotency keys on this path, and the contract is that all three hold. Each is
//! production code, and each is asserted through the boundary that owns it:
//!
//! 1. **the landing table** — `l_whatsapp` is unique on `(source_account, source_message_id)`
//!    (`db/migrations/158_l_call_l_whatsapp_l_calendar.sql:84-85`) and `WhatsAppDao::land` returns
//!    `Ok(inserted.is_some())` (`db/src/whatsapp.rs:109`), so the second insert is reported as "not new".
//! 2. **the canonical inbox** — `integration_inbox` is unique on `(source, source_account, external_event_id)`
//!    (`db/migrations/044_integration_inbox.sql:100-101`), and a replay does not re-process: it re-reads the row
//!    and `replay_outcome` (`db/src/whatsapp.rs:445-466`) maps the recorded status back to the outcome the first
//!    delivery produced.
//! 3. **the interaction** — `workflow_command_receipt` is keyed on
//!    `integration-inbox:{source}:{external_event_id}` (`db/src/whatsapp.rs:262-277`), so even a replay that
//!    re-reached the interaction could not duplicate it.
//!
//! What this test pins is the part that is observable without a database: **the outcomes are distinct, and the
//! service maps each one to its own wire answer.** `WhatsAppProcessOutcome` (`db/src/whatsapp.rs:38-57`) carries
//! seven variants, and collapsing any two of them would be a silent data bug:
//!
//! - `Duplicate { interaction_id }` is NOT `Completed { created: false }`. The first means "we already know this
//!   event"; the second means "we processed it, and this time we did not create". A replay of a *completed* event
//!   returns `Completed { created: false }` (`replay_outcome:447-454`) — not `Duplicate` — and a caller that
//!   treated those the same could never tell a fresh message from a replay.
//! - `Duplicate` reports **no** `resolved_person_id`. A duplicate must not re-resolve a person, or a stale replay
//!   could re-attribute the message to whoever owns the number now.
//! - `Duplicate` with `interaction_id: None` is still a duplicate — the absence of the id degrades the answer,
//!   it does not change what happened.
//!
//! The negative cases are the ways idempotency breaks, and each is refused or reported rather than tolerated:
//!
//! - **a replay is never reported as a fresh completion.** Asserted through the service's own outcome mapping,
//!   which is what a provider reads.
//! - **a duplicate never resolves a person.** `resolved_person_id` is `None`, so a replay cannot re-attribute.
//! - **a duplicate never projects a relationship.** The service increments `relationship_projected` only on
//!   `Completed` (`web/src/whatsapp.rs:184-195`), so a duplicate cannot trigger the read-model refresh that would
//!   otherwise do redundant work on every Meta retry.
//! - **a landing-table "already seen" is not an error.** `land` returning `false` still proceeds to canonical
//!   processing — the landing table records raw arrivals, and refusing to process because the raw copy is already
//!   there would drop the message entirely.
//! - **an in-flight claim is not a duplicate.** `InFlight` means another worker holds it, and is a distinct answer:
//!   reporting it as a duplicate would tell a caller the message was handled when it is still being handled.
//!
//! Level: L1 Component — `web::whatsapp` against a fake repository at the production port, plus the production
//! outcome enum. No database, no socket, no PROD.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test int_whatsapp__004__duplicate_event

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use db::{DbResult, WhatsAppCanonicalInput, WhatsAppLandingInput, WhatsAppProcessOutcome};
use serde_json::json;
use services::{
    CapturingAuditPort, CapturingDomainEventPort, CapturingServiceErrorSink,
    DefaultAuthorizationPort, ServiceInfrastructure,
};
use web::whatsapp::{WhatsAppRepository, WhatsAppService};

const HARNESS: &str = "INT.WHATSAPP/004";

const OWNED_PHONE_NUMBER_ID: &str = "999888";
const OWNED_PHONE: &str = "+17875550000";
const SENDER: &str = "17875551212";

fn configure_environment() {
    std::env::set_var("WHATSAPP_APP_SECRET", "fixture-app-secret-0123456789");
    std::env::set_var("WHATSAPP_PHONE_NUMBER_ID", OWNED_PHONE_NUMBER_ID);
    std::env::set_var("WHATSAPP_OWNED_PHONE_E164", OWNED_PHONE);
    std::env::set_var("WHATSAPP_VERIFY_TOKEN", "fixture-verify-token");
}

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

/// One inbound message. `id` is the external event id — the idempotency key, so two fixtures with the same `id`
/// are the same message as far as every layer below the service is concerned.
fn inbound(id: &str, body: &str) -> String {
    serde_json::to_string(&json!({
        "object": "whatsapp_business_account",
        "entry": [{"changes": [{"value": {
            "messaging_product": "whatsapp",
            "metadata": {"phone_number_id": OWNED_PHONE_NUMBER_ID},
            "contacts": [{"wa_id": SENDER, "profile": {"name": "Ami Torres"}}],
            "messages": [{
                "from": SENDER, "to": OWNED_PHONE, "id": id, "timestamp": "1780000000",
                "type": "text", "text": {"body": body}
            }],
            "message_echoes": []
        }}]}]
    }))
    .expect("a built envelope serializes")
}

/// A repository fake that answers with a scripted outcome per call and records what it was asked.
///
/// `outcomes` is consumed one entry per `process_event`, and the last entry repeats once exhausted, so a two-call
/// test can script "first delivery, then replay" without the fake inventing a sequence.
#[derive(Clone)]
struct ScriptedRepository {
    landed: Arc<Mutex<Vec<WhatsAppLandingInput>>>,
    processed: Arc<Mutex<Vec<WhatsAppCanonicalInput>>>,
    /// What `land` reports as newly inserted, mirroring `Ok(inserted.is_some())`.
    land_inserted: bool,
    outcomes: Arc<Mutex<Vec<WhatsAppProcessOutcome>>>,
    refreshes: Arc<Mutex<usize>>,
}

impl ScriptedRepository {
    fn new(land_inserted: bool, outcomes: Vec<WhatsAppProcessOutcome>) -> Self {
        Self {
            landed: Arc::new(Mutex::new(Vec::new())),
            processed: Arc::new(Mutex::new(Vec::new())),
            land_inserted,
            outcomes: Arc::new(Mutex::new(outcomes)),
            refreshes: Arc::new(Mutex::new(0)),
        }
    }

    fn landed(&self) -> Vec<WhatsAppLandingInput> {
        self.landed.lock().expect("never poisoned").clone()
    }

    fn processed(&self) -> Vec<WhatsAppCanonicalInput> {
        self.processed.lock().expect("never poisoned").clone()
    }

    fn refreshes(&self) -> usize {
        *self.refreshes.lock().expect("never poisoned")
    }
}

#[async_trait]
impl WhatsAppRepository for ScriptedRepository {
    async fn land(&self, input: &WhatsAppLandingInput) -> DbResult<bool> {
        self.landed
            .lock()
            .expect("never poisoned")
            .push(input.clone());
        Ok(self.land_inserted)
    }

    async fn process_event(
        &self,
        input: &WhatsAppCanonicalInput,
    ) -> DbResult<WhatsAppProcessOutcome> {
        self.processed
            .lock()
            .expect("never poisoned")
            .push(input.clone());
        let mut outcomes = self.outcomes.lock().expect("never poisoned");
        let outcome = if outcomes.len() > 1 {
            outcomes.remove(0)
        } else {
            outcomes
                .first()
                .cloned()
                .unwrap_or(WhatsAppProcessOutcome::Duplicate {
                    interaction_id: None,
                })
        };
        Ok(outcome)
    }

    async fn refresh_client_read_models(&self) -> DbResult<()> {
        *self.refreshes.lock().expect("never poisoned") += 1;
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-INT-WHATSAPP-004); the file and the assay use it.
async fn int_whatsapp__004__duplicate_event() {
    configure_environment();

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE HAPPY PATH, so every assertion below is about the duplicate and not about a broken fixture.
    // -----------------------------------------------------------------------------------------------------------
    let repository = ScriptedRepository::new(
        true,
        vec![WhatsAppProcessOutcome::Completed {
            person_id: "person-1".into(),
            interaction_id: "interaction-1".into(),
            created: true,
        }],
    );
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let raw = inbound("wamid.dup.1", "Is the villa still available?");
    let first = service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a signed inbound webhook is processed");

    assert_eq!(
        first.accepted, 1,
        "{HARNESS}: the first delivery is accepted"
    );
    assert_eq!(
        first.relationship_projected, 1,
        "{HARNESS}: the first delivery projects one relationship, which is what triggers the read-model refresh"
    );
    assert_eq!(first.outcomes[0].outcome, "completed");
    assert_eq!(
        first.outcomes[0].resolved_person_id.as_deref(),
        Some("person-1"),
        "{HARNESS}: the first delivery names the person it was attributed to"
    );
    assert_eq!(
        repository.refreshes(),
        1,
        "{HARNESS}: exactly one read-model refresh follows the first delivery"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. A REPLAY OF A COMPLETED EVENT IS `Completed { created: false }`, NOT `Duplicate`. This is the distinction
    //    `replay_outcome` (`db/src/whatsapp.rs:447-454`) exists to make: the event WAS processed, we are simply
    //    not processing it again. Reporting it as a duplicate would tell a caller the message was a repeat
    //    delivery when it was a repeat PROCESSING of the original — a different thing to reason about during an
    //    incident.
    // -----------------------------------------------------------------------------------------------------------
    let repository = ScriptedRepository::new(
        false,
        vec![
            WhatsAppProcessOutcome::Completed {
                person_id: "person-1".into(),
                interaction_id: "interaction-1".into(),
                created: true,
            },
            WhatsAppProcessOutcome::Completed {
                person_id: "person-1".into(),
                interaction_id: "interaction-1".into(),
                created: false,
            },
        ],
    );
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let raw = inbound("wamid.dup.1", "Is the villa still available?");

    let first_delivery = service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a signed inbound webhook is processed");
    let replay = service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a replay is answered, not refused");
    assert_eq!(
        replay.accepted, 1,
        "{HARNESS}: the replay is one delivery, and it is recognised by the repository rather than the service \
         dropping it — both deliveries reach processing"
    );
    assert_eq!(
        first_delivery.outcomes[0].outcome, "completed",
        "{HARNESS}: the first delivery completed"
    );
    assert_eq!(
        replay.outcomes[0].outcome, "completed",
        "{HARNESS}: a replay of a COMPLETED event still reports `completed` — it was processed, we are just not \
         processing it again, and calling that a duplicate would misdescribe the history"
    );
    assert_eq!(
        replay.outcomes[0].resolved_person_id.as_deref(),
        Some("person-1"),
        "{HARNESS}: a completed replay carries the person the original resolved to — the same answer, not a new \
         lookup"
    );
    assert_eq!(
        replay.relationship_projected, 1,
        "{HARNESS}: the replay is reported as a completion, so the refresh gate `web/src/whatsapp.rs:229` fires — \
         which is correct: a completed replay still means the work is done"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. A GENUINE DUPLICATE. `Duplicate { interaction_id }` is the inbox's own record that this external event
    //    has already been accounted for (`replay_outcome:455-457`). It is a DIFFERENT answer from a completed
    //    replay, and the two must not collapse into each other.
    // -----------------------------------------------------------------------------------------------------------
    let repository = ScriptedRepository::new(
        false,
        vec![WhatsAppProcessOutcome::Duplicate {
            interaction_id: Some("interaction-1".into()),
        }],
    );
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let raw = inbound("wamid.dup.2", "Is the villa still available?");
    let duplicate = service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a duplicate is answered, not refused");

    assert_eq!(
        duplicate.accepted, 1,
        "{HARNESS}: the delivery is accepted — a duplicate is a normal answer, not an error"
    );
    assert_eq!(
        duplicate.outcomes[0].outcome, "duplicate",
        "{HARNESS}: the answer says duplicate, which is distinct from the `completed` of section 2"
    );
    assert_ne!(
        duplicate.outcomes[0].outcome, "completed",
        "{HARNESS}: a duplicate is never reported as a completion — that would make a retry indistinguishable \
         from a first delivery"
    );
    assert_eq!(
        duplicate.outcomes[0].interaction_id.as_deref(),
        Some("interaction-1"),
        "{HARNESS}: the interaction the original produced is carried back, so a caller can find the first \
         delivery's record"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — A DUPLICATE NEVER RE-RESOLVES A PERSON. `resolved_person_id` is `None`, so a stale replay
    //    cannot re-attribute a message to whoever owns the number now. This is the failure that would be hardest
    //    to notice and worst to have: the message is filed against the wrong person, with no error anywhere.
    // -----------------------------------------------------------------------------------------------------------
    assert_eq!(
        duplicate.outcomes[0].resolved_person_id, None,
        "{HARNESS}: a duplicate resolves nobody — re-attributing a stale replay to the number's current owner \
         would silently file the message against the wrong person"
    );
    assert_eq!(
        duplicate.relationship_projected, 0,
        "{HARNESS}: a duplicate projects no relationship, because the projection is what writes the link"
    );
    assert_eq!(
        repository.refreshes(),
        0,
        "{HARNESS}: and triggers no read-model refresh — a refresh is only for work that was actually done, and \
         `web/src/whatsapp.rs:229` gates it on `relationship_projected > 0`"
    );
    assert_eq!(
        repository.landed().len(),
        1,
        "{HARNESS}: the landing row is still written: it records that the delivery HAPPENED, which is evidence \
         worth keeping even when the message itself is a duplicate"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. A DUPLICATE WITH NO INTERACTION ID IS STILL A DUPLICATE. Losing the id degrades the answer — a caller
    //    cannot follow the pointer — but it does not change what happened, and it must not become a completion or
    //    a failure.
    // -----------------------------------------------------------------------------------------------------------
    let repository = ScriptedRepository::new(
        false,
        vec![WhatsAppProcessOutcome::Duplicate {
            interaction_id: None,
        }],
    );
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let raw = inbound("wamid.dup.3", "hello");
    let result = service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a duplicate with no interaction id is still answered normally");
    assert_eq!(
        result.outcomes[0].outcome, "duplicate",
        "{HARNESS}: a missing interaction id does not change the answer's kind — the event is still a duplicate"
    );
    assert_eq!(
        result.outcomes[0].interaction_id, None,
        "{HARNESS}: and the missing id is reported as missing rather than invented"
    );
    assert_eq!(
        result.outcomes[0].resolved_person_id, None,
        "{HARNESS}: it still resolves nobody"
    );
    assert_eq!(result.relationship_projected, 0);
    assert_eq!(
        repository.refreshes(),
        0,
        "{HARNESS}: and still triggers no refresh"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. NEGATIVE — A LANDING TABLE THAT ALREADY HAS THE ROW IS NOT AN ERROR. `land` returns
    //    `Ok(inserted.is_some())` (`db/src/whatsapp.rs:109`), so a re-delivery reports `false` rather than
    //    failing. The canonical write still proceeds: the landing table records raw ARRIVALS, and refusing to
    //    process because the raw copy is already there would drop the message entirely.
    // -----------------------------------------------------------------------------------------------------------
    let repository = ScriptedRepository::new(
        false,
        vec![WhatsAppProcessOutcome::Completed {
            person_id: "person-1".into(),
            interaction_id: "interaction-1".into(),
            created: true,
        }],
    );
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let raw = inbound("wamid.dup.4", "hello");
    let already_landed = service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("an already-landed row is not a failure");
    assert_eq!(
        already_landed.accepted, 1,
        "{HARNESS}: a delivery whose landing row already exists is still processed — the landing table is a \
         record of arrivals, not a gate on canonical processing"
    );
    assert_eq!(
        repository.landed().len(),
        1,
        "{HARNESS}: the landing table is asked exactly once per delivery"
    );
    assert_eq!(
        repository.landed()[0].source_message_id, "wamid.dup.4",
        "{HARNESS}: and it is asked with the external event id, which is the key its unique constraint is on \
         (`l_whatsapp_source_unique`)"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. THE OTHER NON-COMPLETION ANSWERS ARE DISTINCT FROM A DUPLICATE. `InFlight`, `ResolutionRequired` and
    //    `Rejected` each say something different, and collapsing any of them into `duplicate` would tell a
    //    provider or an operator the wrong thing:
    //
    //    - `in_flight` — another worker holds the claim. It is NOT handled; it is being handled. Reporting
    //      `duplicate` would make a concurrent worker look like a repeat delivery.
    //    - `resolution_required` — the number resolved to nobody. The message is real and waiting for a person
    //      to be matched, not a duplicate.
    //    - `rejected` — the message was refused outright.
    let distinct: Vec<(WhatsAppProcessOutcome, &str, &str)> = vec![
        (
            WhatsAppProcessOutcome::InFlight,
            "in_flight",
            "another worker holds the claim, so the message is being handled rather than handled",
        ),
        (
            WhatsAppProcessOutcome::ResolutionRequired,
            "resolution_required",
            "the correspondent's number matched nobody, so the message is waiting for a person",
        ),
        (
            WhatsAppProcessOutcome::Rejected,
            "rejected",
            "the message was refused outright",
        ),
        (
            WhatsAppProcessOutcome::FailedRetryable { attempts: 1 },
            "failed_retryable",
            "the write failed and may be retried",
        ),
        (
            WhatsAppProcessOutcome::Poisoned { attempts: 3 },
            "poisoned",
            "the write failed too many times and the event is abandoned",
        ),
    ];

    for (outcome, expected, described) in distinct {
        let repository = ScriptedRepository::new(true, vec![outcome]);
        let service = WhatsAppService::new(repository.clone(), infrastructure());
        let raw = inbound("wamid.dup.distinct", "hello");
        let result = service
            .handle_webhook(&raw, Some(&signature(&raw)))
            .await
            .expect("every non-completion outcome is answered, not an error");
        assert_eq!(
            result.outcomes[0].outcome, expected,
            "{HARNESS}: {described} is reported as `{expected}` and not as a duplicate or a completion"
        );
        assert_eq!(
            result.outcomes[0].resolved_person_id, None,
            "{HARNESS}: {described}, so nobody is resolved"
        );
        assert_eq!(
            result.relationship_projected, 0,
            "{HARNESS}: {described}, so no relationship is projected"
        );
    }

    // `FailedRetryable` additionally sets the flag the route turns into a 503, so Meta is told to come back.
    let repository = ScriptedRepository::new(
        true,
        vec![WhatsAppProcessOutcome::FailedRetryable { attempts: 1 }],
    );
    let service = WhatsAppService::new(repository, infrastructure());
    let raw = inbound("wamid.dup.retryable", "hello");
    let retryable = service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a retryable failure is answered with its outcome");
    assert!(
        retryable.retryable_failure,
        "{HARNESS}: a retryable failure sets the flag the webhook route maps to 503 `WHATSAPP_RETRYABLE_FAILURE`, \
         so Meta redelivers instead of treating the message as delivered"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 8. TWO DIFFERENT MESSAGES ARE NOT DUPLICATES OF EACH OTHER. The idempotency key is the external event id,
    //    so two messages with different ids are two messages. This is the negative control for the whole story:
    //    a dedup that fired on anything other than the key would pass every case above and still be wrong.
    // -----------------------------------------------------------------------------------------------------------
    let repository = ScriptedRepository::new(
        true,
        vec![WhatsAppProcessOutcome::Completed {
            person_id: "person-1".into(),
            interaction_id: "interaction-1".into(),
            created: true,
        }],
    );
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let first_raw = inbound("wamid.dup.a", "one");
    let second_raw = inbound("wamid.dup.b", "two");
    service
        .handle_webhook(&first_raw, Some(&signature(&first_raw)))
        .await
        .expect("the first message is processed");
    service
        .handle_webhook(&second_raw, Some(&signature(&second_raw)))
        .await
        .expect("the second message is processed");

    let processed = repository.processed();
    assert_eq!(
        processed.len(),
        2,
        "{HARNESS}: two messages with different external ids are two messages, both reaching canonical processing"
    );
    let ids: Vec<&str> = processed
        .iter()
        .map(|input| input.external_event_id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec!["wamid.dup.a", "wamid.dup.b"],
        "{HARNESS}: each is keyed on its OWN external event id — the dedup key is the id, not the sender and not \
         the content, so a correspondent sending the same words twice is two messages"
    );

    // The same message with the same content under a different id is a separate delivery of a separate event.
    let repository = ScriptedRepository::new(
        true,
        vec![WhatsAppProcessOutcome::Completed {
            person_id: "person-1".into(),
            interaction_id: "interaction-1".into(),
            created: true,
        }],
    );
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let repeated_words = inbound("wamid.dup.c", "Is the villa still available?");
    service
        .handle_webhook(&repeated_words, Some(&signature(&repeated_words)))
        .await
        .expect("a repeated message body under a new id is processed");
    let processed = repository.processed();
    assert_eq!(
        processed.len(),
        1,
        "{HARNESS}: the repeated body is not deduplicated against the earlier one — a person really can send the \
         same sentence twice, and the external id is the only thing that says they did"
    );
    assert_eq!(
        processed[0].external_event_id, "wamid.dup.c",
        "{HARNESS}: and it is filed under the id Meta gave it"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 9. THE LANDING KEY IS THE ONE THE UNIQUE CONSTRAINT USES. `l_whatsapp` is unique on
    //    `(source_account, source_message_id)`, so those are the two fields a re-delivery collides on. Asserting
    //    them is asserting that the fake was asked the question the database will ask.
    // -----------------------------------------------------------------------------------------------------------
    let repository = ScriptedRepository::new(
        true,
        vec![WhatsAppProcessOutcome::Completed {
            person_id: "person-1".into(),
            interaction_id: "interaction-1".into(),
            created: true,
        }],
    );
    let service = WhatsAppService::new(repository.clone(), infrastructure());
    let raw = inbound("wamid.dup.key", "hello");
    service
        .handle_webhook(&raw, Some(&signature(&raw)))
        .await
        .expect("a signed inbound webhook is processed");

    let landed = repository.landed();
    assert_eq!(
        landed[0].source_message_id, "wamid.dup.key",
        "{HARNESS}: the landing row's source_message_id is Meta's message id — the column its unique constraint \
         is on, so a re-delivery collides here"
    );
    assert_eq!(
        landed[0].source_account.as_deref(),
        Some(format!("meta-{OWNED_PHONE_NUMBER_ID}").as_str()),
        "{HARNESS}: and the other half of that constraint is the source account, so the same message id arriving \
         through a DIFFERENT account is a different landing row"
    );
    // The canonical input carries the same id, because the inbox's unique constraint is
    // `(source, source_account, external_event_id)` — the same pair, one layer up.
    assert_eq!(
        repository.processed()[0].external_event_id, "wamid.dup.key",
        "{HARNESS}: the canonical inbox is keyed on the same pair, so both layers agree on what identifies an \
         event — and a disagreement between them would let one layer dedupe what the other does not"
    );
}
