//! ARCH.ONE_WRITER — command receipts (TST-ARCH-ONE-WRITER-007).
//!
//! Contract: one writer wins per command. `workflow_command_receipt` carries
//! `command_id` as its primary key (migration 018), and the canonical claim in
//! `db/src/command_receipt.rs::claim_tx` inserts with
//! `on conflict(command_id) do nothing`: the same engine `commandId` can never
//! duplicate a business effect, no matter how many times it is retried.
//!
//! Level: L0 Pure — file reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_one_writer__007__command_receipts

use test_harness::source;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-ONE-WRITER-007); the file and the assay use it.
fn arch_one_writer_007__command_receipts() {
    // The table key is the whole guarantee: one row per command id, enforced by the database.
    let migration = source::read(
        &source::workspace_root().join("db/migrations/018_workflow_command_receipt.sql"),
    );
    assert!(
        migration.contains("create table workflow_command_receipt")
            && migration.contains("command_id text primary key"),
        "migration 018 keys the receipt table on command_id"
    );

    // The canonical lifecycle lives in one module: claim (insert-if-absent), finalize, and read-back.
    let writer = source::read(&source::workspace_root().join("db/src/command_receipt.rs"));
    assert!(
        writer.contains("pub async fn claim_tx"),
        "the receipt module owns the claim"
    );
    assert!(
        writer.contains("on conflict(command_id) do nothing"),
        "the claim is insert-if-absent, so a retry never duplicates the effect"
    );
    assert!(
        writer.contains("pub async fn finalize_tx"),
        "the receipt module owns the finalize"
    );
    assert!(
        writer.contains("where command_id=$1"),
        "the receipt module reads back by the same key it writes"
    );

    // The claim answers whether this caller won: insert returns the id, conflict returns nothing.
    assert!(
        writer.contains("returning command_id") && writer.contains("Ok(inserted.is_some())"),
        "claim_tx reports won (Some) versus already-claimed (None)"
    );

    // Negative controls: the guarantee is the key plus the conflict clause, not the table name alone.
    assert!(
        !migration.contains("on conflict"),
        "the migration states the key; the claim adds the race safety"
    );
    assert!(
        !writer.contains("on conflict(command_id) do update"),
        "a claim that overwrote on conflict would let a retry rewrite history"
    );
}
