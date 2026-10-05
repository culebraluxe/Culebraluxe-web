//! DB.SCHEMA — unique indexes that code relies upon exist (TST-DB-SCHEMA-006).
//!
//! CONTRACT. Every unique index the production Rust DAO relies on must exist in the
//! production schema. The reliance is concrete and mechanical: a production statement that
//! writes `ON CONFLICT (<columns>)` cannot run at all unless a unique index (or a unique
//! constraint, which is backed by one) exists on exactly those columns — Postgres refuses
//! the statement at plan time with SQLSTATE 42P10 (`no unique or exclusion constraint
//! matching the ON CONFLICT specification`). The list below is therefore not a wish list:
//! each row is the conflict target of a real upsert in `db/`, and each names the site that
//! depends on it.
//!
//! The test reads the live catalogue through the production database boundary and asserts
//! that a **unique** index whose key columns are exactly the conflict target exists for
//! every row. Matching by columns rather than by index name is deliberate: the contract is
//! "the conflict target is covered", not "a particular object name survived a rename".
//!
//! NEGATIVE CONTROL. A query that answered "covered" for any target could still pass the
//! positive loop, so two deliberate misses are asserted to resolve to nothing (a fabricated
//! table, and a real table with a fabricated column). A final fault case inserts a duplicate
//! into a relied-upon table inside a transaction and asserts the unique index **refuses** it
//! (SQLSTATE 23505), then rolls back — so "the index exists" is proven to mean "the index
//! enforces", and no probe row survives.
//!
//! Level: L2 Persistence — the production database against an isolated disposable DEV/Neon
//! target. The harness refuses PRODUCTION before any socket is opened.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_schema__006__unique_indexes_that_code_relies_upon_exist -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use test_harness::database::TestDatabase;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "DatabaseHarness/L2 Persistence";

/// One unique index the production Rust code relies on: the exact `ON CONFLICT` target of a
/// production upsert, plus the file:line of the statement that depends on it.
struct ReliedUponUniqueIndex {
    /// The table the upsert writes to.
    table: &'static str,
    /// The conflict target columns — a unique index on exactly these must exist.
    columns: &'static [&'static str],
    /// The production code site whose `ON CONFLICT` clause requires the index.
    code: &'static str,
}

