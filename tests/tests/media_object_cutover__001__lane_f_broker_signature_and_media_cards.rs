//! MEDIA.OBJECT_CUTOVER — Lane F: the broker-signature protected asset and the CLI media-cards backfill are
//! object-first (TST-MEDIA-OBJECT-CUTOVER-001).
//!
//! Contract: the two remaining blob dependencies outside Vault/property media are removed from the CODE.
//!
//!   - `db/src/broker_signature.rs` reads the protected signature image from a VERIFIED Vault object and
//!     verifies its SHA-256 and size; the legacy `media.file_data` read survives only in the store-absent
//!     fallback, and the object-first write helper records the asset without a blob column.
//!   - `web/src/vault/applied_signatures.rs` resolves the preview through the object-first resolver, keeps
//!     `authorize` first, and requires a configured store — a deployment without one is a typed configuration
//!     error, and a legacy row on a store-backed deployment is a refusal, not a silent blob. (Lane J removed
//!     the last repository-blob fallback; this file pins the preview's object-first call, not a fallback.)
//!   - `cli/src/main.rs media-cards` reads `original_object_location` + `get_object`, verifies the original,
//!     uploads/verifies the card object, and records it through `insert_copy_object`; it never names the blob
//!     readers/writers.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test media_object_cutover__001__lane_f_broker_signature_and_media_cards

use test_harness::source;

/// The body of one top-level function: from its signature to the first column-zero closing brace.
///
/// The pinned functions all close at column zero, so this is the smallest honest bound; a body that ran on into the
/// next function would make a presence assertion pass for the wrong reason, hence the planted control below.
/// The body of a method inside an `impl` block: from its signature to the first four-space closing brace.
fn method_body<'a>(text: &'a str, signature: &str) -> &'a str {
    let start = text
        .find(signature)
        .unwrap_or_else(|| panic!("`{signature}` must be present in the source"));
    let tail = &text[start..];
    let end = tail.find("\n    }\n").map_or(tail.len(), |index| index + 6);
    &tail[..end]
}

