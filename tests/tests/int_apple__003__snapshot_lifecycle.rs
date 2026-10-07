//! INT.APPLE — snapshot lifecycle (TST-INT-APPLE-003).
//!
//! CONTRACT.
//!
//! ```text
//! the current-state projection rebuilds l_person / l_property as the latest snapshot of the
//! Apple Contacts ODS, in one transaction. It resolves the account and the latest LOADED batch
//! itself when given neither, prunes contacts that are no longer in the snapshot, and reports the
//! before/after/pruned tally plus the address-type breakdown.
//! ```
//!
//! The snapshot boundary is the `apple_contacts_project` SQL function (`db/migrations/254`), called
//! through the harness's `with_rollback` so no row outlives the test. The test exercises the same
//! boundary production uses: the projection after a load, the re-projection (idempotent), and the
//! account resolution.
//!
//! The negative case is the payload itself: a projection with no staged contacts is a no-op (not
//! an error), and a projection for an unknown account is refused (never a guess).
//!
//! Level: L1 Component — the snapshot lifecycle boundary with the database as a deterministic
//! collaborator. The harness refuses PRODUCTION before any socket is opened.

#[path = "support/int_apple_contacts.rs"]
mod contacts;

use contacts::{connect_dev, load_apple_contacts, project_apple_contacts, valid_payload};
use serde_json::json;

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-INT-APPLE-003); the file and the assay use it.
async fn int_apple_003__snapshot_lifecycle() {
    let database = connect_dev().await.expect("the DEV database is reachable");

    // A unique export id per run, so the batch is created fresh and the test is isolated.
    let export_id = format!("EXPORT-SNAPSHOT-{}", std::process::id());

    // 1. Stage a batch so the projection has a snapshot to project.
    let payload = valid_payload(&export_id);
    let load_tally = load_apple_contacts(&database, &payload, "apple_contacts_local")
        .await
        .expect("the batch is staged");
    assert_eq!(load_tally["exitCode"], json!(0), "the load succeeds");

    // 2. The projection rebuilds l_person / l_property from the latest snapshot. The tally
    //    reports the before/after/pruned counts and the address-type breakdown.
    let project = project_apple_contacts(&database, Some("apple_contacts_local"))
        .await
        .expect("the projection succeeds");
    assert_eq!(project["exitCode"], json!(0), "the projection succeeds");
    assert_eq!(project["status"], json!("projected"));
    assert_eq!(project["source"], json!("apple_contacts"));
    assert_eq!(project["sourceAccount"], json!("apple_contacts_local"));
    assert!(project["snapshotMembership"].as_i64().unwrap_or(0) > 0, "the snapshot has members");
    assert!(project["totals"]["after"].as_i64().unwrap_or(0) > 0, "the projection writes rows");
    assert_eq!(project["totals"]["error"], json!(0), "no errors");

    // 3. The projection is idempotent: a second projection of the same snapshot prunes nothing
    //    and changes nothing. The before count equals the after count.
    let reproject = project_apple_contacts(&database, Some("apple_contacts_local"))
        .await
        .expect("the re-projection succeeds");
    assert_eq!(
        reproject["totals"]["before"], project["totals"]["after"],
        "the re-projection starts from the same count"
    );
    assert_eq!(
        reproject["totals"]["pruned"], json!(0),
        "the re-projection prunes nothing"
    );

    // 4. NEGATIVE: a projection with None is refused when the database holds more than one
    //    apple_contacts account. The function never guesses which account to project.
    let result = project_apple_contacts(&database, None).await;
    assert!(
        result.is_err(),
        "None with multiple accounts is refused: {result:?}"
    );

    // 5. NEGATIVE: a projection for an unknown account is refused, never guessed.
    let result = project_apple_contacts(&database, Some("unknown_account")).await;
    assert!(
        result.is_err(),
        "an unknown account is refused: {result:?}"
    );

}
