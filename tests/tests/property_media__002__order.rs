//! PROPERTY.MEDIA — order (TST-PROPERTY-MEDIA-002).
//!
//! Contract: `MediaService::for_property` returns a property's media in a deterministic
//! order — `sort_order` first (ascending, NULLs last), then `created_at`, then the media
//! id. The order is a committed fact the gallery reads, not a property of the query's
//! arrival order.
//!
//! Level: L3 Composition — media service boundary.
//! Harness: PropertyHarness.
//! Requires DEV (`DATABASE_URL_DEV`); the harness refuses PROD before any socket.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_media__002__order

use db::DbTarget;
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

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-MEDIA-002); the file and the assay use it.
async fn property_media_002__order() {
    // 0. L3 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the order proof runs only on an isolated DEV target"
    );
    let ctx = harness.test_context();
    let ns = harness.namespace().to_string();
    let marker = format!("TST-PROPERTY-MEDIA002-{ns}");

    let property_id = harness
        .seed_property(&marker, Some("house"))
        .await
        .expect("property seeds");
    let pool = harness.pool();

    // Fixture setup: three media rows linked with sort_order 2, 0, 1 — deliberately
    // not in arrival order, so the read-back order can only come from sort_order.
    let mut ids: Vec<String> = Vec::new();
    for (label, sort_order) in [("first", 2), ("second", 0), ("third", 1)] {
        let media_id: String = sqlx::query_scalar::<_, String>(
            "insert into media (file_data, filename, mime_type, file_size, media_type)
             values ($1::bytea, $2, 'image/jpeg', 3, 'image') returning id::text",
        )
        .bind(format!("{label}-bytes-{ns}"))
        .bind(format!("{label}.jpg"))
        .fetch_one(pool)
        .await
        .expect("media row seeds");
        sqlx::query(
            "insert into property_media (property_id, media_id, role, sort_order)
             values ($1::uuid, $2::uuid, 'gallery', $3)",
        )
        .bind(&property_id)
        .bind(&media_id)
        .bind(sort_order)
        .execute(pool)
        .await
        .expect("property_media link seeds");
        ids.push(media_id);
    }
    // ids[0] has sort_order 2, ids[1] has sort_order 0, ids[2] has sort_order 1.

    // -----------------------------------------------------------------------------------------------------------
    // 1. Positive: for_property returns the media in sort_order, not arrival order.
    // -----------------------------------------------------------------------------------------------------------
    let assets = harness
        .media_service()
        .for_property(&property_id, &ctx)
        .await
        .expect("for_property runs");
    assert_eq!(assets.len(), 3, "{HARNESS}: three assets attached");
    assert_eq!(assets[0].id, ids[1], "{HARNESS}: sort_order 0 comes first");
    assert_eq!(assets[1].id, ids[2], "{HARNESS}: sort_order 1 comes second");
    assert_eq!(assets[2].id, ids[0], "{HARNESS}: sort_order 2 comes last");

    // -----------------------------------------------------------------------------------------------------------
    // 2. Positive: the order is stable across repeated reads — it is a committed fact.
    // -----------------------------------------------------------------------------------------------------------
    let again = harness
        .media_service()
        .for_property(&property_id, &ctx)
        .await
        .expect("for_property runs again");
    let first_ids: Vec<&str> = assets.iter().map(|asset| asset.id.as_str()).collect();
    let second_ids: Vec<&str> = again.iter().map(|asset| asset.id.as_str()).collect();
    assert_eq!(
        first_ids, second_ids,
        "{HARNESS}: the order is stable across reads"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. Positive: a high sort_order sorts after every lower one.
    // -----------------------------------------------------------------------------------------------------------
    let high_id: String = sqlx::query_scalar::<_, String>(
        "insert into media (file_data, filename, mime_type, file_size, media_type)
         values ($1::bytea, 'high-order.jpg', 'image/jpeg', 3, 'image') returning id::text",
    )
    .bind(format!("high-order-bytes-{ns}"))
    .fetch_one(pool)
    .await
    .expect("high-order media seeds");
    sqlx::query(
        "insert into property_media (property_id, media_id, role, sort_order)
         values ($1::uuid, $2::uuid, 'gallery', 999)",
    )
    .bind(&property_id)
    .bind(&high_id)
    .execute(pool)
    .await
    .expect("high-order link seeds");

    let assets = harness
        .media_service()
        .for_property(&property_id, &ctx)
        .await
        .expect("for_property runs");
    assert_eq!(assets.len(), 4, "{HARNESS}: four assets attached");
    assert_eq!(
        assets[3].id, high_id,
        "{HARNESS}: the high sort_order sorts last"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. Negative: two media with the SAME sort_order fall back to created_at then id —
    //    the order is still deterministic, never arbitrary.
    // -----------------------------------------------------------------------------------------------------------
    let tie_a: String = sqlx::query_scalar::<_, String>(
        "insert into media (file_data, filename, mime_type, file_size, media_type)
         values ($1::bytea, 'tie-a.jpg', 'image/jpeg', 3, 'image') returning id::text",
    )
    .bind(format!("tie-a-bytes-{ns}"))
    .fetch_one(pool)
    .await
    .expect("tie-a media seeds");
    sqlx::query(
        "insert into property_media (property_id, media_id, role, sort_order)
         values ($1::uuid, $2::uuid, 'gallery', 0)",
    )
    .bind(&property_id)
    .bind(&tie_a)
    .execute(pool)
    .await
    .expect("tie-a link seeds");

    let assets = harness
        .media_service()
        .for_property(&property_id, &ctx)
        .await
        .expect("for_property runs");
    assert_eq!(assets.len(), 5, "{HARNESS}: five assets attached");
    // Both sort_order 0: the one created earlier (ids[1]) comes before the newcomer (tie_a).
    let zero_positions: Vec<usize> = assets
        .iter()
        .enumerate()
        .filter(|(_, asset)| asset.sort_order == 0)
        .map(|(index, _)| index)
        .collect();
    assert_eq!(
        zero_positions.len(),
        2,
        "{HARNESS}: two assets share sort_order 0"
    );
    assert_eq!(
        assets[zero_positions[0]].id, ids[1],
        "{HARNESS}: the earlier-created sort_order 0 comes first"
    );
    assert_eq!(
        assets[zero_positions[1]].id, tie_a,
        "{HARNESS}: the later-created sort_order 0 comes second"
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
