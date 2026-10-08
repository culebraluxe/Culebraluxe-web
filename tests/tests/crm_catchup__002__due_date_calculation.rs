//! CRM.CATCHUP — due-date calculation (TST-CRM-CATCHUP-002).
//!
//! Contract: the catch-up queue's task comes from the EARLIEST OPEN follow-up task that HAS a due date.
//! `db/src/catch_up.rs` says so in `next_task` — `where t.status = 'open' and t.person_id is not null
//! and t.due_at is not null`, `order by t.person_id, t.due_at asc, t.created_at asc` — and the reason
//! codes read that one due date: overdue (85) when it is in the past, due-soon (70) when it lands within
//! seven days.
//!
//! Three persons, each seeded for this run and each carrying only tasks (no interaction, no showing), so
//! the task signal is the highest-priority signal on each row:
//!   A. one open task due in two days                    → `follow_up_due_soon`, naming THAT task
//!   B. open tasks at six days and one day, plus an open
//!      task with NO due date                            → the ONE-DAY task is named; a dateless task
//!                                                          takes no part and the later date is not chosen
//!   C. one open task three days overdue                 → `overdue_follow_up`, naming THAT task
//!
//! Fixtures are unique per run (`{marker}-…`, the marker carrying a fresh UUID) and every row this test
//! writes is deleted before it returns, so a shared DEV database is left as it was found.
//!
//! Level: L3 Composition. Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test crm_catchup__002__due_date_calculation -- --ignored

use db::{CatchUpDao, DbTarget};
use model::CatchUpItem;
use test_harness::CrmHarness;
use uuid::Uuid;

const HARNESS: &str = "CrmHarness/L3 Composition";

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

/// A person the queue can see: `status in ('active','warm')` and not archived (`catch_up.rs`, `people`).
async fn seed_catchup_person(harness: &CrmHarness, display_name: &str) -> String {
    sqlx::query_scalar(
        "insert into person (display_name, role, status) values ($1, 'buyer', 'active') returning id::text",
    )
    .bind(display_name)
    .fetch_one(harness.pool())
    .await
    .expect("a catch-up person seeds")
}

/// One task row. `due_in_days` is applied to `now()`; `None` is a task with NO due date.
async fn seed_task(
    harness: &CrmHarness,
    person_id: &str,
    title: &str,
    status: &str,
    due_in_days: Option<i64>,
) -> String {
    sqlx::query_scalar(
        "insert into task (title, person_id, task_kind, priority, status, due_at) \
         values ($1, $2::uuid, 'human', 0, $3, \
                 case when $4::bigint is null then null else now() + make_interval(days => $4::int) end) \
         returning id::text",
    )
    .bind(title)
    .bind(person_id)
    .bind(status)
    .bind(due_in_days)
    .fetch_one(harness.pool())
    .await
    .expect("a task seeds")
}

/// This person's row in the live queue, if the queue holds one.
async fn item_for(dao: &CatchUpDao, person_id: &str) -> Option<CatchUpItem> {
    let snapshot = dao.snapshot().await.expect("the catch-up snapshot reads");
    snapshot
        .items
        .into_iter()
        .find(|item| item.person_id == person_id)
}

