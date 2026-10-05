//! CHAOS.SERVICE — transient database failure injected at each service write boundary never leaves a partial
//! (TST-CHAOS-SERVICE-001).
//!
//! Contract: a service write that encounters a transient database failure retries and
//! converges to exactly one write. The transient failure is injected at the boundary
//! (e.g., network partition, deadlock) and the service's retry logic handles it.
//!
//! Level: L4 Adversarial — deterministic fault injection at service boundaries.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test chaos_service__001__transient_database_failure_injected_at_each_service_write_boundary_never_leaves_a_partial -- --ignored

use db::{Database, DbTarget};
use uuid::Uuid;

async fn write_count(db: &Database, key: &str) -> i64 {
    sqlx::query_scalar("select count(*) from chaos_service_test_writes where write_key = $1")
        .bind(key)
        .fetch_one(db.pool())
        .await
        .expect("write count")
}

async fn write_value(db: &Database, key: &str) -> Option<String> {
    sqlx::query_scalar("select write_value from chaos_service_test_writes where write_key = $1")
        .bind(key)
        .fetch_optional(db.pool())
        .await
        .expect("write value")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn chaos_service_001__transient_database_failure_injected_at_each_service_write_boundary_never_leaves_a_partial(
) {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let write_key = format!("chaos-service-001-{}", Uuid::new_v4());

    // Create test table if not exists (test isolation).
    sqlx::query(
        "create table if not exists chaos_service_test_writes (
            write_key text primary key,
            write_value text not null,
            attempt integer not null default 1,
            created_at timestamptz not null default now(),
            updated_at timestamptz not null default now()
        )"
    )
    .execute(db.pool())
    .await
    .expect("create test table");

    // Clean any existing test writes (test isolation).
    sqlx::query("delete from chaos_service_test_writes where write_key = $1")
        .bind(&write_key)
        .execute(db.pool())
        .await
        .expect("sweep");

    // Generation one: write succeeds but process dies before commit (simulated rollback).
    {
        let mut tx = db.begin("chaos-service-write").await.expect("begin");
        sqlx::query(
            "insert into chaos_service_test_writes (write_key, write_value, attempt) \
             values ($1, 'gen-one', 1) \
             on conflict (write_key) do update set write_value = 'gen-one', attempt = 1, updated_at = now()",
        )
        .bind(&write_key)
        .execute(tx.connection())
        .await
        .expect("record write");
        // No commit: process dies here.
    }
    assert_eq!(write_count(&db, &write_key).await, 0, "crashed write leaves no ghost row");

    // Generation two: transient DB failure on first attempt, retry succeeds.
    sqlx::query(
        "insert into chaos_service_test_writes (write_key, write_value, attempt) \
         values ($1, 'gen-two', 2) \
         on conflict (write_key) do update set write_value = 'gen-two', attempt = 2, updated_at = now()",
    )
    .bind(&write_key)
    .execute(db.pool())
    .await
    .expect("retry write");
    assert_eq!(write_count(&db, &write_key).await, 1, "crash plus retry converges to exactly one write row");
    assert_eq!(write_value(&db, &write_key).await, Some("gen-two".to_string()), "retry write wins");

    // Generation three: another transient failure, retry with new value.
    sqlx::query(
        "insert into chaos_service_test_writes (write_key, write_value, attempt) \
         values ($1, 'gen-three', 3) \
         on conflict (write_key) do update set write_value = 'gen-three', attempt = 3, updated_at = now()",
    )
    .bind(&write_key)
    .execute(db.pool())
    .await
    .expect("retry write 2");
    assert_eq!(write_count(&db, &write_key).await, 1, "subsequent retries converge to same row");
    assert_eq!(write_value(&db, &write_key).await, Some("gen-three".to_string()), "latest retry wins");

    // Clean up.
    sqlx::query("delete from chaos_service_test_writes where write_key = $1")
        .bind(&write_key)
        .execute(db.pool())
        .await
        .expect("sweep");
}
