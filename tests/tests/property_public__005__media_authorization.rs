//! PROPERTY.PUBLIC — media authorization (TST-PROPERTY-PUBLIC-005).
//!
//! Contract: **a photograph of a published Property is servable to an anonymous visitor, and "may this visitor have
//! this media" is answered by `None` — never an error, and never a distinction.** The gate is
//! `PublicListingDao::media_object_location` (`db/src/public_listing.rs`), the object-only resolver: it returns the
//! storage pointer of the copy to serve, or nothing at all. `PublicListingService::media_read` turns a pointer into
//! a short-lived public URL and turns every refusal into the same `None`.
//!
//! **The gate is evaluated on the ORIGINAL, not on the copy.** Every photograph gets derivatives — a `web` copy, a
//! `card`, a `thumb` — and a copy has no `property_media` link of its own. So the query walks
//! `join media root on root.id = coalesce(m.derivative_of, m.id)` and asks whether the ROOT is linked to an
//! unpublished Property. Asking about the copy would answer "no links, therefore fine" for every derivative of every
//! unpublished Property. This is the single most important line in the batch.
//!
//! **"No such media", "not published" and "no object pointer" are the same answer.** All three are `None`, because an
//! anonymous visitor must not be able to tell them apart — a distinguishable "exists but is hidden" is an oracle for
//! which listings exist. A blob-only row (no object) is deliberately NOT servable: the read path no longer has a
//! `file_data` fallback, so it answers like an absent one.
//!
//! The size variant is a presentation choice, not a permission: `card` and `thumb` select that derivative, anything
//! else widens to `web` and then the original.
//!
//! Level: L3 Composition — the real `PublicListingService` with the real `CasbinAuthorizationPort`, the database and
//! object store faked at the `PublicListingRepository`/object-store boundary. The gate SQL is pinned structurally.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_public__005__media_authorization

use std::sync::{Arc, Mutex};

use db::{DbFailure, DbResult, MediaObjectLocation};
use model::{PublicListing, PublicListingCopy, PublicProperty};
use services::{
    AbstractObjectStoreService, CapturingAuditPort, CapturingDomainEventPort, ObjectBucket,
    ServiceActor, ServiceActorKind, ServiceContext, ServiceInfrastructure,
};
use test_harness::source;
use web::public_listings::{PublicListingRepository, PublicListingService};

const HARNESS: &str = "PROPERTY.PUBLIC/005";

fn public_website() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("public-website".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: "property-public-005".into(),
        causation_id: None,
        principal: None,
    }
}

async fn infrastructure() -> ServiceInfrastructure {
    ServiceInfrastructure::new(
        Arc::new(
            web::security::CasbinAuthorizationPort::new()
                .await
                .expect("the production policy loads"),
        ),
        Arc::new(CapturingAuditPort::default()),
        Arc::new(CapturingDomainEventPort::default()),
    )
}

/// A public listing repository whose OBJECT-POINTER gate serves only ids the fake was told are servable, records
/// every `(id, size)` it was asked for, and can be told to fail. A refusal is `None`, exactly as production returns
/// it, so the test can prove a hidden photograph and an absent one are indistinguishable.
#[derive(Clone, Default)]
struct FakePublicListingRepository {
    servable: Arc<Mutex<Vec<String>>>,
    asked: Arc<Mutex<Vec<(String, String)>>>,
    fail: bool,
}

impl FakePublicListingRepository {
    fn serving(self, id: &str) -> Self {
        self.servable
            .lock()
            .expect("the fake repository is never poisoned")
            .push(id.to_owned());
        self
    }

    fn failing() -> Self {
        Self {
            fail: true,
            ..Self::default()
        }
    }

    fn asked(&self) -> Vec<(String, String)> {
        self.asked
            .lock()
            .expect("the fake repository is never poisoned")
            .clone()
    }

    fn serves(&self, id: &str) -> bool {
        self.servable
            .lock()
            .expect("the fake repository is never poisoned")
            .iter()
            .any(|servable| servable == id)
    }
}

#[async_trait::async_trait]
impl PublicListingRepository for FakePublicListingRepository {
    async fn listings(&self) -> DbResult<Vec<PublicListing>> {
        unreachable_media("listings")
    }

    async fn property(&self, _key: &str) -> DbResult<Option<PublicProperty>> {
        unreachable_media("property")
    }

