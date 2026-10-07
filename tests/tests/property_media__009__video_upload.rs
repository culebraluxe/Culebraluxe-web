//! PROPERTY.MEDIA — video upload (TST-PROPERTY-MEDIA-009).
//!
//! Contract: **a Property video is uploaded directly to Mux and is attached to the Property only when Mux says the
//! asset is ready and publicly playable.** A video is the one Property asset the browser never posts to us — the
//! server opens a Direct Upload, hands the browser the Mux URL, and then finalizes.
//!
//! That makes `MediaService::finalize_property_video_upload` (`web/src/media/media_bytes.rs:133-285`) the only
//! place a `media_type = 'video'` row can come into existence, and its whole job is to refuse to create one early.
//! The gates are ordered, and the order is the contract:
//!
//! 1. `property_id` and `upload_id` are both trimmed and must be non-empty — else `VIDEO_UPLOAD_REQUIRED` (:158).
//! 2. `role` must be exactly `video` or `short` — else `MEDIA_ROLE_INVALID` (:164). A photo role here would write
//!    a video row into the hero slot of a Property's gallery.
//! 3. Mux is asked about the upload. `waiting` answers "not yet, nothing attached" (:177); `errored`, `cancelled`
//!    and `timed_out` are refusals (:186); only `asset_created` proceeds (:194).
//! 4. The asset must be `ready` — `preparing` answers "not yet" (:218), `errored` is a refusal (:227).
//! 5. The asset must carry a playback id whose `policy` is exactly `public` (:245-255). A `signed`-only asset is a
//!    video the public site cannot play, and linking it would put a broken player on a live listing.
//! 6. Only then is `repository.attach_property_video` called (:257).
//!
//! Every "not yet" and every refusal above returns **without touching the repository**, which is the property this
//! test pins: `repository.attach_property_video` must be reached on the ready path and on no other path.
//!
//! **No Mux trait exists** — production takes `&MuxClient` concretely (:85, :136) — so the honest seam is the one
//! the client itself exposes: `MuxConfig::base_url` is a `pub` field, and this test points a real `MuxClient` at an
//! in-process `axum` server standing in for Mux. The production client, its `data`-envelope parsing and every
//! service branch above run for real; only Mux's own endpoint is local, and no live provider is contacted.
//!
//! Level: L3 Composition — the real `MediaService` over the production `MediaRepository` port with the database
//! faked, and the real `MuxClient` over loopback HTTP.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_media__009__video_upload

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use apis::mux::{MuxClient, MuxConfig};
use axum::response::Response;
use axum::routing::{get, post};
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
use web::media::{BeginMediaUploadResult, MediaRepository, MediaService};
use web::service_support::CoreServiceError;

const HARNESS: &str = "PROPERTY.MEDIA/009";

