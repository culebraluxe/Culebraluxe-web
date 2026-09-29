//! THE WEBSITE'S OWN ADDRESSES — what the Yew app calls outside the portal (`rust/ui/src/app/api.rs`), answered here at
//! the same paths and in the same shapes the Next relays used. A visitor is the anonymous public guest; a portal user
//! (`ui_auth`) additionally sees what is private to the firm.

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde_json::{json, Value};

use super::context::public_guest_context;
use super::ui_auth::{portal_open, resolve_portal_context};
use super::{ApiError, ApiState};

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/api/media/{id}", get(media))
        .route("/api/rust-ui/public-page", get(public_page))
        .route("/api/rust-ui/client-room", get(client_room))
        .route(
            "/api/rust-ui/website-intake",
            axum::routing::post(website_intake),
        )
}

/// The signed-in external client's own transaction room. The subject comes from the session's resolved
/// app_user.person_id; no caller-selected person id exists on this route.
async fn client_room(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<axum::Json<Value>, ApiError> {
    let resolved = resolve_portal_context(&state, &headers).await?;
    if resolved.acting_user.account_type != "external" {
        return Err(ApiError::forbidden(
            "CLIENT_ROOM_EXTERNAL_ONLY",
            "The client room is for external client accounts.",
        ));
    }
    let Some(person_id) = resolved.acting_user.person_id.as_deref() else {
        return Ok(axum::Json(json!({ "linked": false, "room": null })));
    };
    let room = state
        .services()
        .client_room()
        .snapshot(person_id, &resolved.service)
        .await
        .map_err(|error| {
            ApiError::from(error).with_correlation(resolved.service.correlation_id.clone())
        })?;
    Ok(axum::Json(
        json!({ "linked": room.is_some(), "room": room }),
    ))
}

/// One photograph's bytes. A portal user reads any media the firm holds (never cached by the browser); a visitor only
/// published listing media. A miss is a plain 404; a failure is an `ApiError`, so it is captured like any other.
#[derive(Debug, serde::Deserialize)]
struct MediaQuery {
    /// `card` (1200px, for listing cards and tiles) or `thumb` (400px); otherwise the web copy.
    size: Option<String>,
}

async fn media(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<MediaQuery>,
) -> Result<Response, ApiError> {
    let size = query.size.unwrap_or_default();
    let (found, cache) = if portal_open(&headers) {
        let resolved = resolve_portal_context(&state, &headers).await?;
        let found = state
            .services()
            .media()
            .media_bytes(&id, &resolved.service)
            .await;
        (
            found.map_err(|error| {
                ApiError::from(error).with_correlation(resolved.service.correlation_id.clone())
            })?,
            "private, no-store",
        )
    } else {
        let context = public_guest_context(&headers);
        let found = state
            .services()
            .public_listings()
            .media_bytes(&id, &size, &context)
            .await;
        // A photograph's bytes never change for its id and size (a new photo is a new id), so a public copy is cached
        // for a year and marked immutable: a repeat visitor does not ask again.
        (
            found.map_err(ApiError::from)?,
            "public, max-age=31536000, immutable",
        )
    };
    Ok(match found {
        Some((mime_type, bytes)) => (
            [
                (header::CONTENT_TYPE, mime_type),
                (header::CACHE_CONTROL, cache.to_owned()),
            ],
            Body::from(bytes),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "Not found").into_response(),
    })
}

/// A read the page can live without failed: record it durably, answer without it (as the relay did).
fn tolerated<T>(
    read: &str,
    result: Result<T, crate::service_support::CoreServiceError>,
) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            crate::api::error_capture::record(
                "rust:public-page",
                read,
                &error.to_string(),
                "warn",
                None,
                json!({ "route": "/api/rust-ui/public-page" }),
            );
            None
        }
    }
}

fn to_json<T: serde::Serialize>(value: T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

fn at<'a>(value: &'a Value, key: &str) -> &'a Value {
    value.get(key).unwrap_or(&Value::Null)
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
}

/// A count or a size as a whole number: the services carry some as floats, the site reads integers (as the relay's
/// JSON printed them). Anything that is not a whole number passes through unchanged.
fn whole(value: &Value) -> Value {
    match value.as_f64() {
        Some(number) if number.fract() == 0.0 && number.abs() < 9e15 => json!(number as i64),
        _ => value.clone(),
    }
}

/// A number as JavaScript prints it: no trailing `.0`.
fn js_number(number: f64) -> String {
    if number.fract() == 0.0 && number.abs() < 1e15 {
        format!("{}", number as i64)
    } else {
        format!("{number}")
    }
}

