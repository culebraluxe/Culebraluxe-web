//! DB.SCHEMA — FK targets exist (TST-DB-SCHEMA-005).
//!
//! Contract: every foreign key in the production schema points at a target that exists.
//! The test reads the schema through the production reader
//! [`db::schema_parity::read_snapshot`] — the same catalogue reader the `db:parity`
//! release gate uses — and asserts, against an isolated disposable DEV target:
//!
//! 1. the schema declares foreign keys at all (otherwise the test is vacuous);
//! 2. every FK's parent table is present in the live table set — no dangling target;
//! 3. the canonical property/media core declares both of its FK targets
//!    (`property_media -> property`, `property_media -> media`);
//! 4. the committed relation rows are referentially clean — no orphaned
//!    `property_media` row points at a property or media row that is absent;
//! 5. the database itself refuses a reference to a target that does not exist (a
//!    rolled-back orphan insert), so the invariant cannot be bypassed.
//!
//! The negative control is two-fold: a fabricated table reads absent and a fabricated
//! foreign key reads absent, so a reader that answered "present" for everything could
//! not pass. The orphan insert is rolled back, so no disposable row survives.
//!
//! Level: L2 Persistence — the production snapshot reader plus one rolled-back write
//! against an isolated disposable DEV/Neon target. The harness refuses PRODUCTION
//! before any socket is opened, and the only writes are inside a rollback-only
//! transaction.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_schema__005__fk_targets_exist -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof
//! needs a disposable DEV database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use test_harness::database::TestDatabase;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "DatabaseHarness/L2 Persistence";

/// The FK target relationships the canonical property/media core relies upon:
/// (child table, parent table). A missing pair means a target the domain needs is
/// not declared. `property` is the canonical listing record and `media` the reusable
/// asset, both related through `property_media` (project handbook).
const CANONICAL_FK_TARGETS: &[(&str, &str)] = &[
    ("property_media", "property"),
    ("property_media", "media"),
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

/// `regclass::text` may qualify a relation (`public.property_media`); the snapshot's table
/// set is unqualified, so compare on the bare relation name.
fn bare_relation(relation: &str) -> &str {
    relation
        .rsplit_once('.')
        .map(|(_, bare)| bare)
        .unwrap_or(relation)
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); the harness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-SCHEMA-005); the file and the assay use it.
async fn db_schema_005__fk_targets_exist() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let dev = connect_dev().await;
    assert_eq!(
        dev.target(),
        DbTarget::Dev,
        "{HARNESS}: the FK-target proof runs only on an isolated DEV target"
    );

    // 1. The production snapshot reader — the same reader the parity gate uses.
    let snapshot = db::schema_parity::read_snapshot(dev.database())
        .await
        .expect("the production snapshot reader reads DEV");

    // 2. The subject is non-empty: a schema that declares no foreign keys cannot
    //    demonstrate that FK targets exist.
    assert!(
        !snapshot.fks.is_empty(),
        "{HARNESS}: the schema declares foreign keys — otherwise this test is vacuous"
    );

    // 3. Negative control: fabricated names read absent, so a reader that answered
    //    "present" for everything could not pass this test.
    assert!(
        !snapshot
            .tables
            .iter()
            .any(|table| table == "no_such_table_tst_schema_005"),
        "{HARNESS}: a fabricated table reads absent — the reader discriminates"
    );
    assert!(
        !snapshot.fks.contains_key("no_such_fk_tst_schema_005"),
        "{HARNESS}: a fabricated foreign key reads absent — the reader discriminates"
    );

    // 4. Every FK's parent table exists. (`read_snapshot` stores each FK as
    //    `"child -> parent"`, so a dangling target is visible here.)
    let mut dangling: Vec<String> = Vec::new();
    let mut pairs: Vec<(String, String)> = Vec::new();
    for (constraint, value) in &snapshot.fks {
        let Some((child, parent)) = value.split_once(" -> ") else {
            dangling.push(format!("{constraint}: unreadable FK value {value:?}"));
            continue;
        };
        let child = bare_relation(child);
        let parent = bare_relation(parent);
        pairs.push((child.to_owned(), parent.to_owned()));
        if !snapshot.tables.iter().any(|table| table == parent) {
            dangling.push(format!(
                "{constraint}: {child} -> {parent} targets a table that is absent"
            ));
        }
    }
    assert!(
        dangling.is_empty(),
        "{HARNESS}: foreign keys target tables that do not exist:\n  {}",
        dangling.join("\n  ")
    );

    // 5. The canonical property/media FK targets are declared.
    let mut missing: Vec<String> = Vec::new();
    for (child, parent) in CANONICAL_FK_TARGETS {
        if !pairs
            .iter()
            .any(|(actual_child, actual_parent)| actual_child == child && actual_parent == parent)
        {
            missing.push(format!("{child} -> {parent}"));
        }
    }
    assert!(
        missing.is_empty(),
        "{HARNESS}: canonical FK targets not declared on DEV:\n  {}",
        missing.join("\n  ")
    );

    // 6. Committed relation rows are referentially clean: the FK targets every child row
    //    needs actually exist. A healthy schema returns zero; a non-zero count is a
    //    violation of "FK targets exist". Read through the production pool.
    let mut conn = dev
        .database()
        .pool()
        .acquire()
        .await
        .expect("pool checkout");
    let orphaned: (i64,) = sqlx::query_as(
        "select count(*) from property_media pm \
         where not exists (select 1 from property p where p.id = pm.property_id) \
            or not exists (select 1 from media m where m.id = pm.media_id)",
    )
    .fetch_one(&mut *conn)
    .await
    .expect("count orphaned property_media rows");
    assert_eq!(
        orphaned.0, 0,
        "{HARNESS}: every property_media row must target an existing property and media"
    );

    // 7. Enforcement: a reference to a target that does not exist is refused. Both ids
    //    are freshly generated and therefore absent, so the property/media FK must
    //    reject the row. The transaction is rolled back no matter what, so no
    //    disposable row survives.
    let mut tx = dev.begin().await.expect("begin rollback transaction");
    let orphan_insert = sqlx::query(
        "insert into property_media (property_id, media_id) values (gen_random_uuid(), gen_random_uuid())",
    )
    .execute(&mut *tx.connection())
    .await;
    let _ = tx.rollback().await;

    let error = orphan_insert.expect_err(
        "the database must refuse a property_media row whose property/media targets do not exist",
    );
    let sqlstate = error.as_database_error().and_then(|db_error| db_error.code());
    assert_eq!(
        sqlstate.as_deref(),
        Some("23503"),
        "{HARNESS}: the refusal must be a foreign-key violation (SQLSTATE 23503): {error}"
    );
}
