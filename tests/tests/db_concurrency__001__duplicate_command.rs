//! DB.CONCURRENCY — duplicate command (TST-DB-CONCURRENCY-001).
//!
//! Contract: **one command id, one effect, however many times it is delivered.** A command is admitted by
//! `CommandReceiptDao::claim_tx`, which inserts a `pending` receipt with `on conflict(command_id) do nothing
//! returning command_id` (`db/src/command_receipt.rs:89-117`). Postgres decides the winner, not the caller: the
//! unique key on `command_id` is the arbiter, so of any number of concurrent deliveries exactly one insert returns
//! a row (`true`) and every other one is silently absorbed by the conflict clause (`false`) rather than raising a
//! duplicate-key error. That is the difference between "one effect" and "one error per duplicate".
//!
//! Two further refusals belong to the same rule and are asserted here, because a claim that is only arbitrated
//! while nobody has looked at it yet is not yet idempotent:
//!
//! - **finality.** Once `finalize_tx` has recorded an outcome the command is *no longer claimable at all*: the row
//!   is there, so `on conflict do nothing` matches and every late duplicate is refused. A retried webhook after the
//!   handler finished therefore cannot re-execute, and cannot even re-open the receipt as `pending`.
//! - **crash-before-commit.** A claimant that dies between claiming and committing leaves no row, because
//!   sqlx rolls the dropped transaction back. So a retry is not permanently locked out by a claim that never
//!   committed: the next delivery wins it cleanly. The alternative — a claim that survived its claimant's death —
//!   would strand the command in `pending` forever.
//!
//! The proof is the committed row count, not the return value: after the race the table must hold exactly one
//! receipt for the command, no matter how many callers were told `true`.
//!
//! Level: L4 Adversarial — `RaceHarness` puts every caller inside the racy region at one
//! `ConcurrencyBarrier`, and refuses PRODUCTION before any socket is opened.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_concurrency__001__duplicate_command -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L4 contract needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use test_harness::RaceHarness;

const HARNESS: &str = "RaceHarness/L4 Adversarial";

/// Command metadata for the receipt. Nothing in the contract reads these; they are bound because the production
/// signature binds them.
const KIND: &str = "race-duplicate-command";
const FINGERPRINT: &str = "race-duplicate-fingerprint";
const AGGREGATE: &str = "race-duplicate-aggregate";
const REQUESTED_AT: &str = "2026-01-01T00:00:00+00:00";

/// How many deliveries race. More than two, so "exactly one winner" is not an artifact of a pair.
const DELIVERIES: usize = 8;

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
///
/// This is infrastructure, not the contract: `RaceHarness` still refuses PRODUCTION before any socket is opened.
async fn connect_dev() -> RaceHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match RaceHarness::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; RaceHarness refuses PROD: {}",
        last.unwrap_or_default()
    )
}

