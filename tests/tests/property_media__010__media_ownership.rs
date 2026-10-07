//! PROPERTY.MEDIA — media ownership (TST-PROPERTY-MEDIA-010).
//!
//! Contract: **`property_media` owns the role, and a role belongs to the kind of asset that carries it.** `media` is
//! the reusable asset; `property_media` is what says "this asset is on that Property, in that role, at that
//! position". Two properties follow, and they are enforced in the service rather than left to the caller.
//!
//! **The role is partitioned by media kind.** The database admits five roles —
//! `check (role in ('hero','gallery','video','short','document'))` (`db/migrations/004_document_media.sql:12-13`) —
//! but each door accepts only the roles that make sense for what it writes:
//!
//! - the photograph door, `upload_property_media`, takes `hero` and `gallery` only
//!   (`web/src/media/attach_property_video.rs:97`);
//! - the video door, `attach_property_video`, takes `video` and `short` only
//!   (`web/src/media/attach_property_video.rs:50`).
//!
//! So a video can never be filed into the `hero` slot whose demote-then-promote transition
//! (`db/src/media/assemble_media_upload.rs`) assumes holds a photograph, and a photograph can never be filed as a
//! `document`. Both refuse with `MEDIA_ROLE_INVALID`, and both refuse **before** the repository is called, so the
//! link row is never created to be rejected afterwards.
//!
//! **Ownership is proven, not assumed.** `set_property_hero` and `remove_property_media` both act on a
//! `(property_id, media_id)` pair, and the repository answers `false` when that pair is not a link on that Property.
//! The service turns that into `MEDIA_NOT_ON_PROPERTY` (`web/src/media/media_bytes.rs:603` and `:418`) — one
//! Property's media can neither be promoted nor deleted through another. `property.id` is the stable identity, so
//! a slug or name change never moves a photo between listings.
//!
//! Level: L3 Composition — the real `MediaService` over the production `MediaRepository` port, database faked at
//! that adapter boundary.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_media__010__media_ownership

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

const HARNESS: &str = "PROPERTY.MEDIA/010";

/// The roles each door accepts, as the database declares them for `property_media`.
const PHOTO_ROLES: &[&str] = &["hero", "gallery"];
const VIDEO_ROLES: &[&str] = &["video", "short"];

