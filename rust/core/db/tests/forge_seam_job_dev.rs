//! SEAM-DB-003 — workflow READY role task → durable job identity, against DEV.
//!
//! Run explicitly with:
//!   DATABASE_URL_DEV=... cargo test -p db --test forge_seam_job_dev -- --ignored
//!
//! Why this exists: a Forge role task becomes a durable `jobs` row whose `id` IS the workflow task's UUID, and
//! whose payload carries `taskId` / `processInstanceId` / `serviceKey` (the READY→job bridge in
//! `forge::engine::job`, written through the workflow engine's Neon store). The defect this seam catches is a job
//! that is NOT keyed to its task — two claimable jobs for one task is a double paid execution (GAP-1). The
//! persistence boundary that makes that impossible is the `jobs` table: `jobs.id` is the primary key, so one task
//! id admits exactly one durable job.
//!
//! The `db` test crate links only the persistence layer (`db` + `sqlx`); the workflow engine's `create_job_with_id`
//! wrapper lives in the `workflow` crate and is exercised by the `forge_job__001` L1 contract suite. This test pins
//! the database half of the identity: the task UUID becomes the job id, the payload carries the same UUIDs, the
//! service key survives the round trip, and a re-enqueue of the same task cannot produce a second row.

use db::{Database, DbTarget};

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn a_ready_role_task_is_one_durable_job_keyed_by_its_own_uuid() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let story = format!("ENG-PROOF-JOB-{tag}");

    // The FK graph `jobs` sits in: process_instance_id → process_instances → process_definitions, token_id → tokens,
    // and the workflow task that is the identity source.
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'Job identity proof', 'High', 'In Progress', '')",
    )
    .bind(&story)
    .execute(pool)
    .await
    .expect("insert proof story");

    let definition_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "insert into process_definitions (id, key, name, definition, status)
         values ($1::uuid, $2, 'proof', '{}'::jsonb, 'active')",
    )
    .bind(&definition_id)
    .bind(format!("def-{tag}"))
    .execute(pool)
    .await
    .expect("def");

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
    .expect("instance");

    let token_id = uuid::Uuid::new_v4().to_string();
    sqlx::query("insert into tokens (id, process_instance_id, node_id) values ($1::uuid, $2::uuid, 'smith')")
        .bind(&token_id)
        .bind(&process_instance_id)
        .execute(pool)
        .await
        .expect("token");

    // The workflow task is the identity source: a real UUID, a READY role task on the `smith` node.
    let task_id = uuid::Uuid::new_v4();
    sqlx::query(
        "insert into tasks (id, process_instance_id, token_id, name, node_id, status)
         values ($1::uuid, $2::uuid, $3::uuid, 'smith', 'smith', 'ready')",
    )
    .bind(task_id.to_string())
    .bind(&process_instance_id)
    .bind(&token_id)
    .execute(pool)
    .await
    .expect("task");

    // The durable job, exactly the shape WorkflowJobService::enqueue writes: `id = task id`, `type = 'forge.role'`,
    // and the bridge's payload (serviceKey, nodeId, taskId, processInstanceId, storyId, tokenId).
    let service_key = "forge.smith";
    let payload = serde_json::json!({
        "serviceKey": service_key,
        "nodeId": "smith",
        "taskId": task_id.to_string(),
        "processInstanceId": process_instance_id,
        "storyId": story,
        "tokenId": token_id,
    });
    sqlx::query(
        "insert into jobs (id, process_instance_id, token_id, type, due_at, payload, max_attempts, status)
         values ($1::uuid, $2::uuid, $3::uuid, 'forge.role', now(), $4::jsonb, 5, 'pending')",
    )
    .bind(task_id.to_string())
    .bind(&process_instance_id)
    .bind(&token_id)
    .bind(payload.to_string())
    .execute(pool)
    .await
    .expect("insert durable job");

    // Committed database truth: the job's id is the task's UUID, and the payload carries the same identities.
    let (job_id, payload_task_id, payload_pid, payload_service_key, payload_node, job_type): (
        String,
        String,
        String,
        String,
        String,
        String,
    ) = sqlx::query_as(
        "select id::text, payload->>'taskId', payload->>'processInstanceId', payload->>'serviceKey',
                payload->>'nodeId', type
           from jobs where id = $1::uuid",
    )
    .bind(task_id.to_string())
    .fetch_one(pool)
    .await
    .expect("the durable job is committed");
    assert_eq!(
        job_id,
        task_id.to_string(),
        "jobs.id is the workflow task's UUID"
    );
    assert_eq!(
        payload_task_id,
        task_id.to_string(),
        "payload.taskId is the same UUID"
    );
    assert_eq!(
        payload_pid, process_instance_id,
        "payload.processInstanceId is the same UUID"
    );
    assert_eq!(
        payload_service_key, service_key,
        "the service key survives the round trip"
    );
    assert_eq!(payload_node, "smith");
    assert_eq!(job_type, "forge.role");

    // Re-enqueue the same task: one task admits exactly one durable job. `jobs.id` is the primary key, so a second
    // insert with the same id is refused by the database — no second claimable job can exist.
    let duplicate = sqlx::query(
        "insert into jobs (id, process_instance_id, token_id, type, due_at, payload, max_attempts, status)
         values ($1::uuid, $2::uuid, $3::uuid, 'forge.role', now(), $4::jsonb, 5, 'pending')",
    )
    .bind(task_id.to_string())
    .bind(&process_instance_id)
    .bind(&token_id)
    .bind(payload.to_string())
    .execute(pool)
    .await;
    assert!(
        duplicate.is_err(),
        "a second durable job for one workflow task must be refused at the database"
    );
    let open_jobs: i64 = sqlx::query_scalar("select count(*) from jobs where id = $1::uuid")
        .bind(task_id.to_string())
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(open_jobs, 1, "exactly one durable job for the task");

    // Leave DEV as found, in FK order.
    let _ = sqlx::query("delete from jobs where id = $1::uuid")
        .bind(task_id.to_string())
        .execute(pool)
        .await;
    let _ = sqlx::query("delete from storyboard_story where id = $1")
        .bind(&story)
        .execute(pool)
        .await;
    let _ = sqlx::query("delete from tasks where id = $1::uuid")
        .bind(task_id.to_string())
        .execute(pool)
        .await;
    let _ = sqlx::query("delete from tokens where id = $1::uuid")
        .bind(&token_id)
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
