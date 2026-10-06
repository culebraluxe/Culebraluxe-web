//! DB.SCHEMA — the trigger and the stored routines the code calls exist (TST-DB-SCHEMA-008).
//!
//! CONTRACT. The engine moved its multi-statement behaviour into the database (claim, settle, dispatch, stale
//! recovery) and leans on one trigger — a story that becomes `Ready` gets exactly one open work item. If a routine
//! the Rust code calls is missing the call fails at RUN time (`function … does not exist`), and if the trigger is
//! missing a Ready story simply never runs.
//!
//!   * every `forge_*` routine named in a `select` / `from` / `perform` in the `db` crate's SQL exists in `pg_proc`
//!     (the list is DERIVED from the source, so a new call is covered the day it is written);
//!   * `storyboard_story_ready_dispatch` exists and runs `agent_work_item_dispatch()`;
//!   * and it FIRES: a story moved to Ready gets exactly one open work item, in a transaction that is rolled back.
//!
//! A non-production database only; PROD is refused by the harness before a socket opens.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test db_schema__008__trigger_function_referenced_by_code_exists

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use test_harness::database::TestDatabase;

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
    {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Every `forge_*` routine the db crate's SQL calls: `select forge_x(`, `from forge_x(`, `perform forge_x(`.
fn called_routines() -> BTreeSet<String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../db/src");
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    let mut routines = BTreeSet::new();
    for file in files {
        let text = std::fs::read_to_string(&file)
            .expect("read db source")
            .to_ascii_lowercase();
        for marker in ["select forge_", "from forge_", "perform forge_"] {
            let mut rest = text.as_str();
            while let Some(at) = rest.find(marker) {
                let after = &rest[at + marker.len() - "forge_".len()..];
                let name: String = after
                    .chars()
                    .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')
                    .collect();
                if after[name.len()..].starts_with('(') {
                    routines.insert(name);
                }
                rest = &rest[at + marker.len()..];
            }
        }
    }
    routines
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (APP_ENV=dev): runs only against the disposable DEV branch; PROD is refused"]
async fn db_schema_008__trigger_function_referenced_by_code_exists() {
    let test_db = TestDatabase::connect_from_env().await.expect(
        "a declared non-production database (DATABASE_URL_DEV with APP_ENV/VERCEL_ENV not production)",
    );
    let pool = test_db.database().pool();

    // 1. Every routine the code calls exists.
    let routines = called_routines();
    assert!(
        routines.len() >= 8,
        "the scan must find the engine's stored routines (found {routines:?})"
    );
    let mut missing = Vec::new();
    for routine in &routines {
        let found: i64 = sqlx::query_scalar("select count(*) from pg_proc where proname = $1")
            .bind(routine)
            .fetch_one(pool)
            .await
            .expect("read pg_proc");
        if found == 0 {
            missing.push(routine.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "the db crate calls stored routines that do not exist in the database: {missing:?}"
    );

    // 2. The dispatch trigger exists and runs the dispatch function.
    let definition: String = sqlx::query_scalar(
        "select pg_get_triggerdef(oid) from pg_trigger
          where tgname = 'storyboard_story_ready_dispatch' and tgrelid = 'storyboard_story'::regclass",
    )
    .fetch_optional(pool)
    .await
    .expect("read pg_trigger")
    .expect("storyboard_story_ready_dispatch must exist on storyboard_story");
    assert!(
        definition.contains("agent_work_item_dispatch()"),
        "the trigger must run agent_work_item_dispatch(): {definition}"
    );

    // 3. It fires: Planned → Ready yields exactly one open work item.
    let mut tx = test_db.begin().await.expect("begin transaction");
    sqlx::query(
        "insert into storyboard_story (id, workstream, title, priority, status)
         values ('TST-DB-SCHEMA-008', 'TEST', 'dispatch trigger probe', 'P3', 'Planned')",
    )
    .execute(&mut *tx.connection())
    .await
    .expect("insert a Planned story");
    let before: i64 = sqlx::query_scalar(
        "select count(*) from agent_work_item where story_id = 'TST-DB-SCHEMA-008'",
    )
    .fetch_one(&mut *tx.connection())
    .await
    .expect("count work items before");
    assert_eq!(before, 0, "a Planned story has no work item");
    sqlx::query("update storyboard_story set status = 'Ready' where id = 'TST-DB-SCHEMA-008'")
        .execute(&mut *tx.connection())
        .await
        .expect("move the story to Ready");
    let after: Vec<String> = sqlx::query_scalar(
        "select state from agent_work_item where story_id = 'TST-DB-SCHEMA-008'",
    )
    .fetch_all(&mut *tx.connection())
    .await
    .expect("read the dispatched work item");
    assert_eq!(
        after,
        vec!["Ready".to_string()],
        "a story that becomes Ready must get exactly one open work item"
    );
    let _ = tx.rollback().await;
}
