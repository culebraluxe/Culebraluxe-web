//! ARCH.ONE_WRITER — WBS dependencies (TST-ARCH-ONE-WRITER-012).
//!
//! Contract: `wbs_dependency` edges have exactly one production writer,
//! `WbsDao::insert_dependency` in `db/src/wbs.rs`. The service layer reaches
//! the table only through that DAO (`web/src/wbs/mod.rs` delegates), so the
//! dependency graph cannot fork into two spellings of the same edge.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_one_writer__012__wbs_dependencies

use test_harness::source;

/// True when a code line inserts a dependency edge.
fn inserts_dependency(line: &str) -> bool {
    source::code_of(line)
        .to_lowercase()
        .contains("insert into wbs_dependency")
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-ONE-WRITER-012); the file and the assay use it.
fn arch_one_writer_012__wbs_dependencies() {
    let workspace = source::sources_under(&source::workspace_root());
    let mut holders: Vec<String> = Vec::new();
    for path in &workspace {
        let relative = source::relative(path);
        if relative.contains("/tests/") || relative.contains("/target/") {
            continue;
        }
        if source::read(path).lines().any(inserts_dependency) {
            holders.push(relative);
        }
    }
    holders.sort();
    assert_eq!(
        holders,
        vec!["db/src/wbs.rs"],
        "dependency edges have one writer; a second INSERT into wbs_dependency forks the graph"
    );

    // The one writer inserts the full edge: project, source, target, and kind.
    let dao = source::read(&source::workspace_root().join(&holders[0]));
    assert!(
        dao.contains("insert into wbs_dependency(project_id, source_id, target_id, kind)"),
        "the writer records the whole edge, not a partial one"
    );
    assert!(
        dao.contains("pub async fn insert_dependency"),
        "the writer is a named DAO function the service can delegate to"
    );

    // The service delegates rather than writing: the DAO is the seam production uses.
    let service = source::read(&source::workspace_root().join("web/src/wbs/mod.rs"));
    assert!(
        service.contains("WbsDao::insert_dependency"),
        "the service reaches the table through the DAO, never around it"
    );

    // Negative controls: the detector fires on the insert and stays quiet on reads and prose.
    assert!(inserts_dependency(
        "insert into wbs_dependency(project_id, source_id, target_id, kind) values ($1,$2,$3,$4)"
    ));
    assert!(!inserts_dependency(
        "select project_id, source_id, target_id, kind from wbs_dependency"
    ));
    assert!(!inserts_dependency(
        "// the DAO inserts into wbs_dependency for us"
    ));
}