/// One delivery of one command id: claim it in its own transaction, commit, and report whether it won.
///
/// `RaceHarness::race` has already put every caller inside the racy region at one barrier, so this body does not
/// need a rendezvous of its own — arriving here *is* arriving together.
async fn deliver(harness: RaceHarness, command_id: String) -> bool {
    let mut tx = harness
        .database_handle()
        .begin("test-harness.race.duplicate_command")
        .await
        .expect("a delivery opens a transaction");
    let won = harness
        .receipts()
        .claim_tx(
            &mut tx,
            &command_id,
            KIND,
            FINGERPRINT,
            None,
            AGGREGATE,
            None,
            None,
            None,
            REQUESTED_AT,
        )
        .await
        .expect("claim_tx answers rather than erroring");
    tx.commit().await.expect("a delivery commits");
    won
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); RaceHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-CONCURRENCY-001); the assay uses it.
async fn db_concurrency_001__duplicate_command() {
    // 0. L4 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the duplicate-command proof runs only on an isolated DEV target"
    );
    let marker = format!(
        "tstdbcon001-{}-{}",
        harness.namespace(),
        uuid::Uuid::new_v4()
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE RACE. DELIVERIES concurrent copies of ONE command id, all inside the racy region together.
    //    Exactly one may be told it won; the rest must be refused by the conflict clause, not error.
    // -----------------------------------------------------------------------------------------------------------
    let command_id = format!("{marker}-cmd");
    let racing_id = command_id.clone();
    let answers = harness
        .race(DELIVERIES, move |_index, harness| {
            let command_id = racing_id.clone();
            async move { deliver(harness, command_id).await }
        })
        .await;

    let winners = answers.iter().filter(|won| **won).count();
    assert_eq!(
        winners, 1,
        "{HARNESS}: exactly one of {DELIVERIES} concurrent deliveries of {command_id} wins the claim, got {winners}"
    );
    // Committed truth, not the return value: the table holds one receipt for one command.
    assert_eq!(
        harness
            .receipt_count(&command_id)
            .await
            .expect("the receipt count reads"),
        1,
        "{HARNESS}: {DELIVERIES} concurrent deliveries of one command commit exactly one receipt"
    );

    // The one winner left a `pending` receipt — the claim is durable, so a crash after this point is recoverable
    // by finalizing rather than re-claiming.
    let receipt = harness
        .receipts()
        .find(&command_id)
        .await
        .expect("the receipt reads")
        .expect("the winning claim committed a receipt");
    assert_eq!(
        receipt.outcome, "pending",
        "{HARNESS}: the winning claim leaves the receipt pending until it is finalized"
    );
    assert_eq!(
        receipt.command_id, command_id,
        "{HARNESS}: the receipt is keyed by the command id that was delivered"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. NEGATIVE / REFUSAL — FINALITY. Once the outcome is recorded the command is not claimable at all: a late
    //    duplicate finds the row, `on conflict do nothing` absorbs it, and nothing is re-opened as `pending`.
    //    This is the case that makes a retry safe: without it, a redelivered webhook would re-execute a command
    //    whose handler already finished.
    // -----------------------------------------------------------------------------------------------------------
    let mut tx = harness
        .database_handle()
        .begin("test-harness.race.duplicate_command.finalize")
        .await
        .expect("the finalize transaction opens");
    harness
        .receipts()
        .finalize_tx(
            &mut tx,
            &command_id,
            "ok",
            None,
            Some("the duplicate-command proof"),
            None,
            None,
            None,
        )
        .await
        .expect("finalize_tx records the outcome");
    tx.commit().await.expect("finalize commits");

    // Every late duplicate is refused, and refused *concurrently* — the race is the delivery, not the first call.
    let late_id = command_id.clone();
    let late = harness
        .race(4, move |_index, harness| {
            let late_id = late_id.clone();
            async move {
                let mut tx = harness
                    .database_handle()
                    .begin("test-harness.race.duplicate_command.late")
                    .await
                    .expect("a late duplicate opens a transaction");
                let won = harness
                    .receipts()
                    .claim_tx(
                        &mut tx,
                        &late_id,
                        KIND,
                        FINGERPRINT,
                        None,
                        AGGREGATE,
                        None,
                        None,
                        None,
                        REQUESTED_AT,
                    )
                    .await
                    .expect("a late duplicate answers rather than erroring");
                tx.commit().await.expect("a late duplicate commits");
                won
            }
        })
        .await;
    assert!(
        late.iter().all(|won| !*won),
        "{HARNESS}: a finalized command refuses every late duplicate, got {late:?}"
    );
    assert_eq!(
        harness
            .receipt_count(&command_id)
            .await
            .expect("the receipt count reads after the late duplicates"),
        1,
        "{HARNESS}: late duplicates neither duplicate the receipt nor re-open it"
    );
    let still_ok = harness
        .receipts()
        .find(&command_id)
        .await
        .expect("the receipt reads after the late duplicates")
        .expect("the receipt is still there");
    assert_eq!(
        still_ok.outcome, "ok",
        "{HARNESS}: a late duplicate cannot rewrite the recorded outcome, got {}",
        still_ok.outcome
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE / FAULT — CRASH BEFORE COMMIT. A claimant that dies inside its transaction leaves NO row, so
    //    the command is not stranded: a later delivery claims it cleanly. This is the asymmetry that makes the
    //    rule safe in both directions — a committed claim is final, an uncommitted one never happened.
    // -----------------------------------------------------------------------------------------------------------
    let crashed_id = format!("{marker}-crashed");
    {
        let mut tx = harness
            .database_handle()
            .begin("test-harness.race.duplicate_command.crash")
            .await
            .expect("the crashing claim opens a transaction");
        assert!(
            harness
                .receipts()
                .claim_tx(
                    &mut tx,
                    &crashed_id,
                    KIND,
                    FINGERPRINT,
                    None,
                    AGGREGATE,
                    None,
                    None,
                    None,
                    REQUESTED_AT,
                )
                .await
                .expect("the first claim of a crashed command answers"),
            "{HARNESS}: the first delivery wins inside its own transaction"
        );
        // The process dies here. `tx` is dropped without a commit, so sqlx rolls it back.
    }
    assert_eq!(
        harness
            .receipt_count(&crashed_id)
            .await
            .expect("the receipt count reads after the crash"),
        0,
        "{HARNESS}: a claim that never committed leaves no receipt, so the command is not stranded"
    );

    // The retry therefore wins, exactly as the first delivery would have.
    let retry_id = crashed_id.clone();
    let retry = harness
        .race(2, move |_index, harness| {
            let retry_id = retry_id.clone();
            async move {
                let mut tx = harness
                    .database_handle()
                    .begin("test-harness.race.duplicate_command.retry")
                    .await
                    .expect("the retry opens a transaction");
                let won = harness
                    .receipts()
                    .claim_tx(
                        &mut tx,
                        &retry_id,
                        KIND,
                        FINGERPRINT,
                        None,
                        AGGREGATE,
                        None,
                        None,
                        None,
                        REQUESTED_AT,
                    )
                    .await
                    .expect("the retry answers");
                tx.commit().await.expect("the retry commits");
                won
            }
        })
        .await;
    assert_eq!(
        retry.iter().filter(|won| **won).count(),
        1,
        "{HARNESS}: the retry after an uncommitted claim converges on one winner, got {retry:?}"
    );
    assert_eq!(
        harness
            .receipt_count(&crashed_id)
            .await
            .expect("the receipt count reads after the retry"),
        1,
        "{HARNESS}: a retried command still commits exactly one receipt"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. CLEANUP / NO LEFTOVER. Every row this run seeded is removed; a non-zero leftover fails the proof,
    //    because a stray `pending` receipt is a command that looks claimed forever.
    // -----------------------------------------------------------------------------------------------------------
    harness
        .cleanup(&marker)
        .await
        .expect("the fixture rows are removed");
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("the leftover count reads"),
        0,
        "{HARNESS}: the proof leaves no receipt, run, item, task or story behind"
    );
}
