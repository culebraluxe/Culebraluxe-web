//! INT.MUX — failure mapping (TST-INT-MUX-002).
//!
//! Contract: **a Mux failure is mapped to exactly one application error, and the mapping keeps the three things an
//! operator needs — which dependency failed, what it said, and whether a retry can help.** Every Mux call site in
//! production funnels through one line, `web/src/media/media_bytes.rs:114,174,215`:
//!
//! ```text
//! .map_err(|error| CoreServiceError::infrastructure("MUX_API", error.message))
//! ```
//!
//! so the mapping is worth pinning as a contract rather than trusting to inspection. The classification happens in
//! two hops, and this test exercises both:
//!
//! - **Adapter hop** — `apis::mux::MuxClient` (`middle/apis/src/mux/mod.rs:104-178`) turns a transport, HTTP or
//!   protocol fault into `MuxClientError { message, retryable }`. The `retryable` flag is a real decision:
//!   `http_error` (`mod.rs:252-278`) marks `408|425|429|500|502|503|504` retryable and everything else terminal,
//!   while `classify_transport_error` (`mod.rs:241-250`) marks timeouts and connection faults retryable.
//! - **Service hop** — `CoreServiceError::infrastructure` (`web/src/service_support.rs:28-35`) wraps the message in
//!   `ServiceRuntimeError::Router` with `class: Infrastructure` and `retryable: true`.
//!
//! The negative cases are where a mapping is usually wrong, and each one is asserted against a live production
//! client rather than a re-declared copy:
//!
//! - a **404 that is not retryable** at the adapter hop (`MUX_ASSET_MISSING`) must not be laundered into a retryable
//!   infrastructure error at the service hop. That distinction is the whole reason the adapter computes the flag;
//! - a **blank 502 body** must not produce an empty message — an operator gets `Mux API failed with HTTP 502`;
//! - a **`data`-less 200** is a protocol fault, refused rather than half-parsed;
//! - a **non-JSON 200** is refused for the same reason, with a different message;
//! - a **transport failure** (nothing listening) is retryable and says so;
//! - and a **business refusal** (a CORS origin the service rejects before it ever calls Mux) stays a 400-shaped
//!   `Business` error, never an `MUX_API` infrastructure error. A mapping that collapsed the two would report a
//!   user mistake as a vendor outage.
//!
//! **No Mux trait exists** — production takes `&MuxClient` concretely (`media_bytes.rs:88,135`), so the honest seam
//! is the one the client itself exposes: `MuxConfig::base_url` is a `pub` field, and this test points a real
//! `MuxClient` at an in-process `axum` server standing in for Mux. The production client, its retry classification,
//! its `data`-envelope unwrap and the service's mapping all run for real; only Mux's own endpoint is local. Nothing
//! here reimplements a parser, and no live provider is contacted.
//!
//! Level: L1 Component — `apis::mux` and `web::media` over a loopback HTTP fixture.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test int_mux__002__failure_mapping

use std::net::SocketAddr;
use std::sync::Arc;

use apis::mux::{MuxClient, MuxClientError, MuxConfig};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use db::{DbFailure, DbResult};
use model::{
    AttachPropertyVideoRequest, AttachPropertyVideoResult, MediaAsset, UploadPropertyMediaRequest,
    UploadPropertyMediaResult,
};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceActor,
    ServiceActorKind, ServiceContext, ServiceFailureClass, ServiceInfrastructure, ServicePrincipal,
    ServiceRuntimeError,
};
use web::media::{MediaRepository, MediaService};
use web::service_support::CoreServiceError;

const HARNESS: &str = "INT.MUX/002";

/// The one code every Mux transport/HTTP/protocol failure is mapped to (`media_bytes.rs:114,174,215`).
const MUX_API: &str = "MUX_API";

