//! ARCH.BOUNDARY — no server or UI module bypasses the Abstract Service where a service exists (TST-ARCH-BOUNDARY-001).
//!
//! Contract: a route does not talk to Postgres. `web/src` — 106 files, every route in the portal and the
//! website — contains no `sqlx`, no driver URL, no pool type and no SQL statement in code. Persistence enters through
//! the two approved seams, and the same scan proves they are actually used: 59 of those files name `db::` and 56 name
//! `services::`. A layer that reached the database directly would have to open it here, and that is what this test
//! refuses.
//!
//! The detector reads code, not prose. The real tree contains a sentence that looks like a query — `select fields from
//! their options` in a doc comment at `web/src/api/forms_grok.rs:6` — and it must not be a finding, which is
//! why every check runs on `source::code_of(line)` and why that sentence is asserted here as a control.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_boundary__001__no_server_ui_module_bypasses_the_abstract_service_where_a_service_exists

use test_harness::source;

/// What a server module may never contain: a database client, a driver URL, or a SQL statement in code.
fn direct_db_access(line: &str) -> Option<&'static str> {
    for client in [
        "sqlx",
        "PgPool",
        "PgConnection",
        "postgres://",
        "postgresql://",
    ] {
        if line.contains(client) {
            return Some("a database client in the server");
        }
    }
    let code = source::code_of(line).to_lowercase();
    for statement in [
        "insert into",
        "delete from",
        "alter table",
        "create table",
        "drop table",
        "on conflict",
    ] {
        if code.contains(statement) {
            return Some("a SQL statement in the server");
        }
    }
    if source::contains_word(&code, "select") && source::contains_word(&code, "from") {
        return Some("a SELECT in the server");
    }
    None
}

/// Whether the file reaches persistence through an approved seam.
fn uses_a_seam(text: &str) -> (&'static str, bool) {
    let dao = text.contains("db::");
    let service = text.contains("services::");
    if dao {
        ("db::", true)
    } else if service {
        ("services::", true)
    } else {
        ("neither", false)
    }
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-BOUNDARY-001); the file and the assay use it.
fn arch_boundary_001__no_server_ui_module_bypasses_the_abstract_service_where_a_service_exists() {
    let server_src = source::rust_root().join("web/src");
    let files = source::sources_under(&server_src);
    assert!(
        files.len() >= 80,
        "the server tree is the subject of this contract; only {} files were found under {}",
        files.len(),
        source::relative(&server_src)
    );

    let mut findings = Vec::new();
    let mut via_dao = 0usize;
    let mut via_service = 0usize;
    for path in &files {
        let text = source::read(path);
        if text.contains("db::") {
            via_dao += 1;
        }
        if text.contains("services::") {
            via_service += 1;
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

    assert!(
        findings.is_empty(),
        "the server reaches data through the DAO and service seams, never through SQL of its own:\n{}",
        findings.join("\n")
    );

    // The positive control: the seams must actually be there. A tree with no `db::` and no `services::` would be clean
    // for the wrong reason — this is the same scan that proves the server is a consumer of them.
    assert!(
        via_dao >= 20 && via_service >= 20,
        "the server must reach persistence through db:: and services:: (found {via_dao} and {via_service} files)"
    );

    // The negative control: the detector fires on a query and stays quiet on the prose that is really in the tree.
    assert!(direct_db_access(
        "let rows = sqlx::query(\"select id from property\").fetch_all(pool).await?;"
    )
    .is_some());
    assert!(direct_db_access("let pool = PgPool::connect(&url).await?;").is_some());
    assert!(direct_db_access(
        "let ids: Vec<String> = sql(\"delete from property_media\").fetch(pool).await?;"
    )
    .is_some());
    assert_eq!(
        direct_db_access("//! YYYY-MM-DD, select fields from their options, never an email the agent did not state."),
        None,
        "the prose at web/src/api/forms_grok.rs:6 is not a query"
    );
    assert_eq!(
        direct_db_access("let selected = inventory.filter(|row| row.is_active()).count();"),
        None,
        "`selected` is not SQL"
    );

    // And the seam census itself is proven, so the positive control cannot pass by counting the wrong token.
    assert_eq!(
        uses_a_seam("let row = db::property::load(pool, id).await?;"),
        ("db::", true)
    );
    assert_eq!(
        uses_a_seam("let out = services::command::execute(cmd).await?;"),
        ("services::", true)
    );
    assert_eq!(
        uses_a_seam("fn render(state: &ScreenState) -> Html { }"),
        ("neither", false)
    );
}
