//! PROPERTY.MEDIA — MIME validation (TST-PROPERTY-MEDIA-008).
//!
//! Contract: **a Property photograph is declared as an image, and anything else is refused before it can become a
//! media row.** Every byte the public site later serves is handed out with the MIME stored on the `media` row
//! (`db/src/public_listing.rs:619` returns `coalesce(copy.mime_type, m.mime_type)` as the `Content-Type`), so the
//! declared type is not a label — it is the header an anonymous browser is told to trust.
//!
//! The rule is declared in exactly one place per entry point, and it is the same rule in both:
//!
//! - `MediaService::upload_property_media` (`web/src/media/attach_property_video.rs:115`)
//! - `MediaService::upload_standalone` (`web/src/media/media_bytes.rs:58`)
//!
//! ```text
//! if !mime_type.to_ascii_lowercase().starts_with("image/") {
//!     return Err(CoreServiceError::business("MEDIA_TYPE_INVALID", ...));
//! }
//! ```
//!
//! The comparison is `to_ascii_lowercase`, so the rule is case-insensitive, and it runs **before**
//! `self.repository.*`, so a refused type never reaches persistence. That ordering is the part worth pinning: a
//! check that ran after the write would leave the very row it was meant to prevent already committed.
//!
//! Level: L3 Composition — the real `MediaService` (its own `authorize`/`audit_result` composition) over the
//! production `MediaRepository` port, with the database faked at that adapter boundary and nothing else faked. The
//! service, the authorization port, the audit port and the filename sanitizer are production code; only the
//! database is a double, because the database is the external provider.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_media__008__mime_validation

use std::sync::{Arc, Mutex};

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

const HARNESS: &str = "PROPERTY.MEDIA/008";

/// The one error code every refused MIME type produces, at both entry points.
const MEDIA_TYPE_INVALID: &str = "MEDIA_TYPE_INVALID";

