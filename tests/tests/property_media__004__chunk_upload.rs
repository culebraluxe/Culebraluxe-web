//! PROPERTY.MEDIA — chunk upload (TST-PROPERTY-MEDIA-004).
//!
//! Contract: the chunked upload mechanism. `begin_media_upload` opens a manifest with a
//! declared size and chunk count; `stage_media_chunk` stores each piece (idempotent by
//! (upload, index)); `claim_media_upload` flips the manifest to `complete` so exactly one
//! finisher may assemble. The staged chunks are readable back through the DAO's
//! `assemble_media_upload`, which checks every declared chunk is present and the assembled
//! length matches — WITHOUT writing anything.
//!
//! Level: L3 Composition — media service boundary.
//! Harness: PropertyHarness.
//! Requires DEV (`DATABASE_URL_DEV`); the harness refuses PROD before any socket.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_media__004__chunk_upload

use db::{BeginMediaUpload, DbTarget};
use test_harness::PropertyHarness;

const HARNESS: &str = "PropertyHarness/L3 Composition";

async fn connect_dev() -> PropertyHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match PropertyHarness::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; PropertyHarness refuses PROD: {}",
        last.unwrap_or_default()
    );
}

fn begin_request(
    property_id: &str,
    upload_id: &str,
    byte_size: i64,
    chunk_count: i32,
) -> BeginMediaUpload {
    BeginMediaUpload {
        upload_id: upload_id.to_string(),
        property_id: property_id.to_string(),
        filename: "chunked.jpg".to_string(),
        mime_type: "image/jpeg".to_string(),
        byte_size,
        chunk_count,
        chunk_size: 10,
        sha256: None,
        role: "gallery".to_string(),
        alt_text: None,
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-MEDIA-004); the file and the assay use it.
async fn property_media_004__chunk_upload() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the chunk-upload proof runs only on an isolated DEV target"
    );
    let ctx = harness.test_context();
    let ns = harness.namespace().to_string();
    let marker = format!("TST-PROPERTY-MEDIA004-{ns}");

    let property_id = harness
        .seed_property(&marker, Some("house"))
        .await
        .expect("property seeds");

    // -----------------------------------------------------------------------------------------------------------
    // 1. Positive: begin → stage → claim. The staged chunks assemble to the declared bytes.
    // -----------------------------------------------------------------------------------------------------------
    let upload_id = uuid::Uuid::new_v4().to_string();
    let manifest = harness
        .media_service()
        .begin_media_upload(begin_request(&property_id, &upload_id, 30, 3), &ctx)
        .await
        .expect("begin_media_upload runs");
    assert_eq!(
        manifest.chunk_count, 3,
        "{HARNESS}: the manifest declares 3 chunks"
    );
    assert_eq!(
        manifest.byte_size, 30,
        "{HARNESS}: the manifest declares 30 bytes"
    );

    let chunk0 = b"chunk-zer!"; // 10 bytes
    let chunk1 = b"chunk-one!"; // 10 bytes
    let chunk2 = b"chunk-two!"; // 10 bytes
                                // 10 + 10 + 10 = 30 bytes, matching the declared size.
    assert_eq!(chunk0.len() + chunk1.len() + chunk2.len(), 30);

    let progress = harness
        .media_service()
        .stage_media_chunk(&upload_id, 0, chunk0.to_vec(), &ctx)
        .await
        .expect("chunk 0 stages");
    assert_eq!(progress.received_chunks, 1, "{HARNESS}: one chunk received");
    let progress = harness
        .media_service()
        .stage_media_chunk(&upload_id, 1, chunk1.to_vec(), &ctx)
        .await
        .expect("chunk 1 stages");
    assert_eq!(
        progress.received_chunks, 2,
        "{HARNESS}: two chunks received"
    );
    let progress = harness
        .media_service()
        .stage_media_chunk(&upload_id, 2, chunk2.to_vec(), &ctx)
        .await
        .expect("chunk 2 stages");
    assert_eq!(
        progress.received_chunks, 3,
        "{HARNESS}: three chunks received"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. Negative: a chunk index outside the declared count is refused (while still uploading).
    // -----------------------------------------------------------------------------------------------------------
    let err = harness
        .media_service()
        .stage_media_chunk(&upload_id, 3, b"out of range".to_vec(), &ctx)
        .await
        .expect_err("an out-of-range chunk index must be refused");
    let text = format!("{err}");
    assert!(
        text.contains("outside the declared"),
        "{HARNESS}: the refusal names the declared count: {text}"
    );

    harness
        .media_service()
        .claim_media_upload(&upload_id, &ctx)
        .await
        .expect("claim_media_upload runs");

    // The DAO's assemble reads and checks the staged chunks WITHOUT writing anything.
    let assembly = harness
        .media_dao()
        .assemble_media_upload(&upload_id)
        .await
        .expect("assemble_media_upload runs");
    assert_eq!(
        assembly.bytes,
        [chunk0.as_slice(), chunk1.as_slice(), chunk2.as_slice()].concat(),
        "{HARNESS}: the assembled bytes are the chunks in order"
    );
    assert_eq!(
        assembly.filename, "chunked.jpg",
        "{HARNESS}: the assembly carries the manifest's filename"
    );
    assert_eq!(
        assembly.role, "gallery",
        "{HARNESS}: the assembly carries the manifest's role"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. Positive: begin is idempotent — opening the same upload twice returns the same manifest.
    // -----------------------------------------------------------------------------------------------------------
    let again = harness
        .media_service()
        .begin_media_upload(begin_request(&property_id, &upload_id, 30, 3), &ctx)
        .await
        .expect("re-begin runs");
    assert_eq!(
        again.chunk_count, 3,
        "{HARNESS}: re-begining returns the same manifest"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. Negative: a chunk for an unknown upload is refused.
    // -----------------------------------------------------------------------------------------------------------
    let unknown_id = uuid::Uuid::new_v4().to_string();
    let err = harness
        .media_service()
        .stage_media_chunk(&unknown_id, 0, b"orphan".to_vec(), &ctx)
        .await
        .expect_err("a chunk for an unknown upload must be refused");
    let text = format!("{err}");
    assert!(
        text.contains("unknown or has already been completed"),
        "{HARNESS}: the refusal names the unknown upload: {text}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. Negative: a chunk for an upload that is no longer `uploading` is refused.
    // -----------------------------------------------------------------------------------------------------------
    // The upload was claimed in step 1, so it is `complete` and no longer accepts chunks.
    let err = harness
        .media_service()
        .stage_media_chunk(&upload_id, 0, b"late".to_vec(), &ctx)
        .await
        .expect_err("a chunk for a claimed upload must be refused");
    let text = format!("{err}");
    assert!(
        text.contains("no longer accepts chunks"),
        "{HARNESS}: the refusal names the status: {text}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // Cleanup.
    // -----------------------------------------------------------------------------------------------------------
    let removed = harness.cleanup(&marker).await.expect("cleanup properties");
    assert_eq!(removed, 1, "{HARNESS}: exactly one seeded property removed");
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("leftover count"),
        0
    );
}
