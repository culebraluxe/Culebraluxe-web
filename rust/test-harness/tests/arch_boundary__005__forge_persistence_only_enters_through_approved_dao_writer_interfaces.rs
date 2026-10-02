//! ARCH.BOUNDARY — Forge persistence enters only through approved DAO and writer interfaces (TST-ARCH-BOUNDARY-005).
//!
//! Contract: `rust/forge` decides, records and publishes. It does not own a connection and it holds no statement.
//! Across its 111 source files there is no `sqlx`, no pool, no driver URL and no SQL in code; persistence enters
//! through the `db` crate's DAO and writer functions, which 24 of those files name. That is the difference between a
//! run whose state is queryable and auditable — the same rows every other lane reads — and a run that keeps its own
//! table because it could.
//!
//! The positive control is part of the contract, not decoration: a Forge crate that named no DAO seam at all would be
//! clean for the wrong reason, so the same scan that refuses SQL also has to find the seam in use.
//!
//! **No exception, and that is recent (`2026-10-02`, the seam closure).** `engine/observer.rs` used to write its own
//! `INSERT INTO workflow_execution_trace_event` and was pinned here as a known exception. The statement is the DAO's
//! (`ForgeEngineDao::record_observer`, which also owns the `ON CONFLICT` dedupe), the copy in Forge was dead, and it
//! is gone — so this test now asserts an *empty* finding list. A second file acquiring SQL fails here, and so would
//! the first one coming back.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path rust/Cargo.toml -p test-harness --test arch_boundary__005__forge_persistence_only_enters_through_approved_dao_writer_interfaces

use test_harness::source;

/// What a Forge module may never contain: a database client, a driver URL, or a SQL statement in code.
fn direct_db_access(line: &str) -> Option<&'static str> {
    let code = source::code_of(line);
    for client in [
        "sqlx",
        "PgPool",
        "PgConnection",
        "postgres://",
        "postgresql://",
    ] {
        if code.contains(client) {
            return Some("a database client in Forge");
        }
    }
    let code = code.to_lowercase();
    for statement in [
        "insert into",
        "delete from",
        "alter table",
        "create table",
        "drop table",
        "on conflict",
    ] {
        if code.contains(statement) {
            return Some("a SQL statement in Forge");
        }
    }
    if source::contains_word(&code, "update") && source::contains_word(&code, "set") {
        return Some("an UPDATE in Forge");
    }
    if source::contains_word(&code, "select") && source::contains_word(&code, "from") {
        return Some("a SELECT in Forge");
    }
    None
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-BOUNDARY-005); the file and the assay use it.
fn arch_boundary_005__forge_persistence_only_enters_through_approved_dao_writer_interfaces() {
    let forge_src = source::rust_root().join("forge/src");
    let files = source::sources_under(&forge_src);
    assert!(
        files.len() >= 80,
        "the Forge crate is the subject of this contract; only {} files were found under {}",
        files.len(),
        source::relative(&forge_src)
    );

    let mut findings = Vec::new();
    let mut via_dao = 0usize;
    for path in &files {
        let text = source::read(path);
        if text.contains("db::") {
            via_dao += 1;
        }
        for (number, line) in text.lines().enumerate() {
            if let Some(what) = direct_db_access(line) {
                findings.push(format!(
                    "{}:{}: {what}: {}",
                    source::relative(path),
                    number + 1,
                    line.trim()
                ));
            }
        }
    }

    // No exception and no allowance: every statement in Forge `src/` is a finding, so this list has to be empty. The
    // observer's `INSERT` lives in the DAO (`ForgeEngineDao::record_observer`) and its dead copy here is gone.
    let drivers: Vec<&String> = findings
        .iter()
        .filter(|finding| finding.contains("database client in Forge"))
        .collect();
    let statements: Vec<&String> = findings
        .iter()
        .filter(|finding| !finding.contains("database client in Forge"))
        .collect();
    assert!(
        drivers.is_empty(),
        "Forge owns no connection and links no driver:\n{}",
        drivers
            .iter()
            .map(|finding| finding.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
    let mut sql_files: Vec<String> = statements
        .iter()
        .map(|finding| finding.split(':').next().unwrap_or("").to_string())
        .collect();
    sql_files.sort();
    sql_files.dedup();
    assert!(
        sql_files.is_empty(),
        "Forge holds no SQL of its own; every write goes through a db DAO or writer:\n{}",
        statements
            .iter()
            .map(|finding| finding.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );

    assert!(
        via_dao >= 10,
        "Forge must reach persistence through the `db::` DAO seam (only {via_dao} files name it, so this is not the crate under test)"
    );

    // The statement did not vanish with the copy, so the authority is asserted here: "no SQL in Forge" must never be
    // able to mean "the trace row has no writer". The DAO owns the statement; Forge reaches it through the DAO.
    let dao = source::read(&source::rust_root().join("core/db/src/forge_engine.rs"));
    assert!(
        dao.contains("insert into workflow_execution_trace_event")
            && dao.contains("pub async fn record_observer"),
        "the canonical trace statement lives in `ForgeEngineDao::record_observer` now that Forge's copy is gone"
    );
    let caller = source::read(&forge_src.join("engine/observer.rs"));
    assert!(
        caller.contains("dao.record_observer("),
        "Forge must still write the trace row — through the DAO, never with SQL of its own"
    );

    // The negative control: the detector fires on a query, and stays quiet on Forge's own vocabulary and on prose —
    // including the trait method `.execute(&envelope)` that exists in the tree (`forge/src/engine/git_publish.rs:128`)
    // and is a command dispatch, not a statement.
    assert!(direct_db_access("let rows = sqlx::query_as::<_, Row>(\"select id from storyboard_story\").fetch_all(pool).await?;").is_some());
    assert!(direct_db_access("let pool = PgPool::connect(&database_url).await?;").is_some());
    assert_eq!(
        direct_db_access("let out = envelope.execute(&name, &request.input);"),
        None,
        "an envelope's execute is not a SQL statement"
    );
    assert_eq!(
        direct_db_access("// the DAO selects from the work item table for us"),
        None,
        "prose about a DAO's own query is not a query here"
    );
}
