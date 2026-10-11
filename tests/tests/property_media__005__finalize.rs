//! PROPERTY.MEDIA — finalize (TST-PROPERTY-MEDIA-005).
//!
//! Contract: `MediaService::complete_media_upload` finalizes a chunked upload — it claims
//! the upload, assembles the staged chunks, derives the servable copies, writes the
//! `media` row and the `property_media` link in one transaction, and drops the staged
//! bytes. A finalized upload is attached and readable; its manifest and chunks are gone.
//!
//! Level: L3 Composition — media service boundary.
//! Harness: PropertyHarness.
//! Requires DEV (`DATABASE_URL_DEV`); the harness refuses PROD before any socket.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_media__005__finalize

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

/// A small valid JPEG — the production finalize path decodes the assembled upload to
/// derive its web/thumb copies, so the fixture must be a real image.
fn jpeg_bytes() -> Vec<u8> {
    let mut image = image::RgbImage::new(64, 48);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        *pixel = image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8]);
    }
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90)
        .encode_image(&image::DynamicImage::ImageRgb8(image))
        .expect("the fixture encodes");
    out
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
        filename: "finalized.jpg".to_string(),
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-MEDIA-005); the file and the assay use it.
async fn property_media_005__finalize() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the finalize proof runs only on an isolated DEV target"
    );
    let ctx = harness.test_context();
    let ns = harness.namespace().to_string();
    let marker = format!("TST-PROPERTY-MEDIA005-{ns}");

    let property_id = harness
        .seed_property(&marker, Some("house"))
        .await
        .expect("property seeds");

    // -----------------------------------------------------------------------------------------------------------
    // 1. Positive: begin → stage all chunks → complete. The upload is attached and readable.
    // -----------------------------------------------------------------------------------------------------------
    let original = jpeg_bytes();
    let upload_id = uuid::Uuid::new_v4().to_string();
    // Split the JPEG into two chunks at a fixed boundary.
    let split = original.len() / 2;
    let chunk0 = &original[..split];
    let chunk1 = &original[split..];

    let manifest = harness
        .media_service()
        .begin_media_upload(
            begin_request(&property_id, &upload_id, original.len() as i64, 2),
            &ctx,
        )
        .await
        .expect("begin_media_upload runs");
    assert_eq!(
        manifest.chunk_count, 2,
        "{HARNESS}: the manifest declares 2 chunks"
    );

    harness
        .media_service()
        .stage_media_chunk(&upload_id, 0, chunk0.to_vec(), &ctx)
        .await
        .expect("chunk 0 stages");
    harness
        .media_service()
        .stage_media_chunk(&upload_id, 1, chunk1.to_vec(), &ctx)
        .await
        .expect("chunk 1 stages");

    let result = harness
        .media_service()
        .complete_media_upload(&upload_id, &ctx)
        .await
        .expect("complete_media_upload runs");
    assert!(result.ok, "{HARNESS}: the finalize reports ok");
    assert_eq!(
        result.property_id, property_id,
        "{HARNESS}: the result names the property"
    );
    assert_eq!(
        result.role, "gallery",
        "{HARNESS}: the result names the role"
    );

    // The finalized upload is attached and readable through the production read path.
    let assets = harness
        .media_service()
        .for_property(&property_id, &ctx)
        .await
        .expect("for_property runs");
    assert_eq!(assets.len(), 1, "{HARNESS}: exactly one asset attached");
    assert_eq!(
        assets[0].id, result.media_id,
        "{HARNESS}: the attached asset is the one finalize returned"
    );
    assert_eq!(
        assets[0].role, "gallery",
        "{HARNESS}: the attached asset carries the gallery role"
    );
    assert_eq!(
        assets[0].media_type, "image",
        "{HARNESS}: the attached asset is an image"
    );

    // The staged bytes are gone — the manifest and its chunks were dropped by the commit.
    let state = harness
        .media_service()
        .media_upload_state(&upload_id, &ctx)
        .await
        .expect("media_upload_state runs");
    assert_eq!(
        state, "done",
        "{HARNESS}: a finalized upload reads `done` — its manifest is gone"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. Positive: the finalized upload's bytes are the assembled original (stored, not the chunks).
    // -----------------------------------------------------------------------------------------------------------
    let stored = harness
        .media_dao()
        .original_bytes(&result.media_id)
        .await
        .expect("original_bytes runs")
        .expect("the original is stored");
    assert_eq!(
        stored, original,
        "{HARNESS}: the stored original is the assembled upload"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. Negative: finalizing an upload with a MISSING chunk fails — the incomplete upload
    //    is not storable as a photograph.
    // -----------------------------------------------------------------------------------------------------------
    let incomplete_id = uuid::Uuid::new_v4().to_string();
    let status = harness
        .media_service()
        .begin_media_upload(
            begin_request(&property_id, &incomplete_id, original.len() as i64, 2),
            &ctx,
        )
        .await
        .expect("begin_media_upload runs");
    assert_eq!(
        status.chunk_count, 2,
        "{HARNESS}: the manifest declares 2 chunks"
    );
    // Stage only chunk 0 — chunk 1 never arrives.
    harness
        .media_service()
        .stage_media_chunk(&incomplete_id, 0, chunk0.to_vec(), &ctx)
        .await
        .expect("chunk 0 stages");

    let err = harness
        .media_service()
        .complete_media_upload(&incomplete_id, &ctx)
        .await
        .expect_err("an incomplete upload must not finalize");
    let text = format!("{err}");
    assert!(
        text.contains("incomplete"),
        "{HARNESS}: the refusal names the incomplete upload: {text}"
    );

    // The failed finalize left no attachment behind.
    let assets = harness
        .media_service()
        .for_property(&property_id, &ctx)
        .await
        .expect("for_property runs");
    assert_eq!(
        assets.len(),
        1,
        "{HARNESS}: the failed finalize attached nothing"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. Negative: finalizing an unknown upload fails.
    // -----------------------------------------------------------------------------------------------------------
    let unknown_id = uuid::Uuid::new_v4().to_string();
    let err = harness
        .media_service()
        .complete_media_upload(&unknown_id, &ctx)
        .await
        .expect_err("an unknown upload must not finalize");
    let text = format!("{err}");
    assert!(
        text.contains("unknown or has already been completed")
            || text.contains("already being finished"),
        "{HARNESS}: the refusal names the unknown upload: {text}"
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
