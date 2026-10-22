use super::*;

#[test]
fn prices_and_areas_read_as_the_typescript_cards_did() {
    assert_eq!(format_price(Some(2_350_000.0)), "$2,350,000");
    assert_eq!(format_price(Some(0.0)), "Price Upon Request");
    assert_eq!(format_price(None), "Price Upon Request");
    assert_eq!(
        format_area(Some(1.0), Some("Acres")).as_deref(),
        Some("1 Acre")
    );
    assert_eq!(
        format_area(Some(2.5), Some("acres")).as_deref(),
        Some("2.5 Acres")
    );
    assert_eq!(
        format_area(Some(12500.0), Some("SqFt")).as_deref(),
        Some("12,500 SF")
    );
    assert_eq!(en_us(1234.5), "1,234.5");
    assert_eq!(whole(&json!(2224613.0)), json!(2224613));
    assert_eq!(whole(&json!(1.5)), json!(1.5));
}

/// A page payload the way `PublicListingService::property` returns it: two photographs, a hero marked, and one
/// document that must never receive a CDN URL.
fn property_payload() -> Value {
    json!({
        "id": "property-1",
        "name": "Casa Test",
        "heroMediaId": "media-hero",
        "media": [
            { "id": "media-hero", "mediaType": "image", "role": "hero", "altText": "The house", "caption": null },
            { "id": "media-gallery", "mediaType": "image", "role": "gallery", "altText": null, "caption": "The pool" },
            { "id": "media-doc", "mediaType": "document", "role": "document", "filename": "survey.pdf", "mimeType": "application/pdf" }
        ]
    })
}

/// The three direct copies for one photograph, as the resolver would produce them.
fn resolved(id: &str) -> MediaUrls {
    MediaUrls {
        card: Some(format!("https://cdn.test/media-cdn/media/{id}/card.jpg")),
        web: Some(format!("https://cdn.test/media-cdn/media/{id}/web.jpg")),
        thumb: Some(format!("https://cdn.test/media-cdn/media/{id}/thumb.jpg")),
    }
}

#[test]
fn a_listing_card_uses_the_server_resolved_direct_card_url() {
    let property = json!({ "id": "p1", "name": "Casa", "heroMediaId": "media-hero" });
    let mut cards = std::collections::HashMap::new();
    cards.insert(
        "media-hero".to_owned(),
        "https://cdn.test/media-cdn/media/media-hero/card.jpg".to_owned(),
    );
    let card = listing(&property, &std::collections::HashMap::new(), &cards);
    assert_eq!(
        card["imagePath"],
        json!("https://cdn.test/media-cdn/media/media-hero/card.jpg"),
        "a listing card fetches the direct CDN card copy, not a per-image API redirect"
    );
}

#[test]
fn a_listing_card_falls_back_to_the_guarded_route_when_no_direct_url_resolved() {
    let property = json!({ "id": "p1", "name": "Casa", "heroMediaId": "media-hero" });
    let card = listing(
        &property,
        &std::collections::HashMap::new(),
        &std::collections::HashMap::new(),
    );
    assert_eq!(
        card["imagePath"],
        json!(format!(
            "/api/media/media-hero?size=card&v={MEDIA_CACHE_VERSION}"
        )),
        "an unresolved photograph keeps the existing guarded route"
    );
}

#[test]
fn a_gallery_frame_carries_all_three_direct_copies() {
    let mut urls = std::collections::HashMap::new();
    urls.insert("media-gallery".to_owned(), resolved("media-gallery"));
    let frame = gallery_item(
        &json!({ "id": "media-gallery", "mediaType": "image", "altText": null, "caption": "The pool" }),
        &urls,
        &json!("Casa Test"),
    );
    assert_eq!(
        frame["url"],
        json!("https://cdn.test/media-cdn/media/media-gallery/card.jpg")
    );
    assert_eq!(
        frame["cardUrl"],
        json!("https://cdn.test/media-cdn/media/media-gallery/card.jpg")
    );
    assert_eq!(
        frame["webUrl"],
        json!("https://cdn.test/media-cdn/media/media-gallery/web.jpg")
    );
    assert_eq!(
        frame["thumbUrl"],
        json!("https://cdn.test/media-cdn/media/media-gallery/thumb.jpg")
    );
    // The alt falls back to the property name; the caption is carried through.
    assert_eq!(frame["alt"], json!("Casa Test"));
    assert_eq!(frame["caption"], json!("The pool"));
}

