//! DB.MIGRATION — no backwards destructive migration without explicit marker
//! (TST-DB-MIGRATION-006).
//!
//! Contract: a migration that destroys committed data (drops a table or column, truncates,
//! deletes without a key) may only land carrying an EXPLICIT justification in its own
//! header — what is destroyed, why it is safe, and how the data survives (snapshot table,
//! re-derivable source, superseded-empty replacement, temporary-column promotion). The
//! repo's own convention is the marker: every destructive file to date opens with the
//! reason the destruction is safe, so a future destructive file without one is either an
//! accident or a backwards step smuggled past review.
//!
//! The test is a ratchet over live filesystem truth. A detector finds destructive DDL in
//! executable SQL (comments stripped — prose about destruction is not destruction); the
//! pinned allowlist names the six reviewed destructive files, each with the justification
//! phrase its header must still carry; anything else destructive fails. The negative
//! controls run the detector over synthetic snippets both ways, so neither a blind
//! detector nor a blind allowlist could pass.
//!
//! Level: L2 Persistence — the taxonomy fixes the harness; the DEV connection below proves
//! the target the suite runs under, while the files are read from the repo production
//! applies from. The harness refuses PRODUCTION before any socket is opened.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test db_migration__006__no_backwards_destructive_migration_without_explicit_marker -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the proof needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use test_harness::database::TestDatabase;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "DatabaseHarness/L2 Persistence";

/// Destructive statements, as lowercase needles matched against executable SQL.
/// `DROP INDEX`/`DROP VIEW`/`DROP FUNCTION`/`DROP TRIGGER` are excluded on purpose: they
/// rebuild derived objects, they do not destroy committed data. `DELETE` is excluded too —
/// data migrations delete by key as their job; the ratchet watches schema destruction.
const DESTRUCTIVE_NEEDLES: &[&str] = &["drop table", "drop column", "truncate", "alter table"];

/// Strip `--` line comments and `/* */` block comments: prose about destruction
/// ("no deletes, no truncates") must not read as destruction.
fn executable_sql(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    let mut in_block = false;
    let mut in_string = false;
    while let Some(char) = chars.next() {
        if in_block {
            if char == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block = false;
            }
            continue;
        }
        if in_string {
            out.push(char);
            if char == '\'' {
                if chars.peek() == Some(&'\'') {
                    out.push(chars.next().unwrap_or('\''));
                } else {
                    in_string = false;
                }
            }
            continue;
        }
        if char == '\'' {
            in_string = true;
            out.push(char);
            continue;
        }
        if char == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block = true;
            continue;
        }
        if char == '-' && chars.peek() == Some(&'-') {
            for skipped in chars.by_ref() {
                if skipped == '\n' {
                    out.push('\n');
                    break;
                }
            }
            continue;
        }
        out.push(char);
    }
    out
}

/// The destructive needles present in a file's executable SQL.
///
/// Matching is on word boundaries: prose inside code (a column named `truncated_at`,
/// a string mentioning truncation) must not read as destruction. `ALTER TABLE` alone is
/// not destruction — it is only reported when paired with a drop of a committed-data
/// object in the same file (a dynamic `DROP COLUMN` inside an `EXECUTE`, as 225 does,
/// still names the drop).
fn destructive_hits(sql: &str) -> Vec<String> {
    let code = executable_sql(sql).to_lowercase();
    let found = |needle: &str| {
        code.match_indices(needle).any(|(start, _)| {
            let end = start + needle.len();
            let left = code[..start]
                .chars()
                .next_back()
                .map_or(true, |c| !(c.is_ascii_alphanumeric() || c == '_'));
            let right = code[end..]
                .chars()
                .next()
                .map_or(true, |c| !(c.is_ascii_alphanumeric() || c == '_'));
            left && right
        })
    };
    let mut hits: Vec<String> = Vec::new();
    for needle in ["drop table", "drop column", "truncate"] {
        if found(needle) {
            hits.push(needle.to_string());
        }
    }
    if found("alter table") && found("drop column") {
        hits.push("alter table … drop column".to_string());
    }
    hits
}

/// Reviewed destructive files: each must still carry its justification phrase in its header.
/// (file, justification phrase that must appear in the leading comment block).
const JUSTIFIED_DESTRUCTIVE: &[(&str, &str)] = &[
    ("054_retire_sanity_document_id.sql", "zero non-null values"),
    ("085_storyboard_active_work.sql", "backfills"),
    ("136_drop_forge_phase_artifact.sql", "redundant"),
    ("153_drop_landing_child_tables.sql", "re-derivable"),
    ("160_l_applemail.sql", "never written to and is empty"),
    ("225_property_golden_from_regrid.sql", "snapshotted"),
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DB-MIGRATION-006); the file and the assay use it.
async fn db_migration_006__no_backwards_destructive_migration_without_explicit_marker() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let dev = connect_dev().await;
    assert_eq!(
        dev.target(),
        DbTarget::Dev,
        "{HARNESS}: the destruction ratchet runs only on an isolated DEV target"
    );

    // 1. Negative controls on the detector: executable destruction fires; prose does not;
    //    strings that merely name destruction do not.
    assert!(
        destructive_hits(
            "-- no deletes, no truncates, nothing dropped here\nCREATE TABLE t (id uuid);"
        )
        .is_empty(),
        "{HARNESS}: prose about destruction is not destruction"
    );
    assert!(
        !destructive_hits("DROP TABLE IF EXISTS doomed;").is_empty(),
        "{HARNESS}: an executable DROP TABLE fires the detector"
    );
    assert!(
        destructive_hits(
            "COMMENT ON COLUMN c.x IS 'dropped and re-added for widening';\nALTER TABLE c ALTER COLUMN x TYPE numeric;"
        )
        .is_empty(),
        "{HARNESS}: a comment naming a drop plus a non-destructive ALTER is not destruction"
    );
    assert!(
        !destructive_hits("EXECUTE format('ALTER TABLE t DROP COLUMN %I', col);").is_empty(),
        "{HARNESS}: destruction inside dynamic SQL fires — a string that executes is executable"
    );
    assert!(
        destructive_hits("CREATE TABLE truncated_log (id uuid);").is_empty(),
        "{HARNESS}: an identifier containing a needle is not destruction"
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

    // 3. Every destructive file is on the allowlist, and every allowlisted file still
    //    carries its justification — a silent new DROP, or a stripped rationale, fails.
    let allowlisted: std::collections::BTreeSet<&str> = JUSTIFIED_DESTRUCTIVE
        .iter()
        .map(|(file, _)| *file)
        .collect();
    let mut unmarked: Vec<String> = Vec::new();
    for file in &files {
        let sql =
            std::fs::read_to_string(migrations_dir.join(file)).expect("a migration file reads");
        let hits = destructive_hits(&sql);
        if hits.is_empty() {
            continue;
        }
        if !allowlisted.contains(file.as_str()) {
            unmarked.push(format!("{file} [{}]", hits.join(", ")));
        }
    }
    assert!(
        unmarked.is_empty(),
        "{HARNESS}: destructive migration without an explicit marker:\n  {}",
        unmarked.join("\n  ")
    );

    for (file, justification) in JUSTIFIED_DESTRUCTIVE {
        let sql = std::fs::read_to_string(migrations_dir.join(file))
            .unwrap_or_else(|_| panic!("{HARNESS}: allowlisted file {file} still exists"));
        let header: String = sql
            .lines()
            .take_while(|line| line.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n")
            .to_lowercase();
        assert!(
            header.contains(&justification.to_lowercase()),
            "{HARNESS}: {file} lost its justification marker ({justification})"
        );
    }
}
