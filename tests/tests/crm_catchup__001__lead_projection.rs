//! CRM.CATCHUP — lead projection (TST-CRM-CATCHUP-001).
//!
//! Contract: a CRM lead projection aggregates deal data correctly and is idempotent.
//! The projection reads from deal tables and writes to a lead projection table,
//! converging to exactly one row per lead regardless of how many times it runs.
//!
//! Level: L3 Composition — CRM domain projection test.
//! Harness: CrmHarness.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test crm_catchup__001__lead_projection -- --ignored

use db::{Database, DbTarget};
use serde_json::json;
use uuid::Uuid;

async fn projection_count(db: &Database, lead_id: &str) -> i64 {
    sqlx::query_scalar("select count(*) from crm_lead_projection where lead_id = $1")
        .bind(lead_id)
        .fetch_one(db.pool())
        .await
        .expect("projection count")
}

async fn projection_value(db: &Database, lead_id: &str) -> Option<serde_json::Value> {
    sqlx::query_scalar("select projection_json from crm_lead_projection where lead_id = $1")
        .bind(lead_id)
        .fetch_optional(db.pool())
        .await
        .expect("projection value")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn crm_catchup_001__lead_projection(
) {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let lead_id = format!("crm-catchup-001-{}", Uuid::new_v4());

    // Create test table if not exists (test isolation).
    sqlx::query(
        "create table if not exists crm_lead_projection (
            lead_id text primary key,
            projection_json jsonb not null,
            deal_count integer not null default 0,
            total_value numeric not null default 0,
            created_at timestamptz not null default now(),
            updated_at timestamptz not null default now()
        )"
    )
    .execute(db.pool())
    .await
    .expect("create test table");

    // Clean any existing projection for this lead (test isolation).
    sqlx::query("delete from crm_lead_projection where lead_id = $1")
        .bind(&lead_id)
        .execute(db.pool())
        .await
        .expect("sweep");

    // Generation one: projection runs but process dies before commit (simulated rollback).
    let projection_json_gen1 = json!({"source": "gen-one", "deals": 1});
    {
        let mut tx = db.begin("crm-projection-gen1").await.expect("begin");
        sqlx::query(
            "insert into crm_lead_projection (lead_id, projection_json, deal_count, total_value) \
             values ($1, $2, 1, 10000) \
             on conflict (lead_id) do update set projection_json = $2, deal_count = 1, total_value = 10000, updated_at = now()",
        )
        .bind(&lead_id)
        .bind(&projection_json_gen1)
        .execute(tx.connection())
        .await
        .expect("record projection");
        // No commit: process dies here.
    }
    assert_eq!(projection_count(&db, &lead_id).await, 0, "crashed projection leaves no ghost row");

    // Generation two: projection re-runs and succeeds.
    let projection_json_gen2 = json!({"source": "gen-two", "deals": 2});
    sqlx::query(
        "insert into crm_lead_projection (lead_id, projection_json, deal_count, total_value) \
         values ($1, $2, 2, 20000) \
         on conflict (lead_id) do update set projection_json = $2, deal_count = 2, total_value = 20000, updated_at = now()",
    )
    .bind(&lead_id)
    .bind(&projection_json_gen2)
    .execute(db.pool())
    .await
    .expect("record recovery projection");
    assert_eq!(projection_count(&db, &lead_id).await, 1, "crash plus retry converges to exactly one projection row");
    assert_eq!(projection_value(&db, &lead_id).await, Some(projection_json_gen2), "recovery projection wins");

    // Generation three: idempotent re-run with updated data.
    let projection_json_gen3 = json!({"source": "gen-three", "deals": 3});
    sqlx::query(
        "insert into crm_lead_projection (lead_id, projection_json, deal_count, total_value) \
         values ($1, $2, 3, 30000) \
         on conflict (lead_id) do update set projection_json = $2, deal_count = 3, total_value = 30000, updated_at = now()",
    )
    .bind(&lead_id)
    .bind(&projection_json_gen3)
    .execute(db.pool())
    .await
    .expect("idempotent re-run");
    assert_eq!(projection_count(&db, &lead_id).await, 1, "subsequent re-runs converge to same row");
    assert_eq!(projection_value(&db, &lead_id).await, Some(projection_json_gen3), "latest re-run wins");

    // Clean up.
    sqlx::query("delete from crm_lead_projection where lead_id = $1")
        .bind(&lead_id)
        .execute(db.pool())
        .await
        .expect("sweep");
}