/// A number as `toLocaleString('en-US')` prints it: thousands separators, at most three decimals.
fn en_us(number: f64) -> String {
    let rounded = (number * 1000.0).round() / 1000.0;
    let whole = rounded.trunc().abs() as u64;
    let digits = whole.to_string();
    let mut grouped = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let fraction = format!("{:.3}", rounded.fract().abs());
    let fraction = fraction
        .trim_start_matches('0')
        .trim_end_matches('0')
        .trim_end_matches('.');
    format!(
        "{}{grouped}{fraction}",
        if rounded < 0.0 { "-" } else { "" }
    )
}

fn format_price(price: Option<f64>) -> String {
    match price {
        Some(price) if price > 0.0 => format!("${}", en_us(price)),
        _ => "Price Upon Request".into(),
    }
}

fn format_area(area: Option<f64>, units: Option<&str>) -> Option<String> {
    let area = area?;
    if units.unwrap_or("").eq_ignore_ascii_case("acres") {
        return Some(format!(
            "{} {}",
            js_number(area),
            if area == 1.0 { "Acre" } else { "Acres" }
        ));
    }
    Some(format!("{} SF", en_us(area)))
}

/// A marketing block in the site's shape; a missing block is empty text, not an absent field.
fn block(source: Option<&Value>) -> Value {
    let empty = Value::Null;
    let b = source.unwrap_or(&empty);
    let text_or = |key: &str| b.get(key).and_then(Value::as_str);
    let items: Vec<Value> = b
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| json!({ "key": text_or_empty(item, "key"), "label": item.get("label").and_then(Value::as_str), "value": item.get("value").and_then(Value::as_str) }))
        .collect();
    json!({
        "eyebrow": text_or("eyebrow").unwrap_or(""),
        "title": text_or("title").unwrap_or(""),
        "subtitle": text_or("subtitle").unwrap_or(""),
        "body": text_or("body").unwrap_or(""),
        "ctaLabel": text_or("ctaLabel"),
        "ctaHref": text_or("ctaHref"),
        "imagePath": text_or("imagePath"),
        "imageAlt": text_or("imageAlt"),
        "items": items,
    })
}

fn text_or_empty(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
}

/// One published listing as the site's cards read it.
fn listing(source: &Value, taglines: &std::collections::HashMap<String, String>) -> Value {
    let region = [text(source, "city"), text(source, "stateOrProvince")]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");
    let location = if !region.is_empty() {
        region
    } else {
        text(source, "neighborhood")
            .unwrap_or("Culebra, PR")
            .to_owned()
    };
    let key = text(source, "key").unwrap_or("");
    let number = |key: &str| at(source, key).as_f64();
    json!({
        "id": at(source, "id"),
        "slug": at(source, "key"),
        "name": at(source, "name"),
        "location": location,
        "price": number("listPrice").map(|price| format_price(Some(price))),
        "kind": at(source, "propertyType"),
        // A card shows the card copy, not the 3600px web copy it once pulled (2-3 MB per card).
        "imagePath": text(source, "heroMediaId").map(|id| format!("/api/media/{id}?size=card")),
        "imageAlt": at(source, "heroAlt"),
        "beds": at(source, "bedrooms"),
        "baths": at(source, "bathrooms"),
        "area": format_area(number("lotSize"), text(source, "lotSizeUnits")),
        "interiorArea": number("squareFeet").filter(|area| *area != 0.0).and_then(|area| format_area(Some(area), Some("SF"))),
        "views": source.get("views").filter(|v| !v.is_null()).cloned().unwrap_or(json!([])),
        "beachAccess": at(source, "beachAccess").as_bool() == Some(true),
        "tagline": taglines.get(key),
        "featured": at(source, "featured").as_bool() == Some(true),
    })
}

#[derive(Debug, serde::Deserialize)]
struct PublicPageQuery {
    #[serde(default)]
    screen: String,
    scope: Option<String>,
}

