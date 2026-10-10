//! S05 receipt/settlement DAO contract against an explicitly declared disposable DEV database.
//! This test is ignored by default so it cannot silently select a shared or production target.

use db::{ForgeEngineDao, ForgeEvidencePatch};
use test_harness::database::TestDatabase;

#[tokio::test]
#[ignore = "requires an explicitly disposable PostgreSQL test database with migrations 000, 021, 109, and 290"]
async fn release_receipt_settlement_is_atomic_replayable_and_idempotent() {
    let test_db = TestDatabase::connect_from_env()
        .await
        .expect("declared non-production test database");
    let database = test_db.database().clone();
    let dao = ForgeEngineDao::new(database.clone());
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let definition_id = uuid::Uuid::new_v4().to_string();
    let process_id = uuid::Uuid::new_v4().to_string();
    let story_id = format!("TST-S05-{tag}");
    let command_id = format!("forge.release:{tag}");

    sqlx::query(
        "insert into process_definitions (id, key, version, name, definition, status)
         values ($1::uuid, $2, 1, 'S05 receipt contract', '{}'::jsonb, 'active')",
    )
    .bind(&definition_id)
    .bind(format!("TST-S05-{tag}"))
    .execute(database.pool())
    .await
    .expect("create isolated process definition");
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status)
         values ($1, 'TEST', 'S05 release receipt contract', 'P3', 'Planned')",
    )
    .bind(&story_id)
    .execute(database.pool())
    .await
    .expect("create isolated story");
    sqlx::query(
        "insert into process_instances (id, definition_id, status, subject_type, subject_id)
         values ($1::uuid, $2::uuid, 'active', 'story', $3)",
    )
    .bind(&process_id)
    .bind(&definition_id)
    .bind(&story_id)
    .execute(database.pool())
    .await
    .expect("create isolated process instance");

    let result = serde_json::json!({"kind":"published", "published_main_hash":"abc123"});
    let receipt = dao
        .record_release_operation_receipt(
            &command_id,
            &process_id,
            &story_id,
            "forge.publish_candidate",
            &result,
        )
        .await
        .expect("record observed operation result");
    assert!(!receipt.settled);
    assert_eq!(
        dao.record_release_operation_receipt(
            &command_id,
            &process_id,
            &story_id,
            "forge.publish_candidate",
            &result,
        )
        .await
        .expect("identical result replay is accepted")
        .result,
        result
    );
    assert!(
        dao.record_release_operation_receipt(
            &command_id,
            &process_id,
            &story_id,
            "forge.publish_candidate",
            &serde_json::json!({"kind":"published", "published_main_hash":"different"}),
        )
        .await
        .is_err(),
        "one command identity cannot be reused for a different result"
    );

    let patch = ForgeEvidencePatch {
        publish_succeeded: Some(true),
        published_sha: Some("abc123".into()),
        ..Default::default()
    };
    let (first, second) = tokio::join!(
        dao.settle_release_operation_receipt(&command_id, &process_id, &story_id, &patch, false),
        dao.settle_release_operation_receipt(&command_id, &process_id, &story_id, &patch, false),
    );
    first.expect("first concurrent settlement");
    second.expect("second concurrent settlement replay");
    let settled = dao
        .release_operation_receipt(&command_id)
        .await
        .unwrap()
        .unwrap();
    assert!(settled.settled);
    let evidence: String = sqlx::query_scalar(
        "select published_sha from forge_workflow_evidence where process_instance_id=$1::uuid",
    )
    .bind(&process_id)
    .fetch_one(database.pool())
    .await
    .expect("evidence written in settlement transaction");
    assert_eq!(evidence, "abc123");

    sqlx::query("update forge_workflow_evidence set published_sha='newer-fact' where process_instance_id=$1::uuid")
        .bind(&process_id)
        .execute(database.pool())
        .await
        .expect("write later workflow fact for replay check");
    dao.settle_release_operation_receipt(&command_id, &process_id, &story_id, &patch, false)
        .await
        .expect("settled replay returns without rewriting old evidence");
    let preserved: String = sqlx::query_scalar(
        "select published_sha from forge_workflow_evidence where process_instance_id=$1::uuid",
    )
    .bind(&process_id)
    .fetch_one(database.pool())
    .await
    .expect("read evidence after receipt replay");
    assert_eq!(preserved, "newer-fact");

    sqlx::query("delete from process_instances where id=$1::uuid")
        .bind(&process_id)
        .execute(database.pool())
        .await
        .expect("remove isolated process/receipt/evidence");
    sqlx::query("delete from storyboard_story where id=$1")
        .bind(&story_id)
        .execute(database.pool())
        .await
        .expect("remove isolated story");
    sqlx::query("delete from process_definitions where id=$1::uuid")
        .bind(&definition_id)
        .execute(database.pool())
        .await
        .expect("remove isolated definition");
}
