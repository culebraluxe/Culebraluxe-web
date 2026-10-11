//! PROPERTY.MEDIA — gallery (TST-PROPERTY-MEDIA-003).
//!
//! Contract: the gallery role. Media attached as `gallery` reads back as gallery; a
//! property has exactly ONE hero, and promoting a new hero demotes the previous hero to
//! gallery in the same transaction — the gallery is everything that is not the hero.
//!
//! Level: L3 Composition — media service boundary.
//! Harness: PropertyHarness.
//! Requires DEV (`DATABASE_URL_DEV`); the harness refuses PROD before any socket.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_media__003__gallery

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

fn upload(property_id: &str, role: &str, filename: &str) -> UploadPropertyMediaRequest {
    UploadPropertyMediaRequest {
        property_id: property_id.to_string(),
        role: role.to_string(),
        filename: filename.to_string(),
        mime_type: "image/jpeg".to_string(),
        alt_text: None,
        bytes: format!("{filename}-bytes").into_bytes(),
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-MEDIA-003); the file and the assay use it.
async fn property_media_003__gallery() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the gallery proof runs only on an isolated DEV target"
    );
    let ctx = harness.test_context();
    let ns = harness.namespace().to_string();
    let marker = format!("TST-PROPERTY-MEDIA003-{ns}");

    let property_id = harness
        .seed_property(&marker, Some("house"))
        .await
        .expect("property seeds");

    // -----------------------------------------------------------------------------------------------------------
    // 1. Positive: a gallery upload reads back as gallery.
    // -----------------------------------------------------------------------------------------------------------
    let result = harness
        .media_service()
        .upload_property_media(upload(&property_id, "gallery", "g1.jpg"), &ctx)
        .await
        .expect("gallery upload runs");
    assert_eq!(
        result.role, "gallery",
        "{HARNESS}: the result names the gallery role"
    );
    let assets = harness
        .media_service()
        .for_property(&property_id, &ctx)
        .await
        .expect("for_property runs");
    assert_eq!(assets.len(), 1, "{HARNESS}: one asset attached");
    assert_eq!(
        assets[0].role, "gallery",
        "{HARNESS}: the attached asset is a gallery image"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. Positive: a hero upload makes that image the hero; the gallery image stays gallery.
    // -----------------------------------------------------------------------------------------------------------
    let hero = harness
        .media_service()
        .upload_property_media(upload(&property_id, "hero", "h1.jpg"), &ctx)
        .await
        .expect("hero upload runs");
    assert_eq!(
        hero.role, "hero",
        "{HARNESS}: the result names the hero role"
    );
    let assets = harness
        .media_service()
        .for_property(&property_id, &ctx)
        .await
        .expect("for_property runs");
    assert_eq!(assets.len(), 2, "{HARNESS}: two assets attached");
    let heroes: Vec<&str> = assets
        .iter()
        .filter(|asset| asset.role == "hero")
        .map(|asset| asset.id.as_str())
        .collect();
    assert_eq!(
        heroes,
        vec![hero.media_id.as_str()],
        "{HARNESS}: exactly one hero"
    );
    let galleries: Vec<&str> = assets
        .iter()
        .filter(|asset| asset.role == "gallery")
        .map(|asset| asset.id.as_str())
        .collect();
    assert_eq!(
        galleries,
        vec![result.media_id.as_str()],
        "{HARNESS}: the previous gallery image is still gallery"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. Positive: promoting a SECOND hero demotes the first hero to gallery — one hero, rest gallery.
    // -----------------------------------------------------------------------------------------------------------
    let hero2 = harness
        .media_service()
        .upload_property_media(upload(&property_id, "hero", "h2.jpg"), &ctx)
        .await
        .expect("second hero upload runs");
    let assets = harness
        .media_service()
        .for_property(&property_id, &ctx)
        .await
        .expect("for_property runs");
    assert_eq!(assets.len(), 3, "{HARNESS}: three assets attached");
    let heroes: Vec<&str> = assets
        .iter()
        .filter(|asset| asset.role == "hero")
        .map(|asset| asset.id.as_str())
        .collect();
    assert_eq!(
        heroes,
        vec![hero2.media_id.as_str()],
        "{HARNESS}: exactly one hero — the second one"
    );
    let galleries: Vec<&str> = assets
        .iter()
        .filter(|asset| asset.role == "gallery")
        .map(|asset| asset.id.as_str())
        .collect();
    assert_eq!(
        galleries.len(),
        2,
        "{HARNESS}: the first hero was demoted to gallery"
    );
    assert!(
        galleries.contains(&hero.media_id.as_str()),
        "{HARNESS}: the demoted hero is in the gallery"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. Negative: an invalid role is refused — the gallery/hero pair is the only vocabulary.
    // -----------------------------------------------------------------------------------------------------------
    let err = harness
        .media_service()
        .upload_property_media(upload(&property_id, "invalid_role", "bad.jpg"), &ctx)
        .await
        .expect_err("an invalid role must be refused");
    assert_eq!(
        err.code(),
        "MEDIA_ROLE_INVALID",
        "{HARNESS}: invalid role refused with the correct code"
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
