//! TWO STORIES AT ONCE. The queue is serial per STORY, not per system.
//!
//! Run explicitly with:
//!   DATABASE_URL_DEV=... cargo test -p db --test forge_claim__different_stories_can_be_claimed_while_peer_is_running -- --ignored
//!
//! Why this exists: the Rust port restored a refusal that asked the WHOLE SYSTEM to be idle —
//! `select id::text from agent_work_item where state in ('Claimed','Running') limit 1`, with no story scope — so
//! while one story ran, no other story could be claimed at all and three ready test stories queued behind a single
//! harness run. The same regression had been removed from the TypeScript claim on 2026-09-16
//! (`a3fc7099`, "THE GOVERNOR IS OFF: one serial chain PER STORY, not one per system … three ready stories queued
//! behind each other and only one ran").
//!
//! The acceptance case, exactly as the captain stated it:
//!
//!   Story A Ready -> claim -> Running
//!   Story B Ready -> claim MUST SUCCEED
//!   a second serial item on Story A -> MUST REFUSE
//!
//! Nothing is inferred from a comment: both stories are read back as `Running` at the same moment, and the refusal
//! is the database's own index (`agent_work_item_one_serial_active_per_story`) answering a second insert through the
//! driver, with the constraint name asserted. Both proof stories are deleted at the end — their items and runs
//! cascade — so DEV is left as it was found.

use db::{Database, DbTarget, ForgeEngineDao};

async fn cleanup_story(pool: &sqlx::PgPool, story_id: &str) {
    let _ = sqlx::query("delete from storyboard_story where id = $1")
        .bind(story_id)
        .execute(pool)
        .await;
}

