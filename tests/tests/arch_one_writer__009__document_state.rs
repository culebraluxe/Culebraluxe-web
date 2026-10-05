//! ARCH.ONE_WRITER — document state (TST-ARCH-ONE-WRITER-009).
//!
//! Contract: `transaction_document.state` has a closed writer set of three.
//! The reconcile path owns the lifecycle steps (ready, sent, signed), the vault
//! binding owns supersede, and the vault transition owns guarded moves
//! (`where id = $1 and state = $5`). Media links (`signed_audit_media_id`,
//! `signed_media_id`) are not state and live elsewhere — a fourth file setting
//! `state` is a second owner of the document's lifecycle.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_one_writer__009__document_state

use test_harness::source;

/// True when a file's text sets the state column on the document table.
fn sets_document_state(text: &str) -> bool {
    let lower = text.to_lowercase();
    if !lower.contains("update transaction_document") {
        return false;
    }
    lower
        .lines()
        .any(|line| {
            let code = source::code_of(line);
            let words: Vec<&str> = code.split(|c: char| !c.is_alphanumeric() && c != '_').collect();
            let mut iter = words.iter().peekable();
            while let Some(word) = iter.next() {
                if word.eq_ignore_ascii_case("set") {
                    if let Some(next) = iter.next() {
                        if next.eq_ignore_ascii_case("state") {
                            return true;
                        }
                    }
                }
            }
            false
        })
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-ONE-WRITER-009); the file and the assay use it.
fn arch_one_writer_009__document_state() {
    let root = source::workspace_root();
    let mut writers: Vec<String> = Vec::new();
    for path in source::sources_under(&root.join("db/src")) {
        let relative = source::relative(&path);
        if relative.contains("/tests/") {
            continue;
        }
        if sets_document_state(&source::read(&path)) {
            writers.push(relative);
        }
    }
    writers.sort();
    assert_eq!(
        writers,
        vec![
            "db/src/signature/reconcile_completed.rs",
            "db/src/vault/bind_form_to_contract.rs",
            "db/src/vault/database.rs",
        ],
        "document state has three writers; a fourth file setting state is a second lifecycle owner"
    );

    // Each writer owns distinct values: lifecycle steps, supersede, and the guarded transition.
    let reconcile = source::read(&root.join("db/src/signature/reconcile_completed.rs"));
    assert!(
        reconcile.contains("set state = 'ready'")
            && reconcile.contains("set state = 'sent'")
            && reconcile.contains("set state = 'signed'"),
        "reconcile owns the ready/sent/signed lifecycle steps"
    );
    let binding = source::read(&root.join("db/src/vault/bind_form_to_contract.rs"));
    assert!(
        binding.contains("set state = 'superseded'"),
        "the vault binding owns supersede and nothing else"
    );
    let vault = source::read(&root.join("db/src/vault/database.rs"));
    assert!(
        vault.contains("and state = $5"),
        "the vault transition moves state only from the expected state"
    );

    // The media links are not state: the signing module links artifacts without joining this set.
    let signing = source::read(&root.join("db/src/document_sign.rs"));
    assert!(
        !sets_document_state(&signing),
        "artifact links must not become a fourth state writer"
    );

    // Negative controls on the detector itself.
    assert!(sets_document_state("update transaction_document set state = 'ready'"));
    assert!(!sets_document_state(
        "update transaction_document set signed_media_id = $2::uuid"
    ));
    assert!(!sets_document_state(
        "select state from transaction_document where id = $1::uuid"
    ));
}