/// A public site page's content: marketing copy, the published listings, the guide, or one property's record.
async fn public_page(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<PublicPageQuery>,
) -> Result<Response, ApiError> {
    let context = public_guest_context(&headers);
    let services = state.services();
    let listings_service = services.public_listings();
    let marketing = || async {
        tolerated(
            "marketing-content",
            services.marketing().public_content(&context).await,
        )
        .map(to_json)
    };
    let listings = || async {
        tolerated("listings", listings_service.listings(&context).await)
            .map(to_json)
            .and_then(|list| list.as_array().cloned())
            .unwrap_or_default()
    };
    let block_by_id = |blocks: &Option<Value>, id: &str| {
        blocks
            .as_ref()
            .and_then(Value::as_array)
            .and_then(|list| {
                list.iter()
                    .find(|b| b.get("id").and_then(Value::as_str) == Some(id))
            })
            .map(|b| block(Some(b)))
            .unwrap_or_else(|| block(None))
    };
    let no_taglines = std::collections::HashMap::new();
    let scope = query
        .scope
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let body = match query.screen.as_str() {
        "site-home" => {
            let copy = async {
                tolerated(
                    "listing-copy",
                    listings_service.listing_copy(&context).await,
                )
                .map(to_json)
                .and_then(|list| list.as_array().cloned())
                .unwrap_or_default()
            };
            let (properties, blocks, copy) = tokio::join!(listings(), marketing(), copy);
            let taglines: std::collections::HashMap<String, String> = copy
                .iter()
                .filter_map(|entry| {
                    Some((
                        text(entry, "slug")?.to_owned(),
                        text(entry, "tagline")?.to_owned(),
                    ))
                })
                .collect();
            json!({
                "hero": block_by_id(&blocks, "home.hero"),
                "buyers": block_by_id(&blocks, "home.services.buyers"),
                "sellers": block_by_id(&blocks, "home.services.sellers"),
                "culture": block_by_id(&blocks, "home.culture"),
                "about": block_by_id(&blocks, "home.about"),
                "contact": block_by_id(&blocks, "home.contact"),
                "featured": properties.iter().filter(|p| at(p, "featured").as_bool() == Some(true)).map(|p| listing(p, &taglines)).collect::<Vec<_>>(),
                "listings": properties.iter().map(|p| listing(p, &taglines)).collect::<Vec<_>>(),
            })
        }
        "site-services" => {
            let blocks = marketing().await;
            json!({ "buyers": block_by_id(&blocks, "home.services.buyers"), "sellers": block_by_id(&blocks, "home.services.sellers") })
        }
        "site-sellers" => {
            json!({ "sellers": block_by_id(&marketing().await, "home.services.sellers") })
        }
        "site-about" => json!({ "about": block_by_id(&marketing().await, "home.about") }),
        "site-buyers" => {
            let properties = listings().await;
            json!({
                "listings": properties.iter().map(|p| listing(p, &no_taglines)).collect::<Vec<_>>(),
                "featured": properties.iter().filter(|p| at(p, "featured").as_bool() == Some(true)).map(|p| listing(p, &no_taglines)).collect::<Vec<_>>(),
            })
        }
        "site-favorites" => {
            json!({ "listings": listings().await.iter().map(|p| listing(p, &no_taglines)).collect::<Vec<_>>() })
        }
        "site-faq" => {
            let blocks = marketing().await;
            json!({ "hero": block_by_id(&blocks, "faq.page-hero"), "faq": block_by_id(&blocks, "faq.list") })
        }
        "site-contact" => {
            let blocks = marketing().await;
            let enquiry = match scope {
                Some(id) => listings()
                    .await
                    .iter()
                    .find(|p| text(p, "id") == Some(id))
                    .map(|p| at(p, "name").clone()),
                None => None,
            };
            json!({ "hero": block_by_id(&blocks, "contact.page-hero"), "contact": block_by_id(&blocks, "home.contact"), "enquiryProperty": enquiry })
        }
        "site-guide" => {
            let items = to_json(
                services
                    .guide()
                    .items(&context)
                    .await
                    .map_err(ApiError::from)?,
            );
            const KEYS: &[&str] = &[
                "slug",
                "section",
                "name",
                "eyebrow",
                "subtitle",
                "area",
                "description",
                "note",
                "address",
                "phone",
                "websiteUrl",
                "imagePath",
                "imageAlt",
            ];
            let guide: Vec<Value> = items
                .as_array()
                .into_iter()
                .flatten()
                .map(|item| {
                    Value::Object(
                        KEYS.iter()
                            .map(|k| ((*k).to_owned(), at(item, k).clone()))
                            .collect(),
                    )
                })
                .collect();
            json!({ "guide": guide })
        }
        "site-property-detail" => {
            let Some(slug) = scope else {
                return Err(ApiError::bad_request(
                    "PROPERTY_SLUG_REQUIRED",
                    "a property record needs a slug",
                ));
            };
            let property = listings_service
                .property(slug, &context)
                .await
                .map_err(ApiError::from)?;
            let Some(property) = property.map(to_json) else {
                return Err(ApiError::not_found(
                    "PROPERTY_NOT_FOUND",
                    format!("no property with the slug '{slug}'"),
                ));
            };
            let (similar, slugs) = tokio::join!(
                listings_service.similar(slug, 3, &context),
                listings_service.slugs(&context)
            );
            let (similar, slugs) = (
                to_json(similar.map_err(ApiError::from)?),
                to_json(slugs.map_err(ApiError::from)?),
            );
            property_detail(slug, &property, &similar, slugs)
        }
        other => {
            return Err(ApiError::bad_request(
                "PAGE_UNKNOWN",
                format!("no page content source for screen '{other}'"),
            ))
        }
    };
    Ok(axum::Json(body).into_response())
}

