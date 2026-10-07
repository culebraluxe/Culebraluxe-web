//! Shared fixture for the INT.APPLE Contacts suite (`int_apple__00N__*.rs`).
//!
//! These rails bind the contacts chain — `apple_contacts_load` (staging),
//! `warehouse_promote_apple_contacts` (promotion) and `apple_contacts_project` (snapshot lifecycle)
//! — against the shared DEV database through the harness's `TestDatabase`, which refuses PRODUCTION
//! before any socket is opened.
//!
//! The composition is PRODUCTION: the SQL functions are the same ones the `LandingDao` methods call
//! and the same ones production runs. The database is the shared DEV database; each test uses a
//! unique export id so its rows are isolated and removable, and the harness's `with_rollback`
//! wraps the call so no row outlives the test.
//!
//! Included with `#[path = "support/int_apple_contacts.rs"] mod contacts;` — a directory under
//! `tests/` is not a target.
#![allow(dead_code)]

use db::{Database, LandingDao};
use serde_json::{json, Value};
use test_harness::database::{HarnessDbError, TestDatabase};

/// Connect to the shared DEV database, refusing PRODUCTION before any socket is opened.
pub async fn connect_dev() -> Result<TestDatabase, HarnessDbError> {
    TestDatabase::connect_declared(Some("dev"), Some("dev")).await
}

/// A valid Apple Contacts export payload, the shape `apple_contacts_load` expects.
///
/// The source ids are derived from the export id, so each run stages fresh contacts that the
/// database has not seen before.
pub fn valid_payload(export_id: &str) -> Value {
    // A short unique suffix derived from the export id, so each run stages fresh contacts
    // with fresh emails and phones that the database has not seen before.
    let suffix = export_id.replace(['-', '_'], "");
    json!({
        "exportId": export_id,
        "schemaVersion": 1,
        "sourceSystem": "apple_contacts",
        "exportedAt": "2026-10-07T00:00:00.000Z",
        "fileSha256": "abc123",
        "contacts": [
            {
                "sourceId": format!("A:ABPerson-{suffix}-1"),
                "givenName": "Dana",
                "familyName": "Ruiz",
                "emails": [{"value": format!("dana.{suffix}@example.com")}],
                "phones": [{"value": format!("+17875551234{suffix}")}]
            },
            {
                "sourceId": format!("A:ABPerson-{suffix}-2"),
                "givenName": "Alex",
                "familyName": "Cruz",
                "emails": [{"value": format!("alex.{suffix}@example.com")}],
                "phones": []
            }
        ]
    })
}

/// Call `apple_contacts_load` on the pool and return the tally.
///
/// The call runs on the pool (not inside `with_rollback`), because the SQL function manages its
/// own transaction. The test cleans up its rows by export id afterwards.
pub async fn load_apple_contacts(
    database: &TestDatabase,
    payload: &Value,
    source_account: &str,
) -> Result<Value, String> {
    let dao = LandingDao::new(database.database().clone());
    dao.load_apple_contacts(payload, source_account)
        .await
        .map_err(|e| e.to_string())
}

/// Call `warehouse_promote_apple_contacts` on the pool and return the tally.
pub async fn promote_apple_contacts(
    database: &TestDatabase,
    apply: bool,
) -> Result<Value, String> {
    let dao = LandingDao::new(database.database().clone());
    dao.promote_apple_contacts(apply)
        .await
        .map_err(|e| e.to_string())
}

/// Call `apple_contacts_project` on the pool and return the tally.
pub async fn project_apple_contacts(
    database: &TestDatabase,
    source_account: Option<&str>,
) -> Result<Value, String> {
    let dao = LandingDao::new(database.database().clone());
    dao.project_apple_contacts(source_account)
        .await
        .map_err(|e| e.to_string())
}

/// Delete the rows one contacts-chain call created, by export id.
///
/// The batch is keyed on (source, source_account, exportId). Child rows in
/// `integration_staged_contact_profile` and `relationship_evidence` reference the batch with
/// RESTRICT, so they are deleted first; the snapshot membership cascades.
pub async fn cleanup_export(database: &TestDatabase, export_id: &str) -> Result<(), String> {
    let pool = database.database().pool();
    sqlx::query(
        "delete from integration_staged_contact_profile where integration_intake_batch_id in (select id from integration_intake_batch where source = 'apple_contacts' and external_batch_id = $1)",
    )
    .bind(export_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    sqlx::query("delete from integration_intake_batch where source = 'apple_contacts' and external_batch_id = $1")
        .bind(export_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// The pool the DAO writes to, for reading the committed truth back.
pub fn pool(database: &TestDatabase) -> &sqlx::PgPool {
    database.database().pool()
}

/// A reference to the inner `Database`, for callers that need the production type directly.
pub fn inner(database: &TestDatabase) -> &Database {
    database.database()
}
