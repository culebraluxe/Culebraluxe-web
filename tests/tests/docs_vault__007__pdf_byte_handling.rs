//! DOCS.VAULT-007 — PDF byte handling.
//!
//! CONTRACT. The Vault's completion certificate is real PDF bytes handled exactly:
//! `render_completion_certificate` renders bytes that open with the `%PDF-1.4` magic and
//! close with `%%EOF`, deterministically — identical envelopes render identical bytes, so a
//! future cryptography layer can seal them. Different envelopes render different bytes, so
//! the bytes actually carry the record. And the byte contract is checkable: input that is
//! not a PDF fails the magic, which is what makes the positive assertions meaningful.
//!
//! Level: L3 Composition — the production Vault PDF boundary (`web::vault`), no DB, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test docs_vault__007__pdf_byte_handling

use db::{FinalizeEvent, FinalizeField, FinalizeRecipient};
use web::vault::pdf::escape_text;
use web::vault::signing_certificate::{render_completion_certificate, Certificate};

fn recipient(name: &str) -> FinalizeRecipient {
    FinalizeRecipient {
        id: "recipient-1".to_string(),
        name: name.to_string(),
        email: "signer@example.com".to_string(),
        role: "seller".to_string(),
        signer_order: 1,
        signing_step: 1,
        state: "signed".to_string(),
        consent_version: None,
        consent_sha256: None,
        consent_accepted_at: None,
        completed_at: None,
    }
}

fn render(name: &str) -> Vec<u8> {
    let recipients = [recipient(name)];
    let fields = [FinalizeField {
        field_key: "signature-1".to_string(),
        field_type: "signature".to_string(),
        label: None,
        page_number: 1,
        recipient_id: "recipient-1".to_string(),
        required: true,
        position_x: 100.0,
        position_y: 200.0,
        width: 150.0,
        height: 24.0,
        value: None,
        completed_at: Some("2026-10-01T12:00:00Z".to_string()),
    }];
    let events = [FinalizeEvent {
        event_type: "signed".to_string(),
        occurred_at: "2026-10-01T12:00:00Z".to_string(),
        actor_id: Some("recipient-1".to_string()),
        recipient_id: Some("recipient-1".to_string()),
        evidence: serde_json::json!({}),
    }];
    render_completion_certificate(&Certificate {
        signature_request_id: "request-1",
        transaction_document_id: "document-1",
        document_title: None,
        recipients: &recipients,
        fields: &fields,
        events: &events,
        finalized_at: "2026-10-01T12:00:00Z",
        original_sha256: None,
        sealed_sha256: None,
        page_count: None,
    })
    .expect("the certificate renders")
}

/// The byte contract: a structurally plausible PDF document.
fn is_pdf_document(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF-1.4") && bytes.ends_with(b"%%EOF\n")
}

#[test]
fn docs_vault_007__pdf_byte_handling() {
    // ── 1. THE BYTES ARE A REAL PDF DOCUMENT. ────────────────────────────────
    let first = render("Ada Example");
    assert!(
        !first.is_empty(),
        "the certificate must render bytes, not an empty body"
    );
    assert!(
        is_pdf_document(&first),
        "the bytes must open with the PDF magic and close with %%EOF (got {} bytes starting {:?})",
        first.len(),
        &first[..first.len().min(16)]
    );

    // ── 2. IDENTICAL ENVELOPES RENDER IDENTICAL BYTES. ───────────────────────
    let second = render("Ada Example");
    assert_eq!(
        first, second,
        "identical inputs must render identical bytes — the timestamp travels in, never now()"
    );

    // ── 3. THE BYTES CARRY THE RECORD: DIFFERENT INPUTS, DIFFERENT BYTES. ────
    let other = render("Bob Other");
    assert_ne!(
        first, other,
        "a different signer must render different bytes, or the bytes carry nothing"
    );
    assert!(
        is_pdf_document(&other),
        "the differing render must still be a well-formed document"
    );

    // ── 4. TEXT BYTES ARE ESCAPED, NEVER RAW. ────────────────────────────────
    assert_eq!(
        escape_text("a(b)c\\d"),
        "a\\(b\\)c\\\\d",
        "parentheses and backslashes must be escaped for PDF literal objects"
    );

    // ── 5. NEGATIVE: NON-PDF BYTES FAIL THE CONTRACT. ────────────────────────
    assert!(
        !is_pdf_document(b"not a pdf at all"),
        "the check must be able to fail — otherwise the positive assertions prove nothing"
    );
    let mut tampered = first.clone();
    tampered[0] = b'X';
    assert!(
        !is_pdf_document(&tampered),
        "a corrupted magic must fail the byte contract"
    );
    let truncated = &first[..first.len() - 1];
    assert!(
        !is_pdf_document(truncated),
        "a truncated trailer must fail the byte contract"
    );
}
