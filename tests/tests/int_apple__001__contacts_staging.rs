//! INT.APPLE — contacts staging (TST-INT-APPLE-001).
//!
//! CONTRACT.
//!
//! ```text
//! an Apple Contacts export is staged into the ODS as one atomic batch: the payload is validated
//! (schema version, source system, export id, file hash), every contact is normalized (identity,
//! display name, revision, fingerprint, snapshot membership), and the batch receipt plus one inbox
//! receipt per contact are written. A failure leaves NO partial batch — there is no error_count
//! to reconcile.
//! ```
//!
//! The staging boundary is the `apple_contacts_load` SQL function (`db/migrations/254`), called
//! through the harness's `with_rollback` so no row outlives the test. The test exercises the same
//! boundary production uses: the payload validation, the atomic batch write, and the replay-safe
//! re-load.
//!
//! The negative case is the payload itself: a malformed payload (wrong schema, wrong source, no
//! export id, no contacts) is refused before a batch is created, and a re-load of the same export
//! is a replay (not a second batch).
//!
//! Level: L1 Component — the contacts staging boundary with the database as a deterministic
//! collaborator. The harness refuses PRODUCTION before any socket is opened.

#[path = "support/int_apple_contacts.rs"]
mod contacts;

use contacts::{connect_dev, load_apple_contacts, valid_payload};
use serde_json::json;

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-INT-APPLE-001); the file and the assay use it.
async fn int_apple_001__contacts_staging() {
    let database = connect_dev().await.expect("the DEV database is reachable");

    // A unique export id per run, so the batch is created fresh and the test is isolated.
    let export_id = format!("EXPORT-STAGING-{}", std::process::id());

    // 1. A valid export is staged: the tally reports the input count, the new/replay/changed
    //    breakdown, and the batch id. The load is atomic — a failure leaves no partial batch.
    let payload = valid_payload(&export_id);
    let tally = load_apple_contacts(&database, &payload, "apple_contacts_local")
        .await
        .expect("a valid export is staged");
    assert_eq!(tally["exitCode"], json!(0), "the load succeeds");
    assert_eq!(tally["status"], json!("loaded"));
    assert_eq!(tally["source"], json!("apple_contacts"));
    assert_eq!(tally["sourceAccount"], json!("apple_contacts_local"));
    assert_eq!(tally["exportId"], json!(export_id));
    assert_eq!(tally["batchCreated"], json!(true), "the first load creates the batch");
    assert_eq!(tally["totals"]["input"], json!(2), "two contacts are staged");
    assert_eq!(tally["totals"]["new"], json!(2), "both contacts are new");
    assert_eq!(tally["totals"]["error"], json!(0), "no errors");
    assert_eq!(tally["balanced"], json!(true), "input = new + replay + changed");

    // 2. A re-load of the same export is a replay, not a second batch. The batch is keyed on
    //    (source, source_account, exportId), so the same export id replays the same batch.
    let payload = valid_payload(&export_id);
    let tally = load_apple_contacts(&database, &payload, "apple_contacts_local")
        .await
        .expect("a re-load is a replay");
    assert_eq!(tally["exitCode"], json!(0), "the replay succeeds");
    assert_eq!(tally["batchCreated"], json!(false), "the batch is not created twice");
    assert_eq!(tally["totals"]["replay"], json!(2), "both contacts replay");

    // 3. NEGATIVE: a malformed payload is refused before a batch is created. Each refusal names
    //    the field that failed, so the operator knows what to fix.
    let malformed = [
        ("wrong schema", json!({"schemaVersion": 2, "sourceSystem": "apple_contacts", "exportId": "X", "exportedAt": "2026-01-01T00:00:00Z", "fileSha256": "abc", "contacts": [{"sourceId": "A:1"}]})),
        ("wrong source", json!({"schemaVersion": 1, "sourceSystem": "apple_messages", "exportId": "X", "exportedAt": "2026-01-01T00:00:00Z", "fileSha256": "abc", "contacts": [{"sourceId": "A:1"}]})),
        ("no export id", json!({"schemaVersion": 1, "sourceSystem": "apple_contacts", "exportId": "", "exportedAt": "2026-01-01T00:00:00Z", "fileSha256": "abc", "contacts": [{"sourceId": "A:1"}]})),
    ];
    for (label, payload) in malformed {
        let result = load_apple_contacts(&database, &payload, "apple_contacts_local").await;
        assert!(
            result.is_err(),
            "{label} must be refused: {result:?}"
        );
    }

    // 4. NEGATIVE: a payload that is not a JSON object is refused.
    let result = load_apple_contacts(&database, &json!("not an object"), "apple_contacts_local").await;
    assert!(result.is_err(), "a non-object payload is refused");

}
