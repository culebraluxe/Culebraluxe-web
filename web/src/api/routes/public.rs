//! The public website's endpoints: intake, leads, marketing, guide, listings, media and documents; the Tech cockpit.

#[allow(unused_imports)]
use super::*;

pub(super) async fn submit_website_intake(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<model::WebsiteIntakeRequest>,
) -> Result<Json<ApiSuccess<model::WebsiteIntakeResult>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let value = state
        .services()
        .intake()
        .submit_website(&body, &context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(value, &context.correlation_id))
}

pub(super) async fn submit_catchup_lead(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<model::CatchupLeadRequest>,
) -> Result<Json<ApiSuccess<model::CatchupLeadResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let value = state
        .services()
        .intake()
        .submit_catchup(&body, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(value, &resolved))
}

/// Email the team and the visitor about one website lead the public site has just saved. The body is empty: the id is
/// all the caller supplies, and the emails are written from the stored lead. Sent once per lead.
pub(super) async fn notify_website_lead(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<model::WebsiteLeadNotice>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let notice = state
        .services()
        .website_leads()
        .notify(&id, &context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(notice, &context.correlation_id))
}

/// The Island Guide, through its own Rust service and the anonymous public security door.
pub(super) async fn tech_cockpit(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<TechCockpitQuery>,
) -> Result<Json<ApiSuccess<model::TechCockpitSnapshot>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let snapshot = state
        .services()
        .tech()
        .snapshot(query.selected.as_deref(), &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(snapshot, &resolved))
}

pub(super) async fn tech_command(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<model::TechCommandRequest>,
) -> Result<Json<ApiSuccess<model::TechCommandResult>>, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let result = state
        .services()
        .tech()
        .command(body, &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?;
    Ok(success(result, &resolved))
}

pub(super) async fn public_marketing_content(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<model::MarketingContentBlock>>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let value = state
        .services()
        .marketing()
        .public_content(&context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(value, &context.correlation_id))
}

pub(super) async fn public_guide(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<model::GuideItem>>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let items = state
        .services()
        .guide()
        .items(&context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(items, &context.correlation_id))
}

#[derive(serde::Deserialize)]
pub(super) struct PublicPropertyQuery {
    /// Whatever names the Property: its slug, its name in any case, or its id.
    pub(super) key: String,
}

#[derive(serde::Deserialize)]
pub(super) struct PublicSimilarQuery {
    pub(super) key: String,
    /// The page shows a strip; the service clamps whatever arrives.
    pub(super) limit: Option<i64>,
}

/// Listings like this one, for the property page.
pub(super) async fn public_similar(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<PublicSimilarQuery>,
) -> Result<Json<ApiSuccess<Vec<model::PublicListing>>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let listings = state
        .services()
        .public_listings()
        .similar(query.key.trim(), query.limit.unwrap_or(3), &context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(listings, &context.correlation_id))
}

/// Every slug the site can serve, for the sitemap.
pub(super) async fn public_slugs(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<String>>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let slugs = state
        .services()
        .public_listings()
        .slugs(&context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(slugs, &context.correlation_id))
}

/// One photograph's bytes for the public site: the web copy if there is one, the original otherwise.
///
/// The bytes are returned as they are stored. An id that is unknown, or that belongs to media which is not published,
/// answers 404 either way — an anonymous visitor must not be able to tell those apart.
pub(super) async fn public_media(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<axum::response::Response, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let found = state
        .services()
        .public_listings()
        .media_bytes(&id, "web", &context)
        .await
        .map_err(ApiError::from)?;

    let Some((mime_type, bytes)) = found else {
        return Err(ApiError::not_found("MEDIA_NOT_FOUND", "Not found."));
    };

    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, mime_type)
        // Photograph bytes do not change under an id: a replacement upload is a new row.
        .header(axum::http::header::CACHE_CONTROL, "public, max-age=3600")
        .body(axum::body::Body::from(bytes))
        .map_err(|error| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "MEDIA_RESPONSE_FAILED",
                error.to_string(),
                false,
            )
        })
}

/// One Property, for the public page. Anonymous, the same guest door as the inventory, the same published action.
///
/// A key that resolves to nothing answers `null`, not an error: "no such listing" is a 404 for the page, not a fault
/// in the service.
pub(super) async fn public_property(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<PublicPropertyQuery>,
) -> Result<Json<ApiSuccess<Option<model::PublicProperty>>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let property = state
        .services()
        .public_listings()
        .property(query.key.trim(), &context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(property, &context.correlation_id))
}

/// The inventory of the public site, for the anonymous buyer: the same guest door and the same published action as
/// the listing copy above. This is the read the buyers grid renders.
pub(super) async fn public_listings(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<model::PublicListing>>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let listings = state
        .services()
        .public_listings()
        .listings(&context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(listings, &context.correlation_id))
}

/// The taglines of published listings, for the anonymous public site: the same guest door the public document route
/// uses (no identity headers), authorized as the published `property.public.read`.
pub(super) async fn public_listing_copy(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<model::PublicListingCopy>>>, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let copy = state
        .services()
        .public_listings()
        .listing_copy(&context)
        .await
        .map_err(ApiError::from)?;
    Ok(success_with_correlation(copy, &context.correlation_id))
}

pub(super) async fn vault_public_listing_document_bytes(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<VaultDownloadQuery>,
) -> Result<Response, ApiError> {
    let context = resolve_public_guest_context(&state, &headers)?;
    let id = uuid::Uuid::parse_str(&id)
        .map_err(|_| ApiError::not_found("VAULT_DOCUMENT_NOT_FOUND", "Document not found."))?;
    let vault = state.services().vault();
    let document = vault
        .public_listing_document_bytes(&id.to_string(), &context)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("VAULT_DOCUMENT_NOT_FOUND", "Document not found."))?;
    vault_document_response(document, query.download.as_deref() == Some("1"))
}

pub(super) async fn vault_private_document_bytes(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<VaultDownloadQuery>,
) -> Result<Response, ApiError> {
    let resolved = resolve_request_context(&state, &headers).await?;
    let id = uuid::Uuid::parse_str(&id)
        .map_err(|_| ApiError::not_found("VAULT_DOCUMENT_NOT_FOUND", "Document not found."))?;
    let vault = state.services().vault();
    let document = vault
        .media_bytes(&id.to_string(), &resolved.service)
        .await
        .map_err(|error| correlate(ApiError::from(error), &resolved))?
        .ok_or_else(|| ApiError::not_found("VAULT_DOCUMENT_NOT_FOUND", "Document not found."))?;
    vault_document_response(document, query.download.as_deref() == Some("1"))
}
