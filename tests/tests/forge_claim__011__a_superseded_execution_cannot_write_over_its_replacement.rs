//! FORGE.CLAIM-011 — a superseded execution cannot write over its replacement (TST-FORGE-CLAIM-011).
//!
//! CONTRACT (work order `FORGE-B1` §5, invariants 1–3 and 5). One execution owns each active claim. The claim
//! statement bumps `agent_work_item.claim_generation` in the same write that names the owner, so `(id, generation)`
//! identifies ONE execution; every fenced transition — begin, heartbeat, settlement — validates owner AND generation
//! AND state under a row lock, and answers with a named result rather than a bare row-or-nothing.
//!
//! The three facts this file proves against a real database:
//!   * a claimant that LOST the item cannot begin its replacement's run, refresh its lease, or settle its work;
//!   * reusing a worker NAME does not reuse authority — a second claim by the same name is a new generation, and the
//!     old one is dead;
//!   * settlement idempotency is derived from (item, generation, outcome): the same execution asking twice is a
//!     `Duplicate`, and the same execution changing its verdict under one authority is a `Conflict`, never a second
//!     success.
//!
//! Level: L2 Persistence, harness ForgeHarness — the production DAO against a disposable DEV database; committed
//! rows read back across the pool are the proof. PROD is refused before any socket opens.
//!
//! Run it (ignored by default, like every DEV database contract in this repo):
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!       --test forge_claim__011__a_superseded_execution_cannot_write_over_its_replacement -- --ignored

use db::{AgentWorkOutcome, DbTarget, SettlementResult};
use test_harness::ForgeHarness;

const HARNESS: &str = "ForgeHarness/L2 Persistence";
const OWNER_A: &str = "forge-fence-owner-a";
const OWNER_B: &str = "forge-fence-owner-b";

/// Expire the lease so the queue hands the item to whoever asks next. This is the ordinary way a claim changes
/// hands while the old process is still alive: the worker died, or its beats stopped landing.
async fn expire_lease(pool: &sqlx::PgPool, item_id: &str) {
    sqlx::query(
        "update agent_work_item set lease_expires_at = now() - interval '1 minute' where id = $1::uuid",
    )
    .bind(item_id)
    .execute(pool)
    .await
    .expect("the lease is writable");
}

async fn item_row(pool: &sqlx::PgPool, item_id: &str) -> (String, Option<String>, i64) {
    sqlx::query_as(
        "select state, claimed_by, claim_generation from agent_work_item where id = $1::uuid",
    )
    .bind(item_id)
    .fetch_one(pool)
    .await
    .expect("the committed claim reads back")
}

async fn run_count(pool: &sqlx::PgPool, story_id: &str) -> i64 {
    sqlx::query_scalar("select count(*) from storyboard_story_run where story_id = $1")
        .bind(story_id)
        .fetch_one(pool)
        .await
        .expect("the run ledger is readable")
}

async fn heartbeat_at(pool: &sqlx::PgPool, item_id: &str) -> Option<String> {
    sqlx::query_scalar("select heartbeat_at::text from agent_work_item where id = $1::uuid")
        .bind(item_id)
        .fetch_one(pool)
        .await
        .expect("the lease is readable")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)] // the canonical taxonomy name is the contract, double underscores and all
