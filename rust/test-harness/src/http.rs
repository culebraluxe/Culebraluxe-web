//! Axum request helpers for L1/L3 tests.
//!
//! A route contract is exercised by sending a real `http::Request` through the real `axum::Router` the composition
//! root builds and reading the real `http::Response` back. Nothing here starts a TCP listener: the router is driven
//! in-process with `tower::ServiceExt::oneshot`, which is the same service the server serves, without a port, a
//! client, or a race for one.
//!
//! Every helper takes the router the production code would build. If a test needs a router, it composes it the way
//! `rust/server` composes it; the harness does not own a second one.

use axum::body::{to_bytes, Body, Bytes};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode};
use axum::Router;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tower::ServiceExt;

/// Build one in-process HTTP request.
pub struct TestRequest {
    method: Method,
    uri: String,
    headers: Vec<(HeaderName, HeaderValue)>,
    body: Vec<u8>,
}

impl TestRequest {
    pub fn new(method: Method, path: impl Into<String>) -> Self {
        Self {
            method,
            uri: path.into(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn get(path: impl Into<String>) -> Self {
        Self::new(Method::GET, path)
    }

    pub fn post(path: impl Into<String>) -> Self {
        Self::new(Method::POST, path)
    }

    pub fn put(path: impl Into<String>) -> Self {
        Self::new(Method::PUT, path)
    }

    pub fn patch(path: impl Into<String>) -> Self {
        Self::new(Method::PATCH, path)
    }

    pub fn delete(path: impl Into<String>) -> Self {
        Self::new(Method::DELETE, path)
    }

    /// Set the body to a JSON document and the matching content type.
    pub fn json<T: Serialize>(mut self, value: &T) -> Self {
        self.body = serde_json::to_vec(value).expect("a test fixture must serialize");
        self.headers.push((
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/json"),
        ));
        self
    }

    /// Set a raw body.
    pub fn body(mut self, bytes: impl Into<Vec<u8>>) -> Self {
        self.body = bytes.into();
        self
    }

    /// Set a UTF-8 body.
    pub fn text(self, text: impl Into<String>) -> Self {
        self.body(text.into().into_bytes())
    }

    /// Add a header. Panics on a name or value that is not a valid HTTP header, which a test should never send.
    pub fn header(mut self, name: &str, value: &str) -> Self {
        let name = HeaderName::from_bytes(name.as_bytes()).expect("valid header name");
        let value = HeaderValue::from_str(value).expect("valid header value");
        self.headers.push((name, value));
        self
    }

    /// Add `Authorization: Bearer <token>`.
    pub fn bearer(self, token: &str) -> Self {
        self.header("authorization", &format!("Bearer {token}"))
    }

    /// Add or extend the `Cookie` header.
    pub fn cookie(self, name: &str, value: &str) -> Self {
        self.header("cookie", &format!("{name}={value}"))
    }

    /// The request as the router will receive it.
    pub fn build(self) -> Request<Body> {
        let mut builder = Request::builder().method(self.method).uri(self.uri);
        for (name, value) in self.headers {
            builder = builder.header(name, value);
        }
        builder
            .body(Body::from(self.body))
            .expect("a test request must build")
    }
}

/// Send one request through `router` and read the response.
pub async fn call(router: &Router, request: TestRequest) -> TestResponse {
    let response = router
        .clone()
        .oneshot(request.build())
        .await
        .expect("an axum router is infallible");
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("a test response body must be readable");
    TestResponse {
        status,
        headers,
        body,
    }
}

/// A GET through `router`.
pub async fn get(router: &Router, path: impl Into<String>) -> TestResponse {
    call(router, TestRequest::get(path)).await
}

/// A JSON POST through `router`.
pub async fn post_json<T: Serialize>(
    router: &Router,
    path: impl Into<String>,
    value: &T,
) -> TestResponse {
    call(router, TestRequest::post(path).json(value)).await
}

/// What the router answered.
pub struct TestResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Bytes,
}

impl TestResponse {
    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }

    /// A response header as text, if present and valid UTF-8.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }

    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    pub fn bytes(&self) -> &[u8] {
        &self.body
    }

    /// The body as UTF-8 text (lossy: a binary body is still readable as replacement characters).
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// The body parsed as JSON, panicking with the body text on failure so the assertion message is useful.
    pub fn json<T: DeserializeOwned>(&self) -> T {
        serde_json::from_slice(&self.body).unwrap_or_else(|error| {
            panic!(
                "response was not the expected JSON ({error}); status {}; body: {}",
                self.status,
                self.text()
            )
        })
    }

    /// The body parsed as an untyped JSON value.
    pub fn json_value(&self) -> serde_json::Value {
        self.json()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::{get as route_get, post};

    fn router() -> Router {
        Router::new()
            .route(
                "/v1/echo",
                post(|body: String| async move { format!("hello {body}") }),
            )
            .route(
                "/v1/health",
                route_get(|| async { axum::Json(serde_json::json!({ "ok": true })) }),
            )
    }

    #[tokio::test]
    async fn a_get_returns_the_router_answer() {
        let response = get(&router(), "/v1/health").await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.json_value();
        assert_eq!(body["ok"], true);
    }

    #[tokio::test]
    async fn a_body_reaches_the_handler() {
        let response = call(&router(), TestRequest::post("/v1/echo").text("world")).await;
        assert_eq!(response.text(), "hello world");
    }

    #[tokio::test]
    async fn headers_are_carried_on_the_response() {
        let response = get(&router(), "/v1/health").await;
        assert_eq!(response.header("content-type"), Some("application/json"));
    }
}