/// The authority the item row holds — the fence every claim write takes (migration 278). It is READ from the
/// row rather than invented here: a test that guesses a generation proves something about the guess.
async fn fence_of(pool: &sqlx::PgPool, item_id: &str) -> db::ClaimFence {
    let (owner, generation): (Option<String>, i64) = sqlx::query_as(
        "select claimed_by, claim_generation from agent_work_item where id = $1::uuid",
    )
    .bind(item_id)
    .fetch_one(pool)
    .await
    .expect("the claim fence reads back");
    db::ClaimFence::new(owner.unwrap_or_default(), generation)
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV"]
async fn a_second_story_is_claimed_while_a_peer_runs_and_one_story_still_cannot_be_doubled() {
    let database = Database::connect_target(DbTarget::Dev)
        .await
        .expect("DATABASE_URL_DEV");
    let pool = database.pool();
    let engine = ForgeEngineDao::new(database.clone());

    let tag = uuid::Uuid::new_v4().simple().to_string();
    let story_a = format!("ENG-PROOF-LANE-A-{tag}");
    let story_b = format!("ENG-PROOF-LANE-B-{tag}");

    // 0. This proof owns the `ENG-PROOF-LANE-` namespace and reaps its own leftovers first, because a panicking run
    //    leaves them behind: the first version of this test died on a SQL ambiguity (42702) after inserting story A
    //    with its priority lifted to the top of the queue, and the next run then claimed that orphan instead of the
    //    story it had just created. Run this proof alone (`--test-threads=1`); nothing else uses that namespace.
    let reaped = sqlx::query("delete from storyboard_story where id like 'ENG-PROOF-LANE-%'")
        .execute(pool)
        .await
        .expect("reap this proof's own leftovers")
        .rows_affected();
    if reaped > 0 {
        eprintln!("proof: reaped {reaped} leftover lane-proof story/stories from an earlier run");
    }

    // 1. Two stories on the board at `Ready`. The dispatch trigger gives each exactly one `Ready` item — the same
    //    authority the engine reads — and nothing here writes a work item by hand.
    for story in [&story_a, &story_b] {
        sqlx::query(
            "insert into storyboard_story (id, workstream, title, priority, status, notes)
             values ($1, 'PROOF', 'Lane proof', 'High', 'Ready', '')",
        )
        .bind(story)
        .execute(pool)
        .await
        .expect("insert proof story");
    }

    // 2. Deterministic claim order, stated rather than hoped for: DEV may hold other claimable stories, so both
    //    proof items are lifted above every score `story_priority_score` can produce (its maximum is 100), with A
    //    ahead of B. The claim reads `priority desc, queued_at asc, id`, so the two claims below are provably about
    //    these two stories and no borrowed row has to be put back.
    for (story, priority) in [(&story_a, 1_000_002i32), (&story_b, 1_000_001i32)] {
        let lifted = sqlx::query(
            "update agent_work_item set priority = $2 where story_id = $1 and state = 'Ready'",
        )
        .bind(story)
        .bind(priority)
        .execute(pool)
        .await
        .expect("lift the proof item above the rest of the queue")
        .rows_affected();
        assert_eq!(
            lifted, 1,
            "the board's own trigger must have queued exactly one item for {story}"
        );
    }

    // 3. Story A: claimed, then Running. A live run is what the old refusal reacted to.
    let claim_a = engine
        .claim_next_agent_work("lane-proof:a")
        .await
        .unwrap()
        .expect("story A is Ready and must be claimable");
    assert_eq!(
        claim_a.story_id, story_a,
        "the highest-priority ready story claims first"
    );
    assert_eq!(claim_a.state, "Claimed");
    let run_a = engine
        .begin_agent_work_run(&claim_a.id, &fence_of(&pool, &claim_a.id).await)
        .await
        .unwrap()
        .expect("Claimed -> Running must open the run");

    // 4. THE REGRESSION. Story B must be claimable *while* story A runs. Before 2026-09-29 this returned `None`,
    //    because the claim asked whether ANY item anywhere was active instead of whether THIS story had one.
    let claim_b = engine
        .claim_next_agent_work("lane-proof:b")
        .await
        .unwrap()
        .expect("a different story MUST be claimable while a peer story is running");
    assert_eq!(
        claim_b.story_id, story_b,
        "the claim must be story B: the queue is serial per story, not per system"
    );
    assert_ne!(
        claim_b.story_id, claim_a.story_id,
        "two workers must never hold one story"
    );
    let run_b = engine
        .begin_agent_work_run(&claim_b.id, &fence_of(&pool, &claim_b.id).await)
        .await
        .unwrap()
        .expect("Claimed -> Running must open story B's run");
    assert_ne!(
        run_a.story_run_id, run_b.story_run_id,
        "each story owns its own run row"
    );

    // 4a. Simultaneity, read back at once: two stories, two worker identities, one open item each — the state the
    //     old governor made impossible. Two rows can only be here if both claims were granted while both ran.
    let running: Vec<(String, String, String)> = sqlx::query_as(
        "select i.story_id, i.state, i.claimed_by from agent_work_item i
          where i.story_id = any($1::text[]) and i.state = 'Running'
          order by i.story_id",
    )
    .bind(vec![story_a.clone(), story_b.clone()])
    .fetch_all(pool)
    .await
    .expect("read back both stories at once");
    assert_eq!(
        running.len(),
        2,
        "both proof stories must be Running at the same moment, not one after the other: {running:?}"
    );
    let holders: std::collections::BTreeSet<&str> =
        running.iter().map(|(_, _, by)| by.as_str()).collect();
    assert_eq!(
        holders.len(),
        2,
        "each slot claims under its own identity: {running:?}"
    );

    // 5. ONE STORY, ONE WRITER — the rule that did NOT change. A second serial item (`parallel_group_id is null`)
    //    on a story that already has an open one is refused by the database itself, so no claim path can produce
    //    the twin writers the governor was originally there to stop.
    let doubled = sqlx::query(
        "insert into agent_work_item (story_id, state, priority) values ($1, 'Ready', 1000003)",
    )
    .bind(&story_a)
    .execute(pool)
    .await;
    let error = doubled.expect_err("a second serial item on one story must be refused");
    let constraint = error
        .as_database_error()
        .and_then(|database_error| database_error.constraint())
        .unwrap_or("")
        .to_string();
    assert_eq!(
        constraint, "agent_work_item_one_serial_active_per_story",
        "the refusal must be the per-story index, not a different failure: {error}"
    );

    // 5a. And the by-id claim path refuses too: a story already running cannot be claimed again, by anyone.
    assert!(
        engine
            .claim_specific_agent_work(&claim_a.id, "lane-proof:c")
            .await
            .unwrap()
            .is_none(),
        "an item that is already running must not be claimable a second time"
    );
    assert!(
        engine
            .claim_specific_agent_work(&claim_b.id, "lane-proof:c")
            .await
            .unwrap()
            .is_none(),
        "and the peer story is no more double-claimable than the first"
    );

    // 6. Both runs are closed and both stories are removed: no claim, run or item is left behind on DEV.
    for claim in [&claim_a, &claim_b] {
        engine
            .finish_agent_work_run(
                &claim.id,
                &fence_of(&pool, &claim.id).await,
                db::AgentWorkOutcome::Abandoned,
                Some("proof cleanup"),
            )
            .await
            .expect("the proof must be able to put its own claims back");
    }
    cleanup_story(pool, &story_a).await;
    cleanup_story(pool, &story_b).await;
    let leftovers: i64 =
        sqlx::query_scalar("select count(*) from agent_work_item where story_id = any($1::text[])")
            .bind(vec![story_a.clone(), story_b.clone()])
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(leftovers, 0, "the proof must leave no item behind");
}