/// The unique indexes production code relies on, read directly from `db/`'s `ON CONFLICT` clauses.
const RELIED_UPON_UNIQUE_INDEXES: &[ReliedUponUniqueIndex] = &[
    ReliedUponUniqueIndex {
        table: "schema_migration",
        columns: &["filename", "target"],
        code: "db/src/schema_migration.rs:130 `on conflict (filename, target)`",
    },
    ReliedUponUniqueIndex {
        table: "auth_identity",
        columns: &["provider", "provider_subject"],
        code: "db/src/guest.rs:208 `on conflict (provider, provider_subject)`",
    },
    ReliedUponUniqueIndex {
        table: "property_interest",
        columns: &["person_id", "property_id"],
        code: "db/src/intake.rs:407 `on conflict (person_id, property_id)`",
    },
    ReliedUponUniqueIndex {
        table: "integration_inbox",
        columns: &["source", "source_account", "external_event_id"],
        code: "db/src/whatsapp.rs:143 `on conflict (source, source_account, external_event_id)`",
    },
    ReliedUponUniqueIndex {
        table: "mq_delivery",
        columns: &["message_id", "subscription_id"],
        code: "db/src/outbox.rs:146 `on conflict (message_id, subscription_id)`",
    },
    ReliedUponUniqueIndex {
        table: "interaction",
        columns: &["source_system", "source_external_id"],
        code: "db/src/intake.rs:379 (also landing.rs:238, apple_ods.rs:345, deal_portal/command.rs:215) \
               `on conflict (source_system, source_external_id) where ...`",
    },
    ReliedUponUniqueIndex {
        table: "person_property",
        columns: &["person_id", "property_id", "relation_type"],
        code: "db/src/property/upsert_for_person_tx.rs:159 `on conflict (person_id, property_id, relation_type)`",
    },
    ReliedUponUniqueIndex {
        table: "media_upload_chunk",
        columns: &["upload_id", "chunk_index"],
        code: "db/src/media/media_row.rs:590 `on conflict (upload_id, chunk_index)`",
    },
    ReliedUponUniqueIndex {
        table: "property_media",
        columns: &["property_id", "media_id"],
        code: "db/src/media/media_row.rs:328 `on conflict (property_id, media_id)`",
    },
    ReliedUponUniqueIndex {
        table: "workflow_command_receipt",
        columns: &["command_id"],
        code: "db/src/command_receipt.rs:100 `on conflict(command_id)`",
    },
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

/// Every unique index on `table` whose key columns are exactly `columns`.
///
/// Matching is by the index's actual key columns (`pg_index.indkey`, first `indnkeyatts` entries),
/// not by a name, so an index that exists on the wrong columns does not satisfy the contract. An
/// expression index (or an `INCLUDE`-column index) does not match a plain column target and is
/// reported as absent — which is what an `ON CONFLICT (<columns>)` inference requires.
async fn unique_indexes_covering(
    conn: &mut sqlx::PgConnection,
    table: &str,
    columns: &[&str],
) -> Result<Vec<String>, sqlx::Error> {
    let wanted: Vec<String> = columns.iter().map(|column| (*column).to_string()).collect();
    sqlx::query_scalar::<_, String>(
        "select i.indexrelid::regclass::text \
         from pg_index i \
         join pg_class t on t.oid = i.indrelid \
         join pg_namespace n on n.oid = t.relnamespace \
         where n.nspname = 'public' \
           and t.relname = $1 \
           and i.indisunique \
           and ( \
             select array_agg(a.attname::text order by a.attname) \
             from unnest(i.indkey) with ordinality as k(attnum, ord) \
             join pg_attribute a on a.attrelid = i.indrelid and a.attnum = k.attnum \
             where k.ord <= i.indnkeyatts \
           ) = ( \
             select array_agg(c order by c) from unnest($2::text[]) as c \
           ) \
         order by 1",
    )
    .bind(table)
    .bind(&wanted)
    .fetch_all(conn)
    .await
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); the harness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-SCHEMA-006); the file and the assay use it.
async fn db_schema_006__unique_indexes_that_code_relies_upon_exist() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let dev = connect_dev().await;
    assert_eq!(
        dev.target(),
        DbTarget::Dev,
        "{HARNESS}: the unique-index proof runs only on an isolated DEV target"
    );

    let mut conn = dev
        .database()
        .pool()
        .acquire()
        .await
        .expect("pool checkout");

    // 1. Negative control A: a table that does not exist resolves to no covering index, so a
    //    matcher that answered "covered" for everything could not pass.
    let fabricated_table =
        unique_indexes_covering(&mut conn, "no_such_table_tst_db_schema_006", &["id"])
            .await
            .expect("query the fabricated table");
    assert!(
        fabricated_table.is_empty(),
        "{HARNESS}: a fabricated table must not report a covering unique index, got {fabricated_table:?}"
    );

    // 2. Negative control B: a real table with a fabricated column resolves to no covering
    //    index, so the column match is real and not a table-level existence check.
    let fabricated_columns = unique_indexes_covering(
        &mut conn,
        "schema_migration",
        &["filename", "no_such_column_tst_db_schema_006"],
    )
    .await
    .expect("query the fabricated column set");
    assert!(
        fabricated_columns.is_empty(),
        "{HARNESS}: a fabricated conflict target must not report a covering unique index, got {fabricated_columns:?}"
    );

    // 3. Positive: every `ON CONFLICT` target production code uses has a covering unique index.
    let mut missing: Vec<String> = Vec::new();
    let mut found: Vec<String> = Vec::new();
    for entry in RELIED_UPON_UNIQUE_INDEXES {
        let indexes = unique_indexes_covering(&mut conn, entry.table, entry.columns)
            .await
            .unwrap_or_else(|error| {
                panic!(
                    "{HARNESS}: catalogue read failed for {}.{}: {error}",
                    entry.table,
                    entry.columns.join(", ")
                )
            });
        if indexes.is_empty() {
            missing.push(format!(
                "{}.{} — relied on by {}",
                entry.table,
                entry.columns.join(", "),
                entry.code
            ));
        } else {
            found.push(format!(
                "{}.{} -> {}",
                entry.table,
                entry.columns.join(", "),
                indexes.join(", ")
            ));
        }
    }
    assert!(
        missing.is_empty(),
        "{HARNESS}: unique indexes production code relies on are missing on DEV — \
         an `ON CONFLICT` on any of these would fail with 42P10:\n  {}\nFound:\n  {}",
        missing.join("\n  "),
        found.join("\n  ")
    );

    // 4. Fault case: the relied-upon uniqueness is enforced, not merely declared. Insert one probe
    //    row into `schema_migration` (the standalone relied-upon table), then a duplicate with the
    //    same conflict target. The unique index must refuse the duplicate (23505). Everything runs
    //    in a transaction the harness can only roll back, so no probe row survives.
    let mut tx = dev.begin().await.expect("begin probe transaction");
    let probe_filename = format!("tst_db_schema_006_probe_{}.sql", dev.namespace());

    sqlx::query(
        "insert into schema_migration (filename, checksum, target) values ($1, 'probe', 'baseline')",
    )
    .bind(&probe_filename)
    .execute(tx.connection())
    .await
    .expect("first probe insert into schema_migration succeeds");

    let duplicate = sqlx::query(
        "insert into schema_migration (filename, checksum, target) values ($1, 'probe', 'baseline')",
    )
    .bind(&probe_filename)
    .execute(tx.connection())
    .await;

    let rollback = tx.rollback().await;
    rollback.expect("rollback probe transaction");

    let duplicate_error = duplicate.expect_err(
        "the unique index on schema_migration (filename, target) must refuse a duplicate",
    );
    let sqlstate = duplicate_error
        .as_database_error()
        .and_then(|error| error.code());
    assert_eq!(
        sqlstate.as_deref(),
        Some("23505"),
        "{HARNESS}: the duplicate must be refused as a unique violation (23505), got {duplicate_error}"
    );
}