    async fn similar(&self, _key: &str, _limit: i64) -> DbResult<Vec<PublicListing>> {
        unreachable_media("similar")
    }

    async fn slugs(&self) -> DbResult<Vec<String>> {
        unreachable_media("slugs")
    }

    async fn listing_copy(&self) -> DbResult<Vec<PublicListingCopy>> {
        unreachable_media("listing_copy")
    }

    async fn media_object_location(
        &self,
        id: &str,
        size: &str,
    ) -> DbResult<Option<MediaObjectLocation>> {
        self.asked
            .lock()
            .expect("the fake repository is never poisoned")
            .push((id.to_owned(), size.to_owned()));
        if self.fail {
            return Err(DbFailure::configuration(
                "test.property_public.005",
                "the media pointer read failed",
            ));
        }
        Ok(self.serves(id).then(|| MediaObjectLocation {
            bucket: "property_media_public".into(),
            key: format!("media/{id}/original.jpg"),
            status: "VERIFIED".into(),
            visibility: "public".into(),
        }))
    }
}

fn unreachable_media<T>(method: &'static str) -> DbResult<T> {
    Err(DbFailure::configuration(
        "test.property_public.005",
        format!("{method} is outside the media-authorization subject"),
    ))
}

/// A store that answers public read URLs, so the service can turn a VERIFIED public pointer into a redirect.
struct FakeStore;

#[async_trait::async_trait]
impl AbstractObjectStoreService for FakeStore {
    async fn put_object(
        &self,
        bucket: ObjectBucket,
        key: &str,
        bytes: &[u8],
        content_type: &str,
    ) -> Result<services::ObjectMetadata, services::ObjectStoreError> {
        Ok(services::ObjectMetadata {
            bucket,
            key: key.to_owned(),
            size: bytes.len() as i64,
            content_type: Some(content_type.to_owned()),
            etag: None,
            sha256: None,
        })
    }

    async fn head_object(
        &self,
        bucket: ObjectBucket,
        key: &str,
    ) -> Result<services::ObjectMetadata, services::ObjectStoreError> {
        Ok(services::ObjectMetadata {
            bucket,
            key: key.to_owned(),
            size: 1,
            content_type: None,
            etag: None,
            sha256: None,
        })
    }

    async fn delete_object(
        &self,
        _bucket: ObjectBucket,
        _key: &str,
    ) -> Result<(), services::ObjectStoreError> {
        Ok(())
    }

    async fn public_read_url(
        &self,
        _bucket: ObjectBucket,
        key: &str,
    ) -> Result<String, services::ObjectStoreError> {
        Ok(format!("https://cdn.example/{key}"))
    }

    async fn signed_read_url(
        &self,
        _bucket: ObjectBucket,
        key: &str,
        _ttl: std::time::Duration,
    ) -> Result<String, services::ObjectStoreError> {
        Ok(format!("https://signed.example/{key}"))
    }

    async fn signed_write_url(
        &self,
        _bucket: ObjectBucket,
        key: &str,
        _content_type: &str,
        _ttl: std::time::Duration,
    ) -> Result<String, services::ObjectStoreError> {
        Ok(format!("https://write.example/{key}"))
    }

