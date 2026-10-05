//! DB.SCHEMA — nullable/non-null contract matches Rust (TST-DB-SCHEMA-003).
//!
//! CONTRACT. The nullability the Rust read boundary declares for a column must be the
//! nullability the database enforces. The production read path for media is
//! `MediaDao::for_property` (`db/src/media/media_row.rs`), which selects into the public
//! [`model::MediaAsset`] boundary; that struct is the Rust source of truth: an `Option<T>`
//! field means the column may be absent, a non-optional field means it may not. A
//! non-optional Rust field over a nullable column is a latent decode failure (a `NULL` row
//! breaks the boundary); an `Option<T>` over a `NOT NULL` column is a safe superset and is
//! deliberately excluded from the pinned set below so the contract stays 1:1.
//!
//! The Rust half of every pair is bound at compile time: `contract(...)` takes the real
//! production field by reference, so a change to `MediaAsset`'s field type changes this
//! test's expectation or fails to compile. The database half is read live through the
//! production [`db::schema_parity::read_snapshot`] reader — the same reader the `db:parity`
//! release gate uses — so a passing test means the gate's own reader sees the constraint.
//!
//! TWO GUARDS against a test that passes without exercising the subject:
//!
//! 1. A discriminator: a fabricated column reads absent, and a deliberately wrong
//!    declaration (`sort_order` as `Optional`) must NOT hold against the catalogue, so a
//!    comparison that answered "matches" for everything could not pass.
//! 2. A fault case: against the isolated DEV target, a `NOT NULL` column rejects an explicit
//!    `NULL` and a nullable column accepts one and reads it back, inside transactions the
//!    harness only rolls back — so catalogue metadata is checked against enforced behaviour.
//!
//! Level: L2 Persistence — the production snapshot reader against an isolated disposable
//! DEV/Neon target. [`test_harness::database::guard_target`] refuses PRODUCTION before any
//! socket is opened.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_schema__003__nullable_non_null_contract_matches_rust -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use db::schema_parity::{read_snapshot, SchemaSnapshot};
use db::DbTarget;
use model::MediaAsset;
use test_harness::database::TestDatabase;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "DatabaseHarness/L2 Persistence";

/// Whether Rust models a column boundary as absent-able or required.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Nullability {
    /// `Option<T>` in Rust: the database must allow `NULL`.
    Optional,
    /// A non-optional production type in Rust: the database must be `NOT NULL`.
    Required,
}

/// The Rust type at a column boundary declares its own nullability.
///
/// This is the compile-time half of the contract. `Option<T>` is optional by construction;
/// every concrete production column type is required. Adding a new wrapper here (rather than
/// spelling "required" for a type production spells `Option<T>`) is what keeps the probe
/// honest.
trait RustNullability {
    const NULLABILITY: Nullability;
}

impl<T> RustNullability for Option<T> {
    const NULLABILITY: Nullability = Nullability::Optional;
}

impl RustNullability for String {
    const NULLABILITY: Nullability = Nullability::Required;
}

impl RustNullability for i32 {
    const NULLABILITY: Nullability = Nullability::Required;
}

impl RustNullability for i64 {
    const NULLABILITY: Nullability = Nullability::Required;
}

impl RustNullability for bool {
    const NULLABILITY: Nullability = Nullability::Required;
}

/// One pinned pair: a database column and the production Rust field that reads it.
#[derive(Debug, Clone, Copy)]
struct ColumnContract {
    table: &'static str,
    column: &'static str,
    rust: Nullability,
}

/// Bind a contract entry to the real production field.
///
/// Taking `_field: &T` is the point: it proves the field's type at compile time and reads
/// its nullability from the same type, so the Rust half cannot drift from the declaration.
fn contract<T: RustNullability>(
    table: &'static str,
    column: &'static str,
    _field: &T,
) -> ColumnContract {
    ColumnContract {
        table,
        column,
        rust: T::NULLABILITY,
    }
}

/// The pinned contract: the columns `MediaDao::for_property` reads into `MediaAsset`, where
/// the Rust field shape is 1:1 with the catalogue (an `Option<T>` field over a `NOT NULL`
/// column — a safe superset — is intentionally left out, e.g. `filename`, `created_at`).
fn media_asset_contract(sample: &MediaAsset) -> Vec<ColumnContract> {
    vec![
        contract("media", "id", &sample.id),
        contract("property_media", "property_id", &sample.property_id),
        contract("media", "media_type", &sample.media_type),
        contract("property_media", "role", &sample.role),
        contract("property_media", "sort_order", &sample.sort_order),
        contract("media", "alt_text", &sample.alt_text),
        contract("media", "caption", &sample.caption),
        contract("media", "file_size", &sample.file_size),
        contract("media", "mux_asset_id", &sample.mux_asset_id),
        contract("media", "source_url", &sample.source_url),
    ]
}

/// A production `MediaAsset` value, so every contract entry binds a real field type.
fn media_asset_sample() -> MediaAsset {
    MediaAsset {
        id: String::new(),
        property_id: String::new(),
        media_type: String::new(),
        role: String::new(),
        sort_order: 0,
        filename: None,
        mime_type: None,
        file_size: None,
        alt_text: None,
        caption: None,
        created_at: None,
        mux_asset_id: None,
        mux_playback_id: None,
        duration_seconds: None,
        aspect_ratio: None,
        source_url: None,
        url: String::new(),
    }
}

