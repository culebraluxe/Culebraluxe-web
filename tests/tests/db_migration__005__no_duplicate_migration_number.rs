//! DB.MIGRATION — no duplicate migration number (TST-DB-MIGRATION-005).
//!
//! Contract: the numeric prefix of a migration filename is its position in the chain, and a
//! position holds ONE file. Production enumerates by sorted filename and applies in that
//! order; two files sharing a number make the order between them filesystem luck, and the
//! ledger records by file NAME while promotion reason about NUMBERS — a duplicate is how
//! one half of a number gets applied while the other half is believed done.
//!
//! The check is filesystem truth, read live: every `NNN_*.sql` name in `db/migrations/`
//! contributes its number, and each number must appear exactly once. The negative control
//! is the detector itself — it is run over a synthetic listing containing a duplicate and
//! must report it, so a detector that never fired could not pass this test.
//!
//! Level: L2 Persistence — the taxonomy fixes the harness; the DEV connection below proves
//! the target the suite runs under, while the numbers are read from the repo production
//! applies from. The harness refuses PRODUCTION before any socket is opened.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_migration__005__no_duplicate_migration_number -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use std::collections::BTreeMap;
use test_harness::database::TestDatabase;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "DatabaseHarness/L2 Persistence";

/// The chain position of one migration filename: the numeric prefix before the first `_`.
fn migration_number(file: &str) -> Option<u32> {
    file.split('_').next()?.parse().ok()
}

/// Every number claimed more than once, with the files claiming it.
fn duplicate_numbers(files: &[String]) -> BTreeMap<u32, Vec<String>> {
    let mut by_number: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for file in files {
        if let Some(number) = migration_number(file) {
            by_number.entry(number).or_default().push(file.clone());
        }
    }
    by_number.retain(|_, claimants| claimants.len() > 1);
    by_number
}

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-MIGRATION-005); the file and the assay use it.
async fn db_migration_005__no_duplicate_migration_number() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let dev = connect_dev().await;
    assert_eq!(
        dev.target(),
        DbTarget::Dev,
        "{HARNESS}: the numbering proof runs only on an isolated DEV target"
    );

    // 1. The negative control: the detector fires on a synthetic duplicate, so a clean
    //    result below means clean numbers, not a blind detector.
    let synthetic = vec![
        "201_alpha.sql".to_string(),
        "202_beta.sql".to_string(),
        "202_gamma.sql".to_string(),
    ];
    let synthetic_dupes = duplicate_numbers(&synthetic);
    assert_eq!(
        synthetic_dupes.len(),
        1,
        "{HARNESS}: the detector reports a planted duplicate"
    );
    assert_eq!(
        synthetic_dupes.get(&202).cloned().unwrap_or_default(),
        vec!["202_beta.sql".to_string(), "202_gamma.sql".to_string()],
        "{HARNESS}: the detector names both claimants of the planted number"
    );

    // 2. The repo list, enumerated the way production enumerates it.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let migrations_dir = std::path::Path::new(manifest_dir).join("../db/migrations");
    let mut files: Vec<String> = std::fs::read_dir(&migrations_dir)
        .expect("the migrations directory reads")
        .filter_map(|entry| {
            let name = entry.expect("a directory entry reads").file_name();
            let name = name.to_str().expect("a utf-8 filename").to_string();
            name.ends_with(".sql").then_some(name)
        })
        .collect();
    files.sort();
    assert!(
        files.len() >= 200,
        "{HARNESS}: the chain under test is the whole estate, found {} files",
        files.len()
    );
    assert!(
        files.iter().all(|file| migration_number(file).is_some()),
        "{HARNESS}: every migration file carries a numeric prefix"
    );

    // 3. One number, one file.
    let duplicates = duplicate_numbers(&files);
    let report: Vec<String> = duplicates
        .iter()
        .map(|(number, claimants)| format!("{number:03}: {}", claimants.join(", ")))
        .collect();
    assert!(
        duplicates.is_empty(),
        "{HARNESS}: duplicate migration numbers — one position, two files:\n  {}",
        report.join("\n  ")
    );
}
