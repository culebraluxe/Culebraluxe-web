//! DB.SCHEMA — FK targets exist (TST-DB-SCHEMA-005).
//!
//! CONTRACT. A foreign key is only a promise while the constraint it is made of is in the catalog, is VALIDATED, and
//! its referenced table and columns still resolve. `pg_constraint` is the production catalog, so the proof is read from
//! it: every public FK must be validated, and each of its child columns must find a same-typed column in the table it
//! references. A dropped constraint, a renamed parent column or a left-unvalidated constraint is a schema defect the
//! code silently inherits — the row it used to forbid now writes.
//!
//! Why this reads the catalog and not the data: "no orphaned rows" is what a healthy database already reports whether or
//! not the constraint exists. Drop every FK and that query still returns 0 — the test would pass while the promise it
//! names is gone. The catalog is where the promise lives, so the catalog is what this proves, and the data-level
//! orphan sweep is kept only as supporting evidence that what is declared is what is enforced.
//!
//! The FKs the code leans on are pinned by name and parent table, so a renamed or dropped one fails by identity rather
//! than by coincidence. A non-production database only; PROD is refused by the harness before a socket opens.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_schema__005__fk_targets_exist

use test_harness::database::TestDatabase;

/// `(child table, constraint name, the table it must reference)`.
///
/// Pinned rather than discovered: a constraint that silently disappears is the defect this story exists to catch, and
/// "count the FKs" cannot notice that one is missing — only naming it can.
const RELIED_UPON: [(&str, &str, &str); 7] = [
    // The dispatch rule TST-DB-TRANSACTION-001 refuses to break: an item names the story it works.
    (
        "agent_work_item",
        "agent_work_item_story_id_fkey",
        "storyboard_story",
    ),
    // The run link TST-DB-TRANSACTION-002 reads back after beginning a run.
    (
        "agent_work_item",
        "agent_work_item_story_run_id_fkey",
        "storyboard_story_run",
    ),
    (
        "storyboard_story_run",
        "storyboard_story_run_story_id_fkey",
        "storyboard_story",
    ),
    // The evidence unit TST-DB-TRANSACTION-003 merges into belongs to a real instance and a real story.
    (
        "forge_workflow_evidence",
        "forge_workflow_evidence_process_instance_id_fkey",
        "process_instances",
    ),
    (
        "forge_workflow_evidence",
        "forge_workflow_evidence_story_id_fkey",
        "storyboard_story",
    ),
    // `person_identity` is what makes a person the same person across channels.
    (
        "person_identity",
        "person_identity_person_id_fkey",
        "person",
    ),
    // `deal` is a property's commercial outcome; an orphan deal is a deal nobody can open.
    ("deal", "deal_property_id_fkey", "property"),
];

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (APP_ENV=dev): runs only against the disposable DEV branch; PROD is refused"]
async fn db_schema_005__fk_targets_exist() {
    let test_db = TestDatabase::connect_from_env().await.expect(
        "a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)",
    );
    let pool = test_db.database().pool();

    // 1. The constraints the code leans on exist, by name, and point at the table they promise.
    for (table, constraint, parent) in RELIED_UPON {
        let target: Option<String> = sqlx::query_scalar(
            "select confrelid::regclass::text from pg_constraint
               where conname = $1 and conrelid = $2::regclass and contype = 'f'",
        )
        .bind(constraint)
        .bind(table)
        .fetch_optional(pool)
        .await
        .expect("read pg_constraint")
        .unwrap_or_else(|| {
            panic!(
                "{table}.{constraint} must exist: the code relies on that reference being enforced"
            )
        });
        assert_eq!(
            target.as_deref(),
            Some(parent),
            "{table}.{constraint} must reference {parent}"
        );
    }

    // 2. Every public FK is VALIDATED. An unvalidated constraint is not enforced against the rows already there, so it
    //    reads as a promise in the catalog and behaves as nothing at all.
    let unvalidated: Vec<String> = sqlx::query_scalar(
        "select conrelid::regclass::text || '.' || conname from pg_constraint
           where contype = 'f' and connamespace = 'public'::regnamespace and not convalidated
           order by 1",
    )
    .fetch_all(pool)
    .await
    .expect("read convalidated");
    assert!(
        unvalidated.is_empty(),
        "these foreign keys exist but were never validated, so the rows already in the table were never checked: {unvalidated:?}"
    );

    // 3. Every child column of every public FK resolves to a same-typed column in the table it references. This is the
    //    "target exists" half in full: it holds for all FKs, not only the pinned ones.
    let dangling: Vec<String> = sqlx::query_scalar(
        "select c.conrelid::regclass::text || '.' || c.conname
           from pg_constraint c
           join lateral unnest(c.conkey, c.confkey) with ordinality as k(attnum, parent_att, ord) on true
           left join pg_attribute ca on ca.attrelid = c.conrelid and ca.attnum = k.attnum and not ca.attisdropped
           left join pg_attribute pa on pa.attrelid = c.confrelid and pa.attnum = k.parent_att and not pa.attisdropped
          where c.contype = 'f'
            and c.connamespace = 'public'::regnamespace
            and (ca.attname is null or pa.attname is null
                 or ca.atttypid <> pa.atttypid or ca.atttypmod <> pa.atttypmod)
          order by 1",
    )
    .fetch_all(pool)
    .await
    .expect("resolve every FK target column");
    assert!(
        dangling.is_empty(),
        "these foreign keys name a target column that does not exist, or does not match the child's type: {dangling:?}"
    );

    // 4. NEGATIVE CASE: the reference is enforced, not merely declared. A work item for a story that does not exist is
    //    refused — so the catalog rows above describe a real promise.
    let mut tx = test_db.begin().await.expect("begin transaction");
    let orphan = sqlx::query(
        "insert into agent_work_item (story_id, state) values ('TST-DB-SCHEMA-005-NO-SUCH-STORY', 'Ready')",
    )
    .execute(&mut *tx.connection())
    .await;
    assert!(
        orphan.is_err(),
        "agent_work_item.story_id must refuse a story that does not exist: the FK target is not being enforced"
    );
    let _ = tx.rollback().await;

    // 5. NEGATIVE CASE, and the one that keeps this test honest: the probe must be able to say NO. If the lookup above
    //    matched every name, or the sweep below counted nothing by construction, a dropped constraint would pass this
    //    file. A name that does not exist is therefore asserted to be absent, and a count is asserted non-zero.
    let absent: Option<String> = sqlx::query_scalar(
        "select confrelid::regclass::text from pg_constraint
           where conname = 'no_such_constraint_the_code_cannot_rely_on' and contype = 'f'",
    )
    .fetch_optional(pool)
    .await
    .expect("read pg_constraint for an absent name");
    assert_eq!(
        absent, None,
        "a constraint that does not exist must not be reported as existing, or section 1 proves nothing"
    );

    let fks: i64 = sqlx::query_scalar(
        "select count(*) from pg_constraint
           where contype = 'f' and connamespace = 'public'::regnamespace",
    )
    .fetch_one(pool)
    .await
    .expect("count public foreign keys");
    assert!(
        fks as usize >= RELIED_UPON.len(),
        "the schema must actually carry the foreign keys this story pins: {fks} found, {} pinned",
        RELIED_UPON.len()
    );

    // 6. Supporting evidence that what is declared is what the data obeys. These rows are the same relationships the
    //    catalog sections above cover; they are kept because a validated constraint and a satisfied table are two
    //    different claims, and this one is the second.
    let mut conn = pool.acquire().await.expect("pool checkout");
    for (table, column, parent) in [
        ("property_interest", "property_id", "property"),
        ("deal", "property_id", "property"),
        ("interaction", "property_id", "property"),
        ("deal", "client_person_id", "person"),
        ("person", "assigned_user_id", "app_user"),
    ] {
        // `table`/`column`/`parent` are identifiers from the literal tuple above, never caller text — the same shape the
        // harness itself uses when it names a schema (`database.rs`), so the assertion is the injection audit.
        let orphans: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "select count(*) from {table} child
              where child.{column} is not null
                and not exists (select 1 from {parent} p where p.id = child.{column})"
        )))
        .fetch_one(&mut *conn)
        .await
        .unwrap_or_else(|error| panic!("sweep {table}.{column} -> {parent}: {error}"));
        assert_eq!(
            orphans, 0,
            "{table}.{column} references {parent} and must hold no orphan"
        );
    }
}
