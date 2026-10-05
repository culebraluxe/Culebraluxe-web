//! CRM.CATCHUP — lead projection idempotency (TST-CRM-CATCHUP-002).
//!
//! Contract: the catchup lead projection is idempotent — running it multiple times with the same
//! input converges to exactly one projection row. This test verifies idempotency when the same
//! lead is submitted repeatedly.
//!
//! Level: L3 Composition — CRM domain projection test.
//! Harness: IntakeHarness.
//! Requires DEV (`DATABASE_URL_DEV`); ignored otherwise.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test crm_catchup__002__lead_projection -- --ignored

use db::DbTarget;
use model::{CatchupLeadRequest, CatchupLeadResult};
use test_harness::IntakeHarness;
use uuid::Uuid;

const HARNESS: &str = "IntakeHarness/L3 Composition";

async fn connect_dev() -> IntakeHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match IntakeHarness::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; IntakeHarness refuses PROD: {}",
        last.unwrap_or_default()
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); IntakeHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-CATCHUP-002); the file and the assay use it.
async fn crm_catchup_002__lead_projection() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the idempotency proof runs only on an isolated DEV target"
    );
    let ns = harness.namespace().to_string();
    let marker = format!("TST-CRMCATCHUP002-{ns}");

    // Create test projection table if not exists (test isolation).
    sqlx::query(
        "create table if not exists crm_lead_projection (
            lead_id text primary key,
            projection_json jsonb not null,
            deal_count integer not null default 0,
            total_value numeric not null default 0,
            created_at timestamptz not null default now(),
            updated_at timestamptz not null default now()
        )",
    )
    .execute(harness.pool())
    .await
    .expect("create test table");

    let lead_id = format!("crm-catchup-002-{}-{}", marker, Uuid::new_v4());

    // Clean any existing projection for this lead (test isolation).
    sqlx::query("delete from crm_lead_projection where lead_id = $1")
        .bind(&lead_id)
        .execute(harness.pool())
        .await
        .expect("sweep");

    let request = CatchupLeadRequest {
        name: format!("{marker}-alice"),
        email: Some("alice@example.test".to_string()),
        phone: Some("17875551234".to_string()),
        message: Some("Test lead for idempotency".to_string()),
    };

    // First submission: creates the lead and projection.
    let result1 = harness
        .submit_catchup(&request)
        .await
        .expect("first catchup lead succeeds");
    assert_eq!(
        result1.status, "created",
        "{HARNESS}: first submission creates"
    );
    let person_id1 = result1.person_id.expect("person_id on create");

    // Read the projection created by the first submission.
    let proj1: Option<serde_json::Value> =
        sqlx::query_scalar("select projection_json from crm_lead_projection where lead_id = $1")
            .bind(&lead_id)
            .fetch_optional(harness.pool())
            .await
            .expect("read projection 1");

    // Second submission with same email/phone: should resolve to same person.
    let result2 = harness
        .submit_catchup(&request)
        .await
        .expect("second catchup lead succeeds");
    assert_eq!(
        result2.status, "resolved",
        "{HARNESS}: second submission resolves"
    );
    assert_eq!(
        result2.person_id,
        Some(person_id1.clone()),
        "{HARNESS}: resolves to same person"
    );

    // Read the projection after second submission.
    let proj2: Option<serde_json::Value> =
        sqlx::query_scalar("select projection_json from crm_lead_projection where lead_id = $1")
            .bind(&lead_id)
            .fetch_optional(harness.pool())
            .await
            .expect("read projection 2");

    // Verify idempotency: exactly one projection row exists and it's the same.
    let count: i64 =
        sqlx::query_scalar("select count(*) from crm_lead_projection where lead_id = $1")
            .bind(&lead_id)
            .fetch_one(harness.pool())
            .await
            .expect("projection count");
    assert_eq!(count, 1, "{HARNESS}: exactly one projection row exists");

    // The projection should be stable (same content or updated with same data).
    assert_eq!(proj1, proj2, "{HARNESS}: projection is idempotent");

    // Third submission: another idempotent call.
    let result3 = harness
        .submit_catchup(&request)
        .await
        .expect("third catchup lead succeeds");
    assert_eq!(
        result3.status, "resolved",
        "{HARNESS}: third submission resolves"
    );
    assert_eq!(
        result3.person_id,
        Some(person_id1),
        "{HARNESS}: resolves to same person"
    );

    let count_final: i64 =
        sqlx::query_scalar("select count(*) from crm_lead_projection where lead_id = $1")
            .bind(&lead_id)
            .fetch_one(harness.pool())
            .await
            .expect("projection final count");
    assert_eq!(
        count_final, 1,
        "{HARNESS}: still exactly one projection row after third call"
    );

    // Clean up.
    sqlx::query("delete from crm_lead_projection where lead_id = $1")
        .bind(&lead_id)
        .execute(harness.pool())
        .await
        .expect("sweep projection");
    let removed = harness.cleanup(&marker).await.expect("cleanup persons");
    assert_eq!(removed, 1, "{HARNESS}: exactly one person removed");
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("leftover count"),
        0
    );
}
