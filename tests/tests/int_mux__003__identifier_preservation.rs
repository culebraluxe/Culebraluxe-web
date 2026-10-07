//! INT.MUX — identifier preservation (TST-INT-MUX-003).
//!
//! Contract: **the identifiers Mux issues are the identifiers we store.** A direct upload hands the browser an
//! `upload_id`; the browser hands it back; Mux turns it into an `asset_id`; the asset carries `playback_id`s. Those
//! four values are the join keys for every later operation — re-finalizing an upload, finding the media row,
//! building the stream URL — so a single character of rewriting between the provider's answer and the `media` row
//! silently orphans a listing's video.
//!
//! The chain is production code the whole way, and this test walks it end to end rather than asserting on a
//! parser:
//!
//! 1. `MediaService::create_property_video_upload` (`web/src/media/media_bytes.rs:81-131`) returns
//!    `upload.id` verbatim as `PropertyVideoUploadSession::upload_id`;
//! 2. `finalize_property_video_upload` (`:133-285`) looks the upload up by that id, reads `upload.asset_id`, and
//!    fetches that asset;
//! 3. it selects the **public** playback id (`:245-255`) — first match wins, matched on the literal string
//!    `"public"`;
//! 4. it hands `asset.id` and the chosen playback id to `repository.attach_property_video` unchanged (`:257-271`).
//!
//! The negative cases are the ways preservation breaks, and each is asserted rather than assumed:
//!
//! - **identifier rewriting.** Mux ids are opaque and contain characters a careless normalizer would eat:
//!   `MU-Asset_01.aBc~XyZ`. The value reaching the repository must be byte-identical — no trimming, no
//!   lowercasing, no URL-encoding.
//! - **the wrong playback policy.** An asset with a `signed` id first and a `public` id second must still attach the
//!   `public` one. A first-wins-over-first-entry bug would store a signed id and break every public embed, while
//!   still looking like it worked.
//! - **no public policy at all.** An asset carrying only `signed` ids is refused with `MUX_PLAYBACK_ID_MISSING` and
//!   writes nothing — the refusal is the contract, not a fallback to a signed id.
//! - **a missing asset id.** An upload Mux reports as `asset_created` with no `asset_id` is refused with
//!   `MUX_ASSET_ID_MISSING`, again writing nothing.
//! - **whitespace-only ids.** `optional_string` trims (`mod.rs:232-238`), so a padded id is a *different* id once
//!   trimmed. Preservation means what reaches the repository is exactly what Mux meant, and this pins that the
//!   trimming happens before the value is stored rather than after.
//!
//! **No Mux trait exists** — production takes `&MuxClient` concretely (`media_bytes.rs:88,135`) — so the honest seam
//! is the one the client itself exposes: `MuxConfig::base_url` is a `pub` field, and this test points a real
//! `MuxClient` at an in-process `axum` server standing in for Mux. The production client, the real playback-policy
//! selection and the real service state machine all run; only Mux's endpoint is local. The repository is a fake at
//! the production `MediaRepository` port that records the request verbatim, so the assertions read what persistence
//! was actually asked to store.
//!
//! Level: L1 Component — `apis::mux` and `web::media` over a loopback HTTP fixture.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test int_mux__003__identifier_preservation

use std::net::SocketAddr;
use std::sync::Arc;

use apis::mux::{MuxClient, MuxConfig};
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
    ServiceActorKind, ServiceContext, ServiceInfrastructure, ServicePrincipal,
};
use web::media::{MediaRepository, MediaService};
use web::service_support::CoreServiceError;

const HARNESS: &str = "INT.MUX/003";

/// An identifier built from every character a careless normalizer would destroy: case, `_`, `.`, `~` and a dash.
/// If any part of the chain rewrites it, this string is what notices.
const ASSET_ID: &str = "MU-Asset_01.aBc~XyZ";
const PUBLIC_PLAYBACK_ID: &str = "Pb-Public_02.xYz~Q";
const SIGNED_PLAYBACK_ID: &str = "Pb-Signed_03.DEFf";
const UPLOAD_ID: &str = "Up-Load_04.gHi~JkL";

