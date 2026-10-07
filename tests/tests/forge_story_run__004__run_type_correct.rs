//! FORGE.STORY_RUN — run type correct (TST-FORGE-STORY-RUN-004).
//!
//! Contract: a Story Run records the run type of the work item whose claim opened it. The type is the database's
//! expression, written by the insert that opens the run (`db/migrations/264_forge_agent_work_begin.sql:51`):
//!
//! ```sql
//! coalesce(nullif(trim(i.role), ''), nullif(trim(i.kind), ''), 'dispatch')
//! ```
//!
//! so the precedence is `role`, then `kind`, then the dispatch fallback — and a blank at either link is the ABSENCE
//! of a fact, never a blank type. What this file proves:
//!
//!   1. **`role` wins** — the run opened by a claim whose item carries a role records that role as its `run_type`;
//!   2. **`kind` is the second link** — with no role, the batch kind becomes the run type;
//!   3. **the dispatch fallback** — with a whitespace role and no kind, the run records `dispatch`, never `''` or a
//!      padded word;
//!   4. **the type is frozen at open** — re-stamping the item's `role` after the run opened does not rewrite the
//!      run's `run_type`; the recorded type is the one execution started under;
//!   5. **negative / refusal** — `kind` is CHECK-constrained to its six batch words, so a seventh word never enters
//!      the queue and can never reach a run;
//!   6. **negative / fault** — every recorded `run_type` is non-null, non-empty and trimmed, on every run this file
//!      opens: a blank run type cannot exist without this test failing.
//!
//! Greenfield Rust: not a port of any TypeScript test. The claim and the run open are the production
//! `ForgeEngineDao` methods driven through the `ForgeHarness` against an isolated, disposable DEV/Neon target;
//! raw SQL here is fixture setup and read-back only. No production code is changed to make this pass.
//!
//! Boundary: L2 Persistence, harness `ForgeHarness`.
//!
//! Run with (the DEV branch only):
//!   set -a; . ./.env.local; set +a
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_story_run__004__run_type_correct -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use sqlx::PgPool;
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const PROOF_PREFIX: &str = "TST-FORGE-STORY-RUN-004-";
const OWNER: &str = "forge-story-run-004-owner";
/// The role the first proof item carries, and the type its run must record.
const ROLE: &str = "builder";
/// The batch kind the second proof item carries (one of the six words `agent_work_item_kind_check` allows).
const KIND: &str = "learn";

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
async fn connect_dev() -> ForgeHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match ForgeHarness::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; TestDatabase refuses PROD: {}",
        last.unwrap_or_default()
    );
}

/// Put one disposable story on the board at `Ready` and return the exactly-one work item its dispatch trigger
/// queued — the same queue production dispatches from.
async fn seed(harness: &ForgeHarness, story_id: &str) -> String {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match harness.seed_ready_story(story_id).await {
            Ok(item) => return item,
            Err(error) => {
                let message = error.to_string();
                let landed: Option<String> = sqlx::query_scalar(
                    "select id::text from agent_work_item where story_id=$1 and state='Ready'",
                )
                .bind(story_id)
                .fetch_optional(harness.pool())
                .await
                .ok()
                .flatten();
                if let Some(item) = landed {
                    return item;
                }
                eprintln!("proof: seed attempt {attempt} failed: {message}");
                last = Some(message);
                tokio::time::sleep(std::time::Duration::from_millis(250 * attempt)).await;
            }
        }
    }
    panic!(
        "the board's Ready trigger must queue exactly one work item for {story_id}: {}",
        last.unwrap_or_default()
    );
}

/// Claim the item and open its run through the production seam, returning the run id.
async fn begin(harness: &ForgeHarness, item: &str) -> String {
    let claimed = harness
        .engine()
        .claim_specific_agent_work(item, OWNER)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable");
    assert!(
        !claimed.id.is_empty(),
        "{HARNESS}: control — the claim handed over a row"
    );
    harness
        .engine()
        .begin_agent_work_run(item)
        .await
        .expect("the production begin runs")
        .expect("the claim opens the run")
        .story_run_id
}

