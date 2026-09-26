//! THE WEBSITE'S OWN ADDRESSES — what the Yew app calls outside the portal (`rust/ui/src/app/api.rs`), answered here at
//! the same paths and in the same shapes the Next relays used. A visitor is the anonymous public guest; a portal user
//! (`ui_auth`) additionally sees what is private to the firm.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use super::context::public_guest_context;
use super::ui_auth::{resolve_portal_context, stub_enabled};
use super::{ApiError, ApiState};

pub fn router() -> Router<ApiState> {
    Router::new().route("/api/media/{id}", get(media))
}

/// One photograph's bytes. A portal user reads any media the firm holds (never cached by the browser); a visitor only
/// published listing media. A miss is a plain 404; a failure is an `ApiError`, so it is captured like any other.
async fn media(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let (found, cache) = if stub_enabled() {
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
            .media_bytes(&id, &context)
            .await;
        (found.map_err(ApiError::from)?, "public, max-age=3600")
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