fn context() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("test-actor".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: "property-media-010".into(),
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

/// Records the link each door was about to create, and answers the two ownership questions the hero and remove
/// doors ask: is this `(property_id, media_id)` pair a link, and is it on THIS property.
#[derive(Clone, Default)]
struct RecordingMediaRepository {
    photo_roles: Arc<Mutex<Vec<String>>>,
    video_roles: Arc<Mutex<Vec<String>>>,
    /// The `(property_id, media_id)` pairs this fake considers owned by a Property. Anything else answers `false`.
    owned: Arc<Mutex<Vec<(String, String)>>>,
}

impl RecordingMediaRepository {
    fn new() -> Self {
        Self::default()
    }

    fn owning(&self, property_id: &str, media_id: &str) -> Self {
        self.owned
            .lock()
            .expect("the recording repository is never poisoned")
            .push((property_id.to_owned(), media_id.to_owned()));
        self.clone()
    }

    fn photo_roles(&self) -> Vec<String> {
        self.photo_roles
            .lock()
            .expect("the recording repository is never poisoned")
            .clone()
    }

    fn video_roles(&self) -> Vec<String> {
        self.video_roles
            .lock()
            .expect("the recording repository is never poisoned")
            .clone()
    }

    fn links_created(&self) -> usize {
        self.photo_roles
            .lock()
            .expect("the recording repository is never poisoned")
            .len()
            + self
                .video_roles
                .lock()
                .expect("the recording repository is never poisoned")
                .len()
    }

    fn owns(&self, property_id: &str, media_id: &str) -> bool {
        self.owned
            .lock()
            .expect("the recording repository is never poisoned")
            .iter()
            .any(|(owned_property, owned_media)| {
                owned_property == property_id && owned_media == media_id
            })
    }
}

fn unreachable_here<T>(method: &'static str) -> DbResult<T> {
    Err(DbFailure::configuration(
        "test.property_media.010",
        format!("{method} is outside the media-ownership subject"),
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

    async fn set_property_hero(&self, property_id: &str, media_id: &str) -> DbResult<bool> {
        Ok(self.owns(property_id, media_id))
    }

    async fn upload_property_media(
        &self,
        request: &UploadPropertyMediaRequest,
    ) -> DbResult<UploadPropertyMediaResult> {
        self.photo_roles
            .lock()
            .expect("the recording repository is never poisoned")
            .push(request.role.clone());
        Ok(UploadPropertyMediaResult {
            ok: true,
            media_id: "media-from-repository".into(),
            property_id: request.property_id.clone(),
            role: request.role.clone(),
        })
    }

    async fn attach_property_video(
        &self,
        request: &AttachPropertyVideoRequest,
    ) -> DbResult<AttachPropertyVideoResult> {
        self.video_roles
            .lock()
            .expect("the recording repository is never poisoned")
            .push(request.role.clone());
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

    async fn remove_property_media(&self, property_id: &str, media_id: &str) -> DbResult<bool> {
        Ok(self.owns(property_id, media_id))
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

fn photo_request(role: &str) -> UploadPropertyMediaRequest {
    UploadPropertyMediaRequest {
        property_id: "property-1".into(),
        role: role.into(),
        filename: "front.jpg".into(),
        mime_type: "image/jpeg".into(),
        alt_text: None,
        bytes: vec![0xFF, 0xD8, 0xFF, 0xE0],
    }
}

fn video_request(role: &str) -> AttachPropertyVideoRequest {
    AttachPropertyVideoRequest {
        property_id: "property-1".into(),
        role: role.into(),
        mux_asset_id: "asset-1".into(),
        mux_playback_id: "playback-1".into(),
        duration_seconds: None,
        aspect_ratio: None,
        caption: None,
    }
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-MEDIA-010); the file and the assay use it.
async fn property_media_010__media_ownership() {
    let repository = RecordingMediaRepository::new();
    let service = MediaService::new(repository.clone(), infrastructure());
    let ctx = context();

    // ---- 1. Each door accepts exactly its own roles. ----
    for role in PHOTO_ROLES {
        service
            .upload_property_media(photo_request(role), &ctx)
            .await
            .unwrap_or_else(|error| panic!("{HARNESS}: the photograph door accepts {role:?}, got {error:?}"));
    }
    for role in VIDEO_ROLES {
        service
            .attach_property_video(video_request(role), &ctx)
            .await
            .unwrap_or_else(|error| panic!("{HARNESS}: the video door accepts {role:?}, got {error:?}"));
    }
    assert_eq!(
        repository.photo_roles(),
        PHOTO_ROLES.iter().map(|role| role.to_string()).collect::<Vec<_>>(),
        "{HARNESS}: the photograph door writes exactly the photograph roles"
    );
    assert_eq!(
        repository.video_roles(),
        VIDEO_ROLES.iter().map(|role| role.to_string()).collect::<Vec<_>>(),
        "{HARNESS}: the video door writes exactly the video roles"
    );

    // ---- 2. NEGATIVE: the two doors do not accept each other's roles. ----
    // This is the ownership partition. A video filed as `hero` would sit in the slot whose demote-then-promote
    // transition assumes a photograph; a photograph filed as `document` would appear in the conditional documents
    // list. Both are refused, and refused before the link row exists.
    for role in VIDEO_ROLES {
        let before = repository.links_created();
        match service.upload_property_media(photo_request(role), &ctx).await {
            Ok(accepted) => panic!(
                "{HARNESS}: the photograph door must refuse the video role {role:?}, but it accepted \
                 {} as a photo",
                accepted.media_id
            ),
            Err(error) => assert_eq!(
                error.code(),
                "MEDIA_ROLE_INVALID",
                "{HARNESS}: the photograph door refuses {role:?} with MEDIA_ROLE_INVALID, got {error:?}"
            ),
        }
        assert_eq!(
            repository.links_created(),
            before,
            "{HARNESS}: role {role:?} is refused before the link is created, not rejected by the database after"
        );
    }
    for role in PHOTO_ROLES.iter().copied().chain(["document"]) {
        let before = repository.links_created();
        match service.attach_property_video(video_request(role), &ctx).await {
            Ok(accepted) => panic!(
                "{HARNESS}: the video door must refuse the non-video role {role:?}, but it accepted {}",
                accepted.media_id
            ),
            Err(error) => assert_eq!(
                error.code(),
                "MEDIA_ROLE_INVALID",
                "{HARNESS}: the video door refuses {role:?} with MEDIA_ROLE_INVALID, got {error:?}"
            ),
        }
        assert_eq!(
            repository.links_created(),
            before,
            "{HARNESS}: role {role:?} is refused before the link is created"
        );
    }

    // ---- 3. NEGATIVE: role matching is exact — a role the database accepts is still refused here. ----
    // `property_media_role_check` would accept `Hero`, ` hero ` and `document`, and the role is the only thing
    // standing between a caller and a wrong slot, so the service compares the whole string.
    for sloppy in ["Hero", "HERO", " hero ", "hero ", "doc", ""] {
        match service.upload_property_media(photo_request(sloppy), &ctx).await {
            Ok(accepted) => panic!(
                "{HARNESS}: {sloppy:?} is not a photograph role and must be refused, but it was accepted as {}",
                accepted.media_id
            ),
            Err(error) => assert_eq!(
                error.code(),
                "MEDIA_ROLE_INVALID",
                "{HARNESS}: {sloppy:?} is refused with MEDIA_ROLE_INVALID, got {error:?}"
            ),
        }
    }

    // ---- 4. Ownership: media can be promoted and removed only on the Property that owns it. ----
    let owner = repository.owning("property-1", "media-1");
    let service = MediaService::new(owner.clone(), infrastructure());

    service
        .set_property_hero("property-1", "media-1", &ctx)
        .await
        .expect("a photo on this property may be made the hero");
    service
        .remove_property_media("property-1", "media-1", &ctx)
        .await
        .expect("a photo on this property may be removed");

    // NEGATIVE: the same media id on a different Property is not that Property's media. This is the case that
    // makes `property.id` the identity the handbook demands — a listing whose name or slug changed still owns the
    // same photo, and one Property can never reach another's.
    for (property_id, media_id, what) in [
        ("property-2", "media-1", "another Property's photo id"),
        ("property-1", "media-9", "a photo this Property does not carry"),
    ] {
        match service.set_property_hero(property_id, media_id, &ctx).await {
            Ok(()) => panic!(
                "{HARNESS}: {what} must not be promotable to hero on {property_id} — the link row is what \
                 proves ownership"
            ),
            Err(error) => assert_eq!(
                error.code(),
                "MEDIA_NOT_ON_PROPERTY",
                "{HARNESS}: promoting {what} on {property_id} is refused with MEDIA_NOT_ON_PROPERTY, \
                 got {error:?}"
            ),
        }
        match service.remove_property_media(property_id, media_id, &ctx).await {
            Ok(()) => panic!(
                "{HARNESS}: {what} must not be removable through {property_id} — media may only be \
                 unlinked from the Property that owns it"
            ),
            Err(error) => assert_eq!(
                error.code(),
                "MEDIA_NOT_ON_PROPERTY",
                "{HARNESS}: removing {what} through {property_id} is refused with MEDIA_NOT_ON_PROPERTY, \
                 got {error:?}"
            ),
        }
    }
}
