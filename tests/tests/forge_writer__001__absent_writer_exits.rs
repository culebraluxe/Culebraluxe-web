//! FORGE.WRITER — an absent story writer fails closed before any story mutation (FORGE-FIX-001).
//!
//! CONTRACT. The Forge engine binary writes its story state to the production database. When
//! `DATABASE_URL_PROD` is unset (or the writer otherwise cannot connect), the process must exit non-zero
//! **before any story mutation** — before the packet read, before the claim is opened, before the base commit
//! is stamped — with the reason on stderr. It must never fall back to `NullWriter` on the production path:
//! every `NullWriter` method answers `Ok`, so a PROD run on one silently drops every `mark_story_*`, hold,
//! artifact and spend row.
//!
//! The production binary (`forge/src/bin/forge.rs`) therefore:
//!   1. connects `DbForgeStateWriter::connect_env()` before the claim is opened, and refuses writer-absent with
//!      exit 2 (the same shape as the execution-target refusal), and
//!   2. refuses `drive_with_shared_runtime` without a production writer (`is_production_writer`), so no future
//!      caller can drive the shared runtime on a silent-drop writer by accident.
//!
//! `NullWriter` survives solely for explicitly named non-production contexts — the `FORGE_STORE=memory` local
//! dry run and unit tests — and the mutation door (`forge/src/bin/forge_task.rs`) fails closed too.
//!
//! NO EXTERNAL I/O. `RuntimeHarness` scopes the process environment and restores it on drop; the writer probe
//! reads presence only and never opens a socket. The binary wiring is proven by reading the sources, the way the
//! ARCH.BOUNDARY guards do. Level: L0 Pure.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test forge_writer__001__absent_writer_exits

use forge::engine::db_writer::DbForgeStateWriter;
use forge::engine::writer::{ForgeStateWriter, NullWriter, RecordingWriter};
use test_harness::source;
use test_harness::RuntimeHarness;

/// A non-comment, non-import line that names `NullWriter` must be a construction site, and every construction
/// site must sit inside an explicitly named non-production context (`FORGE_STORE`, the memory dry run).
fn null_writer_construction_outside_named_context(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut findings = Vec::new();
    // An import names the symbol; documentation names the context. Neither constructs a writer — and an import
    // may wrap across lines, so the whole `use …;` statement is skipped, not just the line that opens it.
    let mut in_import = false;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("use ") {
            in_import = true;
        }
        let import_line = in_import;
        if line.contains(';') {
            in_import = false;
        }
        if import_line || trimmed.starts_with("//") {
            continue;
        }
        if !line.contains("NullWriter") {
            continue;
        }
        let from = index.saturating_sub(10);
        let to = (index + 10).min(lines.len().saturating_sub(1));
        let window = lines[from..=to].join("\n");
        if !window.contains("FORGE_STORE") {
            findings.push(format!("line {}: {}", index + 1, line.trim()));
        }
    }
    findings
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name; the file and the assay use it.
fn forge_writer_001__absent_writer_exits() {
    let api = "a Forge run with no story writer must fail closed before any story mutation";

    // LAYER ONE, no I/O: with DATABASE_URL_PROD absent (and a decoy DEV URL present, so a fallback would
    // satisfy it) the production writer refuses to construct, naming the missing variable.
    let mut env = RuntimeHarness::acquire();
    env.remove("DATABASE_URL_PROD");
    env.set(
        "DATABASE_URL_DEV",
        "postgres://dev-only.invalid/culebraluxe",
    );
    env.remove("DATABASE_URL");
    let refusal = match DbForgeStateWriter::connect_env() {
        Ok(_) => panic!("{api}: no DATABASE_URL_PROD must mean no production state writer"),
        Err(message) => message,
    };
    assert!(
        refusal.contains("DATABASE_URL_PROD"),
        "{api}: the refusal must name the missing variable, got: {refusal}",
    );

    // LAYER TWO, no I/O: the marker that tells a silent-drop writer from a real one. `NullWriter` is the only
    // production-crate writer that answers `false`; the real writer and the recording test double answer `true`.
    assert!(
        !NullWriter.is_production_writer(),
        "{api}: NullWriter must be marked non-production, or the drive guard cannot see it",
    );
    assert!(
        RecordingWriter::default().is_production_writer(),
        "a recording writer holds state, so it must read as a production writer",
    );
    env.set(
        "DATABASE_URL_PROD",
        "postgres://placeholder.invalid/culebraluxe",
    );
    assert!(
        DbForgeStateWriter::connect_env()
            .expect("a present DATABASE_URL_PROD must yield a writer without opening a connection")
            .is_production_writer(),
        "the real writer must read as a production writer",
    );

    // LAYER THREE, the binary wiring: the writer is connected before the claim is opened, and writer-absent is
    // refused with exit 2 rather than falling back to NullWriter.
    let forge_bin = source::read(&source::workspace_root().join("forge/src/bin/forge.rs"));
    assert!(
        forge_bin.lines().count() >= 500,
        "forge.rs is the subject of this contract; the read found almost nothing",
    );
    let connect = forge_bin
        .find("DbForgeStateWriter::connect_env()")
        .expect("forge.rs connects the production story writer");
    let claim = forge_bin
        .find("begin_agent_work_run")
        .expect("forge.rs opens the work-item claim");
    assert!(
        connect < claim,
        "{api}: the writer is connected at {connect} but the claim opens at {claim} — \
         a writer-absent run must be refused before it owns a queue row",
    );
    assert!(
        forge_bin.contains("refusing to run without a production story writer"),
        "{api}: the refusal must say why the run stops, on stderr",
    );
    assert!(
        forge_bin.contains("story writer=null (FORGE_STORE=memory; local dry run only"),
        "the NullWriter fallback must name the dry run it belongs to, not read as a quiet default",
    );
    let strays = null_writer_construction_outside_named_context(&forge_bin);
    assert!(
        strays.is_empty(),
        "no code path may construct NullWriter outside an explicitly named non-production context:\n{}",
        strays.join("\n"),
    );

    // LAYER FOUR, the second door: the shared (production) drive refuses a non-production writer even if a
    // future caller gets past the binary's own refusal.
    let drive = forge_bin
        .find("fn drive_with_shared_runtime")
        .expect("forge.rs drives the shared runtime from one function");
    let after = forge_bin[drive..]
        .find("\nfn ")
        .map(|end| &forge_bin[drive..drive + end])
        .unwrap_or(&forge_bin[drive..]);
    assert!(
        after.contains("is_production_writer"),
        "drive_with_shared_runtime must consult the production-writer marker",
    );
    assert!(
        after.contains("Err("),
        "drive_with_shared_runtime must return Err on a silent-drop writer, not drive on",
    );

    // LAYER FIVE: the mutation door holds no NullWriter construction at all — it fails closed like its store.
    let task_bin = source::read(&source::workspace_root().join("forge/src/bin/forge_task.rs"));
    assert!(
        !task_bin.contains("NullWriter"),
        "forge_task.rs must fail closed on writer-absent rather than hold a silent-drop fallback",
    );
}
