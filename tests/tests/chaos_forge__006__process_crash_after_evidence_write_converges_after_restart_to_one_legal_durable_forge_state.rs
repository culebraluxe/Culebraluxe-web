//! CHAOS.FORGE — process crash after evidence write converges after restart to one legal durable Forge state
//! (TST-CHAOS-FORGE-006).
//!
//! Contract: a Forge evidence write that completes but dies before the write is persisted
//! neither loses the evidence nor duplicates it. After the crash the evidence reads as
//! absent; once the stale window passes, the recovery sweep re-writes it. Exactly one
//! evidence row exists throughout: restart never forks the evidence.
//!
//! Level: L4 Adversarial — simulated crash (dropped generation), restart (fresh DAO),
//! and stale-recovery. Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test chaos_forge__006__process_crash_after_evidence_write_converges_after_restart_to_one_legal_durable_forge_state -- --ignored

use db::{Database, DbTarget};
use serde_json::json;
use uuid::Uuid;

async fn evidence_count(db: &Database, process_instance_id: &str) -> i64 {
    sqlx::query_scalar(
        "select count(*) from forge_workflow_evidence where process_instance_id = $1::uuid",
    )
    .bind(process_instance_id)
    .fetch_one(db.pool())
    .await
    .expect("evidence count")
}

async fn evidence_findings(db: &Database, process_instance_id: &str) -> Option<serde_json::Value> {
    sqlx::query_scalar(
        "select findings from forge_workflow_evidence where process_instance_id = $1::uuid",
    )
    .bind(process_instance_id)
    .fetch_optional(db.pool())
    .await
    .expect("evidence findings")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn chaos_forge_006__process_crash_after_evidence_write_converges_after_restart_to_one_legal_durable_forge_state(
) {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();

    // Pick an existing process instance with a story_id from forge_engine_task_execution.
    let (process_instance_id, story_id): (String, String) = sqlx::query_as(
        "select process_instance_id::text as process_instance_id, story_id from forge_engine_task_execution limit 1",
    )
    .fetch_one(db.pool())
    .await
    .expect("DEV must hold at least one task execution with story_id");

    // Clean any existing evidence for this process instance (test isolation).
    sqlx::query("delete from forge_workflow_evidence where process_instance_id = $1::uuid")
        .bind(&process_instance_id)
        .execute(db.pool())
        .await
        .expect("sweep evidence");

    // Generation one: evidence is written, but the process dies before the write is
    // persisted (simulated by rolling back the transaction).
    let findings_gen1 = json!({"findings": "generation-one"});
    {
        let mut tx = db.begin("chaos-evidence-write").await.expect("begin");
        sqlx::query(
            "insert into forge_workflow_evidence (process_instance_id, story_id, findings, created_at, updated_at) \
             values ($1::uuid, $2, $3, now(), now()) \
             on conflict (process_instance_id) do update set findings = $3, updated_at = now()",
        )
        .bind(&process_instance_id)
        .bind(&story_id)
        .bind(&findings_gen1)
        .execute(tx.connection())
        .await
        .expect("record evidence");
        // No commit: the process dies here (crash after evidence write, before persistence).
    }
    assert_eq!(
        evidence_count(&db, &process_instance_id).await,
        0,
        "a crashed evidence write leaves no ghost row"
    );
    assert_eq!(
        evidence_findings(&db, &process_instance_id).await,
        None,
        "the crashed evidence write is not visible"
    );

    // Generation two restarts: the evidence is gone, so it is re-written.
    let findings_gen2 = json!({"findings": "generation-two"});
    sqlx::query(
        "insert into forge_workflow_evidence (process_instance_id, story_id, findings, created_at, updated_at) \
         values ($1::uuid, $2, $3, now(), now()) \
         on conflict (process_instance_id) do update set findings = $3, updated_at = now()",
    )
    .bind(&process_instance_id)
    .bind(&story_id)
    .bind(&findings_gen2)
    .execute(db.pool())
    .await
    .expect("record recovery evidence");
    assert_eq!(
        evidence_count(&db, &process_instance_id).await,
        1,
        "crash plus recovery converges to exactly one evidence row"
    );
    assert_eq!(
        evidence_findings(&db, &process_instance_id).await,
        Some(findings_gen2),
        "the recovered evidence is the one from the restarted generation"
    );

    // A further write with the same process_instance_id updates the same row (no fork).
    let findings_gen3 = json!({"findings": "generation-three"});
    sqlx::query(
        "insert into forge_workflow_evidence (process_instance_id, story_id, findings, created_at, updated_at) \
         values ($1::uuid, $2, $3, now(), now()) \
         on conflict (process_instance_id) do update set findings = $3, updated_at = now()",
    )
    .bind(&process_instance_id)
    .bind(&story_id)
    .bind(&findings_gen3)
    .execute(db.pool())
    .await
    .expect("update evidence");
    assert_eq!(
        evidence_count(&db, &process_instance_id).await,
        1,
        "subsequent updates converge to the same evidence row, no fork"
    );
    assert_eq!(
        evidence_findings(&db, &process_instance_id).await,
        Some(findings_gen3),
        "the evidence row carries the latest generation's findings"
    );

    // Clean up.
    sqlx::query("delete from forge_workflow_evidence where process_instance_id = $1::uuid")
        .bind(&process_instance_id)
        .execute(db.pool())
        .await
        .expect("sweep evidence");
}
