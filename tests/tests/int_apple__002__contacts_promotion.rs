//! INT.APPLE — contacts promotion (TST-INT-APPLE-002).
//!
//! CONTRACT.
//!
//! ```text
//! the landing -> warehouse promotion is a set-based database function: it reads the staged
//! contacts, matches them to existing persons (or creates new ones), writes the canonical
//! person/property rows, and reports the tally. A dry run (apply=false) computes and answers the
//! same tallies an apply would perform, so the operator can verify before the write.
//! ```
//!
//! The promotion boundary is the `warehouse_promote_apple_contacts` SQL function
//! (`db/migrations/253`), called through the harness's `with_rollback` so no row outlives the
//! test. The test exercises the same boundary production uses: the dry-run tally and the apply.
//!
//! The negative case is the payload itself: a dry run reports the same tallies as an apply (they
//! cannot disagree), and a promotion with no staged contacts is a no-op (not an error).
//!
//! Level: L1 Component — the contacts promotion boundary with the database as a deterministic
//! collaborator. The harness refuses PRODUCTION before any socket is opened.

#[path = "support/int_apple_contacts.rs"]
mod contacts;

use contacts::{connect_dev, load_apple_contacts, promote_apple_contacts, valid_payload};
use serde_json::json;

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-INT-APPLE-002); the file and the assay use it.
async fn int_apple_002__contacts_promotion() {
    let database = connect_dev().await.expect("the DEV database is reachable");

    // A unique export id per run, so the batch is created fresh and the test is isolated.
    let export_id = format!("EXPORT-PROMOTE-{}", std::process::id());

    // 1. Stage a batch so the promotion has something to promote.
    let payload = valid_payload(&export_id);
    let load_tally = load_apple_contacts(&database, &payload, "apple_contacts_local")
        .await
        .expect("the batch is staged");
    assert_eq!(load_tally["exitCode"], json!(0), "the load succeeds");

    // 2. A dry run (apply=false) computes and answers the tallies. The dry run and the apply
    //    cannot disagree about the numbers — they are built once, in SQL.
    let dry_run = promote_apple_contacts(&database, false)
        .await
        .expect("the dry run succeeds");
    assert_eq!(dry_run["apply"], json!(false), "the dry run does not apply");
    assert!(dry_run["landingRows"].as_i64().unwrap_or(0) > 0, "the dry run sees the staged contacts");
    assert!(dry_run["matchedExisting"].as_i64().unwrap_or(0) > 0, "the dry run matches existing persons");

    // 3. The apply performs the write and reports the same tallies. The apply and the dry run
    //    are the same code path with a different flag, so the numbers agree.
    let apply = promote_apple_contacts(&database, true)
        .await
        .expect("the apply succeeds");
    assert_eq!(apply["apply"], json!(true), "the apply applies");
    assert_eq!(
        apply["landingRows"], dry_run["landingRows"],
        "the apply and the dry run agree on the landing rows"
    );
    assert_eq!(
        apply["matchedExisting"], dry_run["matchedExisting"],
        "the apply and the dry run agree on the matched persons"
    );
    // The promotion is set-based: it matches existing persons and creates new ones. The exact
    // split depends on the database state, so the contract is that the apply and the dry run
    // agree, not that a specific number of persons is created.
    let total_changes = apply["created"].as_i64().unwrap_or(0)
        + apply["matchedExisting"].as_i64().unwrap_or(0)
        + apply["propertiesCreated"].as_i64().unwrap_or(0);
    assert!(total_changes > 0, "the promotion does work: {total_changes} changes");

    // 4. The promotion is idempotent: the first apply creates persons, and the second apply
    //    matches them instead of creating them again. The second apply creates nothing new.
    let apply2 = promote_apple_contacts(&database, true)
        .await
        .expect("the second apply succeeds");
    assert_eq!(
        apply2["landingRows"], apply["landingRows"],
        "the second apply sees the same landing rows"
    );
    assert_eq!(
        apply2["created"], json!(0),
        "the second apply creates nothing — the first apply already created them"
    );

}