async fn forge_claim_011__a_superseded_execution_cannot_write_over_its_replacement() {
    let harness = ForgeHarness::connect_declared(None, Some("test"))
        .await
        .expect(
            "DATABASE_URL_DEV must point at a disposable DEV database; the harness refuses PROD before connecting",
        );
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: an ownership contract must never run against PROD"
    );
    let pool = harness.pool();
    let tag = harness.database().namespace().to_owned();
    let story = format!("FORGE-CLAIM-011-FENCE-{tag}");

    let item = harness
        .seed_ready_story(&story)
        .await
        .expect("the Ready trigger must queue exactly one item");

    // ── A claims: generation 1 belongs to A. ─────────────────────────────────
    let claimed_a = harness
        .engine()
        .claim_specific_agent_work(&item, OWNER_A)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable");
    assert_eq!(claimed_a.claimed_by.as_deref(), Some(OWNER_A));
    let (_, _, generation_a) = item_row(pool, &item).await;
    assert_eq!(
        claimed_a.claim_generation, generation_a,
        "{HARNESS}: the claim returns the generation it granted, not a default"
    );
    assert!(
        generation_a >= 1,
        "{HARNESS}: a claim opens a NEW authority (got generation {generation_a})"
    );
    let fence_a = harness.claim_fence(&item).await.expect("A's fence reads");

    // ── The lease dies; B takes the item at the NEXT generation. ─────────────
    expire_lease(pool, &item).await;
    harness
        .engine()
        .claim_specific_agent_work(&item, OWNER_B)
        .await
        .expect("the reclaim runs")
        .expect("an expired lease is claimable again");
    let (_, owner_now, generation_b) = item_row(pool, &item).await;
    assert_eq!(owner_now.as_deref(), Some(OWNER_B));
    assert!(
        generation_b > generation_a,
        "{HARNESS}: a reclaim is a NEW authority ({generation_a} -> {generation_b})"
    );
    let fence_b = harness.claim_fence(&item).await.expect("B's fence reads");

    // ── A cannot open the run B now holds. ───────────────────────────────────
    let a_begin = harness
        .engine()
        .begin_agent_work_run(&item, &fence_a)
        .await
        .expect("the production begin runs");
    assert_eq!(
        a_begin, None,
        "{HARNESS}: a superseded execution must not begin its replacement's run"
    );
    assert_eq!(
        run_count(pool, &story).await,
        0,
        "{HARNESS}: a refused begin must not have opened a Story Run"
    );

    // ── B opens it, and the run is B's. ─────────────────────────────────────
    let b_begin = harness
        .engine()
        .begin_agent_work_run(&item, &fence_b)
        .await
        .expect("the production begin runs")
        .expect("the owner of the claim opens its own run");
    assert_eq!(
        b_begin.claim_generation, generation_b,
        "{HARNESS}: the begin answers with the generation it matched"
    );
    assert_eq!(run_count(pool, &story).await, 1);

    // ── A cannot refresh B's lease. ─────────────────────────────────────────
    let before = heartbeat_at(pool, &item).await;
    let a_beat = harness
        .engine()
        .heartbeat_agent_work(&item, &fence_a, std::time::Duration::from_secs(300))
        .await
        .expect("the production heartbeat runs");
    assert!(
        !a_beat,
        "{HARNESS}: a superseded execution must not renew a lease it no longer holds"
    );
    assert_eq!(
        heartbeat_at(pool, &item).await,
        before,
        "{HARNESS}: the refused beat must not have moved the replacement's lease"
    );
    let b_beat = harness
        .engine()
        .heartbeat_agent_work(&item, &fence_b, std::time::Duration::from_secs(300))
        .await
        .expect("the production heartbeat runs");
    assert!(b_beat, "{HARNESS}: the holder's own beat renews its lease");

    // ── A cannot settle B's work; B can. ────────────────────────────────────
    let a_settle = harness
        .engine()
        .finish_agent_work_run(
            &item,
            &fence_a,
            AgentWorkOutcome::Error,
            Some("A lost the claim"),
        )
        .await
        .expect("the production settle runs");
    assert!(
        matches!(a_settle, SettlementResult::RefusedOwnership(_)),
        "{HARNESS}: A's settle is refused by name, not reported as a verdict (got {})",
        a_settle.name()
    );
    assert!(
        !a_settle.wrote(),
        "{HARNESS}: a refused settle writes nothing"
    );
    let (state_after_refusal, owner_after_refusal, _) = item_row(pool, &item).await;
    assert_eq!(state_after_refusal, "Running");
    assert_eq!(owner_after_refusal.as_deref(), Some(OWNER_B));

    let b_settle = harness
        .engine()
        .finish_agent_work_run(
            &item,
            &fence_b,
            AgentWorkOutcome::Error,
            Some("B's verdict"),
        )
        .await
        .expect("the production settle runs");
    assert!(
        b_settle.wrote(),
        "{HARNESS}: the claim's holder settles it (got {})",
        b_settle.name()
    );

    harness
        .cleanup_story(&story)
        .await
        .expect("the fixture is swept");
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV: runs only against the disposable DEV branch (PROD is refused)"]
#[allow(non_snake_case)] // the canonical taxonomy name is the contract, double underscores and all
async fn forge_claim_011__reusing_a_worker_name_does_not_reuse_authority() {
    let harness = ForgeHarness::connect_declared(None, Some("test"))
        .await
        .expect("DATABASE_URL_DEV must point at a disposable DEV database");
    let pool = harness.pool();
    let tag = harness.database().namespace().to_owned();
    let story = format!("FORGE-CLAIM-011-NAME-{tag}");

    let item = harness
        .seed_ready_story(&story)
        .await
        .expect("the Ready trigger must queue exactly one item");

    // The SAME name claims twice, with the lease dying in between: two authorities, one name.
    harness
        .engine()
        .claim_specific_agent_work(&item, OWNER_A)
        .await
        .expect("the production claim runs")
        .expect("a Ready item must be claimable");
    let (_, _, first_generation) = item_row(pool, &item).await;

    expire_lease(pool, &item).await;
    harness
        .engine()
        .claim_specific_agent_work(&item, OWNER_A)
        .await
        .expect("the reclaim runs")
        .expect("an expired lease is claimable again by any name, including the old one");
    let (_, owner_now, second_generation) = item_row(pool, &item).await;
    assert_eq!(owner_now.as_deref(), Some(OWNER_A), "the name is the same");
    assert!(
        second_generation > first_generation,
        "{HARNESS}: the same name must not inherit the predecessor's authority \
         ({first_generation} -> {second_generation})"
    );

    // The predecessor's authority is dead: it cannot begin the run its own name now holds.
    let stale = db::ClaimFence::new(OWNER_A, first_generation);
    let stale_begin = harness
        .engine()
        .begin_agent_work_run(&item, &stale)
        .await
        .expect("the production begin runs");
    assert_eq!(
        stale_begin, None,
        "{HARNESS}: a name is not an authority — the old generation must not open the new run"
    );
    assert_eq!(run_count(pool, &story).await, 0);

    let current = harness.claim_fence(&item).await.expect("the fence reads");
    assert_eq!(current.generation, second_generation);
    harness
        .engine()
        .begin_agent_work_run(&item, &current)
        .await
        .expect("the production begin runs")
        .expect("the current generation opens its own run");

    // Settlement idempotency is bound to the execution identity AND the outcome.
    let settled = harness
        .engine()
        .finish_agent_work_run(
            &item,
            &current,
            AgentWorkOutcome::Error,
            Some("the verdict"),
        )
        .await
        .expect("the production settle runs");
    assert!(settled.wrote(), "{HARNESS}: the first settle writes");

    let repeated = harness
        .engine()
        .finish_agent_work_run(
            &item,
            &current,
            AgentWorkOutcome::Error,
            Some("the verdict"),
        )
        .await
        .expect("the repeated settle runs");
    assert!(
        matches!(repeated, SettlementResult::Duplicate(_)),
        "{HARNESS}: a legitimate retry of the same settle is idempotent (got {})",
        repeated.name()
    );
    assert!(!repeated.wrote(), "{HARNESS}: a duplicate writes nothing");

    let changed = harness
        .engine()
        .finish_agent_work_run(&item, &current, AgentWorkOutcome::Done, None)
        .await
        .expect("the conflicting settle runs");
    assert!(
        matches!(changed, SettlementResult::Conflict(_)),
        "{HARNESS}: changing the outcome under one authority is a conflict, not a second verdict (got {})",
        changed.name()
    );
    assert!(!changed.wrote(), "{HARNESS}: a conflict writes nothing");
    let (final_state, _, _) = item_row(pool, &item).await;
    assert_eq!(
        final_state, "Error",
        "{HARNESS}: the stored verdict is the one the first settle wrote"
    );

    harness
        .cleanup_story(&story)
        .await
        .expect("the fixture is swept");
}
