//! ARCH.ONE_WRITER — signature request state (TST-ARCH-ONE-WRITER-008).
//!
//! Contract: the canonical `luxesign_request.status` moves by compare-and-swap
//! only. Every production `UPDATE` of that column carries an expected-status
//! guard (`and status = $N`), so two lanes racing the same envelope cannot both
//! advance it: the loser matches zero rows instead of overwriting the winner.
//! The guarded swap lives in `db/src/signature/database.rs::set_status_tx`
//! (expected, target); the reconcile path in
//! `db/src/signature/reconcile_completed.rs` plays by the same rule.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_one_writer__008__signature_request_state

use test_harness::source;

/// True when a code line updates the canonical status column (whole-word `update`,
/// so `updated` and `updated_at` do not count).
fn updates_status(line: &str) -> bool {
    let code = source::code_of(line).to_lowercase();
    source::contains_word(&code, "update") && code.contains("luxesign_request")
}

/// True when a file guards its status update with an expected-status predicate.
fn has_status_guard(text: &str) -> bool {
    text.lines().any(|line| {
        let code = source::code_of(line).to_lowercase();
        code.contains("and status = $")
    })
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-ONE-WRITER-008); the file and the assay use it.
fn arch_one_writer_008__signature_request_state() {
    let root = source::workspace_root();
    let mut updaters: Vec<String> = Vec::new();
    // Collect every non-test production file that updates the canonical status.
    for dir in ["db/src", "web/src"] {
        for path in source::sources_under(&root.join(dir)) {
            let relative = source::relative(&path);
            if relative.contains("/tests/") {
                continue;
            }
            if source::read(&path).lines().any(updates_status) {
                updaters.push(relative);
            }
        }
    }
    updaters.sort();
    assert_eq!(
        updaters,
        vec![
            "db/src/signature/database.rs",
            "db/src/signature/reconcile_completed.rs",
        ],
        "the canonical status has a closed updater set; a third writer is a race"
    );

    // Every updater guards on the expected status: no blind writes.
    for relative in &updaters {
        let text = source::read(&root.join(relative));
        assert!(
            has_status_guard(&text),
            "{relative} updates luxesign_request without an expected-status guard"
        );
    }

    // The canonical swap names both sides: expected in, target out.
    let database = source::read(&root.join("db/src/signature/database.rs"));
    assert!(
        database.contains("pub async fn set_status_tx") && database.contains("expected:"),
        "the status swap takes the expected status it guards on"
    );
    let service = source::read(&root.join("web/src/signature/mod.rs"));
    assert!(
        service.contains("pub async fn transition_transactional"),
        "the service drives status through the canonical transition, not raw SQL"
    );

    // Negative controls: the detector fires on an update and stays quiet on reads and prose.
    assert!(updates_status(
        "update luxesign_request set status = $3 where id = $1::uuid"
    ));
    assert!(!updates_status(
        "select status from luxesign_request where id = $1::uuid"
    ));
    assert!(!updates_status(
        "// the service updates luxesign_request for us"
    ));
    assert!(!updates_status("luxesign_request: updated,"));
}
