//! DB.SCHEMA — expected type matches schema (TST-DB-SCHEMA-002).
//!
//! Contract: the selected columns (the TST-DB-SCHEMA-001 selection) carry the types the
//! domain relies on — uuid identity keys, text descriptors, timestamptz audit columns,
//! the integer sort order. A column that exists with the wrong type is the drift this
//! test forbids: the repository-boundary normalization (`timestamps to ISO strings,
//! counts/numerics to safe types`) is written against these catalogue types, so a silent
//! change here breaks the boundary above it.
//!
//! Compared values use the production snapshot format (`"data_type[ NOT NULL]"`, exactly
//! as [`db::schema_parity::read_snapshot`] builds it for the `db:parity` gate), read live
//! from DEV — no catalogue values are baked in. The negative control asserts a deliberately
//! wrong expectation mismatches, so a comparison that answered "match" for everything
//! could not pass.
//!
//! Level: L2 Persistence — the production snapshot reader against an isolated disposable
//! DEV/Neon target. The harness refuses PRODUCTION before any socket is opened.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_schema__002__expected_type_matches_schema -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use test_harness::database::TestDatabase;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "DatabaseHarness/L2 Persistence";

/// The expected types: (table, column, snapshot value) triples the domain relies on.
const EXPECTED_TYPES: &[(&str, &str, &str)] = &[
    ("property", "id", "uuid NOT NULL"),
    ("property", "slug", "text"),
    ("property", "status", "text NOT NULL"),
    (
        "property",
        "created_at",
        "timestamp with time zone NOT NULL",
    ),
    ("media", "id", "uuid NOT NULL"),
    ("media", "media_type", "text NOT NULL"),
    ("media", "created_at", "timestamp with time zone NOT NULL"),
    ("property_media", "property_id", "uuid NOT NULL"),
    ("property_media", "media_id", "uuid NOT NULL"),
    ("property_media", "role", "text NOT NULL"),
    ("property_media", "sort_order", "integer NOT NULL"),
    ("storyboard_story", "id", "text NOT NULL"),
    ("storyboard_story", "status", "text NOT NULL"),
    ("schema_migration", "filename", "text NOT NULL"),
    ("schema_migration", "checksum", "text NOT NULL"),
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-SCHEMA-002); the file and the assay use it.
async fn db_schema_002__expected_type_matches_schema() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let dev = connect_dev().await;
    assert_eq!(
        dev.target(),
        DbTarget::Dev,
        "{HARNESS}: the type-match proof runs only on an isolated DEV target"
    );

    // 1. The production snapshot reader — the same reader the parity gate uses.
    let snapshot = db::schema_parity::read_snapshot(dev.database())
        .await
        .expect("the production snapshot reader reads DEV");

    // 2. The negative control: a deliberately wrong expectation mismatches, so a
    //    comparison that answered "match" for everything could not pass this test.
    let actual_id = snapshot
        .columns
        .get("property")
        .and_then(|columns| columns.get("id"))
        .expect("property.id exists — TST-DB-SCHEMA-001 pins existence");
    assert_ne!(
        actual_id, "text",
        "{HARNESS}: property.id is not text — the comparison discriminates"
    );

    // 3. Every expected type matches the catalogue exactly.
    let mut mismatched: Vec<String> = Vec::new();
    for (table, column, expected) in EXPECTED_TYPES {
        match snapshot
            .columns
            .get(*table)
            .and_then(|columns| columns.get(*column))
        {
            Some(actual) if actual == expected => {}
            Some(actual) => mismatched.push(format!(
                "{table}.{column}: expected {expected}, catalogue holds {actual}"
            )),
            None => mismatched.push(format!(
                "{table}.{column}: expected {expected}, column is absent"
            )),
        }
    }
    assert!(
        mismatched.is_empty(),
        "{HARNESS}: type drift on DEV:\n  {}",
        mismatched.join("\n  ")
    );
}
