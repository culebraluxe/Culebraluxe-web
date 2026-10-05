//! ARCH.ONE_WRITER — workflow evidence (TST-ARCH-ONE-WRITER-006).
//!
//! Contract: `forge_workflow_evidence` has exactly one production writer. The
//! engine decides, records and publishes through the `db` DAO layer; no crate
//! outside `db` holds an `INSERT` into this table, so two lanes can never keep
//! two competing spellings of the same evidence row.
//!
//! The positive control is part of the contract: a table nobody writes would be
//! clean for the wrong reason, so the same scan that refuses a second writer
//! also has to find the one writer in use.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_one_writer__006__workflow_evidence

use test_harness::source;

/// True when a code line (comments stripped) inserts into the evidence table.
fn inserts_evidence(line: &str) -> bool {
    source::code_of(line)
        .to_lowercase()
        .contains("insert into forge_workflow_evidence")
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-ONE-WRITER-006); the file and the assay use it.
fn arch_one_writer_006__workflow_evidence() {
    let workspace = source::sources_under(&source::workspace_root());
    assert!(
        workspace.len() >= 300,
        "the workspace scan is the subject; only {} Rust files were found",
        workspace.len()
    );
    let mut holders: Vec<String> = Vec::new();
    for path in &workspace {
        let relative = source::relative(path);
        if relative.contains("/tests/") || relative.contains("/target/") {
            continue;
        }
        if source::read(path).lines().any(inserts_evidence) {
            holders.push(relative);
        }
    }
    holders.sort();
    assert_eq!(
        holders,
        vec!["db/src/forge_engine.rs"],
        "workflow evidence has one writer; a second INSERT into forge_workflow_evidence is a copy to keep in step"
    );

    // The positive control: the one writer is in use, and the table exists in the migration chain.
    let writer = source::read(&source::workspace_root().join(&holders[0]));
    assert!(
        writer.contains("forge_workflow_evidence"),
        "the writer must name the table it owns"
    );
    let migration = source::read(
        &source::workspace_root().join("db/migrations/109_forge_v10_workflow_evidence.sql"),
    );
    assert!(
        migration.to_lowercase().contains("create table")
            && migration.contains("forge_workflow_evidence"),
        "migration 109 creates the evidence table the writer owns"
    );

    // Negative controls: the detector fires on an insert and stays quiet on prose.
    assert!(inserts_evidence(
        "INSERT INTO forge_workflow_evidence (story_id) VALUES ($1)"
    ));
    assert!(!inserts_evidence(
        "// the DAO inserts into the evidence table for us"
    ));
    assert!(!inserts_evidence(
        "let evidence = load_workflow_evidence(&story_id).await?;"
    ));
}