/// The catalogue's nullability for a column, or `None` when it does not exist.
///
/// The snapshot value is `"data_type[ NOT NULL]"` (`db::schema_parity::read_snapshot`), so
/// the `NOT NULL` suffix is the nullability the database enforces.
fn catalogue_allows_null(snapshot: &SchemaSnapshot, table: &str, column: &str) -> Option<bool> {
    snapshot
        .columns
        .get(table)
        .and_then(|columns| columns.get(column))
        .map(|value| !value.ends_with(" NOT NULL"))
}

/// Whether the catalogue answers the Rust declaration.
fn contract_holds(rust: Nullability, catalogue_allows_null: bool) -> bool {
    match rust {
        Nullability::Optional => catalogue_allows_null,
        Nullability::Required => !catalogue_allows_null,
    }
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-SCHEMA-003); the file and the assay use it.
async fn db_schema_003__nullable_non_null_contract_matches_rust() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let dev = connect_dev().await;
    assert_eq!(
        dev.target(),
        DbTarget::Dev,
        "{HARNESS}: the nullability proof runs only on an isolated DEV target"
    );

    // 1. The production snapshot reader — the same reader the parity gate uses.
    let snapshot = read_snapshot(dev.database())
        .await
        .expect("the production snapshot reader reads DEV");

    // 2. The discriminator: a fabricated column reads absent, and a deliberately wrong
    //    declaration must not hold — so a comparison that answered "matches" for everything
    //    could not pass this test.
    assert!(
        catalogue_allows_null(&snapshot, "media", "no_such_column_tst_db_schema_003").is_none(),
        "{HARNESS}: a fabricated column reads absent — the reader discriminates"
    );
    let sort_order_allows_null = catalogue_allows_null(&snapshot, "property_media", "sort_order")
        .expect("property_media.sort_order exists — TST-DB-SCHEMA-001 pins existence");
    assert!(
        !contract_holds(Nullability::Optional, sort_order_allows_null),
        "{HARNESS}: declaring sort_order Optional must not hold — the comparison discriminates"
    );
    assert!(
        contract_holds(Nullability::Required, sort_order_allows_null),
        "{HARNESS}: sort_order is required in Rust and NOT NULL in the catalogue"
    );

    // 3. Every pinned column's database nullability matches the Rust field that reads it.
    let sample = media_asset_sample();
    let contracts = media_asset_contract(&sample);
    let mut missing: Vec<String> = Vec::new();
    let mut mismatched: Vec<String> = Vec::new();
    for entry in &contracts {
        match catalogue_allows_null(&snapshot, entry.table, entry.column) {
            None => missing.push(format!("{}.{}", entry.table, entry.column)),
            Some(allows_null) if contract_holds(entry.rust, allows_null) => {}
            Some(allows_null) => mismatched.push(format!(
                "{}.{}: Rust {:?}, catalogue allows NULL={}",
                entry.table, entry.column, entry.rust, allows_null
            )),
        }
    }
    assert!(
        missing.is_empty(),
        "{HARNESS}: pinned columns absent on DEV:\n  {}",
        missing.join("\n  ")
    );
    assert!(
        mismatched.is_empty(),
        "{HARNESS}: nullable/non-null drift on DEV:\n  {}",
        mismatched.join("\n  ")
    );

    // 4. The fault case: catalogue metadata must equal enforced behaviour. Both probes run
    //    inside transactions the harness only rolls back, so no row survives the test.
    //
    //    A Required column (`media.media_type`, bound above) rejects an explicit NULL.
    let required_rejected = dev
        .with_rollback(|conn| {
            Box::pin(async move {
                let attempt = sqlx::query(
                    "insert into media (filename, mime_type, media_type) \
                     values ('tst-003-required', 'text/plain', null)",
                )
                .execute(&mut *conn)
                .await;
                Ok(attempt.is_err())
            })
        })
        .await
        .expect("the NOT NULL probe runs on DEV and rolls back");
    assert!(
        required_rejected,
        "{HARNESS}: a NULL into the required media.media_type column must be refused"
    );

    //    A nullable column (`media.alt_text`, bound above) accepts one, and it reads back as NULL.
    let optional_stored = dev
        .with_rollback(|conn| {
            Box::pin(async move {
                let inserted: Option<String> = sqlx::query_scalar(
                    "insert into media (filename, mime_type, alt_text) \
                     values ('tst-003-optional', 'text/plain', null) returning id::text",
                )
                .fetch_optional(&mut *conn)
                .await
                .ok()
                .flatten();
                let Some(id) = inserted else {
                    return Ok(None);
                };
                let stored_is_null: Option<bool> =
                    sqlx::query_scalar("select alt_text is null from media where id = $1::uuid")
                        .bind(&id)
                        .fetch_optional(&mut *conn)
                        .await
                        .ok()
                        .flatten();
                Ok(stored_is_null)
            })
        })
        .await
        .expect("the nullable probe runs on DEV and rolls back");
    assert_eq!(
        optional_stored,
        Some(true),
        "{HARNESS}: a NULL into the nullable media.alt_text column must be accepted and read back as NULL"
    );
}
