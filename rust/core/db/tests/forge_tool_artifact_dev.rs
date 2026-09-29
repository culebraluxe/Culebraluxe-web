//! The `forge_tool_artifact` funnel against DEV.
//!
//! Run explicitly with:
//!   DATABASE_URL_DEV=... cargo test -p db --test forge_tool_artifact_dev -- --ignored
//!
//! Why this exists: migration 130 created `forge_tool_artifact` for "per-execution tool verdicts/outputs … that
//! future Forge agents / operator / Cline should query, not re-derive", and the port wrote **no row ever** — no
//! reader, no writer, no artifact. A unit test cannot see a table nobody writes to; this one exercises the funnel
//! against the real schema (the FK to `storyboard_story`, the uuid FK to the run) and the real ruling column:
//!
//!   * a `run-verdict` artifact that agrees with its run keeps its verdict;
//!   * one that contradicts its run keeps its summary and carries no verdict;
//!   * an unruled run lends no verdict to anything — whether the run exists and is unruled, or is not named at all;
//!   * an assay artifact keeps its own `PASS` while the run is unruled;
//!   * an artifact for a story that does not exist is refused by the schema.
//!
//! It leaves DEV as it found it: the proof story and its run are deleted (artifacts cascade with the story).

use db::{Database, DbTarget, ForgeEngineDao, NewToolArtifact};

async fn cleanup_story(pool: &sqlx::PgPool, story_id: &str) {
    let _ = sqlx::query("delete from storyboard_story where id = $1")
        .bind(story_id)
        .execute(pool)
        .await;
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn a_tool_artifact_carries_its_run_ruling_and_never_a_second_opinion() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    let engine = ForgeEngineDao::new(database.clone());

    let tag = uuid::Uuid::new_v4().simple().to_string();
    let story = format!("ENG-PROOF-ARTIFACT-{tag}");

    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status, notes)
         values ($1, 'PROOF', 'Tool artifact proof', 'High', 'In Progress', '')",
    )
    .bind(&story)
    .execute(pool)
    .await
    .expect("insert proof story");

    let run: String = sqlx::query_scalar(
        "insert into storyboard_story_run (story_id, started_at, execution_environment, run_type)
         values ($1, now(), 'DEV', 'dispatch') returning id::text",
    )
    .bind(&story)
    .fetch_one(pool)
    .await
    .expect("open a proof run");

    let artifact = |kind: &str, verdict: Option<&str>, summary: Option<&str>| NewToolArtifact {
        story_id: story.clone(),
        story_run_id: Some(run.clone()),
        tool: "opencode".into(),
        kind: kind.into(),
        verdict: verdict.map(str::to_string),
        summary: summary.map(str::to_string),
        detail: None,
        sha: None,
    };

    // 1. A run that is still unruled lends its artifact no verdict: nothing was ruled, so nothing can agree.
    let unruled = engine
        .record_tool_artifact(&artifact(
            "run-verdict",
            Some("Failed"),
            Some("the lane said fail"),
        ))
        .await
        .unwrap();
    assert_eq!(
        unruled.verdict, None,
        "an unruled run certifies nothing, so no artifact may carry its verdict"
    );
    assert_eq!(
        unruled.summary.as_deref(),
        Some("the lane said fail"),
        "the summary is the artifact's own text and stays"
    );
    assert_eq!(unruled.story_id, story);
    assert_eq!(unruled.story_run_id.as_deref(), Some(run.as_str()));

    // 2. An assay's own reading is a measurement, not a claim about the run: it keeps `PASS` while unruled.
    let assay = engine
        .record_tool_artifact(&artifact("qa-assay-evidence", Some("PASS"), None))
        .await
        .unwrap();
    assert_eq!(assay.verdict.as_deref(), Some("PASS"));
    assert_eq!(assay.tool, "opencode");

    // 2b. The other door to the same rule: an artifact that names no run at all has no ruling to agree with, so a
    //     `run-verdict` cannot be written into it either.
    let no_run = NewToolArtifact {
        story_run_id: None,
        ..artifact("run-verdict", Some("Complete"), Some("no run named"))
    };
    let unnamed = engine.record_tool_artifact(&no_run).await.unwrap();
    assert_eq!(unnamed.verdict, None);
    assert_eq!(unnamed.story_run_id, None);
    assert_eq!(unnamed.summary.as_deref(), Some("no run named"));

    // 3. The run is ruled `Complete`; a contradicting `run-verdict` is refused its verdict and keeps its summary.
    sqlx::query(
        "update storyboard_story_run
            set ended_at=now(), result_status='Complete', updated_at=now()
          where id=$1::uuid",
    )
    .bind(&run)
    .execute(pool)
    .await
    .expect("rule the proof run");

    let contradicting = engine
        .record_tool_artifact(&artifact(
            "run-verdict",
            Some("Hold"),
            Some("detail stays with the artifact"),
        ))
        .await
        .unwrap();
    assert_eq!(contradicting.verdict, None);
    assert_eq!(
        contradicting.summary.as_deref(),
        Some("detail stays with the artifact")
    );

    // 4. Polarity, not spelling: `Complete` + `PASS` is the same agreement said twice.
    let agreeing = engine
        .record_tool_artifact(&artifact(
            "run-verdict",
            Some("PASS"),
            Some("lane finished clean"),
        ))
        .await
        .unwrap();
    assert_eq!(agreeing.verdict.as_deref(), Some("PASS"));

    // 5. Every row above is a row in the table, keyed to this run — the point of the funnel.
    let written: i64 =
        sqlx::query_scalar("select count(*) from forge_tool_artifact where story_run_id = $1::uuid")
            .bind(&run)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        written, 4,
        "four artifacts were recorded against the proof run"
    );

    // 6. A row that names a story which does not exist is refused by the schema, not repaired by the writer.
    let orphan = NewToolArtifact {
        story_id: format!("ENG-PROOF-ARTIFACT-MISSING-{tag}"),
        ..artifact("run-verdict", Some("Complete"), None)
    };
    assert!(
        engine.record_tool_artifact(&orphan).await.is_err(),
        "an artifact for a story that does not exist must be refused by the foreign key"
    );

    cleanup_story(pool, &story).await;
}
