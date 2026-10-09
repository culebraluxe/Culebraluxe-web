//! FORGE-B3 receipt and completion proof against DEV.
//!
//! This exercises the real receipt idempotency function and Batch 1's completion transaction. It is
//! ignored by default and the harness refuses PROD before opening a socket.
//!
//! Run explicitly after migration 281 is present in DEV:
//!   cargo test -p test-harness --test forge_assay_receipt_dev -- --ignored --test-threads=1

use db::{
    CompletionApply, CompletionAssayReceipt, CompletionUnit, ForgeEngineDao, ForgeEvidencePatch,
    NewToolArtifact,
};
use test_harness::database::TestDatabase;

const PLAN_HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const CANDIDATE: &str = "0123456789abcdef0123456789abcdef01234567";

async fn cleanup(db: &db::Database, story_id: &str, instance_id: &str, command_id: &str) {
    sqlx::query("delete from workflow_command_receipt where command_id=$1")
        .bind(command_id)
        .execute(db.pool())
        .await
        .expect("delete completion receipt");
    sqlx::query("delete from forge_workflow_evidence where process_instance_id=$1::uuid")
        .bind(instance_id)
        .execute(db.pool())
        .await
        .expect("delete workflow evidence");
    sqlx::query("delete from process_events where process_instance_id=$1::uuid")
        .bind(instance_id)
        .execute(db.pool())
        .await
        .expect("delete process events");
    sqlx::query("delete from process_instances where id=$1::uuid")
        .bind(instance_id)
        .execute(db.pool())
        .await
        .expect("delete process instance");
    sqlx::query("delete from storyboard_story where id=$1")
        .bind(story_id)
        .execute(db.pool())
        .await
        .expect("delete fixture story and cascading artifacts/runs");
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV and migration 281; TestDatabase refuses PROD"]
async fn receipt_is_idempotent_and_batch_one_completion_links_it_once() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("a declared DEV database");
    let db = test_db.database().clone();
    let dao = ForgeEngineDao::new(db.clone());
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let story_id = format!("TST-FORGE-B3-{tag}");
    let instance_id = uuid::Uuid::new_v4().to_string();
    let run_id = uuid::Uuid::new_v4().to_string();
    let completion_id = format!("forge.completion:forge-b3-{tag}");
    let plan_hash = PLAN_HASH.to_string();
    let candidate = CANDIDATE.to_string();
    let key = format!("forge-assay-v1:{run_id}:qa_verify:{plan_hash}:{candidate}");

    // Fixture rows are unique to this run. Cleanup also runs before the test so an interrupted prior
    // attempt with the same generated tag cannot leave a partial proof behind.
    cleanup(&db, &story_id, &instance_id, &completion_id).await;
    let setup = async {
        sqlx::query(
            "insert into storyboard_story (id, workstream, title, priority, status)
             values ($1, 'TEST', 'Forge B3 assay receipt', 'P3', 'Planned')",
        )
        .bind(&story_id)
        .execute(db.pool())
        .await?;
        sqlx::query(
            "insert into process_instances (id, definition_id, status, subject_type, subject_id)
             select $1::uuid, id, 'active', 'story', $2
               from process_definitions where key='FORGE_SDLC' limit 1",
        )
        .bind(&instance_id)
        .bind(&story_id)
        .execute(db.pool())
        .await?;
        sqlx::query(
            "insert into storyboard_story_run (id, story_id, started_at)
             values ($1::uuid, $2, now())",
        )
        .bind(&run_id)
        .bind(&story_id)
        .execute(db.pool())
        .await?;
        Ok::<(), sqlx::Error>(())
    }
    .await;
    if let Err(error) = setup {
        cleanup(&db, &story_id, &instance_id, &completion_id).await;
        panic!("create DEV fixture: {error}");
    }

    let detail = serde_json::json!({
        "receipt_schema_version": 1,
        "story_id": story_id,
        "story_run_id": run_id,
        "measurement_node": "qa_verify",
        "gate_verdict": "Pass",
        "plan_sha256": plan_hash,
        "candidate_sha": candidate,
        "commands": [{"command_id": "check", "status": "passed"}],
        "checks": [{"check_id": "required", "observation": "passed"}]
    });
    let artifact = NewToolArtifact {
        story_id: story_id.clone(),
        story_run_id: Some(run_id.clone()),
        tool: "assay".into(),
        kind: "qa-assay-evidence".into(),
        verdict: Some("PASS".into()),
        summary: Some("DEV receipt idempotency proof".into()),
        detail: Some(detail.clone()),
        sha: Some(candidate.clone()),
        idempotency_key: Some(key.clone()),
    };

    // Same key + same immutable content resolves to one row.
    let first = dao
        .record_tool_artifact(&artifact)
        .await
        .expect("write receipt");
    let retry = dao
        .record_tool_artifact(&artifact)
        .await
        .expect("retry receipt");
    assert_eq!(first.id, retry.id);
    let persisted = db::forge_assay::ForgeAssayDao::new(db.clone())
        .receipt_for_key(&key)
        .await
        .expect("read receipt")
        .expect("receipt persisted before workflow completion");
    assert_eq!(persisted.detail.as_ref(), Some(&detail));

    // Same key + different observations is a provenance conflict, not a successful retry.
    let mut conflicting = artifact.clone();
    conflicting.detail = Some(serde_json::json!({
        "receipt_schema_version": 1,
        "measurement_node": "qa_verify",
        "gate_verdict": "Pass",
        "plan_sha256": plan_hash,
        "candidate_sha": candidate,
        "checks": [{"check_id": "required", "observation": "failed"}]
    }));
    assert!(dao.record_tool_artifact(&conflicting).await.is_err());

    // A persistence failure has no receipt row and therefore cannot be linked to a QA PASS.
    let mut invalid_story = artifact.clone();
    invalid_story.story_id = format!("MISSING-{tag}");
    invalid_story.story_run_id = None;
    invalid_story.idempotency_key = Some(format!("invalid:{tag}"));
    assert!(dao.record_tool_artifact(&invalid_story).await.is_err());
    assert!(db::forge_assay::ForgeAssayDao::new(db.clone())
        .receipt_for_key(invalid_story.idempotency_key.as_deref().unwrap())
        .await
        .expect("check failed persistence")
        .is_none());

    // Simulate restart after the receipt commit but before task completion: apply the exact saved
    // receipt, then replay the completion. Batch 1 commits its evidence and link exactly once.
    let receipt_link = CompletionAssayReceipt {
        artifact_id: &first.id,
        story_run_id: &run_id,
        idempotency_key: &key,
        measurement_node: "qa_verify",
        gate_verdict: "Pass",
        plan_sha256: Some(&plan_hash),
        candidate_sha: Some(&candidate),
    };
    let patch = ForgeEvidencePatch {
        qa_passed: Some(true),
        qa_verified_sha: Some(candidate.clone()),
        candidate_sha: Some(candidate.clone()),
        role_output_schema_version: Some(1),
        role_output_diagnostic: Some(
            "ROLE_OUTPUT_ACCEPTED: schema=1; producer=repair_smith".into(),
        ),
        ..Default::default()
    };
    let fingerprint = format!("story:{story_id};assay-receipt:{key}");
    let unit = CompletionUnit {
        command_id: &completion_id,
        process_instance_id: &instance_id,
        story_id: &story_id,
        node_id: Some("qa_verify"),
        evidence: &patch,
        spend: None,
        fingerprint: &fingerprint,
        assay_receipt: Some(&receipt_link),
    };
    assert_eq!(
        dao.apply_completion(&unit)
            .await
            .expect("link receipt atomically"),
        CompletionApply::Applied
    );
    assert_eq!(
        dao.apply_completion(&unit)
            .await
            .expect("replay completion"),
        CompletionApply::AlreadyApplied
    );
    let linked_key: Option<String> = sqlx::query_scalar(
        "select result_payload->'assay_receipt'->>'idempotency_key'
           from workflow_command_receipt where command_id=$1",
    )
    .bind(&completion_id)
    .fetch_optional(db.pool())
    .await
    .expect("read completion proof")
    .flatten();
    assert_eq!(linked_key.as_deref(), Some(key.as_str()));
    let (schema_version, diagnostic): (Option<i64>, Option<String>) = sqlx::query_as(
        "select role_output_schema_version, role_output_diagnostic
           from forge_workflow_evidence where process_instance_id=$1::uuid",
    )
    .bind(&instance_id)
    .fetch_one(db.pool())
    .await
    .expect("read durable role-output provenance");
    assert_eq!(schema_version, Some(1));
    assert!(diagnostic
        .as_deref()
        .unwrap_or_default()
        .contains("schema=1"));

    cleanup(&db, &story_id, &instance_id, &completion_id).await;
}