    async fn verify_object(
        &self,
        bucket: ObjectBucket,
        key: &str,
        _expected_sha256: &str,
        _expected_size: i64,
    ) -> Result<services::ObjectMetadata, services::ObjectStoreError> {
        self.head_object(bucket, key).await
    }
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-PUBLIC-005); the file and the assay use it.
async fn property_public_005__media_authorization() {
    // `media-1` is a published Property's photograph. `media-hidden` stands for a photograph of an unpublished
    // Property; `media-absent` for an id that does not exist. The fake gate refuses both of the latter the same way.
    let repository = FakePublicListingRepository::default().serving("media-1");
    let service = PublicListingService::new(repository.clone(), infrastructure().await)
        .with_object_store(Arc::new(FakeStore));
    let ctx = public_website();

    // ---- 1. A published Property's photograph is servable to an anonymous visitor, as a direct URL. ----
    let served = service
        .media_read("media-1", "web", &ctx)
        .await
        .expect("a published photograph is a published read")
        .expect("media-1 is a published Property's photograph");
    match served {
        web::media::MediaRead::Redirect { url } => assert_eq!(
            url, "https://cdn.example/media/media-1/original.jpg",
            "{HARNESS}: a VERIFIED public object is served as its public URL"
        ),
        web::media::MediaRead::Bytes { .. } => panic!(
            "{HARNESS}: the public media door is object-only; it must redirect, never stream a blob"
        ),
    }

    // ---- 2. NEGATIVE: a hidden photograph and an absent id are the SAME answer. ----
    // An anonymous visitor must not be able to tell "this listing exists but is not published" from "there is no
    // such media". A distinguishable answer is an oracle for which listings exist.
    let hidden = service.media_read("media-hidden", "web", &ctx).await;
    let absent = service.media_read("media-absent", "web", &ctx).await;
    match (&hidden, &absent) {
        (Ok(None), Ok(None)) => {}
        (Ok(None), Ok(Some(_))) | (Ok(Some(_)), Ok(None)) => panic!(
            "{HARNESS}: a hidden photograph and an absent id must be indistinguishable — one answering \
             `None` and the other a pointer tells an anonymous visitor that the hidden listing exists"
        ),
        (Ok(Some(_)), Ok(Some(_))) => panic!(
            "{HARNESS}: neither a hidden photograph nor an absent id may be servable, but both resolved"
        ),
        (Err(error), _) | (_, Err(error)) => panic!(
            "{HARNESS}: a refusal is `None`, not an error; an error would distinguish 'hidden' from 'absent' \
             and from a fault alike. Got {error:?}"
        ),
    }

    // ---- 3. The size variant is a presentation choice and is forwarded verbatim. ----
    for size in ["card", "thumb", "web", "original", "", "full"] {
        let before = repository.asked().len();
        let answer = service.media_read("media-1", size, &ctx).await;
        assert_eq!(
            &repository.asked()[before],
            &("media-1".to_owned(), size.to_owned()),
            "{HARNESS}: the service must forward the requested size verbatim — the DAO, not the service, owns \
             which derivative that selects"
        );
        assert!(
            matches!(answer, Ok(Some(web::media::MediaRead::Redirect { .. }))),
            "{HARNESS}: size {size:?} must still yield a pointer; an unknown variant widens rather than failing"
        );
    }

    // ---- 4. NEGATIVE: a failed pointer read is an error, never a silent refusal. ----
    let failing = PublicListingService::new(
        FakePublicListingRepository::failing(),
        infrastructure().await,
    )
    .with_object_store(Arc::new(FakeStore));
    match failing.media_read("media-1", "web", &ctx).await {
        Ok(Some(_)) => panic!("{HARNESS}: a failed read must not serve a photograph"),
        Ok(None) => panic!(
            "{HARNESS}: a failed read must surface as an error; answering `None` makes a database outage \
             indistinguishable from a hidden listing"
        ),
        Err(_) => {}
    }

    // ---- 5. The gate itself, pinned where it lives. ----
    let dao = source::read(&source::workspace_root().join("db/src/public_listing.rs"));
    let code: String = dao
        .lines()
        .map(source::code_of)
        .collect::<Vec<_>>()
        .join("\n");

    // THE line. The gate must be asked about the ROOT of a derivative chain. Without the coalesce, a `web` copy
    // has no `property_media` link of its own, the `not exists` subquery finds nothing, and every derivative of
    // every unpublished Property is served to the public.
    assert!(
        code.contains("join media root on root.id = coalesce(m.derivative_of, m.id)"),
        "{HARNESS}: the media gate MUST be evaluated on the derivative chain's ROOT. A copy has no \
         `property_media` link of its own, so asking about the copy answers 'no links, therefore fine' and \
         exposes every copy of every unpublished Property"
    );
    assert!(
        code.contains("where pm.media_id = root.id"),
        "{HARNESS}: the visibility subquery must join on `root.id`, not on `m.id`"
    );
    assert!(
        code.contains("p.archived_at is null")
            && code.contains("p.status in ('active', 'under_contract', 'sold')"),
        "{HARNESS}: the media gate must carry the same visibility rule as every other public read"
    );
    // The resolver returns an OBJECT POINTER, never a blob: the public door has no `file_data` fallback.
    assert!(
        code.contains("m.object_bucket") && code.contains("m.object_key"),
        "{HARNESS}: the public media gate must resolve an object pointer"
    );
    assert!(
        !code.contains("file_data"),
        "{HARNESS}: db/src/public_listing.rs must not mention media.file_data — the public door is object-only \
         and the blob column is being dropped"
    );
}
