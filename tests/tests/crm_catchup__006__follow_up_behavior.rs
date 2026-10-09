//! CRM.CATCHUP — follow-up behavior (TST-CRM-CATCHUP-006).
//!
//! Contract: an item the operator has dealt with leaves the queue, and it comes back only when the world
//! changes — a NEWER signal, or an expired snooze. `db/src/catch_up.rs` reads that rule from
//! `catch_up_disposition`:
//!
//!   where (disposition.snoozed_until is null or disposition.snoozed_until <= now())
//!     and (disposition.handled_at is null or disposition.handled_at < s.signal_at)
//!
//! So `handle` silences a row until something newer arrives, and `snooze` silences it for a window — a
//! window this proof moves into the past as DATA rather than waiting three days for it.
//!
//! The signal is an inbound message with no later reply (`unanswered_inbound`, priority 100).
//!
//! Level: L3 Composition. Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test crm_catchup__006__follow_up_behavior -- --ignored

use db::{CatchUpDao, DbTarget};
use model::CatchUpItem;
use test_harness::CrmHarness;
use uuid::Uuid;

const HARNESS: &str = "CrmHarness/L3 Composition";
const REASON: &str = "unanswered_inbound";

async fn connect_dev() -> CrmHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match CrmHarness::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; CrmHarness refuses PROD: {}",
        last.unwrap_or_default()
    );
}

async fn seed_catchup_person(harness: &CrmHarness, display_name: &str) -> String {
    sqlx::query_scalar(
        "insert into person (display_name, role, status) values ($1, 'buyer', 'active') returning id::text",
    )
    .bind(display_name)
    .fetch_one(harness.pool())
    .await
    .expect("a catch-up person seeds")
}

/// One inbound message. `offset` is added to `now()` so the ordering the queue tests is explicit rather
/// than a race between two clock reads.
async fn seed_inbound(harness: &CrmHarness, person_id: &str, offset: &str) -> String {
    sqlx::query_scalar(
        "insert into interaction (person_id, channel, event_type, direction, occurred_at, title, summary) \
         values ($1::uuid, 'email', 'inbound_message', 'inbound', now() + $2::interval, 'Client wrote in', 'No reply yet') \
         returning id::text",
    )
    .bind(person_id)
    .bind(offset)
    .fetch_one(harness.pool())
    .await
    .expect("an inbound interaction seeds")
}

async fn item_for(dao: &CatchUpDao, person_id: &str) -> Option<CatchUpItem> {
    let snapshot = dao.snapshot().await.expect("the catch-up snapshot reads");
    snapshot
        .items
        .into_iter()
        .find(|item| item.person_id == person_id)
}

/// The disposition row for this person and reason: (handled_at is set, snoozed_until).
async fn disposition(
    harness: &CrmHarness,
    person_id: &str,
) -> Option<(bool, Option<chrono::DateTime<chrono::Utc>>)> {
    sqlx::query_as::<_, (bool, Option<chrono::DateTime<chrono::Utc>>)>(
        "select handled_at is not null, snoozed_until from catch_up_disposition \
         where person_id = $1::uuid and reason_code = $2",
    )
    .bind(person_id)
    .bind(REASON)
    .fetch_optional(harness.pool())
    .await
    .expect("the disposition reads")
}

async fn sweep(harness: &CrmHarness, marker: &str) {
    let pattern = format!("{marker}%");
    for statement in [
        "delete from catch_up_disposition where person_id in (select id::text from person where display_name like $1)",
        "delete from interaction where person_id in (select id from person where display_name like $1)",
        "delete from task where person_id in (select id from person where display_name like $1)",
    ] {
        let _ = sqlx::query(statement).bind(&pattern).execute(harness.pool()).await;
    }
    let _ = harness.cleanup(marker).await;
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); CrmHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-CATCHUP-006); the file and the assay use it.
async fn crm_catchup_006__follow_up_behavior() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the follow-up proof runs only on an isolated DEV target"
    );
    let ns = harness.namespace().to_string();
    let marker = format!(
        "TST-CRMCATCHUP006-{ns}-{}",
        &Uuid::new_v4().to_string()[..8]
    );
    let dao = CatchUpDao::new(harness.database().database().clone());

    let person = seed_catchup_person(&harness, &format!("{marker}-g")).await;
    let _first_message = seed_inbound(&harness, &person, "-1 hour").await;

    // 1. An inbound message with no reply is on the queue at the top priority.
    let item = item_for(&dao, &person)
        .await
        .expect("an unanswered inbound message is a catch-up item");
    assert_eq!(
        item.reason_code, REASON,
        "{HARNESS}: an unanswered inbound message reads as {REASON}, not {}",
        item.reason_code
    );
    assert_eq!(
        item.priority, 100,
        "{HARNESS}: an unanswered inbound message outranks everything else"
    );

    // 2. Handling it takes the row off the queue and records the handling.
    dao.handle(&person, REASON)
        .await
        .expect("the item is handled");
    assert_eq!(
        item_for(&dao, &person).await,
        None,
        "{HARNESS}: a handled item leaves the queue until a newer signal arrives"
    );
    let (handled, snoozed_until) = disposition(&harness, &person)
        .await
        .expect("handling records a disposition row");
    assert!(handled, "{HARNESS}: handling stamps handled_at");
    assert_eq!(snoozed_until, None, "{HARNESS}: handling is not a snooze");

    // 3. A NEWER signal revives it: handled_at < signal_at is the rule, so a message that arrives after
    //    the handling is enough. The offset is explicit, so this cannot turn on clock resolution.
    let _second_message = seed_inbound(&harness, &person, "+1 second").await;
    let revived = item_for(&dao, &person)
        .await
        .expect("a newer inbound message puts the person back on the queue");
    assert_eq!(
        revived.reason_code, REASON,
        "{HARNESS}: the revived row carries the same reason, recalculated"
    );

    // 4. Snoozing silences it for a window, and the window is what is recorded.
    dao.snooze(&person, REASON, 3)
        .await
        .expect("the item is snoozed");
    assert_eq!(
        item_for(&dao, &person).await,
        None,
        "{HARNESS}: a snoozed item leaves the queue for its window"
    );
    let (handled, snoozed_until) = disposition(&harness, &person)
        .await
        .expect("snoozing records a disposition row");
    assert!(!handled, "{HARNESS}: a snooze clears handled_at");
    let until = snoozed_until.expect("a snooze records when it ends");
    assert!(
        until > chrono::Utc::now() + chrono::Duration::days(2),
        "{HARNESS}: a three-day snooze ends about three days out, not {until}"
    );

    // 5. An expired window puts it back: the window is data, so the proof moves it rather than waiting.
    sqlx::query(
        "update catch_up_disposition set snoozed_until = now() - interval '1 hour' \
         where person_id = $1::uuid and reason_code = $2",
    )
    .bind(&person)
    .bind(REASON)
    .execute(harness.pool())
    .await
    .expect("the snooze window moves into the past");
    assert!(
        item_for(&dao, &person).await.is_some(),
        "{HARNESS}: an expired snooze returns the person to the queue"
    );

    sweep(&harness, &marker).await;
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("leftover count"),
        0,
        "{HARNESS}: the run's persons and messages are gone"
    );
}
