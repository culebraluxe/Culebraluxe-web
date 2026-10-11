//! PROPERTY.MEDIA — duplicate chunk (TST-PROPERTY-MEDIA-006).
//!
//! Contract: staging the same chunk index twice REPLACES the stored bytes rather than
//! duplicating the row — the primary key is (upload, index), so a retried chunk is safe.
//! The received-chunk count does not double-count, and the assembled upload carries the
//! latest bytes for that index.
//!
//! Level: L3 Composition — media service boundary.
//! Harness: PropertyHarness.
//! Requires DEV (`DATABASE_URL_DEV`); the harness refuses PROD before any socket.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_media__006__duplicate_chunk

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

fn begin_request(property_id: &str, upload_id: &str) -> BeginMediaUpload {
    BeginMediaUpload {
        upload_id: upload_id.to_string(),
        property_id: property_id.to_string(),
        filename: "duplicate.jpg".to_string(),
        mime_type: "image/jpeg".to_string(),
        byte_size: 24,
        chunk_count: 2,
        chunk_size: 12,
        sha256: None,
        role: "gallery".to_string(),
        alt_text: None,
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-MEDIA-006); the file and the assay use it.
async fn property_media_006__duplicate_chunk() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the duplicate-chunk proof runs only on an isolated DEV target"
    );
    let ctx = harness.test_context();
    let ns = harness.namespace().to_string();
    let marker = format!("TST-PROPERTY-MEDIA006-{ns}");

    let property_id = harness
        .seed_property(&marker, Some("house"))
        .await
        .expect("property seeds");
    let pool = harness.pool();

    let upload_id = uuid::Uuid::new_v4().to_string();
    harness
        .media_service()
        .begin_media_upload(begin_request(&property_id, &upload_id), &ctx)
        .await
        .expect("begin_media_upload runs");
    // The manifest is open; chunks may now be staged.

    // -----------------------------------------------------------------------------------------------------------
    // 1. Positive: staging the same index twice replaces the bytes — one row, latest bytes.
    // -----------------------------------------------------------------------------------------------------------
    let first = b"first-bytes!";
    let second = b"second-bytes";
    assert_eq!(first.len(), 12);
    assert_eq!(second.len(), 12);

    harness
        .media_service()
        .stage_media_chunk(&upload_id, 0, first.to_vec(), &ctx)
        .await
        .expect("first stage of chunk 0");
    let progress = harness
        .media_service()
        .stage_media_chunk(&upload_id, 0, second.to_vec(), &ctx)
        .await
        .expect("second stage of chunk 0 (the duplicate)");
    assert_eq!(
        progress.received_chunks, 1,
        "{HARNESS}: the duplicate does not double-count — one chunk received"
    );

    // Read the committed chunk row back from the pool: exactly one row, with the LATEST bytes.
    let (index, bytes): (i32, Vec<u8>) = sqlx::query_as(
        "select chunk_index, bytes from media_upload_chunk
          where upload_id = $1::uuid and chunk_index = 0",
    )
    .bind(&upload_id)
    .fetch_one(pool)
    .await
    .expect("the chunk row reads back");
    assert_eq!(index, 0, "{HARNESS}: the row is for chunk 0");
    assert_eq!(
        bytes,
        second.as_slice(),
        "{HARNESS}: the stored bytes are the LATEST stage, not the first"
    );

    // Exactly one row exists for (upload, 0) — the PK held.
    let row_count: i64 = sqlx::query_scalar(
        "select count(*) from media_upload_chunk where upload_id = $1::uuid and chunk_index = 0",
    )
    .bind(&upload_id)
    .fetch_one(pool)
    .await
    .expect("the chunk count reads back");
    assert_eq!(
        row_count, 1,
        "{HARNESS}: exactly one row for the duplicated index — no duplicate row"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. Positive: the assembled upload carries the latest bytes for the duplicated index.
    // -----------------------------------------------------------------------------------------------------------
    let chunk1 = b"chunk-one-by";
    harness
        .media_service()
        .stage_media_chunk(&upload_id, 1, chunk1.to_vec(), &ctx)
        .await
        .expect("chunk 1 stages");
    harness
        .media_service()
        .claim_media_upload(&upload_id, &ctx)
        .await
        .expect("claim_media_upload runs");
    // The upload is claimed; only a finisher may assemble it now.
    let assembly = harness
        .media_dao()
        .assemble_media_upload(&upload_id)
        .await
        .expect("assemble_media_upload runs");
    assert_eq!(
        assembly.bytes,
        [second.as_slice(), chunk1.as_slice()].concat(),
        "{HARNESS}: the assembly uses the latest bytes for the duplicated index"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. Negative: a duplicate chunk that would push the upload past its declared size is refused.
    // -----------------------------------------------------------------------------------------------------------
    let oversize_id = uuid::Uuid::new_v4().to_string();
    harness
        .media_service()
        .begin_media_upload(begin_request(&property_id, &oversize_id), &ctx)
        .await
        .expect("begin_media_upload runs");
    // The oversize upload is open; its declared size is 20 bytes.
    // Stage chunk 0 with 12 bytes (already over the declared 20 when combined with chunk 1).
    harness
        .media_service()
        .stage_media_chunk(&oversize_id, 0, b"twelve-bytes!".to_vec(), &ctx)
        .await
        .expect("chunk 0 stages");
    // Re-stage chunk 0 with even more bytes — the running total now exceeds the declared size.
    let err = harness
        .media_service()
        .stage_media_chunk(
            &oversize_id,
            0,
            b"this-is-a-much-larger-chunk".to_vec(),
            &ctx,
        )
        .await
        .expect_err("a chunk that exceeds the declared size must be refused");
    let text = format!("{err}");
    assert!(
        text.contains("declared"),
        "{HARNESS}: the refusal names the declared size: {text}"
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
