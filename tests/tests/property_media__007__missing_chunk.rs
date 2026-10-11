//! PROPERTY.MEDIA — missing chunk (TST-PROPERTY-MEDIA-007).
//!
//! Contract: an incomplete upload cannot be finalized. `assemble_media_upload` checks
//! that every declared chunk is present before it returns anything — a missing chunk
//! means the upload is refused, and a partial upload is never storable as a photograph.
//!
//! Level: L3 Composition — media service boundary.
//! Harness: PropertyHarness.
//! Requires DEV (`DATABASE_URL_DEV`); the harness refuses PROD before any socket.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_media__007__missing_chunk

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

fn begin_request(property_id: &str, upload_id: &str, chunk_count: i32) -> BeginMediaUpload {
    begin_request_with_size(property_id, upload_id, chunk_count, 47)
}

fn begin_request_with_size(
    property_id: &str,
    upload_id: &str,
    chunk_count: i32,
    byte_size: i64,
) -> BeginMediaUpload {
    BeginMediaUpload {
        upload_id: upload_id.to_string(),
        property_id: property_id.to_string(),
        filename: "missing.jpg".to_string(),
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-MEDIA-007); the file and the assay use it.
async fn property_media_007__missing_chunk() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the missing-chunk proof runs only on an isolated DEV target"
    );
    let ctx = harness.test_context();
    let ns = harness.namespace().to_string();
    let marker = format!("TST-PROPERTY-MEDIA007-{ns}");

    let property_id = harness
        .seed_property(&marker, Some("house"))
        .await
        .expect("property seeds");

    // -----------------------------------------------------------------------------------------------------------
    // 1. Negative: an upload with a missing chunk cannot be assembled — the incomplete
    //    upload is refused, not stored as a shorter photograph.
    // -----------------------------------------------------------------------------------------------------------
    let upload_id = uuid::Uuid::new_v4().to_string();
    let manifest = harness
        .media_service()
        .begin_media_upload(begin_request(&property_id, &upload_id, 3), &ctx)
        .await
        .expect("begin_media_upload runs");
    assert_eq!(
        manifest.chunk_count, 3,
        "{HARNESS}: the manifest declares 3 chunks"
    );

    // Stage chunks 0 and 1 — chunk 2 never arrives.
    harness
        .media_service()
        .stage_media_chunk(&upload_id, 0, b"chunk-zero-bytes".to_vec(), &ctx)
        .await
        .expect("chunk 0 stages");
    harness
        .media_service()
        .stage_media_chunk(&upload_id, 1, b"chunk-one-bytes!".to_vec(), &ctx)
        .await
        .expect("chunk 1 stages");

    // The upload is still uploading (not claimed), so assemble is not yet possible.
    // complete_media_upload will claim it, then finish_media_upload will catch the missing chunk.
    let err = harness
        .media_service()
        .complete_media_upload(&upload_id, &ctx)
        .await
        .expect_err("an incomplete upload must not finalize");
    let text = format!("{err}");
    assert!(
        text.contains("incomplete"),
        "{HARNESS}: the finalize refusal names the incomplete upload: {text}"
    );
    assert!(
        text.contains("2 of 3"),
        "{HARNESS}: the refusal names how many chunks arrived: {text}"
    );

    // Nothing was attached.
    let assets = harness
        .media_service()
        .for_property(&property_id, &ctx)
        .await
        .expect("for_property runs");
    assert!(
        assets.is_empty(),
        "{HARNESS}: the refused finalize attached nothing"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. Positive: once the missing chunk arrives, the same upload assembles cleanly.
    //    The previous upload is now 'failed', so we create a new one to test the happy path.
    // -----------------------------------------------------------------------------------------------------------
    let complete_id = uuid::Uuid::new_v4().to_string();
    harness
        .media_service()
        .begin_media_upload(
            begin_request_with_size(&property_id, &complete_id, 2, 32),
            &ctx,
        )
        .await
        .expect("begin_media_upload runs");
    harness
        .media_service()
        .stage_media_chunk(&complete_id, 0, b"chunk-zero-bytes".to_vec(), &ctx)
        .await
        .expect("chunk 0 stages");
    harness
        .media_service()
        .stage_media_chunk(&complete_id, 1, b"chunk-one-bytes!".to_vec(), &ctx)
        .await
        .expect("chunk 1 stages");
    harness
        .media_service()
        .claim_media_upload(&complete_id, &ctx)
        .await
        .expect("claim_media_upload runs");
    let assembly = harness
        .media_dao()
        .assemble_media_upload(&complete_id)
        .await
        .expect("assemble_media_upload runs");
    assert_eq!(
        assembly.bytes, b"chunk-zero-byteschunk-one-bytes!",
        "{HARNESS}: the assembled bytes are all chunks in order"
    );
    let assembly = harness
        .media_dao()
        .assemble_media_upload(&upload_id)
        .await
        .expect("the now-complete upload assembles");
    assert_eq!(
        assembly.bytes, b"chunk-zero-byteschunk-one-bytes!chunk-two-bytes!",
        "{HARNESS}: the assembled bytes are all three chunks in order"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. Negative: an upload with ONLY some chunks staged (none missing by index but
    //    the count short) is still incomplete — the check is on the count, not the indexes.
    // -----------------------------------------------------------------------------------------------------------
    let partial_id = uuid::Uuid::new_v4().to_string();
    harness
        .media_service()
        .begin_media_upload(begin_request(&property_id, &partial_id, 3), &ctx)
        .await
        .expect("begin_media_upload runs");
    // The partial upload is open; only chunk 2 will be staged.
    // Stage only chunk 2 — chunks 0 and 1 are missing.
    harness
        .media_service()
        .stage_media_chunk(&partial_id, 2, b"chunk-two-bytes!".to_vec(), &ctx)
        .await
        .expect("chunk 2 stages");
    harness
        .media_service()
        .claim_media_upload(&partial_id, &ctx)
        .await
        .expect("claim_media_upload runs");
    // The partial upload is claimed; its missing chunks will be caught at assemble.
    let result = harness.media_dao().assemble_media_upload(&partial_id).await;
    let text = match result {
        Ok(_) => panic!("{HARNESS}: an upload with only chunk 2 must not assemble"),
        Err(err) => format!("{err}"),
    };
    assert!(
        text.contains("1 of 3"),
        "{HARNESS}: the refusal names how many chunks arrived: {text}"
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