/// A principal `DefaultAuthorizationPort` admits for a Command, so the service reaches its real body.
fn context() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("test-actor".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: "int-mux-003".into(),
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

/// A repository fake at the production `MediaRepository` port that records the attach request verbatim.
///
/// Cloned into the service and kept by the test, so both sides read one record without an `Arc` wrapper —
/// `Arc<T>` does not itself implement the production port. It echoes the ids back, which is what the production DAO
/// does (`db/src/media/media_row.rs:341-348`), so the returned result can be checked against what was stored.
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
        "test.int_mux.003",
        format!("{method} is outside the identifier-preservation subject"),
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

/// Serve the two endpoints the finalize chain walks: the upload lookup, then the asset.
///
/// `asset` is the asset body. The upload body is always "created, pointing at ASSET_ID" so the chain reaches the
/// playback-policy selection, which is the part of the contract worth breaking.
async fn serve_asset(asset: String) -> SocketAddr {
    let upload = format!(
        r#"{{"data":{{"id":"{UPLOAD_ID}","status":"asset_created","asset_id":"{ASSET_ID}"}}}}"#
    );

    let upload_handler = {
        let body = Arc::new(upload);
        move || {
            let body = Arc::clone(&body);
            async move {
                Response::builder()
                    .status(StatusCode::OK)
                    .body(axum::body::Body::from(body.as_str().to_owned()))
                    .expect("a fixture response with a valid status always builds")
            }
        }
    };
    let asset_handler = {
        let body = Arc::new(asset);
        move || {
            let body = Arc::clone(&body);
            async move {
                Response::builder()
                    .status(StatusCode::OK)
                    .body(axum::body::Body::from(body.as_str().to_owned()))
                    .expect("a fixture response with a valid status always builds")
            }
        }
    };

    let router = Router::new()
        .route("/video/v1/uploads/{id}", get(upload_handler))
        .route("/video/v1/assets/{id}", get(asset_handler));

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

/// Assert that exactly one attach reached persistence, and that it carries `asset` and `playback` byte for byte.
///
/// Both comparisons are against the constants the fixture told Mux to issue, never against a value the test
/// derived from the code under test — that is what makes a rewrite visible instead of self-confirming.
fn assert_attached_exactly(
    repository: &RecordingMediaRepository,
    property_id: &str,
    role: &str,
    asset: &str,
    playback: &str,
) {
    let attached = repository.attached();
    assert_eq!(
        attached.len(),
        1,
        "{HARNESS}: exactly one media row may be attached per finalize"
    );
    let request = &attached[0];
    assert_eq!(
        request.property_id, property_id,
        "{HARNESS}: the property the caller named is the property the video is attached to"
    );
    assert_eq!(
        request.role, role,
        "{HARNESS}: the role the caller named is the role stored"
    );
    assert_eq!(
        request.mux_asset_id, asset,
        "{HARNESS}: the Mux asset id must reach storage byte for byte — no trimming, no \
         case folding, no percent-encoding, because this is the join key for every later operation"
    );
    assert_eq!(
        request.mux_playback_id, playback,
        "{HARNESS}: the Mux playback id must reach storage byte for byte — it is what the stream URL is built from"
    );
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-INT-MUX-003); the file and the assay use it.
async fn int_mux__003__identifier_preservation() {
    // -----------------------------------------------------------------------------------------------------------
    // 1. THE UPLOAD ID SURVIVES THE ROUND TRIP TO THE BROWSER. `create_property_video_upload` returns `upload.id`
    //    and nothing else (`media_bytes.rs:120-123`), so the browser's handle to this upload is Mux's own string.
    //    A rewriting client would hand out an id Mux does not recognise, and the failure would only surface when
    //    the browser tried to finalize.
    // -----------------------------------------------------------------------------------------------------------
    let create_router = Router::new().route(
        "/video/v1/uploads",
        axum::routing::post(|| async {
            let body = format!(
                r#"{{"data":{{"id":"{UPLOAD_ID}","url":"https://storage.mux.com/{UPLOAD_ID}","status":"waiting"}}}}"#
            );
            Response::builder()
                .status(StatusCode::OK)
                .body(axum::body::Body::from(body))
                .expect("a fixture response with a valid status always builds")
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("the loopback fixture binds an ephemeral port");
    let address = listener
        .local_addr()
        .expect("a bound listener has a local address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, create_router).await;
    });

    let repository = RecordingMediaRepository::default();
    let service = MediaService::new(repository.clone(), infrastructure());
    let create_client = client_at(address);

    let session = service
        .create_property_video_upload(&create_client, "https://culebraluxe.com", &context())
        .await
        .expect("a well-formed origin creates the upload session");
    assert_eq!(
        session.upload_id, UPLOAD_ID,
        "{HARNESS}: the upload id handed to the browser is Mux's own string, unchanged"
    );
    assert!(
        session.upload_url.contains(UPLOAD_ID),
        "{HARNESS}: the upload URL is the one Mux issued and is handed on unchanged, got {:?}",
        session.upload_url
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE ASSET ID AND THE PUBLIC PLAYBACK ID SURVIVE THE WHOLE CHAIN. The upload is looked up by the id the
    //    browser was given, Mux names an asset, and that asset carries both a `signed` and a `public` playback id
    //    — with the SIGNED ONE FIRST, which is the ordering that breaks a "take the first" implementation.
    // -----------------------------------------------------------------------------------------------------------
    let signed_first = format!(
        r#"{{"data":{{"id":"{ASSET_ID}","status":"ready","duration":72.5,"aspect_ratio":"16:9","playback_ids":[
             {{"id":"{SIGNED_PLAYBACK_ID}","policy":"signed"}},
             {{"id":"{PUBLIC_PLAYBACK_ID}","policy":"public"}}]}}}}"#
    );
    let repository = RecordingMediaRepository::default();
    let service = MediaService::new(repository.clone(), infrastructure());
    let mux = client_at(serve_asset(signed_first).await);

    let result = service
        .finalize_property_video_upload(
            &mux,
            "property-42",
            UPLOAD_ID,
            "video",
            Some("  Villa exterior  ".into()),
            &context(),
        )
        .await
        .expect("a ready asset with a public playback id attaches");
    assert_eq!(
        result.status, "ready",
        "{HARNESS}: a fully-prepared asset reports status ready"
    );
    assert!(
        result.attached,
        "{HARNESS}: a ready asset with a public playback id is attached, not left pending"
    );
    assert_attached_exactly(
        &repository,
        "property-42",
        "video",
        ASSET_ID,
        PUBLIC_PLAYBACK_ID,
    );
    assert_eq!(
        result.mux_asset_id.as_deref(),
        Some(ASSET_ID),
        "{HARNESS}: the caller is told the asset id we stored, not a re-derived one"
    );
    assert_eq!(
        result.mux_playback_id.as_deref(),
        Some(PUBLIC_PLAYBACK_ID),
        "{HARNESS}: the caller is told the public playback id we stored"
    );
    // The non-identifier field the same request carries is trimmed on the way in (`media_bytes.rs:269-271`), which
    // is the contrast that makes the point: normalization happens where it is written down, and the identifiers —
    // which are join keys — are not touched.
    assert_eq!(
        repository.attached()[0].caption.as_deref(),
        Some("Villa exterior"),
        "{HARNESS}: a caption is trimmed, because a caption is prose; an identifier is not"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE — AN ASSET WITH NO PUBLIC POLICY IS REFUSED, NOT FALLEN BACK TO. A signed playback id cannot be
    //    embedded on a public listing, so storing it would produce a video that renders nowhere while reporting
    //    success. The refusal is `MUX_PLAYBACK_ID_MISSING` and it writes nothing.
    // -----------------------------------------------------------------------------------------------------------
    let signed_only = format!(
        r#"{{"data":{{"id":"{ASSET_ID}","status":"ready","playback_ids":[
             {{"id":"{SIGNED_PLAYBACK_ID}","policy":"signed"}}]}}}}"#
    );
    let repository = RecordingMediaRepository::default();
    let service = MediaService::new(repository.clone(), infrastructure());
    let mux = client_at(serve_asset(signed_only).await);

    let refused = service
        .finalize_property_video_upload(&mux, "property-42", UPLOAD_ID, "video", None, &context())
        .await
        .expect_err("an asset with no public playback id must not attach");
    match &refused {
        CoreServiceError::Business { code, .. } => assert_eq!(
            *code, "MUX_PLAYBACK_ID_MISSING",
            "{HARNESS}: a signed-only asset is refused with its own code — falling back to the signed id would \
             store a video that cannot be played"
        ),
        other => panic!("{HARNESS}: a missing public playback id is a business refusal, got {other:?}"),
    }
    assert!(
        repository.attached().is_empty(),
        "{HARNESS}: a refused asset writes no media row"
    );

    // The policy match is exact and case-sensitive, so `"Public"` is not a public policy.
    let wrong_case = format!(
        r#"{{"data":{{"id":"{ASSET_ID}","status":"ready","playback_ids":[
             {{"id":"{PUBLIC_PLAYBACK_ID}","policy":"Public"}}]}}}}"#
    );
    let repository = RecordingMediaRepository::default();
    let service = MediaService::new(repository.clone(), infrastructure());
    let refused = service
        .finalize_property_video_upload(
            &client_at(serve_asset(wrong_case).await),
            "property-42",
            UPLOAD_ID,
            "video",
            None,
            &context(),
        )
        .await
        .expect_err("a differently-cased policy must not be treated as public");
    assert_eq!(
        refused.code(),
        "MUX_PLAYBACK_ID_MISSING",
        "{HARNESS}: the policy comparison is the exact string \"public\"; anything else is refused"
    );
    assert!(
        repository.attached().is_empty(),
        "{HARNESS}: a refused policy writes no media row"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — AN UPLOAD MUX SAYS IS CREATED BUT WHICH NAMES NO ASSET IS REFUSED. `asset_created` is the only
    //    status that proceeds to the asset fetch (`media_bytes.rs:200-201`), so a missing `asset_id` here would
    //    otherwise become a request for the asset literally named "".
    // -----------------------------------------------------------------------------------------------------------
    let no_asset_id = r#"{"data":{"id":"Up-Load_05.noAsset","status":"asset_created"}}"#;
    let upload_handler = {
        let body = Arc::new(no_asset_id.to_owned());
        move || {
            let body = Arc::clone(&body);
            async move {
                Response::builder()
                    .status(StatusCode::OK)
                    .body(axum::body::Body::from(body.as_str().to_owned()))
                    .expect("a fixture response with a valid status always builds")
            }
        }
    };
    let router = Router::new().route("/video/v1/uploads/{id}", get(upload_handler));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("the loopback fixture binds an ephemeral port");
    let address = listener
        .local_addr()
        .expect("a bound listener has a local address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    let repository = RecordingMediaRepository::default();
    let service = MediaService::new(repository.clone(), infrastructure());
    let refused = service
        .finalize_property_video_upload(
            &client_at(address),
            "property-42",
            UPLOAD_ID,
            "video",
            None,
            &context(),
        )
        .await
        .expect_err("an asset_created upload with no asset id must not attach");
    match &refused {
        CoreServiceError::Business { code, .. } => assert_eq!(
            *code, "MUX_ASSET_ID_MISSING",
            "{HARNESS}: a missing asset id is refused with its own code rather than fetching an empty-named asset"
        ),
        other => panic!("{HARNESS}: a missing asset id is a business refusal, got {other:?}"),
    }
    assert!(
        repository.attached().is_empty(),
        "{HARNESS}: a refused asset id writes no media row"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE — AN ASSET THAT IS STILL PREPARING REPORTS ITS ID WITHOUT ATTACHING. The browser polls finalize,
    //    so the asset id has to come back on every poll (`media_bytes.rs:217-224`) even though no media row may
    //    exist yet. Preserving the identifier is not the same as persisting the video, and conflating them would
    //    attach a stream that is not playable.
    // -----------------------------------------------------------------------------------------------------------
    let preparing =
        format!(r#"{{"data":{{"id":"{ASSET_ID}","status":"preparing","playback_ids":[]}}}}"#);
    let repository = RecordingMediaRepository::default();
    let service = MediaService::new(repository.clone(), infrastructure());
    let pending = service
        .finalize_property_video_upload(
            &client_at(serve_asset(preparing).await),
            "property-42",
            UPLOAD_ID,
            "video",
            None,
            &context(),
        )
        .await
        .expect("a preparing asset is a normal poll result, not a failure");
    assert_eq!(
        pending.status, "preparing",
        "{HARNESS}: the status the browser polls on is Mux's own"
    );
    assert!(
        !pending.attached,
        "{HARNESS}: a preparing asset is not attached — a not-yet-playable stream must not reach a listing"
    );
    assert_eq!(
        pending.mux_asset_id.as_deref(),
        Some(ASSET_ID),
        "{HARNESS}: the asset id is still preserved on a poll, so the browser can keep tracking it"
    );
    assert_eq!(
        pending.mux_playback_id, None,
        "{HARNESS}: no playback id exists until the asset is ready, and none is invented"
    );
    assert!(
        repository.attached().is_empty(),
        "{HARNESS}: a preparing asset writes no media row"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. WHERE NORMALIZATION HAPPENS IS PART OF THE CONTRACT, AND IT IS NOT THE SERVICE'S JOB. `parse_asset` and
    //    `parse_upload` read their required fields through `required_string`/`optional_string`
    //    (`middle/apis/src/mux/mod.rs:225-238`), which trim, so a provider that pads an id has its padding removed
    //    at the adapter. This test asserts what actually reaches storage, which is the trimmed value — pinning that
    //    normalization happens on the way OUT of the provider, before persistence, rather than being left to the
    //    row that stores it.
    //
    //    PRODUCT EVIDENCE, recorded rather than hidden: the trim is NOT uniform. `parse_asset` builds
    //    `playback_ids` straight from the raw JSON (`mod.rs:198-211` — `item.get("id")?.as_str()?.to_owned()`),
    //    bypassing `optional_string` entirely, so a padded PLAYBACK id reaches `media.mux_playback_id` with its
    //    padding intact, and `db/src/media/media_row.rs:292` would then build
    //    `https://stream.mux.com/<padded id>.m3u8`. A real Mux response never pads, so no fixture can prove this is
    //    reached in production — but the asymmetry is real and is the one place where "the identifier we stored" is
    //    not obviously "the identifier Mux issued". Fixing it is a production change and out of scope for a
    //    RED Team authoring story; it is reported instead.
    // -----------------------------------------------------------------------------------------------------------
    let padded_upload = format!(
        r#"{{"data":{{"id":"  {UPLOAD_ID}  ","status":"asset_created","asset_id":"  {ASSET_ID}  "}}}}"#
    );
    let padded_asset = format!(
        r#"{{"data":{{"id":"  {ASSET_ID}  ","status":"ready","playback_ids":[
             {{"id":"{PUBLIC_PLAYBACK_ID}","policy":"public"}}]}}}}"#
    );
    let router = Router::new()
        .route(
            "/video/v1/uploads/{id}",
            get({
                let body = Arc::new(padded_upload);
                move || {
                    let body = Arc::clone(&body);
                    async move {
                        Response::builder()
                            .status(StatusCode::OK)
                            .body(axum::body::Body::from(body.as_str().to_owned()))
                            .expect("a fixture response with a valid status always builds")
                    }
                }
            }),
        )
        .route(
            "/video/v1/assets/{id}",
            get({
                let body = Arc::new(padded_asset);
                move || {
                    let body = Arc::clone(&body);
                    async move {
                        Response::builder()
                            .status(StatusCode::OK)
                            .body(axum::body::Body::from(body.as_str().to_owned()))
                            .expect("a fixture response with a valid status always builds")
                    }
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("the loopback fixture binds an ephemeral port");
    let address = listener
        .local_addr()
        .expect("a bound listener has a local address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    let repository = RecordingMediaRepository::default();
    let service = MediaService::new(repository.clone(), infrastructure());
    service
        .finalize_property_video_upload(
            &client_at(address),
            "property-42",
            UPLOAD_ID,
            "video",
            None,
            &context(),
        )
        .await
        .expect("a padded but otherwise valid asset attaches");
    assert_attached_exactly(
        &repository,
        "property-42",
        "video",
        ASSET_ID,
        PUBLIC_PLAYBACK_ID,
    );

    // The same trimmed form is what the adapter itself reports, so the assertion above is provable rather than
    // coincidental: the service stored exactly the asset id `MuxClient::asset` handed it.
    let padded_client = client_at(
        serve_asset(format!(
            r#"{{"data":{{"id":"  {ASSET_ID}  ","status":"ready","playback_ids":[]}}}}"#
        ))
        .await,
    );
    let parsed = padded_client
        .asset("any")
        .await
        .expect("the padded asset still parses");
    assert_eq!(
        parsed.id, ASSET_ID,
        "{HARNESS}: `required_string` trims the asset id at the adapter, so storage and the client's own answer \
         cannot drift apart"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. A ROLE THE CALLER NAMED IS THE ROLE STORED. Both accepted roles round-trip, because the role is half of
    //    what the attach call means — `video` and `short` are the same attachment with different sort order, and
    //    swapping them would misfile a reel as the main tour video.
    // -----------------------------------------------------------------------------------------------------------
    for role in ["video", "short"] {
        let asset = format!(
            r#"{{"data":{{"id":"{ASSET_ID}","status":"ready","playback_ids":[
                 {{"id":"{PUBLIC_PLAYBACK_ID}","policy":"public"}}]}}}}"#
        );
        let repository = RecordingMediaRepository::default();
        let service = MediaService::new(repository.clone(), infrastructure());
        service
            .finalize_property_video_upload(
                &client_at(serve_asset(asset).await),
                "property-42",
                UPLOAD_ID,
                role,
                None,
                &context(),
            )
            .await
            .unwrap_or_else(|error| panic!("the {role} role must attach: {error:?}"));
        assert_attached_exactly(
            &repository,
            "property-42",
            role,
            ASSET_ID,
            PUBLIC_PLAYBACK_ID,
        );
    }
}