fn body_of<'a>(text: &'a str, signature: &str) -> &'a str {
    let start = text
        .find(signature)
        .unwrap_or_else(|| panic!("`{signature}` must be present in the source"));
    let tail = &text[start..];
    let end = tail.find("\n}\n").map_or(tail.len(), |index| index + 3);
    &tail[..end]
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-MEDIA-OBJECT-CUTOVER-001); the file and the assay use it.
fn media_object_cutover_001__lane_f_broker_signature_and_media_cards() {
    let root = source::workspace_root();
    // 0. The walkers found a tree; every read below is silent about a missing path, so the floor comes first.
    let files = source::sources_under(&root);
    assert!(
        files.len() >= 200,
        "the workspace sources must be walked; {} files found",
        files.len()
    );

    // NEGATIVE CONTROL: the extractor clips at the first column-zero brace, so it cannot read the next function's body.
    let planted = "fn first() {\n    one();\n}\n\nfn second() {\n    two();\n}\n";
    let first = body_of(planted, "fn first(");
    assert!(first.contains("one()") && !first.contains("two()"));

    // 1. THE BROKER SIGNATURE ASSET: object-first read, verified; legacy blob read confined to the fallback.
    let broker = source::read(&root.join("db/src/broker_signature.rs"));

    let object_start = broker
        .find("async fn load_protected_asset_object(")
        .expect("the object-first asset loader is the path a store-backed deployment takes");
    let legacy_start = broker
        .find("async fn load_legacy_protected_asset(")
        .expect("the legacy fallback exists only for deployments with no store");
    assert!(
        object_start < legacy_start,
        "the object-first loader is defined before the fallback it is preferred over"
    );
    // Stop before the legacy function's own doc comment: its prose names the blob column it explains.
    let legacy_doc = broker
        .find("/// LEGACY BLOB READ")
        .expect("the legacy fallback carries its own doc comment");
    assert!(object_start < legacy_doc);
    let object_path: String = broker[object_start..legacy_doc]
        .lines()
        .map(source::code_of)
        .collect::<Vec<_>>()
        .join("\n");
    for required in [
        "object_bucket",
        "object_key",
        "object_sha256",
        "object_size",
        "storage_status",
        "visibility",
        "and media_type = 'image'",
    ] {
        assert!(
            object_path.contains(required),
            "the object-first asset read must carry `{required}`"
        );
    }
    assert!(
        !object_path.contains("file_data"),
        "the object-first asset read must never select media.file_data"
    );
    for required in ["reader.read(", "verify_protected_asset("] {
        assert!(
            object_path.contains(required),
            "the object-first asset read must go through `{required}`"
        );
    }

    let legacy = body_of(&broker, "async fn load_legacy_protected_asset(");
    assert!(
        legacy.contains("file_data"),
        "the legacy fallback is the ONE place the blob may still be read"
    );
    assert!(
        legacy.contains("media_type = 'image'"),
        "the fallback keeps the image-type restriction"
    );

    // The verified-object contract: size AND recorded SHA-256, plus the vault/vault pointer.
    let verify = body_of(&broker, "fn verify_protected_asset(");
    for required in [
        "object.status != \"VERIFIED\"",
        "(\"vault\", \"vault\") | (\"property_media_private\", \"private\")",
        "must never be public",
        "bytes.len() as i64 != object.size",
        "asset object has no recorded SHA-256",
        "eq_ignore_ascii_case",
        "recorded_caption_checksum",
    ] {
        assert!(
            verify.contains(required),
            "the asset verifier must keep `{required}`"
        );
    }

    // The object-first write: object columns + the canonical purpose, no blob column.
    let writer = method_body(&broker, "pub async fn insert_protected_signature_object(");
    for required in [
        "object_bucket",
        "object_key",
        "object_sha256",
        "object_size",
        "storage_status",
        "DEFAULT_BROKER_SIGNATURE_PURPOSE",
    ] {
        assert!(
            writer.contains(required),
            "the protected-signature writer must record `{required}`"
        );
    }
    assert!(
        !writer.contains("file_data"),
        "the protected-signature writer must not write media.file_data"
    );
    assert!(
        broker.contains("fn validate_protected_signature_object("),
        "the object-first writer validates its storage domain before SQL"
    );
    assert!(
        broker.contains("pub async fn resolve_applied_signatures_object_first<"),
        "the preview's object-first resolver is the entry the web service calls"
    );

    // 2. THE WEB PREVIEW: authorization first, object-first when a store exists, legacy only as the no-store branch.
    let preview = source::read(&root.join("web/src/vault/applied_signatures.rs"));
    let authorize_at = preview
        .find("authorize(")
        .expect("the preview keeps its authorization check");
    let object_first_at = preview
        .find("resolve_applied_signatures_object_first")
        .expect("the preview calls the object-first resolver");
    assert!(
        authorize_at < object_first_at,
        "authorization must run before any asset resolution"
    );
    assert!(
        preview.contains("requires object storage"),
        "the preview requires a configured store (typed configuration error without one)"
    );
    assert!(
        preview.contains("OwnerByConstruction"),
        "a preview applies nothing, so it resolves under OwnerByConstruction"
    );
    assert!(
        !preview.contains("self.repository.resolve_applied_signatures"),
        "the repository (legacy) resolver is gone: the preview is object-only now"
    );
    assert!(
        preview.contains("get_object(bucket, &key)"),
        "the preview's loader reads the object through the store port"
    );

    // 3. THE CLI BACKFILL: object adapters only, no blob reader/writer named.
    let cli = source::read(&root.join("cli/src/main.rs"));
    let cli_body = body_of(&cli, "async fn media_cards(");
    let cli_helper = body_of(&cli, "async fn verify_original_object(");
    let cli_object_path = format!("{cli_body}\n{cli_helper}");
    for required in [
        "original_object_location",
        "get_object",
        "head_object",
        "verify_object",
        "put_object",
        "insert_copy_object",
        "MediaObjectMetadata",
        "VERIFIED",
        "MEDIA_OBJECT_STORE",
    ] {
        assert!(
            cli_object_path.contains(required),
            "media-cards must use the object-first adapter `{required}`"
        );
    }
    // Blob API *usage*, not the prose that explains its absence: a call or SQL reference, never a message string.
    for forbidden in [
        "original_bytes(",
        "insert_copy(",
        "select file_data",
        "insert into media (file_data",
    ] {
        assert!(
            !cli_object_path.contains(forbidden),
            "media-cards must not name the blob API `{forbidden}`"
        );
    }
    assert!(
        !cli.contains("MediaDao::original_bytes"),
        "nothing in the CLI reaches the legacy blob reader"
    );
    assert!(
        cli_body.contains("delete_object"),
        "a card object whose DB row could not be written is removed, not orphaned"
    );
}
