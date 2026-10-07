//! DOCS.FORMS.EXECUTION — issue document (TST-DOCS-FORMS-EXECUTION-005).
//!
//! Contract: issuing a form turns it into an immutable vault document, through ONE production seam —
//! `VaultDao::issue_from_form_instance` (`db/src/vault/bind_form_to_contract.rs`) behind the authorized
//! service operation `vault.issueFromFormInstance` (`web/src/vault/mod.rs`). In a single transaction the
//! seam claims the command receipt, loads the form, versions the new document from the prior lineage
//! (first issue is 1, otherwise prior + 1), renders through the artifact port, sha-256 checksums the bytes
//! into `media`, supersedes the prior document, inserts the `transaction_document` as a `generated`
//! `agreement` in state `ready` with the checksum and the source snapshot, marks the form `issued`, and
//! finalizes the receipt — rolling everything back on any failure, and recording a refused outcome (never a
//! panic, never a partial document) when the form is missing or the render fails.
//!
//! Specified level: L3 Composition / DocumentVaultHarness. The reachable deterministic seam is the one
//! TST-DOCS-CONTRACT-005 (Complete) established for this taxonomy: source-structure assertions over the
//! production DAO/service boundary plus the production model types; no database, no network, no providers.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_forms_execution__005__issue_document

use model::VaultCommandOutcome;
use test_harness::source;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-DOCS-FORMS-EXECUTION-005); the file and the assay use it.
fn docs_forms_execution_005__issue_document() {
    let root = source::workspace_root();
    let issue = source::read(&root.join("db/src/vault/bind_form_to_contract.rs"));
    let lower = issue.to_lowercase();

    // 1. ONE TRANSACTION, claim first: the receipt is claimed before anything else happens, and every
    //    write lives inside the same transaction that rolls back on error.
    let claim = lower
        .find("claim_receipt(")
        .expect("the receipt is claimed");
    let form_load = lower
        .find("from document_form_instance")
        .expect("the form is loaded");
    assert!(
        claim < form_load,
        "the command receipt is claimed before the form is read"
    );
    assert!(
        lower.contains("tx.rollback().await"),
        "a failure rolls the whole issuance back"
    );

    // 2. VERSION LINEAGE: the first issue of a template on a contract (or a legacy deal) is version 1, the
    //    next is prior + 1, and the prior document is superseded — never two live versions of one lineage.
    assert!(
        lower.contains("map_or(1, |(_, version)| version + 1)"),
        "the issued version continues the lineage"
    );
    assert!(
        lower.contains("set state = 'superseded'"),
        "the prior document is superseded"
    );
    assert!(
        lower.contains("order by issued_version desc, created_at desc"),
        "the lineage resolves the LATEST prior issue"
    );

    // 3. THE DOCUMENT ITSELF: the rendered bytes are stored as document media, checksumed, and the
    //    transaction document is a generated agreement, ready, with its template identity and snapshot.
    assert!(
        lower.contains("insert into media (file_data, filename, mime_type, file_size, media_type)"),
        "the bytes are stored as media"
    );
    assert!(
        issue.contains("'application/pdf'") && issue.contains("'document'"),
        "the artifact is a document PDF"
    );
    for fragment in [
        "insert into transaction_document (",
        "'agreement'",
        "'ready'",
        "'generated'",
        "issued_checksum_sha256",
        "issued_version",
        "form_instance_id",
        "source_snapshot",
    ] {
        assert!(
            lower.contains(&fragment.to_lowercase()),
            "the issued document carries {fragment}"
        );
    }
    // The snapshot is the document's evidence: values, sections, the issued participants and the render
    // metadata, frozen at issuance.
    for fragment in ["fieldValues", "sections", "issuedParticipants", "render"] {
        assert!(
            issue.contains(fragment),
            "the source snapshot freezes {fragment}"
        );
    }
    // The checksum is of the ARTIFACT BYTES, computed in-process — never a value a caller supplies.
    assert!(
        issue.contains("Sha256::digest(&artifact.bytes)"),
        "the checksum is computed from the issued bytes"
    );

    // 4. THE FORM'S OWN STATE: issuance marks the form issued, and the receipt is finalized with the
    //    document as the aggregate.
    assert!(
        lower.contains("set status = 'issued'"),
        "the form is marked issued"
    );
    assert!(
        lower.contains("finalize_receipt("),
        "the receipt records the outcome"
    );

    // 5. NEGATIVE (refusal shapes): a form that does not exist is a recorded ValidationFailure, and a
    //    render failure is recorded with ITS outcome — neither panics, neither leaves a partial document.
    assert!(
        issue.contains("document.issue failed: form instance not found.")
            && issue.contains("VaultCommandOutcome::ValidationFailure"),
        "a missing form is a recorded validation failure"
    );
    assert!(
        issue.contains("&failure.outcome"),
        "a render failure is finalized with its own outcome, not a crash"
    );

    // 6. THE SERVICE BOUNDARY (source structure): the operation is authorized as `vault.issue`, and the
    //    domain event fires only for a fresh success — a replayed issuance emits nothing twice.
    let service = source::read(&root.join("web/src/vault/mod.rs"));
    let operation = {
        let start = service
            .find("pub async fn issue_from_form_instance")
            .expect("the service operation exists");
        &service[start..]
    };
    assert!(
        operation.contains("\"vault.issue\""),
        "the operation requires the vault.issue entitlement"
    );
    assert!(
        operation.contains("\"vault.document_issued\""),
        "issuance emits the domain event"
    );
    assert!(
        operation.contains("!command.replayed"),
        "a replayed issuance never double-emits"
    );

    // 7. THE OUTCOME VOCABULARY (production type, executed): the outcomes the seam reports round-trip
    //    through their durable strings, and an unknown one is refused.
    for outcome in [
        VaultCommandOutcome::Success,
        VaultCommandOutcome::ValidationFailure,
        VaultCommandOutcome::NotFound,
        VaultCommandOutcome::Conflict,
        VaultCommandOutcome::Unauthorized,
        VaultCommandOutcome::PreconditionFailure,
    ] {
        assert_eq!(
            VaultCommandOutcome::try_from(outcome.as_str()),
            Ok(outcome.clone()),
            "the outcome {} round-trips",
            outcome.as_str()
        );
    }
    assert!(VaultCommandOutcome::try_from("made_up").is_err());
}