fn context() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("test-actor".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: "property-media-009".into(),
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

/// Records every `attach_property_video` the service attempts, so "was the video attached?" is answerable without a
/// database — and, more importantly, so "was it attached when it should NOT have been?" is too.
#[derive(Clone, Default)]
struct RecordingMediaRepository {
    attached: Arc<Mutex<Vec<AttachPropertyVideoRequest>>>,
}

impl RecordingMediaRepository {
    fn new() -> Self {
        Self::default()
    }

    fn attached(&self) -> Vec<AttachPropertyVideoRequest> {
        self.attached
            .lock()
            .expect("the recording repository is never poisoned")
            .clone()
    }

    fn attached_count(&self) -> usize {
        self.attached
            .lock()
            .expect("the recording repository is never poisoned")
            .len()
    }
}

fn unreachable_here<T>(method: &'static str) -> DbResult<T> {
    Err(DbFailure::configuration(
        "test.property_media.009",
        format!("{method} is outside the video-upload subject"),
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
    ) -> DbResult<BeginMediaUploadResult> {
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
/// Three scripted bodies, one per endpoint the client calls. It replays bytes and knows nothing about Mux's rules:
/// if a service branch above is wrong, no fixture wording can hide it.
async fn serve(create_body: String, upload_body: String, asset_body: String) -> SocketAddr {
    let create_body = Arc::new(create_body);
    let upload_body = Arc::new(upload_body);
    let asset_body = Arc::new(asset_body);

    let answer = |body: Arc<String>| {
        move || {
            let body = Arc::clone(&body);
            async move {
                Response::new(axum::body::Body::from(body.as_str().to_owned()))
            }
        }
    };

    let router = Router::new()
        .route("/video/v1/uploads", post(answer(create_body)))
        .route("/video/v1/uploads/{id}", get(answer(upload_body)))
        .route("/video/v1/assets/{id}", get(answer(asset_body)));

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

/// A Direct Upload Mux has accepted and turned into an asset.
fn upload_asset_created() -> String {
    serde_json::json!({
        "data": {
            "id": "upload-1",
            "status": "asset_created",
            "url": "https://storage.googleapis.com/video-storage-us-east1/upload-1",
            "asset_id": "asset-1"
        }
    })
    .to_string()
}

/// A ready, publicly playable asset — the only shape that may become a video row.
fn asset_ready(policy: &str) -> String {
    serde_json::json!({
        "data": {
            "id": "asset-1",
            "status": "ready",
            "duration_seconds": "42.5",
            "aspect_ratio": "16:9",
            "playback_ids": [{ "id": "playback-1", "policy": policy }]
        }
    })
    .to_string()
}

fn assert_code(result: Result<impl std::fmt::Debug, CoreServiceError>, expect: &str, what: &str) {
    match result {
        Ok(value) => panic!("{HARNESS}: {what} must be refused with {expect}, got {value:?}"),
        Err(error) => assert_eq!(
            error.code(),
            expect,
            "{HARNESS}: {what} must be refused with {expect}, got {error:?}"
        ),
    }
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-MEDIA-009); the file and the assay use it.
async fn property_media_009__video_upload() {
    // ---- 1. Opening the upload: a browser origin is required, and Mux is handed a public-playback upload. ----
    let opened = serve(
        serde_json::json!({
            "data": {
                "id": "upload-1",
                "status": "waiting",
                "url": "https://storage.googleapis.com/video-storage-us-east1/upload-1"
            }
        })
        .to_string(),
        upload_asset_created(),
        asset_ready("public"),
    )
    .await;
    let mux = client_at(opened);

    let repository = RecordingMediaRepository::new();
    let service = MediaService::new(repository.clone(), infrastructure());
    let ctx = context();

    let session = service
        .create_property_video_upload(&mux, "https://culebraluxe.com", &ctx)
        .await
        .expect("a valid browser origin opens a Direct Upload");
    assert_eq!(
        session.upload_id, "upload-1",
        "{HARNESS}: the session carries the upload id Mux issued, so finalize can ask about that exact upload"
    );
    assert!(
        session.upload_url.starts_with("https://"),
        "{HARNESS}: the browser is handed Mux's own HTTPS upload URL, never a proxy through us"
    );

    // ---- 2. NEGATIVE: a CORS origin that is not a browser origin is refused before Mux is called. ----
    for hostile in ["", "   ", "javascript:alert(1)", "file://", "culebraluxe.com"] {
        assert_code(
            service
                .create_property_video_upload(&mux, hostile, &ctx)
                .await
                .map(|session| session.upload_id),
            "MUX_CORS_ORIGIN_INVALID",
            &format!("a CORS origin of {hostile:?}"),
        );
    }

    // ---- 3. The ready path: a public, ready asset is attached, with Mux's identifiers intact. ----
    let finalized = service
        .finalize_property_video_upload(&mux, "property-1", "upload-1", "video", Some("Tour".into()), &ctx)
        .await
        .expect("an asset_created upload whose asset is ready must attach");
    assert!(
        finalized.attached,
        "{HARNESS}: a ready, publicly playable asset is attached"
    );
    assert_eq!(
        finalized.status, "ready",
        "{HARNESS}: the terminal status reported to the browser is the asset's own status"
    );

    let attached = repository.attached();
    assert_eq!(
        attached.len(),
        1,
        "{HARNESS}: exactly one video row is created by one finalize"
    );
    assert_eq!(attached[0].property_id, "property-1");
    assert_eq!(
        attached[0].mux_asset_id, "asset-1",
        "{HARNESS}: the Mux asset id reaches persistence unaltered — it is the key the stream URL is built from"
    );
    assert_eq!(
        attached[0].mux_playback_id, "playback-1",
        "{HARNESS}: the PUBLIC playback id is stored, not a signed one"
    );
    assert_eq!(
        attached[0].role, "video",
        "{HARNESS}: the role the caller asked for is the role the row carries"
    );
    assert_eq!(
        attached[0].caption.as_deref(),
        Some("Tour"),
        "{HARNESS}: the caption reaches persistence"
    );

    // ---- 4. NEGATIVE: a photo role is refused, and Mux is never asked. ----
    // `hero` and `gallery` are the photo roles. Accepting one here would put a video into the gallery slot a
    // Property's hero transition manages (`db/src/media/assemble_media_upload.rs`).
    for role in ["hero", "gallery", "document", "", "VIDEO"] {
        let before = repository.attached_count();
        assert_code(
            service
                .finalize_property_video_upload(&mux, "property-1", "upload-1", role, None, &ctx)
                .await
                .map(|result| result.status),
            "MEDIA_ROLE_INVALID",
            &format!("a video role of {role:?}"),
        );
        assert_eq!(
            repository.attached_count(),
            before,
            "{HARNESS}: role {role:?} is refused before Mux is asked and before anything is attached"
        );
    }

    // ---- 5. NEGATIVE: a video that is not ready yet is reported, not attached. ----
    // `waiting` is the ordinary state of a video the browser is still uploading, so this is the branch the product
    // hits on every single upload the first time the user asks.
    let waiting = serve(
        String::new(),
        serde_json::json!({ "data": { "id": "upload-1", "status": "waiting" } }).to_string(),
        asset_ready("public"),
    )
    .await;
    let waiting_mux = client_at(waiting);

    let still_uploading = service
        .finalize_property_video_upload(&waiting_mux, "property-1", "upload-1", "video", None, &ctx)
        .await
        .expect("a video that has not finished uploading is a status, not a failure");
    assert_eq!(
        still_uploading.status, "waiting",
        "{HARNESS}: the browser is told the truth about an unfinished upload"
    );
    assert!(
        !still_uploading.attached && still_uploading.media_id.is_none(),
        "{HARNESS}: an unfinished upload attaches nothing — no row, no media id"
    );
    assert_eq!(
        repository.attached_count(),
        1,
        "{HARNESS}: only the one ready asset was ever attached; `waiting` reached no repository call"
    );

    // ---- 6. NEGATIVE: an upload Mux reports as failed is a refusal, never a silent success. ----
    for failed in ["errored", "cancelled", "timed_out"] {
        let failing = serve(
            String::new(),
            serde_json::json!({
                "data": {
                    "id": "upload-1",
                    "status": failed,
                    "error": { "message": "The upload was rejected by the provider." }
                }
            })
            .to_string(),
            asset_ready("public"),
        )
        .await;
        assert_code(
            service
                .finalize_property_video_upload(
                    &client_at(failing),
                    "property-1",
                    "upload-1",
                    "video",
                    None,
                    &ctx,
                )
                .await
                .map(|result| result.status),
            "MUX_UPLOAD_FAILED",
            &format!("an upload Mux reports as {failed}"),
        );
    }
    assert_eq!(
        repository.attached_count(),
        1,
        "{HARNESS}: a failed upload attaches nothing"
    );

    // ---- 7. NEGATIVE: a signed-only asset is refused — the public site could not play it. ----
    // The gate at `media_bytes.rs:245-255` looks for `policy == "public"` specifically. A `signed` playback id is
    // a real id and would satisfy a looser check, so this is the case that proves the check reads the policy and
    // not merely the presence of an id.
    let signed_only = serve(
        String::new(),
        upload_asset_created(),
        asset_ready("signed"),
    )
    .await;
    assert_code(
        service
            .finalize_property_video_upload(
                &client_at(signed_only),
                "property-1",
                "upload-1",
                "video",
                None,
                &ctx,
            )
            .await
            .map(|result| result.status),
        "MUX_PLAYBACK_ID_MISSING",
        "an asset whose only playback id is signed",
    );
    assert_eq!(
        repository.attached_count(),
        1,
        "{HARNESS}: a signed-only asset attaches nothing — a video row pointing at an unplayable id would put a \
         broken player on a live listing"
    );

    // ---- 8. And the `short` role is accepted on the same ready path. ----
    let short = service
        .finalize_property_video_upload(
            &mux,
            "property-1",
            "upload-1",
            "short",
            None,
            &ctx,
        )
        .await
        .expect("`short` is a video role and takes the same ready path");
    assert!(short.attached, "{HARNESS}: a `short` video attaches like a `video`");
    assert_eq!(repository.attached_count(), 2);
}