#[test]
fn a_gallery_frame_without_resolution_keeps_the_guarded_route() {
    let frame = gallery_item(
        &json!({ "id": "media-gallery", "mediaType": "image" }),
        &std::collections::HashMap::new(),
        &json!("Casa Test"),
    );
    assert_eq!(
        frame["url"],
        json!(format!(
            "/api/media/media-gallery?size=card&v={MEDIA_CACHE_VERSION}"
        ))
    );
    assert_eq!(frame["cardUrl"], Value::Null);
    assert_eq!(frame["webUrl"], Value::Null);
    assert_eq!(frame["thumbUrl"], Value::Null);
}

#[test]
fn the_property_page_uses_direct_copies_and_never_touches_documents() {
    let property = property_payload();
    let mut urls = std::collections::HashMap::new();
    urls.insert("media-hero".to_owned(), resolved("media-hero"));
    urls.insert("media-gallery".to_owned(), resolved("media-gallery"));
    let similar = json!([
        { "id": "p2", "name": "Other", "heroMediaId": "media-similar" }
    ]);
    let mut similar_cards = std::collections::HashMap::new();
    similar_cards.insert(
        "media-similar".to_owned(),
        "https://cdn.test/media-cdn/media/media-similar/card.jpg".to_owned(),
    );

    let page = property_detail(
        "casa-test",
        &property,
        &similar,
        json!(["casa-test"]),
        &urls,
        &similar_cards,
    );
    let property = &page["property"];

    // The hero is the direct card copy.
    assert_eq!(
        property["heroUrl"],
        json!("https://cdn.test/media-cdn/media/media-hero/card.jpg")
    );

    // The gallery excludes the hero and carries all three copies for its own frame.
    let gallery = property["gallery"].as_array().expect("a gallery");
    assert_eq!(gallery.len(), 1, "the hero is not repeated in the gallery");
    assert_eq!(
        gallery[0]["url"],
        json!("https://cdn.test/media-cdn/media/media-gallery/card.jpg")
    );
    assert_eq!(
        gallery[0]["webUrl"],
        json!("https://cdn.test/media-cdn/media/media-gallery/web.jpg")
    );

    // Documents keep their guarded shape: id/title/filename/mimeType, and no CDN fields at all.
    let documents = property["documents"].as_array().expect("documents");
    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0]["filename"], json!("survey.pdf"));
    assert_eq!(documents[0]["cardUrl"], Value::Null);
    assert_eq!(documents[0]["webUrl"], Value::Null);
    assert_eq!(documents[0]["url"], Value::Null);

    // Similar listing cards are direct too.
    assert_eq!(
        property["similar"][0]["imagePath"],
        json!("https://cdn.test/media-cdn/media/media-similar/card.jpg")
    );
}

#[test]
fn an_unresolved_property_page_still_builds_with_the_guarded_route() {
    let page = property_detail(
        "casa-test",
        &property_payload(),
        &json!([]),
        json!([]),
        &std::collections::HashMap::new(),
        &std::collections::HashMap::new(),
    );
    let property = &page["property"];
    assert_eq!(
        property["heroUrl"],
        json!(format!(
            "/api/media/media-hero?size=card&v={MEDIA_CACHE_VERSION}"
        ))
    );
    assert_eq!(
        property["gallery"][0]["url"],
        json!(format!(
            "/api/media/media-gallery?size=card&v={MEDIA_CACHE_VERSION}"
        ))
    );
}

#[test]
fn the_page_collects_one_deduplicated_hero_id_list() {
    let properties = vec![
        json!({ "id": "p1", "heroMediaId": "media-a" }),
        json!({ "id": "p2", "heroMediaId": "media-b" }),
        json!({ "id": "p3", "heroMediaId": "media-a" }),
        json!({ "id": "p4", "heroMediaId": null }),
    ];
    assert_eq!(
        hero_ids(&properties),
        vec!["media-a".to_owned(), "media-b".to_owned()],
        "each hero is resolved once, in a stable order, and a missing hero is skipped"
    );
}

#[test]
fn only_resolved_cards_become_listing_image_urls() {
    let mut urls = std::collections::HashMap::new();
    urls.insert("media-a".to_owned(), resolved("media-a"));
    urls.insert(
        "media-b".to_owned(),
        MediaUrls {
            card: None,
            ..Default::default()
        },
    );
    let cards = card_urls(&urls);
    assert_eq!(
        cards.get("media-a").map(String::as_str),
        Some("https://cdn.test/media-cdn/media/media-a/card.jpg")
    );
    assert!(
        !cards.contains_key("media-b"),
        "a photograph whose card copy did not resolve falls back in the builder instead of entering the map"
    );
}