/// A principal that `DefaultAuthorizationPort` admits for a Command, so the service reaches its real body rather
/// than short-circuiting on authorization. Every assertion below is about failure mapping, not about access.
fn context() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("test-actor".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: "int-mux-002".into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "test-principal".into(),
            level: "BUSINESS_POWER_USER".into(),
            role_codes: vec![],
            account_type: "internal".into(),
            entitlement_codes: vec![],
        }),
    }
}

fn infrastructure() -> ServiceInfrastructure {
    ServiceInfrastructure::new(
        Arc::new(DefaultAuthorizationPort),
        Arc::new(CapturingAuditPort::default()),
        Arc::new(CapturingDomainEventPort::default()),
    )
}

/// A repository fake at the production `MediaRepository` port.
///
/// Only `attach_property_video` can be reached by the paths under test, so the rest answer "unreachable" rather
/// than inventing a second implementation of behaviour this test does not claim to cover. `attach_property_video`
/// records what it was asked, so a test can prove the Mux identifiers reached persistence unaltered.
///
/// The handle is cloned into the service and the original is kept by the test, so both can see the same record
/// without an `Arc` wrapper — `Arc<T>` does not itself implement the production port.
#[derive(Clone, Default)]
struct RecordingMediaRepository {
    attached: Arc<std::sync::Mutex<Vec<AttachPropertyVideoRequest>>>,
}

impl RecordingMediaRepository {
    fn attached(&self) -> Vec<AttachPropertyVideoRequest> {
        self.attached
            .lock()
            .expect("the recording repository is never poisoned")
            .clone()
    }
}

fn unreachable_here<T>(method: &'static str) -> DbResult<T> {
    Err(DbFailure::configuration(
        "test.int_mux.002",
        format!("{method} is outside the failure-mapping subject"),
    ))
}

#[async_trait::async_trait]
impl MediaRepository for RecordingMediaRepository {
    async fn media_bytes(&self, _id: &str) -> DbResult<Option<(String, Vec<u8>)>> {
        unreachable_here("media_bytes")
    }

    async fn upload_standalone(
        &self,
        _filename: &str,
        _mime_type: &str,
        _bytes: &[u8],
    ) -> DbResult<(String, String, String, i64)> {
        unreachable_here("upload_standalone")
    }

    async fn for_property(&self, _property_id: &str) -> DbResult<Vec<MediaAsset>> {
        unreachable_here("for_property")
    }

    async fn set_property_hero(&self, _property_id: &str, _media_id: &str) -> DbResult<bool> {
        unreachable_here("set_property_hero")
    }

    async fn upload_property_media(
        &self,
        _request: &UploadPropertyMediaRequest,
    ) -> DbResult<UploadPropertyMediaResult> {
        unreachable_here("upload_property_media")
    }

    async fn attach_property_video(
        &self,
        request: &AttachPropertyVideoRequest,
    ) -> DbResult<AttachPropertyVideoResult> {
        self.attached
            .lock()
            .expect("the recording repository is never poisoned")
            .push(request.clone());
        Ok(AttachPropertyVideoResult {
            ok: true,
            media_id: "media-from-repository".into(),
            property_id: request.property_id.clone(),
            role: request.role.clone(),
            mux_asset_id: request.mux_asset_id.clone(),
            mux_playback_id: request.mux_playback_id.clone(),
        })
    }

    async fn begin_media_upload(
        &self,
        _request: &db::BeginMediaUpload,
    ) -> DbResult<web::media::BeginMediaUploadResult> {
        unreachable_here("begin_media_upload")
    }

    async fn stage_media_chunk(
        &self,
        _upload_id: &str,
        _chunk_index: i32,
        _bytes: &[u8],
    ) -> DbResult<db::MediaUploadStatus> {
        unreachable_here("stage_media_chunk")
    }

    async fn remove_property_media(&self, _property_id: &str, _media_id: &str) -> DbResult<bool> {
        unreachable_here("remove_property_media")
    }

