//! PROPERTY.MEDIA — attach (TST-PROPERTY-MEDIA-001).
//!
//! Contract: `MediaService::upload_property_media` attaches an image to a property — it
//! writes the `media` row and the `property_media` link in one transaction, returns the
//! new media id and role, and the attachment is readable back through `for_property`.
//!
//! Level: L3 Composition — media service boundary.
//! Harness: PropertyHarness.
//! Requires DEV (`DATABASE_URL_DEV`); the harness refuses PROD before any socket.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_media__001__attach

use db::DbTarget;
use model::UploadPropertyMediaRequest;
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

fn attach_request(
    property_id: &str,
    role: &str,
    filename: &str,
    bytes: &[u8],
) -> UploadPropertyMediaRequest {
    attach_request_with_mime(property_id, role, filename, "image/jpeg", bytes)
}

fn attach_request_with_mime(
    property_id: &str,
    role: &str,
    filename: &str,
    mime_type: &str,
    bytes: &[u8],
) -> UploadPropertyMediaRequest {
    UploadPropertyMediaRequest {
        property_id: property_id.to_string(),
        role: role.to_string(),
        filename: filename.to_string(),
        mime_type: mime_type.to_string(),
        alt_text: None,
        bytes: bytes.to_vec(),
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-MEDIA-001); the file and the assay use it.
async fn property_media_001__attach() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the attach proof runs only on an isolated DEV target"
    );
    let ctx = harness.test_context();
    let ns = harness.namespace().to_string();
    let marker = format!("TST-PROPERTY-MEDIA001-{ns}");

    let property_id = harness
        .seed_property(&marker, Some("house"))
        .await
        .expect("property seeds");

    // -----------------------------------------------------------------------------------------------------------
    // 1. Positive: an image attaches — media row + property_media link in one transaction.
    // -----------------------------------------------------------------------------------------------------------
    let result = harness
        .media_service()
        .upload_property_media(
            attach_request(&property_id, "gallery", "front.jpg", b"fake-jpeg-bytes"),
            &ctx,
        )
        .await
        .expect("upload_property_media runs");
    assert!(result.ok, "{HARNESS}: the upload reports ok");
    assert_eq!(
        result.property_id, property_id,
        "{HARNESS}: the result names the property"
    );
    assert_eq!(
        result.role, "gallery",
        "{HARNESS}: the result names the role"
    );
    assert!(
        !result.media_id.is_empty(),
        "{HARNESS}: the result carries the new media id"
    );

    // Read back through the production read path.
    let assets = harness
        .media_service()
        .for_property(&property_id, &ctx)
        .await
        .expect("for_property runs");
    assert_eq!(assets.len(), 1, "{HARNESS}: exactly one asset attached");
    assert_eq!(
        assets[0].id, result.media_id,
        "{HARNESS}: the attached asset is the one the upload returned"
    );
    assert_eq!(
        assets[0].role, "gallery",
        "{HARNESS}: the attached asset carries the gallery role"
    );
    assert_eq!(
        assets[0].media_type, "image",
        "{HARNESS}: the attached asset is an image"
    );
    assert_eq!(
        assets[0].filename.as_deref(),
        Some("front.jpg"),
        "{HARNESS}: the attached asset keeps its filename"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. Positive: a second image attaches alongside the first — the link is per (property, media).
    // -----------------------------------------------------------------------------------------------------------
    let second = harness
        .media_service()
        .upload_property_media(
            attach_request(&property_id, "gallery", "back.jpg", b"more-fake-jpeg-bytes"),
            &ctx,
        )
        .await
        .expect("second upload runs");
    assert!(second.ok, "{HARNESS}: the second upload reports ok");
    let assets = harness
        .media_service()
        .for_property(&property_id, &ctx)
        .await
        .expect("for_property runs");
    assert_eq!(assets.len(), 2, "{HARNESS}: two assets attached");

    // -----------------------------------------------------------------------------------------------------------
    // 3. Negative: an invalid role is refused — the service validates before the DAO writes.
    // -----------------------------------------------------------------------------------------------------------
    let err = harness
        .media_service()
        .upload_property_media(
            attach_request(&property_id, "invalid_role", "bad.jpg", b"fake-jpeg-bytes"),
            &ctx,
        )
        .await
        .expect_err("an invalid role must be refused");
    assert_eq!(
        err.code(),
        "MEDIA_ROLE_INVALID",
        "{HARNESS}: invalid role refused with the correct code"
    );
    // The refusal wrote nothing.
    let assets = harness
        .media_service()
        .for_property(&property_id, &ctx)
        .await
        .expect("for_property runs");
    assert_eq!(
        assets.len(),
        2,
        "{HARNESS}: the refused upload attached nothing"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. Negative: empty bytes are refused.
    // -----------------------------------------------------------------------------------------------------------
    let err = harness
        .media_service()
        .upload_property_media(
            attach_request(&property_id, "gallery", "empty.jpg", b""),
            &ctx,
        )
        .await
        .expect_err("empty bytes must be refused");
    assert_eq!(
        err.code(),
        "MEDIA_EMPTY",
        "{HARNESS}: empty bytes refused with the correct code"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. Negative: a non-image mime type is refused.
    // -----------------------------------------------------------------------------------------------------------
    let err = harness
        .media_service()
        .upload_property_media(
            attach_request_with_mime(
                &property_id,
                "gallery",
                "doc.pdf",
                "application/pdf",
                b"pdf-bytes",
            ),
            &ctx,
        )
        .await
        .expect_err("a non-image mime type must be refused");
    assert_eq!(
        err.code(),
        "MEDIA_TYPE_INVALID",
        "{HARNESS}: non-image mime type refused with the correct code"
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
