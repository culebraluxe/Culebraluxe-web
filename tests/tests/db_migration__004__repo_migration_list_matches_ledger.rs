//! DB.MIGRATION — repo migration list matches ledger (TST-DB-MIGRATION-004).
//!
//! Contract: the `schema_migration` ledger answers "what is applied where", and it can only
//! do that while the repo list and the ledger agree. The test checks the agreement from the
//! side the story's boundary permits — the disposable DEV target, through the production
//! [`db::SchemaMigrationDao`] — in both directions:
//!
//! - ledger → disk: every DEV-target row naming a `db/migrations/` file resolves to a file
//!   on disk. A stale row (a file renamed without its row) is a mismatch the next reader
//!   trips over.
//! - disk → ledger, for files PROVEN applied on DEV: five post-baseline files whose effects
//!   exist on DEV right now (verified live while authoring) must hold DEV ledger rows. An
//!   applied-but-unrecorded migration is exactly the drift the ledger exists to prevent —
//!   it is how PROD once went weeks without 116–122/138.
//!
//! What this test deliberately does NOT assert: presence on the DEV ledger of files that
//! are legitimately recorded for PROD only. PROD is unreadable from this boundary, and a
//! prod-side recording is a match, not a gap — asserting DEV-presence for those would
//! manufacture failure. The pre-baseline files (numbers below 145) are likewise exempt by
//! design: the ledger is authoritative only from the 2026-09-10 baseline forward.
//!
//! Level: L2 Persistence — the production DAO against an isolated disposable DEV/Neon target.
//! The harness refuses PRODUCTION before any socket is opened.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_migration__004__repo_migration_list_matches_ledger -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use db::{DbTarget, SchemaMigrationDao};
use std::collections::BTreeSet;
use test_harness::database::TestDatabase;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "DatabaseHarness/L2 Persistence";

/// The ledger's join key, exactly as production computes it (`db-tool` matches by file NAME:
/// the migrations folder moved twice, so paths are history, not identity).
fn ledger_key(file: &str) -> String {
    file.rsplit('/').next().unwrap_or(file).to_owned()
}

/// Post-baseline files whose effects were verified present on DEV while authoring, with the
/// live effect that proves them applied: (file, effect table or object).
///
/// If DEV is ever refreshed from a state where an effect is absent, that pair's effect
/// assertion names the changed world instead of silently flipping the verdict.
const APPLIED_SPOT_CHECKS: &[(&str, &str)] = &[
    ("224_person_civil_status.sql", "person.civil_status"),
    (
        "225_property_golden_from_regrid.sql",
        "property_regrid_snapshot_225",
    ),
    ("226_wbs_planned_dates.sql", "wbs_item.planned_start"),
    ("227_wbs_dependency.sql", "wbs_dependency"),
    ("228_person_merge.sql", "merge_person()"),
];

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
///
/// Infrastructure, not the contract: `TestDatabase` still refuses PRODUCTION before any socket is opened.
async fn connect_dev() -> TestDatabase {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match TestDatabase::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; the harness refuses PROD: {}",
        last.unwrap_or_default()
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); the harness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-MIGRATION-004); the file and the assay use it.
async fn db_migration_004__repo_migration_list_matches_ledger() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let dev = connect_dev().await;
    assert_eq!(
        dev.target(),
        DbTarget::Dev,
        "{HARNESS}: the ledger-match proof runs only on an isolated DEV target"
    );
    let ledger = SchemaMigrationDao::new(dev.database().clone());
    assert!(
        ledger.present().await.expect("the ledger presence reads"),
        "{HARNESS}: the schema_migration ledger exists on DEV — a missing ledger is itself a mismatch"
    );

    // 1. The repo list, enumerated the way production enumerates it (sorted `.sql` basenames).
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let migrations_dir = std::path::Path::new(manifest_dir).join("../db/migrations");
    let disk: BTreeSet<String> = std::fs::read_dir(&migrations_dir)
        .expect("the migrations directory reads")
        .filter_map(|entry| {
            let name = entry.expect("a directory entry reads").file_name();
            let name = name.to_str().expect("a utf-8 filename").to_string();
            name.ends_with(".sql").then_some(name)
        })
        .collect();
    assert!(
        disk.len() >= 200,
        "{HARNESS}: the repo list under test is the whole estate, found {} files",
        disk.len()
    );

    // 2. Direction ledger → disk: every DEV-target row for a migrations file resolves.
    //    The negative control is built in: a row for a file that no longer exists must
    //    surface here, because the next `recorded_checksum` reader would otherwise trust
    //    history over the repo.
    let rows = ledger
        .rows(DbTarget::Dev)
        .await
        .expect("the DEV ledger rows read");
    let mut orphans: Vec<String> = Vec::new();
    for row in &rows {
        let key = ledger_key(&row.filename);
        if key == "<baseline>" || !key.ends_with(".sql") {
            continue;
        }
        // Only migrations rows are in scope: loads, seeds and one-offs live under other
        // directories and are recorded history, not repo-list claims.
        let recorded_under_migrations = row.filename == key
            || row.filename.starts_with("db/migrations/")
            || row.filename.starts_with("legacy/db/migrations/")
            || row.filename.starts_with("../db/migrations/");
        if recorded_under_migrations && !disk.contains(&key) {
            orphans.push(format!("{} (target {})", row.filename, row.target));
        }
    }
    assert!(
        orphans.is_empty(),
        "{HARNESS}: DEV ledger rows with no repo file — renamed without their row, history over repo:\n  {}",
        orphans.join("\n  ")
    );

    // 3. Direction disk → ledger for files PROVEN applied on DEV: each spot-check effect
    //    is read live first (proving applied), then its DEV ledger row is required.
    let mut unrecorded: Vec<String> = Vec::new();
    for (file, effect) in APPLIED_SPOT_CHECKS {
        assert!(
            disk.contains(*file),
            "{HARNESS}: spot-check file {file} is on disk — the repo moved under the test"
        );
        let effect_present: bool = if let Some(function) = effect.strip_suffix("()") {
            let count: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_proc WHERE proname = $1")
                .bind(function)
                .fetch_one(dev.database().pool())
                .await
                .expect("the catalogue reads");
            count > 0
        } else if let Some((table, column)) = effect.split_once('.') {
            let count: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM information_schema.columns \
                 WHERE table_schema = 'public' AND table_name = $1 AND column_name = $2",
            )
            .bind(table)
            .bind(column)
            .fetch_one(dev.database().pool())
            .await
            .expect("the catalogue reads");
            count > 0
        } else {
            let present: Option<String> =
                sqlx::query_scalar("SELECT to_regclass('public.' || $1)::text")
                    .bind(*effect)
                    .fetch_one(dev.database().pool())
                    .await
                    .expect("the catalogue reads");
            present.is_some()
        };
        assert!(
            effect_present,
            "{HARNESS}: spot-check effect {effect} is absent on DEV — DEV state changed, re-verify the pair for {file}"
        );
        let recorded = ledger
            .recorded_checksum(file, DbTarget::Dev)
            .await
            .expect("the checksum reads");
        if recorded.is_none() {
            unrecorded.push((*file).to_string());
        }
    }
    assert!(
        unrecorded.is_empty(),
        "{HARNESS}: applied on DEV but recorded nowhere on DEV — the drift the ledger exists to prevent:\n  {}",
        unrecorded.join("\n  ")
    );
}