/// Delete everything this run created: its tasks, its dispositions, then its persons (the harness deletes
/// persons by display-name prefix, so a marker carrying a fresh UUID can only match this run).
async fn sweep(harness: &CrmHarness, marker: &str) {
    let pattern = format!("{marker}%");
    for statement in [
        "delete from task where person_id in (select id from person where display_name like $1)",
        "delete from catch_up_disposition where person_id in (select id::text from person where display_name like $1)",
    ] {
        let _ = sqlx::query(statement).bind(&pattern).execute(harness.pool()).await;
    }
    let _ = harness.cleanup(marker).await;
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); CrmHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-CATCHUP-002); the file and the assay use it.
async fn crm_catchup_002__due_date_calculation() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the due-date proof runs only on an isolated DEV target"
    );
    let ns = harness.namespace().to_string();
    let marker = format!("TST-CRMCATCHUP002-{ns}-{}", &Uuid::new_v4().to_string()[..8]);
    let dao = CatchUpDao::new(harness.database().database().clone());

    // A — one open task due in two days: the row is due-soon and names that task.
    let person_a = seed_catchup_person(&harness, &format!("{marker}-a")).await;
    let task_a = seed_task(&harness, &person_a, "A: due in two days", "open", Some(2)).await;

    // B — an open task at six days, an open task at one day, and an open task with NO due date.
    let person_b = seed_catchup_person(&harness, &format!("{marker}-b")).await;
    let task_b_later = seed_task(&harness, &person_b, "B: due in six days", "open", Some(6)).await;
    let task_b_earliest = seed_task(&harness, &person_b, "B: due in one day", "open", Some(1)).await;
    let task_b_dateless = seed_task(&harness, &person_b, "B: no due date", "open", None).await;

    // C — one open task three days overdue.
    let person_c = seed_catchup_person(&harness, &format!("{marker}-c")).await;
    let task_c = seed_task(&harness, &person_c, "C: overdue", "open", Some(-3)).await;

    // 1. A: due within seven days reads as due-soon, and the row carries that task and its due date.
    let item_a = item_for(&dao, &person_a)
        .await
        .expect("person A is in the queue: an open task due in two days is a catch-up signal");
    assert_eq!(
        item_a.reason_code, "follow_up_due_soon",
        "{HARNESS}: a task due in two days is due-soon, not {}",
        item_a.reason_code
    );
    assert_eq!(item_a.priority, 70, "{HARNESS}: due-soon carries priority 70");
    assert_eq!(
        item_a.task_id.as_deref(),
        Some(task_a.as_str()),
        "{HARNESS}: the row names the task whose due date it calculated"
    );
    assert!(
        item_a.due_at_label.is_some(),
        "{HARNESS}: the due date is presented, not just stored"
    );

    // 2. B: the EARLIEST open dated task is the one named — not the later one, and not the dateless one.
    let item_b = item_for(&dao, &person_b)
        .await
        .expect("person B is in the queue: an open task due in one day is a catch-up signal");
    assert_eq!(
        item_b.reason_code, "follow_up_due_soon",
        "{HARNESS}: B's earliest task is due in a day, so B is due-soon, not {}",
        item_b.reason_code
    );
    assert_eq!(
        item_b.task_id.as_deref(),
        Some(task_b_earliest.as_str()),
        "{HARNESS}: the calculation picks the earliest due date, not the first row written"
    );
    assert_ne!(
        item_b.task_id.as_deref(),
        Some(task_b_later.as_str()),
        "{HARNESS}: the later due date is not the one calculated"
    );
    assert_ne!(
        item_b.task_id.as_deref(),
        Some(task_b_dateless.as_str()),
        "{HARNESS}: a task with no due date takes no part in the due-date calculation"
    );

    // 3. C: a past due date reads as overdue, and names that task.
    let item_c = item_for(&dao, &person_c)
        .await
        .expect("person C is in the queue: an overdue open task is a catch-up signal");
    assert_eq!(
        item_c.reason_code, "overdue_follow_up",
        "{HARNESS}: a task due three days ago is overdue, not {}",
        item_c.reason_code
    );
    assert_eq!(item_c.priority, 85, "{HARNESS}: overdue carries priority 85");
    assert_eq!(
        item_c.task_id.as_deref(),
        Some(task_c.as_str()),
        "{HARNESS}: the overdue row names the overdue task"
    );

    // 4. The queue is derived, not stored: no catch-up row of its own exists for these persons.
    let dispositions: i64 = sqlx::query_scalar(
        "select count(*) from catch_up_disposition where person_id = any($1::uuid[])",
    )
    .bind(
        [&person_a, &person_b, &person_c]
            .iter()
            .map(|value| value.as_str())
            .collect::<Vec<_>>(),
    )
    .fetch_one(harness.pool())
    .await
    .expect("the disposition count reads");
    assert_eq!(
        dispositions, 0,
        "{HARNESS}: reading the queue writes no disposition"
    );

    sweep(&harness, &marker).await;
    assert_eq!(
        harness.leftover_count(&marker).await.expect("leftover count"),
        0,
        "{HARNESS}: the run's persons and tasks are gone"
    );
}