/// One property's public record: the facts, the hero, the gallery, the films, the documents, and similar listings.
fn property_detail(slug: &str, p: &Value, similar: &Value, public_slugs: Value) -> Value {
    let media: Vec<&Value> = p
        .get("media")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .collect();
    let kind = |item: &Value| text(item, "mediaType").unwrap_or("").to_owned();
    let images: Vec<&Value> = media
        .iter()
        .copied()
        .filter(|m| kind(m) == "image")
        .collect();
    let hero = match text(p, "heroMediaId") {
        Some(hero_id) => images
            .iter()
            .copied()
            .find(|m| text(m, "id") == Some(hero_id)),
        None => images.first().copied(),
    };
    let hero_id = hero.and_then(|h| text(h, "id"));
    let name = at(p, "name");
    let gallery: Vec<Value> = images
        .iter()
        .filter(|m| text(m, "id") != hero_id)
        .map(|m| {
            json!({
                "url": format!("/api/media/{}", text(m, "id").unwrap_or("")),
                "alt": m.get("altText").filter(|v| !v.is_null()).unwrap_or(name),
                "caption": at(m, "caption"),
            })
        })
        .collect();
    let videos: Vec<Value> = media
        .iter()
        .filter(|m| kind(m) == "video" && text(m, "muxPlaybackId").is_some())
        .map(|m| json!({
            "id": at(m, "id"),
            "playbackId": at(m, "muxPlaybackId"),
            "role": if text(m, "role") == Some("short") { "short" } else { "video" },
            "title": m.get("filename").filter(|v| !v.is_null()).or_else(|| m.get("caption").filter(|v| !v.is_null())).unwrap_or(name),
            "caption": at(m, "caption"),
            "aspectRatio": at(m, "aspectRatio"),
            "durationSeconds": at(m, "durationSeconds"),
        }))
        .collect();
    let documents: Vec<Value> = media
        .iter()
        .filter(|m| kind(m) == "document")
        .map(|m| {
            json!({
                "id": at(m, "id"),
                "title": text(m, "caption").or_else(|| text(m, "filename")).unwrap_or("Document"),
                "filename": text(m, "filename").unwrap_or("document"),
                "mimeType": text(m, "mimeType").unwrap_or("application/octet-stream"),
                "fileSize": whole(at(m, "fileSize")),
                "sortOrder": whole(at(m, "sortOrder")),
            })
        })
        .collect();
    let location = [
        text(p, "neighborhood"),
        text(p, "city"),
        text(p, "stateOrProvince"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(", ");
    let or = |key: &str, fallback: Value| {
        p.get(key)
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or(fallback)
    };
    let no_taglines = std::collections::HashMap::new();
    let fields: Vec<(&str, Value)> = vec![
        ("id", json!(at(p, "id"))),
        ("slug", json!(slug)),
        (
            "title",
            json!(p
                .get("name")
                .filter(|v| !v.is_null())
                .cloned()
                .unwrap_or(json!(slug))),
        ),
        ("kind", json!(at(p, "propertyType"))),
        (
            "price",
            json!(at(p, "listPrice")
                .as_f64()
                .map(|price| format_price(Some(price)))),
        ),
        ("beds", json!(at(p, "bedrooms"))),
        ("baths", json!(at(p, "bathrooms"))),
        (
            "area",
            json!(at(p, "squareFeet")
                .as_f64()
                .map(|area| format!("{} sq ft", js_number(area)))),
        ),
        (
            "location",
            json!(if location.is_empty() {
                Value::Null
            } else {
                json!(location)
            }),
        ),
        (
            "description",
            json!(p
                .get("editorialDescription")
                .filter(|v| !v.is_null())
                .unwrap_or(at(p, "shortDescription"))),
        ),
        ("shortDescription", json!(at(p, "shortDescription"))),
        ("yearBuilt", json!(at(p, "yearBuilt"))),
        ("architecture", json!(at(p, "architectureNotes"))),
        ("status", json!(at(p, "status"))),
        ("neighborhood", json!(at(p, "neighborhood"))),
        ("city", json!(at(p, "city"))),
        ("stateOrProvince", json!(at(p, "stateOrProvince"))),
        (
            "lotSize",
            json!(format_area(
                at(p, "lotSize").as_f64(),
                text(p, "lotSizeUnits")
            )),
        ),
        ("lotSizeSqft", json!(at(p, "lotSizeSqft"))),
        ("roadFrontageFeet", json!(at(p, "roadFrontageFeet"))),
        ("roadSurfaceType", json!(at(p, "roadSurfaceType"))),
        ("lotDescription", json!(at(p, "lotDescription"))),
        ("utilitiesNotes", json!(at(p, "utilitiesNotes"))),
        ("livingArea", json!(at(p, "squareFeet"))),
        ("bathroomsFull", json!(at(p, "bathroomsFull"))),
        ("bathroomsHalf", json!(at(p, "bathroomsHalf"))),
        ("stories", json!(at(p, "stories"))),
        ("parkingSpaces", json!(at(p, "parkingSpaces"))),
        ("waterAccess", json!(or("waterAccess", json!(false)))),
        ("beachAccess", json!(or("beachAccess", json!(false)))),
        ("amenities", json!(or("amenities", json!([])))),
        ("viewType", json!(or("viewType", json!([])))),
        ("lifestyleTags", json!(or("lifestyleTags", json!([])))),
        ("listingAgentName", json!(at(p, "listingAgentName"))),
        ("listingAgentPhone", json!(at(p, "listingAgentPhone"))),
        ("listingAgentEmail", json!(at(p, "listingAgentEmail"))),
        ("listingOffice", json!(at(p, "listingOffice"))),
        ("listingId", json!(at(p, "listingIdentifier"))),
        ("latitude", json!(at(p, "latitude"))),
        ("longitude", json!(at(p, "longitude"))),
        (
            "heroUrl",
            json!(hero_id.map(|id| format!("/api/media/{id}"))),
        ),
        ("gallery", json!(gallery)),
        ("videos", json!(videos)),
        ("documents", json!(documents)),
        (
            "similar",
            json!(similar
                .as_array()
                .into_iter()
                .flatten()
                .map(|l| listing(l, &no_taglines))
                .collect::<Vec<_>>()),
        ),
        ("publicSlugs", json!(public_slugs)),
    ];
    let record: serde_json::Map<String, Value> =
        fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
    json!({ "property": record })
}

#[cfg(test)]
mod format_tests {
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
}

/// A lead from the website's forms: validated, saved, and then the team and the visitor are emailed. The lead is saved
/// before the mail goes, so a mail failure is recorded and the visitor still gets their thank-you.
async fn website_intake(
    State(state): State<ApiState>,
    headers: HeaderMap,
    body: Option<axum::Json<Value>>,
) -> Result<Response, ApiError> {
    let invalid = |status: StatusCode| {
        (
            status,
            axum::Json(json!({ "accepted": false, "status": "invalid" })),
        )
            .into_response()
    };
    let Some(axum::Json(Value::Object(raw))) = body else {
        return Ok(invalid(StatusCode::BAD_REQUEST));
    };
    // Only the form's fields, and only as text a person could have typed.
    const FIELDS: &[&str] = &[
        "submissionId",
        "requestType",
        "propertyId",
        "name",
        "email",
        "message",
        "service",
        "company",
    ];
    let fields: serde_json::Map<String, Value> = FIELDS
        .iter()
        .filter_map(|key| {
            let value = raw.get(*key)?.as_str()?;
            (value.chars().count() <= 5000).then(|| ((*key).to_owned(), json!(value)))
        })
        .collect();
    let lead = match crate::intake::normalize_website_intake(&Value::Object(fields)) {
        Ok(Some(lead)) => lead,
        Ok(None) => {
            return Ok(
                axum::Json(json!({ "accepted": true, "status": "accepted" })).into_response(),
            )
        }
        Err(_) => return Ok(invalid(StatusCode::UNPROCESSABLE_ENTITY)),
    };
    let context = public_guest_context(&headers);
    let result = state
        .services()
        .intake()
        .submit_website(&lead, &context)
        .await
        .map_err(ApiError::from)?;
    if result.accepted {
        tolerated(
            "notify-lead",
            state
                .services()
                .website_leads()
                .notify(&lead.submission_id, &context)
                .await,
        );
    }
    let status = if result.accepted {
        StatusCode::OK
    } else {
        StatusCode::UNPROCESSABLE_ENTITY
    };
    Ok((status, axum::Json(to_json(result))).into_response())
}
