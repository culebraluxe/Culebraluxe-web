//! SEAM-DB-001 — the hold record's schema round trip, and the C1 UUID regression.
//!
//! Run explicitly with:
//!   DATABASE_URL_DEV=... cargo test -p db --test forge_seam_hold_dev -- --ignored
//!
//! Why this exists: `ForgeEngineDao::open_hold` writes a `forge_hold_record` row whose `task_id` column is a
//! `uuid` (migration 113), but the DAO binds `task_id` as an `Option<&str>`. C1 shipped that bind with no cast —
//! `values($1::uuid,$2,…)` — and Postgres has no implicit `text → uuid` cast, so the first hold ever written against
//! a live task failed at the seam. Component mocks did not see it because no mock is a real `uuid` column. This test
//! drives the production DAO against DEV with a real UUID task id and proves the full row round-trips — including
//! `task_id` stored and read back as the exact UUID, not a text remnant.
//!
//! It leaves DEV as it found it: every fixture row is deleted in reverse-FK order.

use db::{Database, DbTarget, ForgeEngineDao};

async fn delete_story(pool: &sqlx::PgPool, story_id: &str) {
    let _ = sqlx::query("delete from storyboard_story where id = $1")
        .bind(story_id)
        .execute(pool)
        .await;
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn hold_record_round_trips_every_column_and_task_id_is_a_real_uuid() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    let engine = ForgeEngineDao::new(database.clone());
    let tag = uuid::Uuid::new_v4().simple().to_string();

    // The FK graph `forge_hold_record` sits in: process_instance_id → process_instances → process_definitions,
    // task_id → tasks, story_id → storyboard_story. Each is a real row with a real UUID, so the DAO's casts run
    // against the same types production writes.
    let story = format!("ENG-PROOF-HOLD-{tag}");
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'Hold proof', 'High', 'In Progress', '')",
    )
    .bind(&story)
    .execute(pool)
    .await
    .expect("insert proof story");

    let definition_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "insert into process_definitions (id, key, name, definition, status)
         values ($1::uuid, $2, 'Hold proof definition', '{}'::jsonb, 'active')",
    )
    .bind(&definition_id)
    .bind(format!("proof-def-{tag}"))
    .execute(pool)
    .await
    .expect("insert process definition");

    let process_instance_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "insert into process_instances (id, definition_id, status, subject_type, subject_id)
         values ($1::uuid, $2::uuid, 'active', 'story', $3)",
    )
    .bind(&process_instance_id)
    .bind(&definition_id)
    .bind(&story)
    .execute(pool)
    .await
    .expect("insert process instance");

    // A real task id — the exact value the C1 seam choked on.
    let task_id = uuid::Uuid::new_v4();
    sqlx::query(
        "insert into tasks (id, process_instance_id, name, status)
         values ($1::uuid, $2::uuid, 'proof task', 'ready')",
    )
    .bind(task_id.to_string())
    .bind(&process_instance_id)
    .execute(pool)
    .await
    .expect("insert workflow task");

    // Drive the production DAO. This is the C1 regression: if `task_id` is not bound-and-cast as `uuid`, this
    // insert raises "column task_id is of type uuid but expression is of type text" and the hold never lands.
    let hold_id = engine
        .open_hold(
            &process_instance_id,
            Some(&task_id.to_string()),
            &story,
            "proof reason",
            Some("smith"),
            Some("Infrastructure"),
            Some("resume-here"),
        )
        .await
        .expect("open_hold must land a real UUID task id");

    // Read back every column from the committed row, not the returned struct.
    let (pi, task, st, reason, node, class, resume): (
        String,
        Option<String>,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "select process_instance_id::text, task_id::text, story_id, reason,
                originating_node, failure_class, resume_target
           from forge_hold_record where id = $1::bigint",
    )
    .bind(hold_id.parse::<i64>().expect("bigserial id"))
    .fetch_one(pool)
    .await
    .expect("the hold row is committed");
    assert_eq!(pi, process_instance_id);
    assert_eq!(
        task.as_deref(),
        Some(task_id.to_string().as_str()),
        "task_id must round-trip as the exact UUID"
    );
    assert_eq!(st, story);
    assert_eq!(reason, "proof reason");
    assert_eq!(node.as_deref(), Some("smith"));
    assert_eq!(class.as_deref(), Some("Infrastructure"));
    assert_eq!(resume.as_deref(), Some("resume-here"));

    // C1, driven again and typed: the column stores a `uuid`, so reading it back with a `::uuid` cast and matching
    // the original proves the value is a real UUID and not a text remnant.
    let task_id_round_tripped: Option<String> = sqlx::query_scalar(
        "select (task_id::uuid)::text from forge_hold_record where id = $1::bigint",
    )
    .bind(hold_id.parse::<i64>().expect("id"))
    .fetch_one(pool)
    .await
    .expect("task_id reads back as uuid");
    assert_eq!(
        task_id_round_tripped.as_deref(),
        Some(task_id.to_string().as_str())
    );

    // The open-hold read seam sees the same row.
    let latest = engine
        .latest_open_hold(&story)
        .await
        .unwrap()
        .expect("an open hold is readable");
    assert_eq!(latest.reason, "proof reason");
    assert_eq!(latest.originating_node.as_deref(), Some("smith"));

    // Leave DEV as found. The story delete cascades `forge_hold_record` (story_id) and any work items; the task,
    // instance and definition go next.
    delete_story(pool, &story).await;
    let _ = sqlx::query("delete from tasks where id = $1::uuid")
        .bind(task_id.to_string())
        .execute(pool)
        .await;
    let _ = sqlx::query("delete from process_instances where id = $1::uuid")
        .bind(&process_instance_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("delete from process_definitions where id = $1::uuid")
        .bind(&definition_id)
        .execute(pool)
        .await;
}

/// The C1 cast is pinned in source, not only observed at runtime: the DAO must cast both uuid binds, and the
/// `task_id` bind must be `$2::uuid` (text → uuid), not a bare `$2`.
#[test]
fn open_hold_casts_the_task_id_bind_as_uuid() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let dao = std::fs::read_to_string(root.join("rust/core/db/src/forge_engine.rs")).unwrap();
    assert!(
        dao.contains("values($1::uuid,$2::uuid,$3,$4,$5,$6,$7,null,null,null,null)"),
        "open_hold must cast both process_instance_id ($1::uuid) and task_id ($2::uuid)"
    );
}
