//! MEDIA.OBJECT_CUTOVER — Lane J: the Vault is OBJECT-ONLY (TST-MEDIA-OBJECT-CUTOVER-002).
//!
//! Contract: the Vault has no legacy blob path left. `db/src/vault` never names `media.file_data`; the
//! private, guest, signing, signed-copy and completion doors resolve the exact same entitlement SQL over
//! storage pointers and fetch bytes from the object store; and Vault issuance requires a store — the
//! rendered PDF is uploaded before its media row is written and the broker's standing pre-signature is
//! read through the store, with no store-absent blob branch and no `MEDIA_OBJECT_STORAGE_READS` switch in
//! the Vault.
//!
//! A deployment without a store answers the typed `VAULT_OBJECT_STORE_UNAVAILABLE` configuration error
//! rather than serving a blob. (The composition root still calls `with_object_reads` for signature
//! compatibility; the Vault service accepts and ignores it — every byte it serves already comes from the
//! store.)
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test media_object_cutover__002__lane_g_issuance_applied_signatures

use test_harness::source;

/// The body of one method inside an `impl` block (the same smallest honest bound the other pins use).
fn method_body<'a>(text: &'a str, signature: &str) -> &'a str {
    let start = text
        .find(signature)
        .unwrap_or_else(|| panic!("`{signature}` must be present in the source"));
    let tail = &text[start..];
    let end = tail.find("\n    }\n").map_or(tail.len(), |index| index + 6);
    &tail[..end]
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-MEDIA-OBJECT-CUTOVER-002); the file and the assay use it.
fn media_object_cutover_002__lane_g_issuance_applied_signatures() {
    let root = source::workspace_root();
    let issue = source::read(&root.join("db/src/vault/bind_form_to_contract.rs"));
    let dao = source::read(&root.join("db/src/vault/database.rs"));
    let vault = source::read(&root.join("web/src/vault/mod.rs"));
    let preview = source::read(&root.join("web/src/vault/applied_signatures.rs"));

    // 1. ZERO BLOB REFERENCES IN THE VAULT. The module owns entitlement SQL and storage pointers only.
    for (name, text) in [
        ("db/src/vault/database.rs", &dao),
        ("db/src/vault/bind_form_to_contract.rs", &issue),
        ("web/src/vault/mod.rs", &vault),
    ] {
        assert!(
            !text.to_lowercase().contains("file_data"),
            "{name} must not name the legacy media blob column, even in prose or a predicate"
        );
    }

    // 2. THE ISSUANCE SEAM REQUIRES THE STORE. The PDF is uploaded before its row, and the applied signature
    //    is resolved object-first — no Option, no legacy resolver on the transaction connection.
    assert!(
        issue.contains("read_protected_asset: R,"),
        "issuance requires the store reader; a deployment without a store is refused before this seam"
    );
    assert!(
        issue.contains("resolve_applied_signatures_object_first("),
        "issuance resolves the broker pre-signature through the object-first resolver"
    );
    assert!(
        !issue.contains("resolve_applied_signatures_in(")
            && !issue.contains("resolve_for_issuance"),
        "issuance must not reach the legacy blob resolver"
    );
    assert!(
        issue.contains("insert_media_object_with_id_tx"),
        "the issued PDF is persisted as an object-first media row"
    );
    assert!(
        !issue.contains("insert into media (file_data"),
        "the issued PDF must never be written as a media blob"
    );
    let upload_at = issue
        .find("upload(&media_id, &artifact.filename, &artifact.bytes)")
        .expect("the PDF is uploaded");
    let row_at = issue
        .find("insert_media_object_with_id_tx")
        .expect("the object row is written");
    assert!(
        upload_at < row_at,
        "the object must be uploaded and verified BEFORE the media row that names it"
    );

    // 3. THE DB READERS ARE OBJECT-ONLY. Every door's reader selects the pointer, and the SQL guards live on
    //    the object methods — the byte twins are gone.
    for method in [
        "media_document_object",
        "public_listing_document_object",
        "signing_document_object",
        "signing_signed_object",
        "completion_artifact_objects",
    ] {
        let body = source::impl_body(&dao, "VaultDao", method);
        assert!(
            body.contains("object_bucket is not null") && body.contains("object_key is not null"),
            "{method} must serve only object-backed rows"
        );
        assert!(
            body.to_lowercase().contains("media_type = 'document'"),
            "{method} must serve documents only"
        );
    }
    for gone in [
        "pub async fn media_bytes",
        "pub async fn media_object_location",
        "pub async fn public_listing_document_bytes",
        "pub async fn signing_document_bytes",
        "pub async fn signing_signed_bytes",
        "pub async fn completion_artifacts",
    ] {
        assert!(
            !dao.contains(gone),
            "the legacy blob reader `{gone}` must be gone from the Vault DAO"
        );
    }

    // 4. THE WEB DOORS ARE OBJECT-ONLY AND REQUIRE A STORE. The migration switch is gone from the Vault; a
    //    deployment without a store gets the typed configuration error.
    assert!(
        vault.contains("fn vault_store(&self)") && vault.contains("VAULT_OBJECT_STORE_UNAVAILABLE"),
        "Vault byte paths require the store and answer a typed configuration error without one"
    );
    assert!(
        !vault.contains("MEDIA_OBJECT_STORAGE_READS"),
        "the Vault must not consult MEDIA_OBJECT_STORAGE_READS"
    );
    assert!(
        !vault.contains("object_reads:"),
        "the Vault service has no legacy-read switch left; `with_object_reads` is a compatibility no-op"
    );
    let doors = [
        (
            "media_bytes",
            method_body(&vault, "pub async fn media_bytes("),
        ),
        (
            "media_read",
            method_body(&vault, "pub async fn media_read("),
        ),
        (
            "public_listing_document_bytes",
            method_body(&vault, "pub async fn public_listing_document_bytes("),
        ),
        (
            "public_listing_document_read",
            method_body(&vault, "pub async fn public_listing_document_read("),
        ),
        (
            "signing_document_bytes",
            method_body(&vault, "pub async fn signing_document_bytes("),
        ),
        (
            "signing_document_read",
            method_body(&vault, "pub async fn signing_document_read("),
        ),
        (
            "signing_signed_bytes",
            method_body(&vault, "pub async fn signing_signed_bytes("),
        ),
        (
            "signing_signed_read",
            method_body(&vault, "pub async fn signing_signed_read("),
        ),
        (
            "completion_artifacts",
            method_body(&vault, "pub async fn completion_artifacts("),
        ),
    ];
    for (name, body) in doors {
        assert!(
            body.contains("vault_store()"),
            "`{name}` must require the object store (typed configuration error without one)"
        );
        assert!(
            !body.contains("repository.media_bytes")
                && !body.contains("repository.media_object_location"),
            "`{name}` must not fall back to a repository blob reader"
        );
    }
    // The writer and asset reader are bound to a required store, not an Option.
    assert!(
        vault.contains("pub fn new(store: Arc<dyn AbstractObjectStoreService>)"),
        "both object ports are constructed from a required store"
    );
    assert!(
        !vault.contains("object_store_configured"),
        "there is no store-present/absent branch left in the Vault"
    );
    assert!(
        vault.contains("get_object(bucket, key)"),
        "completion attachments and previews read the object through the store"
    );

    // 5. THE PREVIEW is object-only too: a configured store and database are required, and the legacy
    //    repository resolver is not a fallback.
    assert!(
        preview.contains("VAULT_OBJECT_STORE_UNAVAILABLE"),
        "the preview refuses with the typed configuration error without a store"
    );
    assert!(
        preview.contains("resolve_applied_signatures_object_first"),
        "the preview resolves the protected asset object-first"
    );
    assert!(
        !preview.contains("self.repository.resolve_applied_signatures"),
        "the preview must not fall back to the legacy repository resolver"
    );
}
