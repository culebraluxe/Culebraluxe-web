//! CRM.CATCHUP — completed tasks excluded (TST-CRM-CATCHUP-003).
//!
//! Contract: a COMPLETED task takes no part in the catch-up queue's due-date calculation. `db/src/catch_up.rs`
//! says so in `next_task` — `where t.status = 'open' and t.person_id is not null and t.due_at is not null` —
//! so a finished task can neither raise a follow-up signal nor stand in as the row's task.
//!
//! Three persons, each seeded for this run:
//!   D. ONLY a completed task, three days overdue   → no task-driven reason and no task named: the overdue
//!                                                     date of work already done is not a reason to follow up
//!   E. a completed task due in one day AND an open
//!      task due in three days                      → the OPEN task is named; the earlier completed date is not
//!   F. an OPEN task three days overdue             → `overdue_follow_up`: the control that proves D's silence
//!                                                     is exclusion, not a queue that stopped working
//!
//! Level: L3 Composition. Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test crm_catchup__003__completed_tasks_excluded -- --ignored

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

async fn seed_catchup_person(harness: &CrmHarness, display_name: &str) -> String {
    sqlx::query_scalar(
        "insert into person (display_name, role, status) values ($1, 'buyer', 'active') returning id::text",
    )
    .bind(display_name)
    .fetch_one(harness.pool())
    .await
    .expect("a catch-up person seeds")
}

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

async fn item_for(dao: &CatchUpDao, person_id: &str) -> Option<CatchUpItem> {
    let snapshot = dao.snapshot().await.expect("the catch-up snapshot reads");
    snapshot
        .items
        .into_iter()
        .find(|item| item.person_id == person_id)
}

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-CATCHUP-003); the file and the assay use it.
async fn crm_catchup_003__completed_tasks_excluded() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the completed-task proof runs only on an isolated DEV target"
    );
    let ns = harness.namespace().to_string();
    let marker = format!("TST-CRMCATCHUP003-{ns}-{}", &Uuid::new_v4().to_string()[..8]);
    let dao = CatchUpDao::new(harness.database().database().clone());

    // D — finished work only: a completed task whose date is three days past.
    let person_d = seed_catchup_person(&harness, &format!("{marker}-d")).await;
    let task_d_done = seed_task(&harness, &person_d, "D: done, overdue", "completed", Some(-3)).await;

    // E — finished work beside open work: a completed task due in one day, an open task due in three.
    let person_e = seed_catchup_person(&harness, &format!("{marker}-e")).await;
    let task_e_done = seed_task(&harness, &person_e, "E: done, due tomorrow", "completed", Some(1)).await;
    let task_e_open = seed_task(&harness, &person_e, "E: open, due in three days", "open", Some(3)).await;

    // F — the control: an OPEN task three days overdue.
    let person_f = seed_catchup_person(&harness, &format!("{marker}-f")).await;
    let task_f_open = seed_task(&harness, &person_f, "F: open, overdue", "open", Some(-3)).await;

    // 1. D: the completed task raises nothing. D still has a quiet relationship (no contact recorded), so
    //    the row exists — what must NOT happen is a task-driven reason or a task id.
    let item_d = item_for(&dao, &person_d)
        .await
        .expect("person D is in the queue for the quiet-relationship reason");
    assert_ne!(
        item_d.reason_code, "overdue_follow_up",
        "{HARNESS}: a completed task three days overdue is not an overdue follow-up"
    );
    assert_ne!(
        item_d.reason_code, "follow_up_due_soon",
        "{HARNESS}: a completed task is not a due-soon follow-up"
    );
    assert_eq!(
        item_d.task_id, None,
        "{HARNESS}: no open dated task exists, so no task is named (completed ids are {task_d_done})"
    );
    assert_eq!(
        item_d.reason_code, "relationship_quiet",
        "{HARNESS}: with the task excluded, D's only signal is the quiet relationship"
    );

    // 2. E: the OPEN task is named, even though the completed task has the earlier due date.
    let item_e = item_for(&dao, &person_e)
        .await
        .expect("person E is in the queue: an open task due in three days is a catch-up signal");
    assert_eq!(
        item_e.reason_code, "follow_up_due_soon",
        "{HARNESS}: E's open task is due within seven days, not {}",
        item_e.reason_code
    );
    assert_eq!(
        item_e.task_id.as_deref(),
        Some(task_e_open.as_str()),
        "{HARNESS}: the open task is the one calculated"
    );
    assert_ne!(
        item_e.task_id.as_deref(),
        Some(task_e_done.as_str()),
        "{HARNESS}: the completed task is excluded even though its due date is earlier"
    );

    // 3. F: the control — the same shape with an OPEN task does raise the overdue signal, and names it.
    let item_f = item_for(&dao, &person_f)
        .await
        .expect("person F is in the queue: an overdue open task is a catch-up signal");
    assert_eq!(
        item_f.reason_code, "overdue_follow_up",
        "{HARNESS}: an open task three days overdue is overdue, not {}",
        item_f.reason_code
    );
    assert_eq!(
        item_f.task_id.as_deref(),
        Some(task_f_open.as_str()),
        "{HARNESS}: the control names its open task"
    );

    sweep(&harness, &marker).await;
    assert_eq!(
        harness.leftover_count(&marker).await.expect("leftover count"),
        0,
        "{HARNESS}: the run's persons and tasks are gone"
    );
}
