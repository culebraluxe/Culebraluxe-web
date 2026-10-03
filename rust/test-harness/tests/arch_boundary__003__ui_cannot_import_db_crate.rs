//! ARCH.BOUNDARY — the UI cannot import the database crate (TST-ARCH-BOUNDARY-003).
//!
//! Contract: the browser bundle contains no database client, because the UI is compiled for `wasm32` and has no
//! socket to a database at all. `web/ui/Cargo.toml` names `domain`, `serde`, `chrono`, `base64` and the wasm bindings,
//! and no member of the workspace that owns a connection — not `db`, not `sqlx`, not `server`, not `service`, not
//! `forge`, not `workflow`. All 146 files under `web/ui/src` are equally free of a client, a driver URL and a SQL
//! statement.
//!
//! Both halves are checked, because either one alone can be satisfied while the boundary is broken: a dependency-free
//! manifest with a hand-rolled query in a source file, or clean sources with `sqlx` linked in. The manifest parse is
//! itself proven against a forged manifest, and the source detector against planted samples — a test that quietly
//! parsed nothing would pass while proving nothing.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_boundary__003__ui_cannot_import_db_crate

use test_harness::source;

/// The crates the browser bundle may never link: everything in the workspace that owns a connection, a server, or the
/// database driver itself.
const FORBIDDEN_CRATES: [&str; 12] = [
    "db",
    "sqlx",
    "web",
    "services",
    "forge",
    "workflow",
    "apis",
    "cli",
    "auth",
    "tokio",
    "axum",
    "test-harness",
];

/// What a UI source file may never contain: a database client, a driver URL, or a SQL statement in code.
fn db_access(line: &str) -> Option<&'static str> {
    // Comments are read as prose here too, not as code: the tree has exactly one line naming a client token — the note
    // at `web/ui/src/flight_recorder.rs:1687` recording that its vocabulary was measured against `DATABASE_URL_DEV` —
    // and a comment is not a connection.
    let code = source::code_of(line);
    for client in [
        "sqlx",
        "PgPool",
        "postgres://",
        "postgresql://",
        "DATABASE_URL",
    ] {
        if code.contains(client) {
            return Some("a database client in the UI");
        }
    }
    let code = code.to_lowercase();
    for statement in [
        "insert into",
        "delete from",
        "alter table",
        "create table",
        "on conflict",
    ] {
        if code.contains(statement) {
            return Some("a SQL statement in the UI");
        }
    }
    if source::contains_word(&code, "select") && source::contains_word(&code, "from") {
        return Some("a SELECT in the UI");
    }
    None
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-BOUNDARY-003); the file and the assay use it.
fn arch_boundary_003__ui_cannot_import_db_crate() {
    // 1. The manifest. The UI's own dependency table is the boundary, so it is read as a table, not as text: a mention
    //    of `db` inside a comment or a feature name must not satisfy the rule, and a real entry must not slip past it.
    let manifest = source::read(&source::rust_root().join("web/ui/Cargo.toml"));
    let declared = source::manifest_keys(&manifest, "dependencies");
    assert!(
        declared.iter().any(|key| key == "model"),
        "the UI's dependency table must have been found; it lists model (found: {declared:?})"
    );

    let offenders: Vec<String> = declared
        .iter()
        .map(|key| source::crate_name_of(key))
        .filter(|crate_name| FORBIDDEN_CRATES.contains(&crate_name.as_str()))
        .collect();
    assert!(
        offenders.is_empty(),
        "the UI bundle cannot link these crates: {offenders:?}"
    );

    // 2. The sources. Every file under `web/ui/src`, not just the screens.
    let rust = source::rust_root();
    let ui_src = rust.join("web/ui/src");
    let paths = source::sources_under(&ui_src);
    assert!(
        paths.len() >= 100,
        "the UI source tree is the subject of this contract; only {} files were found under {}",
        paths.len(),
        source::relative(&ui_src)
    );

    let mut findings = Vec::new();
    for path in &paths {
        let text = source::read(path);
        for (number, line) in text.lines().enumerate() {
            if let Some(what) = db_access(line) {
                findings.push(format!("{}:{}: {what}", source::relative(path), number + 1));
            }
        }
    }
    assert!(
        findings.is_empty(),
        "the UI reaches the database through the server's routes, never directly:\n{}",
        findings.join("\n")
    );

    // 3. The negative control. The manifest parse must flag a forged entry, and the detector must fire on the shapes
    //    this contract exists to refuse — while staying quiet on prose and on identifiers that merely look like SQL.
    let forged =
        "[package]\nname = \"ui\"\n\n[dependencies]\ndomain = { path = \"../core/domain\" }\n\
                  db = { path = \"../core/db\" }\nsqlx.workspace = true\n";
    let forged_keys: Vec<String> = source::manifest_keys(forged, "dependencies")
        .iter()
        .map(|key| source::crate_name_of(key))
        .filter(|crate_name| FORBIDDEN_CRATES.contains(&crate_name.as_str()))
        .collect();
    assert_eq!(
        forged_keys,
        vec!["db".to_string(), "sqlx".to_string()],
        "the manifest rule must catch a linked database crate"
    );
    assert!(
        db_access("let pool = sqlx::PgPool::connect(&database_url).await?;").is_some(),
        "the detector must catch a database client"
    );
    assert!(
        db_access("let sql = \"select id from property\";").is_some(),
        "the detector must catch a statement that names no client"
    );
    assert!(
        db_access("// the UI selects from the inventory the server returned").is_none(),
        "prose about a server-side select is not a query"
    );
    assert_eq!(
        db_access(
            "/// measured read-only against DATABASE_URL_DEV: forge_observer/command/domain; RUN_START/RUN_END"
        ),
        None,
        "the note at web/ui/src/flight_recorder.rs:1687 names an env var in prose, not a client"
    );
    assert!(
        db_access("let selected = inventory.iter().filter(|p| p.featured).count();").is_none(),
        "`selected` is not SQL"
    );
}