/// A principal `DefaultAuthorizationPort` admits for a Command, so the service reaches its real body rather than
/// short-circuiting on authorization. Every assertion below is about MIME validation, not about access.
fn context() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("test-actor".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: "property-media-008".into(),
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

/// What the repository was actually asked to store.
///
/// `property_media` and `standalone` record the MIME they were handed. That is the whole point of the fake: it is
/// the last place a declared type can be observed before it becomes a `media.mime_type`, so "was it refused?" and
/// "was it persisted verbatim?" are both answerable without a database.
#[derive(Clone, Default)]
struct RecordingMediaRepository {
    property_media: Arc<Mutex<Vec<String>>>,
    standalone: Arc<Mutex<Vec<String>>>,
    begin_upload: Arc<Mutex<Vec<String>>>,
}

impl RecordingMediaRepository {
    fn new() -> Self {
        Self::default()
    }

    fn property_media_types(&self) -> Vec<String> {
        self.property_media
            .lock()
            .expect("the recording repository is never poisoned")
            .clone()
    }

    fn standalone_types(&self) -> Vec<String> {
        self.standalone
            .lock()
            .expect("the recording repository is never poisoned")
            .clone()
    }

    fn begin_upload_types(&self) -> Vec<String> {
        self.begin_upload
            .lock()
            .expect("the recording repository is never poisoned")
            .clone()
    }
}

fn unreachable_here<T>(method: &'static str) -> DbResult<T> {
    Err(DbFailure::configuration(
        "test.property_media.008",
        format!("{method} is outside the MIME-validation subject"),
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
        mime_type: &str,
        _bytes: &[u8],
    ) -> DbResult<(String, String, String, i64)> {
        self.standalone
            .lock()
            .expect("the recording repository is never poisoned")
            .push(mime_type.to_owned());
        Ok((
            "standalone-id".into(),
            _filename.to_owned(),
            mime_type.to_owned(),
            _bytes.len() as i64,
        ))
    }

    async fn for_property(&self, _property_id: &str) -> DbResult<Vec<MediaAsset>> {
        unreachable_here("for_property")
    }

    async fn set_property_hero(&self, _property_id: &str, _media_id: &str) -> DbResult<bool> {
        unreachable_here("set_property_hero")
    }

    async fn upload_property_media(
        &self,
        request: &UploadPropertyMediaRequest,
    ) -> DbResult<UploadPropertyMediaResult> {
        self.property_media
            .lock()
            .expect("the recording repository is never poisoned")
            .push(request.mime_type.clone());
        Ok(UploadPropertyMediaResult {
            ok: true,
            media_id: "media-from-repository".into(),
            property_id: request.property_id.clone(),
            role: request.role.clone(),
        })
    }

    async fn attach_property_video(
        &self,
        _request: &AttachPropertyVideoRequest,
    ) -> DbResult<AttachPropertyVideoResult> {
        unreachable_here("attach_property_video")
    }

    async fn begin_media_upload(
        &self,
        request: &db::BeginMediaUpload,
    ) -> DbResult<BeginMediaUploadResult> {
        self.begin_upload
            .lock()
            .expect("the recording repository is never poisoned")
            .push(request.mime_type.clone());
        Ok(BeginMediaUploadResult {
            upload_id: request.upload_id.clone(),
            chunk_size: request.chunk_size,
            chunk_count: request.chunk_count,
            byte_size: request.byte_size,
        })
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

/// The request shape both the single-shot and the chunked entry point start from.
fn property_request(mime_type: &str) -> UploadPropertyMediaRequest {
    UploadPropertyMediaRequest {
        property_id: "property-1".into(),
        role: "gallery".into(),
        filename: "front.jpg".into(),
        mime_type: mime_type.into(),
        alt_text: None,
        bytes: vec![0xFF, 0xD8, 0xFF, 0xE0],
    }
}

fn assert_refused(result: Result<UploadPropertyMediaResult, CoreServiceError>, what: &str) {
    match result {
        Ok(accepted) => panic!(
            "{HARNESS}: {what} must be refused with {MEDIA_TYPE_INVALID}, but it was accepted as {:?}",
            accepted.media_id
        ),
        Err(error) => assert_eq!(
            error.code(),
            MEDIA_TYPE_INVALID,
            "{HARNESS}: {what} must be refused with {MEDIA_TYPE_INVALID}, got {error:?}"
        ),
    }
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-MEDIA-008); the file and the assay use it.
async fn property_media_008__mime_validation() {
    let repository = RecordingMediaRepository::new();
    let service = MediaService::new(repository.clone(), infrastructure());
    let ctx = context();

    // ---- 1. An image is accepted, and reaches persistence with its type intact. -------------------
    let accepted = service
        .upload_property_media(property_request("image/jpeg"), &ctx)
        .await
        .expect("image/jpeg is an image and must be accepted");
    assert_eq!(
        accepted.role, "gallery",
        "{HARNESS}: an accepted upload keeps the role it was given"
    );

    // ---- 2. The comparison is case-insensitive, so `IMAGE/PNG` is not a bypass. --------------------
    service
        .upload_property_media(property_request("IMAGE/PNG"), &ctx)
        .await
        .expect("the rule is `to_ascii_lowercase()`, so IMAGE/PNG is still an image");

    // ---- 3. NEGATIVE: a non-image is refused, and refused BEFORE the repository. ------------------
    // These are the cases that make the test unable to pass without exercising the subject: an executable
    // payload, a desktop installer and a bare octet-stream are all things a browser or a desktop would act on
    // if the server echoed the declared type back as `Content-Type`.
    for hostile in [
        "text/html",
        "application/x-msdownload",
        "application/octet-stream",
        "image",
        "",
    ] {
        let repository_before = repository.property_media_types().len();
        assert_refused(
            service.upload_property_media(property_request(hostile), &ctx).await,
            &format!("a declared type of {hostile:?}"),
        );
        assert_eq!(
            repository.property_media_types().len(),
            repository_before,
            "{HARNESS}: {hostile:?} must be refused before the repository is called — a check that ran \
             after the write would leave the row it exists to prevent already committed"
        );
    }

    // ---- 4. Exactly the two accepted types were persisted, verbatim. -------------------------------
    assert_eq!(
        repository.property_media_types(),
        vec!["image/jpeg".to_owned(), "IMAGE/PNG".to_owned()],
        "{HARNESS}: only image types reach persistence, and each is stored exactly as declared — the \
         service validates the prefix, it does not rewrite the value"
    );

    // ---- 5. The standalone entry point enforces the same rule, not a laxer one. ---------------------
    service
        .upload_standalone("front.jpg", "image/webp", vec![1, 2, 3], &ctx)
        .await
        .expect("image/webp is an image and must be accepted");

    let standalone_repository = repository.standalone_types().len();
    match service
        .upload_standalone("payload.html", "text/html", vec![1, 2, 3], &ctx)
        .await
    {
        Ok(_) => panic!("{HARNESS}: a standalone upload of text/html must be refused"),
        Err(error) => assert_eq!(
            error.code(),
            MEDIA_TYPE_INVALID,
            "{HARNESS}: the standalone entry point enforces the same rule as the property one, got {error:?}"
        ),
    }
    assert_eq!(
        repository.standalone_types().len(),
        standalone_repository,
        "{HARNESS}: the standalone refusal also happens before the repository is called"
    );
    assert_eq!(
        repository.standalone_types(),
        vec!["image/webp".to_owned()],
        "{HARNESS}: only image types reach standalone persistence"
    );

    // ---- 6. NEGATIVE: the chunked upload protocol must not be a way around the rule. ---------------
    // `begin_media_upload` is the other door into a `media` row, and it is the one the browser uses for a
    // large photograph: `MediaService::begin_media_upload` (`web/src/media/media_bytes.rs:289-316`) forwards
    // `db::BeginMediaUpload` straight to `repository.begin_media_upload` with no MIME check of its own, the
    // route above it validates only filename and shape (`web/src/api/routes/media.rs:270-298`), and the
    // declared type is then bound verbatim into `media_upload.mime_type`
    // (`db/src/media/media_row.rs:369`) and copied onto the finished `media` row
    // (`db/src/media/assemble_media_upload.rs:99-117`). The database has no backstop either: `media` carries
    // `media_media_type_check` on `media_type in ('image','video','document')`
    // (`db/migrations/004_document_media.sql:5-6`) but **no CHECK constraint on `mime_type` at all**.
    let chunked = db::BeginMediaUpload {
        upload_id: "11111111-1111-4111-8111-111111111111".into(),
        property_id: "property-1".into(),
        filename: "front.html".into(),
        mime_type: "text/html".into(),
        byte_size: 4,
        chunk_count: 1,
        chunk_size: 4,
        sha256: None,
        role: "gallery".into(),
        alt_text: None,
    };

    let outcome = service.begin_media_upload(chunked, &ctx).await;
    assert!(
        outcome.is_err(),
        "{HARNESS}: the chunked upload protocol must apply the same image-only rule as the single-shot one. \
         `begin_media_upload` accepted a declared type of text/html, and that value is bound verbatim into \
         `media_upload.mime_type` and copied onto the finished `media` row, so a non-image reaches the \
         public site and is served back to browsers as its own `Content-Type`."
    );
    assert!(
        repository.begin_upload_types().is_empty(),
        "{HARNESS}: a refused MIME type must never reach `begin_media_upload`'s repository call; it \
         persisted {:?}",
        repository.begin_upload_types()
    );
}