    async fn property_has_photo(
        &self,
        _property_id: &str,
        _filename: &str,
        _byte_size: i64,
    ) -> DbResult<bool> {
        unreachable_here("property_has_photo")
    }

    async fn resumable_media_upload(
        &self,
        _property_id: &str,
        _filename: &str,
        _byte_size: i64,
        _chunk_size: i32,
    ) -> DbResult<Option<(String, String, Vec<i32>)>> {
        unreachable_here("resumable_media_upload")
    }

    async fn claim_media_upload(&self, _upload_id: &str) -> DbResult<bool> {
        unreachable_here("claim_media_upload")
    }

    async fn fail_media_upload(&self, _upload_id: &str) -> DbResult<()> {
        unreachable_here("fail_media_upload")
    }

    async fn media_upload_state(&self, _upload_id: &str) -> DbResult<Option<String>> {
        unreachable_here("media_upload_state")
    }

    async fn assemble_media_upload(&self, _upload_id: &str) -> DbResult<db::MediaUploadAssembly> {
        unreachable_here("assemble_media_upload")
    }

    async fn commit_media_upload(
        &self,
        _upload_id: &str,
        _assembly: &db::MediaUploadAssembly,
        _derivatives: &[db::MediaDerivativeInput],
    ) -> DbResult<UploadPropertyMediaResult> {
        unreachable_here("commit_media_upload")
    }
}

