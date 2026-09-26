//! THE PORTAL'S IN-PROCESS DOOR TO `/v1`.
//!
//! The Next relays composed `/v1` answers over HTTP. Their Rust replacements (`portal_ui`) compose the same answers by
//! calling the same `/v1` handlers in process, as the portal user, so every service call, authority check and error
//! shape stays exactly where it already lives. A relay port is then a translation of its composition, nothing more.
//! A handler that needs a service directly may still call it; this is the default, not a rule.

use std::sync::OnceLock;

use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use axum::Router;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tower::ServiceExt;

use super::context::ResolvedRequestContext;
use super::ui_auth::{resolve_portal_context, stub_provider_identity};
use super::{ApiError, ApiState};

static V1: OnceLock<Router> = OnceLock::new();

/// A caller of `/v1` acting as one resolved portal user.
pub struct V1 {
    router: Router,
    key: String,
    provider: String,
    subject: String,
    pub resolved: ResolvedRequestContext,
}

impl V1 {
    /// The portal user behind this request (see `ui_auth`), or 401.
    pub async fn portal(state: &ApiState, headers: &HeaderMap) -> Result<Self, ApiError> {
        let resolved = resolve_portal_context(state, headers).await?;
        let (provider, subject) = stub_provider_identity();
        Ok(Self {
            router: V1
                .get_or_init(|| super::routes::router(state.clone()))
                .clone(),
            key: state.internal_api_key().to_owned(),
            provider,
            subject,
            resolved,
        })
    }

    pub async fn get<T: DeserializeOwned>(&self, path: impl AsRef<str>) -> Result<T, ApiError> {
        self.call(Method::GET, path, None).await
    }

    pub async fn post<T: DeserializeOwned>(
        &self,
        path: impl AsRef<str>,
        body: Value,
    ) -> Result<T, ApiError> {
        self.call(Method::POST, path, Some(body)).await
    }

    pub async fn put<T: DeserializeOwned>(
        &self,
        path: impl AsRef<str>,
        body: Value,
    ) -> Result<T, ApiError> {
        self.call(Method::PUT, path, Some(body)).await
    }

    pub async fn patch<T: DeserializeOwned>(
        &self,
        path: impl AsRef<str>,
        body: Value,
    ) -> Result<T, ApiError> {
        self.call(Method::PATCH, path, Some(body)).await
    }

    pub async fn delete<T: DeserializeOwned>(&self, path: impl AsRef<str>) -> Result<T, ApiError> {
        self.call(Method::DELETE, path, None).await
    }

    /// One `/v1` call; answers its `value`, or its failure as an `ApiError` (already recorded where it happened).
    pub async fn call<T: DeserializeOwned>(
        &self,
        method: Method,
        path: impl AsRef<str>,
        body: Option<Value>,
    ) -> Result<T, ApiError> {
        let path = path.as_ref();
        let correlation = self.resolved.service.correlation_id.clone();
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header("x-culebra-internal-key", &self.key)
            .header("x-culebra-auth-provider", &self.provider)
            .header("x-culebra-auth-sub", &self.subject)
            .header("x-culebra-correlation-id", &correlation);
        if body.is_some() {
            request = request.header("content-type", "application/json");
        }
        let request = request
            .body(
                body.map(|value| Body::from(value.to_string()))
                    .unwrap_or_else(Body::empty),
            )
            .map_err(|error| internal("V1_REQUEST_INVALID", error.to_string(), &correlation))?;
        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .unwrap_or_else(|never| match never {});
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .map_err(|error| internal("V1_BODY_UNREADABLE", error.to_string(), &correlation))?;
        let answer: Value = serde_json::from_slice(&bytes).map_err(|error| {
            internal(
                "V1_ANSWER_NOT_JSON",
                format!("{path} ({status}): {error}"),
                &correlation,
            )
        })?;
        if !status.is_success() || answer.get("ok") == Some(&Value::Bool(false)) {
            return Err(relayed(status, &answer).with_correlation(correlation));
        }
        let value = answer.get("value").cloned().unwrap_or(answer);
        serde_json::from_value(value)
            .map_err(|error| internal("V1_ANSWER_SHAPE", format!("{path}: {error}"), &correlation))
    }
}

fn relayed(status: StatusCode, answer: &Value) -> ApiError {
    let error = answer.get("error").cloned().unwrap_or_default();
    let text = |key: &str| error.get(key).and_then(Value::as_str).map(str::to_owned);
    let status = if status.is_success() {
        StatusCode::INTERNAL_SERVER_ERROR
    } else {
        status
    };
    ApiError::relayed(
        status,
        text("code").unwrap_or_else(|| "FAILED".into()),
        text("message").unwrap_or_else(|| format!("The request failed ({status}).")),
        error
            .get("retryable")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        text("incidentId"),
    )
}

fn internal(code: &str, message: String, correlation: &str) -> ApiError {
    ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, code, message, false)
        .with_correlation(correlation)
}

/// One path segment or query value, percent-encoded as `encodeURIComponent` would.
pub fn enc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn enc_matches_encode_uri_component() {
        assert_eq!(super::enc("a b&c/é"), "a%20b%26c%2F%C3%A9");
        assert_eq!(super::enc("it's-(ok)_~.*!"), "it's-(ok)_~.*!");
    }
}
