//! MEDIA.OBJECT_CUTOVER — Lane G: Vault ISSUANCE resolves the broker pre-signature object-first
//! (TST-MEDIA-OBJECT-CUTOVER-002).
//!
//! Contract: the last blob dependency in Vault issuance is gone. `VaultDao::issue_from_form_instance`
//! (`db/src/vault/bind_form_to_contract.rs`) now receives the store through
//! `IssuedSignatureAssetReader`. When the deployment owns an object store, issuance resolves Lisa's
//! standing pre-signature through `VaultDao::resolve_applied_signatures_object_first` — the SAME
//! policy, declared-signer check, slot mapping and `ApplyingActor` authority as before, but with the
//! protected asset fetched from a VERIFIED object and checked against its media record; a
//! missing/unverified object is a refusal, never a silent `media.file_data` read. The legacy resolver
//! (`resolve_applied_signatures_in` → `broker_signature::resolve_for_issuance`) runs only in the
//! store-absent (tests/dev) branch.
//!
//! The only `media.file_data` write left in the issuance file is the legacy PDF blob insert for the
//! store-absent branch; the applied-signature path never selects it.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test media_object_cutover__002__lane_g_issuance_applied_signatures

use test_harness::source;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-MEDIA-OBJECT-CUTOVER-002); the file and the assay use it.
fn media_object_cutover_002__lane_g_issuance_applied_signatures() {
    let root = source::workspace_root();
    let issue = source::read(&root.join("db/src/vault/bind_form_to_contract.rs"));
    let vault = source::read(&root.join("web/src/vault/mod.rs"));

    // 1. THE SEAM: issuance takes the store-reader port, and the store-backed branch is the OBJECT-FIRST
    //    resolver. Source order is the branch order: the object-first call must precede the legacy one.
    assert!(
        issue.contains("read_protected_asset: Option<R>"),
        "issuance must accept the object reader, so a store-backed deployment never needs the blob"
    );
    let object_first = issue
        .find("resolve_applied_signatures_object_first(")
        .expect("issuance must call the object-first resolver");
    let legacy = issue
        .find(".resolve_applied_signatures_in(")
        .expect("the legacy resolver must remain as the store-absent fallback");
    assert!(
        object_first < legacy,
        "the store-backed branch must be resolved object-first"
    );
    assert!(
        issue.contains("Some(read_object) =>"),
        "a lent store selects the object-first branch"
    );
    let object_branch = &issue[object_first..object_first + 360];
    assert!(
        object_branch.contains("SignatureAuthority::ApplyingActor"),
        "the object-first issuance keeps the ApplyingActor authority (a refusal fails the document)"
    );
    assert!(
        object_branch.contains("read_object,"),
        "the object-first branch hands its store reader to the resolver"
    );
    assert!(
        issue.matches("SignatureAuthority::ApplyingActor").count() >= 2,
        "BOTH branches resolve under the issuance authority, object-first or legacy"
    );
    // The legacy branch is the one that runs inside the transaction's own connection.
    assert!(
        issue.contains(".resolve_applied_signatures_in(\n                        tx.connection(),"),
        "the legacy branch keeps the transaction connection (no store => legacy read)"
    );

    // 2. NO SIGNATURE BLOB READ ON THE OBJECT PATH. The issuance file never names the blob loader and never
    //    selects file_data; the ONE remaining file_data reference is the legacy PDF-write blob branch.
    assert!(
        !issue.contains("load_protected_asset("),
        "issuance must resolve the asset through the resolver, not load the blob itself"
    );
    assert!(
        !issue.contains("select file_data"),
        "issuance must not select media.file_data for the signature asset"
    );
    assert_eq!(
        issue.matches("insert into media (file_data").count(),
        1,
        "the only blob write left is the legacy PDF insert for the store-absent branch"
    );

    // 3. THE WEB ADAPTER: the service lends ITS store, and only when one is configured does the DAO take the
    //    object-first path. The reader is a verified-object `get_object` read, with no blob fallback.
    assert!(
        vault.contains("pub trait IssuedSignatureAssetReader"),
        "the store port issuance needs is declared on the web side"
    );
    assert!(
        vault.contains("ObjectStoreSignatureAssetReader::new(self.object_store.clone())"),
        "issuance lends the SAME configured store that writes the PDF"
    );
    // `then_some` (not a bare `then`) keeps the selection eager and side-effect free: the closure only
    // captures the store handle it would read through.
    assert!(
        vault.contains("object_store_configured()") && vault.contains(".then_some("),
        "only a configured store selects object-first; no store keeps the legacy read"
    );
    assert!(
        vault.contains("async fn read_object(&self, bucket: &str, key: &str)"),
        "the reader is the object-bytes read the resolver needs"
    );
    assert!(
        vault.contains(".get_object(bucket, key)"),
        "the reader fetches the protected asset from object storage"
    );
    // The adapter maps the DB bucket string to the logical bucket; an unknown domain is a refusal.
    assert!(
        vault.contains("unknown object bucket: {bucket}"),
        "a bucket the store does not know is refused, never guessed"
    );
}
