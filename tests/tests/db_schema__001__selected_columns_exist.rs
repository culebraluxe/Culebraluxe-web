//! DB.SCHEMA — selected columns exist (TST-DB-SCHEMA-001).
//!
//! Contract: the canonical records name their columns, and the database holds them. The
//! selection below is the contract this test pins — the identity and media core from the
//! project handbook (`property` is the canonical listing record, `media` the reusable
//! asset, `property_media` the role/order relation) plus the workflow control plane the
//! migration suite reasons about (`storyboard_story`, `schema_migration`):
//!
//! - `property`: id, slug, status, created_at
//! - `media`: id, media_type, created_at
//! - `property_media`: property_id, media_id, role, sort_order
//! - `storyboard_story`: id, status
//! - `schema_migration`: filename, checksum
//!
//! Read through the production [`db::schema_parity::read_snapshot`] — the same reader the
//! `db:parity` release gate uses — so a passing test means the gate's own reader sees the
//! columns. The negative control asserts a fabricated column is reported absent, so a
//! reader that answered "present" for everything could not pass.
//!
//! Level: L2 Persistence — the production snapshot reader against an isolated disposable
//! DEV/Neon target. The harness refuses PRODUCTION before any socket is opened.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_schema__001__selected_columns_exist -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use test_harness::database::TestDatabase;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "DatabaseHarness/L2 Persistence";

/// The selected columns: (table, column) pairs the canonical records promise.
const SELECTED_COLUMNS: &[(&str, &str)] = &[
    ("property", "id"),
    ("property", "slug"),
    ("property", "status"),
    ("property", "created_at"),
    ("media", "id"),
    ("media", "media_type"),
    ("media", "created_at"),
    ("property_media", "property_id"),
    ("property_media", "media_id"),
    ("property_media", "role"),
    ("property_media", "sort_order"),
    ("storyboard_story", "id"),
    ("storyboard_story", "status"),
    ("schema_migration", "filename"),
    ("schema_migration", "checksum"),
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-SCHEMA-001); the file and the assay use it.
async fn db_schema_001__selected_columns_exist() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let dev = connect_dev().await;
    assert_eq!(
        dev.target(),
        DbTarget::Dev,
        "{HARNESS}: the column-existence proof runs only on an isolated DEV target"
    );

    // 1. The production snapshot reader — the same reader the parity gate uses.
    let snapshot = db::schema_parity::read_snapshot(dev.database())
        .await
        .expect("the production snapshot reader reads DEV");

    // 2. The negative control: a fabricated column is absent, so a reader that answered
    //    "present" for everything could not pass this test.
    let fabricated = snapshot
        .columns
        .get("property")
        .and_then(|columns| columns.get("no_such_column_tst_schema_001"));
    assert!(
        fabricated.is_none(),
        "{HARNESS}: a fabricated column reads absent — the reader discriminates"
    );

    // 3. Every selected column exists.
    let mut missing: Vec<String> = Vec::new();
    for (table, column) in SELECTED_COLUMNS {
        let present = snapshot
            .columns
            .get(*table)
            .and_then(|columns| columns.get(*column))
            .is_some();
        if !present {
            missing.push(format!("{table}.{column}"));
        }
    }
    assert!(
        missing.is_empty(),
        "{HARNESS}: selected columns missing on DEV:\n  {}",
        missing.join("\n  ")
    );
}