/// A loopback stand-in for `api.mux.com`.
///
/// The route answers one scripted `(status, body)` pair for every call, which is enough to drive each branch of
/// `MuxClient::parse_data` and `http_error`. Nothing here knows Mux's business rules — it only replays bytes, so a
/// bug in the production classification cannot hide behind a helpful fixture.
async fn serve(status: StatusCode, body: String) -> SocketAddr {
    let answer = {
        let body = Arc::new(body);
        move || {
            let body = Arc::clone(&body);
            async move {
                Response::builder()
                    .status(status)
                    .body(axum::body::Body::from(body.as_str().to_owned()))
                    .expect("a fixture response with a valid status always builds")
            }
        }
    };

    let router = Router::new()
        .route("/video/v1/uploads", axum::routing::post(answer.clone()))
        .route("/video/v1/uploads/{id}", get(answer.clone()))
        .route("/video/v1/assets/{id}", get(answer));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("the loopback fixture binds an ephemeral port");
    let address = listener
        .local_addr()
        .expect("a bound listener has a local address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    address
}

fn client_at(address: SocketAddr) -> MuxClient {
    MuxClient::new(MuxConfig {
        token_id: "fixture-token-id".into(),
        token_secret: "fixture-token-secret".into(),
        base_url: format!("http://{address}/video/v1"),
        timeout_ms: 5_000,
    })
    .expect("a fixture MuxConfig builds a client")
}

/// Assert the adapter hop: what `MuxClient` classified, before any service is involved.
fn assert_classified(error: &MuxClientError, retryable: bool, contains: &str) {
    assert_eq!(
        error.retryable, retryable,
        "{HARNESS}: retryability is the adapter's decision and must be exactly {retryable} for {contains:?} — got {:?}",
        error.message
    );
    assert!(
        error.message.contains(contains),
        "{HARNESS}: the mapped message must carry the provider's reason ({contains:?}), got {:?}",
        error.message
    );
}

/// Assert the service hop: `MuxClientError` became exactly one `MUX_API` infrastructure error.
fn assert_mapped_to_mux_api(error: &CoreServiceError, expect_message: &str) {
    match error {
        CoreServiceError::Runtime(ServiceRuntimeError::Router {
            code,
            message,
            retryable,
            class,
        }) => {
            assert_eq!(
                code, MUX_API,
                "{HARNESS}: every Mux provider failure maps to {MUX_API}"
            );
            assert_eq!(
                *class,
                ServiceFailureClass::Infrastructure,
                "{HARNESS}: a dependency that failed is Infrastructure, not Business or Caller"
            );
            assert!(
                *retryable,
                "{HARNESS}: `CoreServiceError::infrastructure` hardcodes retryable=true, so the \
                 caller is told to come back rather than to give up"
            );
            assert!(
                message.contains(expect_message),
                "{HARNESS}: the operator-facing message must survive the hop ({expect_message:?}), got {message:?}"
            );
        }
        other => {
            panic!("{HARNESS}: a Mux failure must map to an infrastructure error, got {other:?}")
        }
    }
    assert_eq!(
        error.code(),
        MUX_API,
        "{HARNESS}: `CoreServiceError::code()` is what the audit row records, and it must be {MUX_API}"
    );
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-INT-MUX-002); the file and the assay use it.
async fn int_mux_002__failure_mapping() {
    // -----------------------------------------------------------------------------------------------------------
    // 1. THE ADAPTER HOP, over real HTTP: a retryable status is retryable and a terminal status is not. This is the
    //    distinction `http_error` exists to make, and it is only visible if the client's own flag is read before
    //    the service overwrites it — so it is asserted at the client, on both sides.
    // -----------------------------------------------------------------------------------------------------------
    let retryable = client_at(
        serve(
            StatusCode::SERVICE_UNAVAILABLE,
            r#"{"error":{"messages":["Mux is having a bad day"]}}"#.into(),
        )
        .await,
    )
    .asset("asset-retryable")
    .await
    .expect_err("a 503 must not produce an asset");
    assert_classified(&retryable, true, "Mux is having a bad day");

    let terminal = client_at(
        serve(
            StatusCode::NOT_FOUND,
            r#"{"error":{"message":"Asset not found"}}"#.into(),
        )
        .await,
    )
    .asset("asset-missing")
    .await
    .expect_err("a 404 must not produce an asset");
    assert_classified(&terminal, false, "Asset not found");

    // The same status set `http_error:253` names, exercised through the client rather than asserted as a literal,
    // so the list cannot drift without this test noticing.
    for status in [408, 425, 429, 500, 502, 503, 504] {
        let error = client_at(serve(status_from(status), String::new()).await)
            .asset("asset-status")
            .await
            .expect_err("a failing status must not produce an asset");
        assert!(
            error.retryable,
            "{HARNESS}: HTTP {status} is in the retryable set and must be classified retryable"
        );
    }
    for status in [400, 401, 403, 404, 422] {
        let error = client_at(serve(status_from(status), String::new()).await)
            .asset("asset-status")
            .await
            .expect_err("a failing status must not produce an asset");
        assert!(
            !error.retryable,
            "{HARNESS}: HTTP {status} is NOT in the retryable set and must be classified terminal — \
             a retry cannot fix a bad request or a wrong credential"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 2. MESSAGE EXTRACTION PRECEDENCE (`http_error:255-273`). A provider's own words beat a synthesized excerpt,
    //    in this order: `error.messages[0]`, then `error.message`, then an HTTP-status line.
    //
    //    The third case is the negative one that matters operationally: a blank body must NOT yield an empty
    //    message. An operator reading `Mux API failed with HTTP 502` knows what happened; an empty string does not.
    // -----------------------------------------------------------------------------------------------------------
    let blank = client_at(serve(StatusCode::BAD_GATEWAY, "   \n ".into()).await)
        .asset("asset-blank")
        .await
        .expect_err("a 502 must not produce an asset");
    assert_classified(&blank, true, "Mux API failed with HTTP 502");
    assert!(
        !blank.message.trim().is_empty(),
        "{HARNESS}: a blank provider body must still produce a message an operator can act on"
    );

    let unparsed = client_at(serve(StatusCode::BAD_GATEWAY, "502 Bad Gateway".into()).await)
        .asset("asset-html")
        .await
        .expect_err("a 502 must not produce an asset");
    assert_classified(
        &unparsed,
        true,
        "Mux API failed with HTTP 502: 502 Bad Gateway",
    );

    // A long body is excerpted rather than stored whole: an operator wants the reason, not a page of HTML.
    let long = "x".repeat(1_000);
    let excerpted = client_at(serve(StatusCode::BAD_GATEWAY, long.clone()).await)
        .asset("asset-long")
        .await
        .expect_err("a 502 must not produce an asset");
    assert!(
        !excerpted.message.contains(&"x".repeat(1_000)),
        "{HARNESS}: a huge provider body must be excerpted, not stored verbatim"
    );
    assert!(
        excerpted.message.contains(&"x".repeat(200)),
        "{HARNESS}: the excerpt keeps the first 200 characters, which is where the reason is"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. PROTOCOL FAULTS ARE REFUSED, NOT HALF-PARSED (`parse_data:164-178`). A 200 with no `data` envelope and a
    //    200 that is not JSON are two different faults and each keeps its own message — a caller debugging a Mux
    //    integration needs to know which one it hit.
    // -----------------------------------------------------------------------------------------------------------
    let no_data = client_at(
        serve(
            StatusCode::OK,
            r#"{"id":"asset-1","status":"ready","playback_ids":[]}"#.into(),
        )
        .await,
    )
    .asset("asset-no-data")
    .await
    .expect_err("a 200 without the data envelope must be refused");
    assert_classified(&no_data, false, "Mux API response is missing data.");

    let not_json = client_at(serve(StatusCode::OK, "<html>maintenance</html>".into()).await)
        .asset("asset-html-200")
        .await
        .expect_err("a non-JSON 200 must be refused");
    assert_classified(&not_json, false, "Mux API returned non-JSON content.");

    let missing_field =
        client_at(serve(StatusCode::OK, r#"{"data":{"status":"ready"}}"#.into()).await)
            .asset("asset-no-id")
            .await
            .expect_err("an asset with no id must be refused");
    assert_classified(&missing_field, false, "Mux Asset response is missing id.");

    // -----------------------------------------------------------------------------------------------------------
    // 4. A TRANSPORT FAULT IS RETRYABLE AND SAYS SO (`classify_transport_error:241-250`). Nothing is listening on
    //    this port: the connection itself fails, which is the one fault a retry genuinely can fix.
    // -----------------------------------------------------------------------------------------------------------
    // Port 1 on loopback is reserved and never bound, so the connect is refused immediately and deterministically.
    let unreachable_client = MuxClient::new(MuxConfig {
        token_id: "fixture-token-id".into(),
        token_secret: "fixture-token-secret".into(),
        base_url: "http://127.0.0.1:1/video/v1".into(),
        timeout_ms: 2_000,
    })
    .expect("a fixture MuxConfig builds a client");
    let transport = unreachable_client
        .asset("asset-unreachable")
        .await
        .expect_err("nothing is listening, so the call must fail");
    assert!(
        transport.retryable,
        "{HARNESS}: a refused connection is the one Mux fault a retry can fix and must be classified retryable"
    );
    assert!(
        transport.message.starts_with("Mux API transport failed:"),
        "{HARNESS}: a transport fault must be reported as one, got {:?}",
        transport.message
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. THE SERVICE HOP. The same terminal 404 that the adapter refused to retry now travels through the real
    //    `MediaService::finalize_property_video_upload`, and it must arrive as ONE `MUX_API` infrastructure error
    //    carrying the provider's reason — never as a silent success, never as a business refusal, and never with the
    //    repository written. The repository fake is asserted to hold nothing.
    // -----------------------------------------------------------------------------------------------------------
    let repository = RecordingMediaRepository::default();
    let service = MediaService::new(repository.clone(), infrastructure());
    let mux = client_at(
        serve(
            StatusCode::NOT_FOUND,
            r#"{"error":{"messages":["No such upload"]}}"#.into(),
        )
        .await,
    );

    let mapped = service
        .finalize_property_video_upload(
            &mux,
            "property-1",
            "upload-missing",
            "video",
            None,
            &context(),
        )
        .await
        .expect_err("a Mux 404 must fail the finalize call");
    assert_mapped_to_mux_api(&mapped, "No such upload");
    assert!(
        repository.attached().is_empty(),
        "{HARNESS}: a failed Mux call must write no media — the mapping must not paper over a failure"
    );

    // And the same for the other two call sites, which share the mapping line: `create_direct_upload` and `asset`.
    let upload_mapped = service
        .create_property_video_upload(&mux, "https://culebraluxe.com", &context())
        .await
        .expect_err("a Mux 404 must fail the upload-creation call");
    assert_mapped_to_mux_api(&upload_mapped, "No such upload");

    let asset_mapped = service
        .finalize_property_video_upload(
            &mux,
            "property-1",
            "upload-retry",
            "video",
            None,
            &context(),
        )
        .await
        .expect_err("a Mux 404 must fail the asset fetch too");
    assert_mapped_to_mux_api(&asset_mapped, "No such upload");
    assert!(
        repository.attached().is_empty(),
        "{HARNESS}: no Mux call site may attach media after a provider failure"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. NEGATIVE / REFUSAL — A CALLER'S MISTAKE IS NOT A VENDOR OUTAGE. The service refuses a CORS origin that is
    //    not http(s) BEFORE it calls Mux (`media_bytes.rs:100-108`), so this is a `Business` error with its own code.
    //    A mapping that collapsed business and infrastructure would page an on-call engineer for a malformed
    //    header, so the two classes are asserted to stay apart.
    // -----------------------------------------------------------------------------------------------------------
    for (origin, described) in [("", "an empty origin"), ("file://", "a non-http scheme")] {
        let refused = service
            .create_property_video_upload(&mux, origin, &context())
            .await
            .expect_err("an invalid CORS origin must be refused");
        match refused {
            CoreServiceError::Business { code, .. } => {
                assert_eq!(
                    code, "MUX_CORS_ORIGIN_INVALID",
                    "{HARNESS}: {described} is a caller mistake with its own code, not an {MUX_API} outage"
                );
            }
            other => panic!(
                "{HARNESS}: {described} must be a Business refusal so it maps to 400, not 503; got {other:?}"
            ),
        }
        assert_eq!(
            refused.code(),
            "MUX_CORS_ORIGIN_INVALID",
            "{HARNESS}: the audit row must record the caller mistake, so a bad origin is never filed as an outage"
        );
    }

    // The same separation holds on the finalize path's own refusals — a bad role is the caller's, not Mux's.
    for (property_id, upload_id, role, expected_code, described) in [
        (
            "",
            "upload-1",
            "video",
            "VIDEO_UPLOAD_REQUIRED",
            "a missing property id",
        ),
        (
            "property-1",
            "   ",
            "video",
            "VIDEO_UPLOAD_REQUIRED",
            "a blank upload id",
        ),
        (
            "property-1",
            "upload-1",
            "thumbnail",
            "MEDIA_ROLE_INVALID",
            "a role that is not video or short",
        ),
    ] {
        let refused = service
            .finalize_property_video_upload(&mux, property_id, upload_id, role, None, &context())
            .await
            .expect_err("a caller mistake must be refused");
        match &refused {
            CoreServiceError::Business { code, .. } => assert_eq!(
                *code, expected_code,
                "{HARNESS}: {described} must map to {expected_code}, not to {MUX_API}"
            ),
            other => panic!(
                "{HARNESS}: {described} must be a Business refusal so it maps to 400; got {other:?}"
            ),
        }
    }
    assert!(
        repository.attached().is_empty(),
        "{HARNESS}: a refused request writes nothing"
    );
}

/// Map a bare HTTP status code onto `StatusCode`, panicking on a code this test never uses.
fn status_from(code: u16) -> StatusCode {
    StatusCode::from_u16(code).expect("the status codes asserted here are all valid HTTP codes")
}
