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
use web::media::{MediaRepository, MediaService};
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

    async fn remove_property_media(&self, _property_id: &str, _media_id: &str) -> DbResult<bool> {
        unreachable_here("remove_property_media")
    }
}

/// The request shape the direct multipart entry point starts from.
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
            service
                .upload_property_media(property_request(hostile), &ctx)
                .await,
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
}
