//! CRM.CLIENT — agents (TST-CRM-CLIENT-004).
//!
//! Contract: **the assignable-agent list offers exactly the internal accounts that are still active.** The list is
//! what the client screens populate "assign to" from, and it is read through `ClientService::agents`
//! (`web/src/clients/mod.rs:260-281`), which authorizes `person.read` and then hands the query to the DAO:
//! `select id::text, display_name from app_user where active = true order by display_name asc`
//! (`db/src/client/detail.rs:145-156`).
//!
//! Two things are therefore load-bearing and both are asserted here: the **active filter** (a deactivated account
//! must not be offered — assigning a client to somebody who has left is the defect this contract exists to prevent)
//! and the **stable order** (the same call twice answers the same sequence, so the picker does not shuffle under the
//! user). The list is read through the production service on a real pool, not a re-declared copy of its SQL.
//!
//! The negative case is the inactive row: it is seeded beside the active one, under the same run marker, and the
//! service must not return it. The control is the active row in the same call — so a green run cannot come from a
//! service that returned nothing at all.
//!
//! Level: L3 Composition — the production client service against an isolated, disposable DEV/Neon target.
//! `ClientHarness` refuses PRODUCTION before any socket is opened (`tests/src/database.rs:68-75`). Fixture `app_user`
//! rows are named under a unique run marker and deleted at the end; a zero-leftover count is asserted, so DEV is left
//! as it was found. No `person` row is written at all: this list reads `app_user` alone.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test crm_client__004__agents -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L3 contract needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use test_harness::ClientHarness;

const HARNESS: &str = "ClientHarness/L3 Composition";

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
///
/// This is infrastructure, not the contract: `ClientHarness` still refuses PRODUCTION before any socket is opened.
async fn connect_dev() -> ClientHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match ClientHarness::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; ClientHarness refuses PROD: {}",
        last.unwrap_or_default()
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); ClientHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-CLIENT-004); the file and the assay use it.
async fn crm_client_004__agents() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the assignable-agent proof runs only on an isolated DEV target"
    );
    let ctx = harness.test_context();
    let marker = format!("TST-CRM-CLIENT-004-{}-", harness.namespace());

    // One active account and one that has been deactivated — the same fixture, one field apart.
    let active_name = format!("{marker}Active Steward");
    let retired_name = format!("{marker}Retired Steward");
    let active_id = harness
        .seed_app_user(&active_name, true)
        .await
        .expect("proof: an app_user fixture row must be insertable on DEV");
    let retired_id = harness
        .seed_app_user(&retired_name, false)
        .await
        .expect("proof: an inactive app_user fixture row must be insertable on DEV");

    // 1. The list through the production service, and the same call again: the answer is stable.
    let first = harness
        .service()
        .agents(&ctx)
        .await
        .expect("proof: clients.agents must answer on the composed service");
    let second = harness
        .service()
        .agents(&ctx)
        .await
        .expect("proof: clients.agents must answer on the composed service");
    assert_eq!(
        first, second,
        "{HARNESS}: the picker's list must not shuffle between two identical calls"
    );

    // 2. This run's rows, and only this run's: the active account is offered, the retired one is not.
    let seeded: Vec<&str> = first
        .iter()
        .filter(|agent| agent.display_name.starts_with(marker.as_str()))
        .map(|agent| agent.display_name.as_str())
        .collect();
    assert_eq!(
        seeded,
        vec![active_name.as_str()],
        "{HARNESS}: exactly the active seeded account is assignable; a deactivated account must never be offered"
    );
    let active = first
        .iter()
        .find(|agent| agent.display_name == active_name)
        .expect("proof: the active seeded account must be in the list");
    assert_eq!(
        active.id, active_id,
        "{HARNESS}: each agent carries the app_user id the assignee is written with (id::text, not a label)"
    );
    assert!(
        !first.iter().any(|agent| agent.id == retired_id),
        "{HARNESS}: the deactivated account must be absent even though it exists in app_user"
    );
    assert!(
        !first.iter().any(|agent| agent.display_name == retired_name),
        "{HARNESS}: and absent by name too — nothing about the retired account reaches the picker"
    );

    // 3. The list is the whole internal estate, so the seeded pair is not the only content: the catalogue the UI
    //    picks from is non-empty on DEV, which is what an "assign to" control needs.
    assert!(
        !first.is_empty(),
        "{HARNESS}: the assignable-agent list must never be empty where an account exists"
    );

    // 4. Teardown: exactly this run's two rows, and none left behind.
    let removed = harness
        .cleanup_app_users(&marker)
        .await
        .expect("proof: the fixture rows must be deletable");
    assert_eq!(
        removed, 2,
        "{HARNESS}: teardown must remove exactly the two rows this run seeded"
    );
    let leftover = harness
        .app_user_leftover_count(&marker)
        .await
        .expect("proof: the leftover count must be readable");
    assert_eq!(
        leftover, 0,
        "{HARNESS}: DEV must be left as it was found — zero fixture rows remain"
    );
}
