//! The `candidate-code` fail-safe round trip against DEV.
//!
//! Run explicitly with:
//!   DATABASE_URL_DEV=... cargo test -p db --test forge_candidate_code_dev -- --ignored
//!
//! Why this exists: the fail-safe is two halves in two crates — the Smith lane writes the candidate patch into
//! `forge_tool_artifact.detail` (`kind='candidate-code'`, `engine::runner::smith_candidate_artifact`) and
//! `forge salvage` reads the newest one back (`ForgeEngineDao::candidate_code_for_story`). A unit test can hold
//! either half in its hand; only the schema can say whether a diff survives the trip as `jsonb`, whether a story
//! with no capture reads as empty rather than as an error, and whether the *newest* capture is really what a
//! second run leaves behind — which is the whole question a recovery run is asking.
//!
//! It leaves DEV as it found it: the proof story and its run are deleted (artifacts cascade with the story).

use db::{Database, DbTarget, ForgeEngineDao, NewToolArtifact};
use serde_json::json;

/// A capture shaped exactly the way the Smith lane writes one, so the test cannot pass on a shape the engine
/// would never produce.
fn capture(story: &str, patch: &str, sha: &str) -> NewToolArtifact {
    NewToolArtifact {
        story_id: story.to_string(),
        story_run_id: None,
        tool: "smith".into(),
        kind: "candidate-code".into(),
        verdict: None,
        summary: Some(format!("1 file(s), {} patch byte(s) at {sha}", patch.len())),
        detail: Some(json!({
            "base": "b".repeat(40),
            "candidateSha": sha,
            "changedFiles": ["rust/test-harness/tests/t.rs"],
            "patchBytes": patch.len(),
            "patch": patch,
        })),
        sha: Some(sha.to_string()),
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn the_captured_candidate_comes_back_unchanged_and_newest_first() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    let engine = ForgeEngineDao::new(database.clone());

    let tag = uuid::Uuid::new_v4().simple().to_string();
    let story = format!("ENG-PROOF-CANDIDATE-{tag}");
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'Candidate fail-safe proof', 'High', 'In Progress', '')",
    )
    .bind(&story)
    .execute(pool)
    .await
    .expect("insert proof story");

    // 1. A story nobody captured reads as *empty*, not as a failure: "nothing was captured" and "the read
    //    broke" have to stay distinguishable, because `forge salvage` turns the first into a sentence and the
    //    second into a database error.
    assert!(
        engine.candidate_code_for_story(&story).await.unwrap().is_none(),
        "a story with no capture reads as no capture"
    );

    // 2. The patch survives the jsonb round trip byte for byte. A diff is the deliverable here, so anything
    //    that mangles it — re-escaping, newline folding — is the fail-safe failing open.
    let patch = "diff --git a/rust/test-harness/tests/t.rs b/rust/test-harness/tests/t.rs\n\
                 --- a/rust/test-harness/tests/t.rs\n\
                 +++ b/rust/test-harness/tests/t.rs\n\
                 @@ -1,3 +1,4 @@\n\
                 +fn the_new_assertion() { assert_eq!(1, 1); }\n";
    let first_sha = "a".repeat(40);
    engine
        .record_tool_artifact(&capture(&story, patch, &first_sha))
        .await
        .unwrap();

    let read = engine
        .candidate_code_for_story(&story)
        .await
        .unwrap()
        .expect("the capture is readable");
    assert_eq!(read["patch"], json!(patch), "the code comes back byte for byte");
    assert_eq!(read["candidateSha"], json!(first_sha));
    assert_eq!(read["base"], json!("b".repeat(40)));
    assert_eq!(read["changedFiles"][0], json!("rust/test-harness/tests/t.rs"));

    // 3. A second run's capture is the one a recovery replays. The first row is backdated rather than relied on
    //    to be older: two inserts microseconds apart would otherwise make this an assertion about clock
    //    resolution instead of about `order by created_at desc`.
    sqlx::query("update forge_tool_artifact set created_at = now() - interval '1 hour' where story_id = $1")
        .bind(&story)
        .execute(pool)
        .await
        .expect("backdate the first capture");
    let second_sha = "c".repeat(40);
    engine
        .record_tool_artifact(&capture(&story, "the second run's patch", &second_sha))
        .await
        .unwrap();

    let read = engine
        .candidate_code_for_story(&story)
        .await
        .unwrap()
        .expect("the second capture is readable");
    assert_eq!(
        read["patch"],
        json!("the second run's patch"),
        "the newest capture is the candidate that was lost"
    );
    assert_eq!(read["candidateSha"], json!(second_sha));

    // 4. It is a report, not a verdict: neither row may claim one about the run.
    let verdicts: Vec<Option<String>> = sqlx::query_scalar(
        "select verdict from forge_tool_artifact where story_id = $1 and kind = 'candidate-code'",
    )
    .bind(&story)
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(verdicts.len(), 2);
    assert!(
        verdicts.iter().all(Option::is_none),
        "a captured candidate asserts nothing about the run: {verdicts:?}"
    );

    // 5. The foreign key is real, so a capture cannot drift loose of the story it belongs to.
    let orphan = capture(&format!("ENG-PROOF-MISSING-{tag}"), patch, &first_sha);
    assert!(
        engine.record_tool_artifact(&orphan).await.is_err(),
        "a capture for a story that does not exist is refused"
    );

    let _ = sqlx::query("delete from storyboard_story where id = $1")
        .bind(&story)
        .execute(pool)
        .await;
}