/// The `run_type` committed on the run row.
async fn run_type(pool: &PgPool, run_id: &str) -> Option<String> {
    sqlx::query_scalar("select run_type from storyboard_story_run where id=$1::uuid")
        .bind(run_id)
        .fetch_one(pool)
        .await
        .expect("the run's run_type is readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-STORY-RUN-004).
async fn forge_story_run_004__run_type_correct() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the proof runs only on an isolated DEV target"
    );
    let pool = harness.pool().clone();
    let ns = harness.database().namespace().to_string();

    let story_role = format!("{PROOF_PREFIX}role-{ns}");
    let story_kind = format!("{PROOF_PREFIX}kind-{ns}");
    let story_dispatch = format!("{PROOF_PREFIX}dispatch-{ns}");

    // 1. THE ROLE LINK: the item carries a role, the run must record it.
    let item_role = seed(&harness, &story_role).await;
    sqlx::query("update agent_work_item set role=$2 where id=$1::uuid")
        .bind(&item_role)
        .bind(ROLE)
        .execute(&pool)
        .await
        .expect("a role is set on the queued item");
    let run_role = begin(&harness, &item_role).await;
    let type_role = run_type(&pool, &run_role).await;

    // 2. THE KIND LINK: no role at all, so the batch kind becomes the run type.
    let item_kind = seed(&harness, &story_kind).await;
    sqlx::query("update agent_work_item set role=null, kind=$2 where id=$1::uuid")
        .bind(&item_kind)
        .bind(KIND)
        .execute(&pool)
        .await
        .expect("the batch kind is set on the queued item");
    let run_kind = begin(&harness, &item_kind).await;
    let type_kind = run_type(&pool, &run_kind).await;

    // 3. THE DISPATCH FALLBACK: a whitespace role is the absence of a fact, and there is no kind, so the run must
    //    record `dispatch` — not `''`, not `'   '`.
    let item_dispatch = seed(&harness, &story_dispatch).await;
    sqlx::query("update agent_work_item set role='   ', kind=null where id=$1::uuid")
        .bind(&item_dispatch)
        .execute(&pool)
        .await
        .expect("a whitespace role is one of the column's own shapes");
    let run_dispatch = begin(&harness, &item_dispatch).await;
    let type_dispatch = run_type(&pool, &run_dispatch).await;

    // 4. FROZEN AT OPEN: the item's role is re-stamped after its run opened; the run keeps the type execution
    //    started under, so a later envelope edit can never relabel a run that already happened.
    sqlx::query("update agent_work_item set role='assay' where id=$1::uuid")
        .bind(&item_role)
        .execute(&pool)
        .await
        .expect("the item's role can be re-stamped");
    let type_role_after = run_type(&pool, &run_role).await;
    let role_after: Option<String> =
        sqlx::query_scalar("select role from agent_work_item where id=$1::uuid")
            .bind(&item_role)
            .fetch_one(&pool)
            .await
            .expect("the item's role is readable");

    // 5. NEGATIVE / REFUSAL — THE KIND COLUMN ONLY HOLDS ITS SIX WORDS, so an invented batch word can never enter
    //    the queue and therefore can never reach a run's `run_type`.
    let kind_refused = sqlx::query("update agent_work_item set kind='invented' where id=$1::uuid")
        .bind(&item_kind)
        .execute(&pool)
        .await
        .is_err();

    // 6. NEGATIVE / FAULT — read every run this file opened; a blank or padded type anywhere fails the proof.
    let types: Vec<(String, Option<String>)> = sqlx::query_as(
        "select story_id, run_type from storyboard_story_run
          where story_id like $1 order by story_id",
    )
    .bind(format!("{PROOF_PREFIX}%-{ns}"))
    .fetch_all(&pool)
    .await
    .expect("every proof run is readable");
    let blanks: Vec<String> = types
        .iter()
        .filter(|(_, run_type)| match run_type {
            None => true,
            Some(value) => value.trim().is_empty() || value.trim() != value.as_str(),
        })
        .map(|(story, _)| story.clone())
        .collect();

    // 7. CLEANUP / ROLLBACK: the proof stories are deleted; their items and runs go with them.
    for story in [&story_role, &story_kind, &story_dispatch] {
        harness
            .cleanup_story(story)
            .await
            .expect("reap the proof story");
    }
    let scope = format!("{PROOF_PREFIX}%-{ns}");
    let stories_left: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story where id like $1")
            .bind(&scope)
            .fetch_one(&pool)
            .await
            .unwrap();
    let items_left: i64 =
        sqlx::query_scalar("select count(*) from agent_work_item where story_id like $1")
            .bind(&scope)
            .fetch_one(&pool)
            .await
            .unwrap();
    let runs_left: i64 =
        sqlx::query_scalar("select count(*) from storyboard_story_run where story_id like $1")
            .bind(&scope)
            .fetch_one(&pool)
            .await
            .unwrap();

    // ---------------------------------------------------------------------------------------------------------
    // 8. THE CONTRACT.
    // ---------------------------------------------------------------------------------------------------------
    assert_eq!(
        type_role.as_deref(),
        Some(ROLE),
        "{HARNESS}: the run type is the claim's role — the first link of the precedence"
    );
    assert_eq!(
        type_kind.as_deref(),
        Some(KIND),
        "{HARNESS}: with no role, the run type is the item's batch kind"
    );
    assert_eq!(
        type_dispatch.as_deref(),
        Some("dispatch"),
        "{HARNESS}: a blank role and no kind fall through to the dispatch fallback, never to a blank type"
    );
    assert_eq!(
        role_after.as_deref(),
        Some("assay"),
        "{HARNESS}: control — the item's own role really did change after the run opened"
    );
    assert_eq!(
        type_role_after.as_deref(),
        Some(ROLE),
        "{HARNESS}: the recorded run type is frozen at open; a later envelope edit never relabels the run"
    );
    assert_eq!(
        types.len(),
        3,
        "{HARNESS}: exactly three proof runs were opened, one per precedence link"
    );
    assert!(
        blanks.is_empty(),
        "{HARNESS}: every recorded run type is present and trimmed; blank on: {blanks:?}"
    );

    // 9. NEGATIVE / REFUSAL.
    assert!(
        kind_refused,
        "{HARNESS}: a seventh batch word must be refused by the kind column's CHECK"
    );

    // 10. CLEANUP VERDICT: no proof row survives the test.
    assert_eq!(
        stories_left, 0,
        "{HARNESS}: the proof must leave no story behind"
    );
    assert_eq!(
        items_left, 0,
        "{HARNESS}: the proof must leave no work item behind"
    );
    assert_eq!(
        runs_left, 0,
        "{HARNESS}: the proof must leave no Story Run behind"
    );
}
