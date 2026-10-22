//! The public site's listing copy, as a service: the taglines of published listings.
//!
//! AUTHORIZED AS `property.public.read`, the named published action (security/entitlements.rs): anyone may run it,
//! queries only. It is not `property.read`, so this door reaches published copy and nothing else.

use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, PublicListingDao};
use model::{PublicListing, PublicListingCopy, PublicProperty};
use services::{
    AbstractObjectStoreService, ObjectBucket, OperationKind, ServiceContext, ServiceInfrastructure,
    ServiceRuntime,
};
use std::sync::Arc;

#[async_trait]
pub trait PublicListingRepository: Send + Sync {
    async fn listings(&self) -> DbResult<Vec<PublicListing>>;
    /// `None` means no such Property — a genuine not-found, not a failure.
    async fn property(&self, key: &str) -> DbResult<Option<PublicProperty>>;
    /// Listings like this one, by the same visibility rule as the inventory.
    async fn similar(&self, key: &str, limit: i64) -> DbResult<Vec<PublicListing>>;
    /// Every slug the site can serve, for the sitemap.
    async fn slugs(&self) -> DbResult<Vec<String>>;
    async fn listing_copy(&self) -> DbResult<Vec<PublicListingCopy>>;

    /// The storage pointer of the copy to serve, selected by `size`. `None` means no such media, or media that is
    /// not published — the two are indistinguishable on purpose. Test repositories return None.
    async fn media_object_location(
        &self,
        _id: &str,
        _size: &str,
    ) -> DbResult<Option<db::MediaObjectLocation>> {
        Ok(None)
    }
}

#[async_trait]
impl PublicListingRepository for PublicListingDao {
    async fn listings(&self) -> DbResult<Vec<PublicListing>> {
        db::retrying_read!(PublicListingDao::listings(self))
    }

    async fn property(&self, key: &str) -> DbResult<Option<PublicProperty>> {
        let key = key.to_owned();
        db::retrying_read!(PublicListingDao::property(self, &key))
    }

    async fn similar(&self, key: &str, limit: i64) -> DbResult<Vec<PublicListing>> {
        let key = key.to_owned();
        db::retrying_read!(PublicListingDao::similar(self, &key, limit))
    }

    async fn slugs(&self) -> DbResult<Vec<String>> {
        db::retrying_read!(PublicListingDao::slugs(self))
    }

    async fn listing_copy(&self) -> DbResult<Vec<PublicListingCopy>> {
        db::retrying_read!(PublicListingDao::listing_copy(self))
    }

    async fn media_object_location(
        &self,
        id: &str,
        size: &str,
    ) -> DbResult<Option<db::MediaObjectLocation>> {
        let (id, size) = (id.to_owned(), size.to_owned());
        db::retrying_read!(PublicListingDao::media_object_location(self, &id, &size))
    }
}

pub struct PublicListingService<R> {
    repository: R,
    runtime: ServiceRuntime,
    object_store: Option<Arc<dyn AbstractObjectStoreService>>,
}

impl<R: PublicListingRepository> PublicListingService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
            object_store: None,
        }
    }

    pub fn with_object_store(mut self, store: Arc<dyn AbstractObjectStoreService>) -> Self {
        self.object_store = Some(store);
        self
    }

    pub async fn listings(
        &self,
        context: &ServiceContext,
    ) -> Result<Vec<PublicListing>, CoreServiceError> {
        const OP: &str = "property.publicListings";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.public.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.listings().await.map_err(Into::into);
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    /// One Property for the public site, resolved by any identifier that names it.
    pub async fn property(
        &self,
        key: &str,
        context: &ServiceContext,
    ) -> Result<Option<PublicProperty>, CoreServiceError> {
        const OP: &str = "property.publicProperty";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.public.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.property(key).await.map_err(Into::into);
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    /// The final public CDN URL for one photograph copy, resolved exactly as `media_read` does — same derivative
    /// choice, same public+VERIFIED gate — WITHOUT the per-image authorization/audit.
    ///
    /// WHY NO AUTHORIZE/AUDIT HERE: a page resolves dozens of images, and the door that authorized the page
    /// (`publicListings`/`publicProperty`, `property.public.read`) already ran and was audited once. Repeating that
    /// decision per image would add dozens of policy evaluations and audit rows for one page view while changing no
    /// answer; this method is read-only and returns only a URL for media the public door may already serve.
    ///
    /// `None` is the same indistinguishable "not servable" answer as `media_read`: an absent id, an unpublished
    /// listing, a legacy blob-only row or a non-public copy. Infrastructure failures are errors, so a caller can fall
    /// back to the guarded route rather than silently drop the photograph.
    pub async fn media_public_url(
        &self,
        id: &str,
        size: &str,
    ) -> Result<Option<String>, CoreServiceError> {
        self.resolve_public_url(id, size).await
    }

    /// The shared resolver: a VERIFIED object in the public bucket with public visibility becomes its direct CDN
    /// URL; everything else is `None`. No authorization or audit — the two doors above own those.
    async fn resolve_public_url(
        &self,
        id: &str,
        size: &str,
    ) -> Result<Option<String>, CoreServiceError> {
        let store = self.object_store.as_ref().ok_or_else(|| {
            CoreServiceError::infrastructure(
                "MEDIA_OBJECT_STORE_NOT_CONFIGURED",
                "the public media door requires media object storage",
            )
        })?;
        let Some(location) = self.repository.media_object_location(id, size).await? else {
            // No row, unpublished listing, legacy blob-only row: one indistinguishable answer.
            return Ok(None);
        };
        if location.status != "VERIFIED" {
            return Ok(None);
        }
        if ObjectBucket::from_db(&location.bucket) != Some(ObjectBucket::PropertyMediaPublic)
            || location.visibility != "public"
        {
            return Ok(None);
        }
        store
            .public_read_url(ObjectBucket::PropertyMediaPublic, &location.key)
            .await
            .map(Some)
            .map_err(|error| {
                CoreServiceError::infrastructure("MEDIA_OBJECT_READ_URL_FAILED", error.to_string())
            })
    }

    /// The public site's media resolver (the `/api/media/{id}` 302 door). Object-only, unconditionally.
    pub async fn media_read(
        &self,
        id: &str,
        size: &str,
        context: &ServiceContext,
    ) -> Result<Option<crate::media::MediaRead>, CoreServiceError> {
        const OP: &str = "property.publicMediaRead";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.public.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .resolve_public_url(id, size)
            .await
            .map(|url| url.map(|url| crate::media::MediaRead::Redirect { url }));
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    /// Listings like this one, for the property page's strip.
    pub async fn similar(
        &self,
        key: &str,
        limit: i64,
        context: &ServiceContext,
    ) -> Result<Vec<PublicListing>, CoreServiceError> {
        const OP: &str = "property.publicSimilar";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.public.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        // A limit from a caller is a request, not an instruction: the page shows a strip, so it can never ask for a
        // report. Clamped rather than trusted.
        let limit = limit.clamp(1, 24);
        let result = self
            .repository
            .similar(key, limit)
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    /// Every slug the site can serve, for the sitemap.
    pub async fn slugs(&self, context: &ServiceContext) -> Result<Vec<String>, CoreServiceError> {
        const OP: &str = "property.publicSlugs";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.public.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.slugs().await.map_err(Into::into);
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    pub async fn listing_copy(
        &self,
        context: &ServiceContext,
    ) -> Result<Vec<PublicListingCopy>, CoreServiceError> {
        const OP: &str = "property.publicListingCopy";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.public.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.listing_copy().await.map_err(Into::into);
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::CasbinAuthorizationPort;
    use services::{CapturingAuditPort, CapturingDomainEventPort, ServiceActor, ServiceActorKind};
    use std::sync::Arc;

    #[derive(Default)]
    struct FakeRepository;

    #[async_trait]
    impl PublicListingRepository for FakeRepository {
        async fn listings(&self) -> DbResult<Vec<PublicListing>> {
            Ok(vec![PublicListing {
                key: "estate".into(),
                id: "estate-id".into(),
                name: "Estate".into(),
                property_type: Some("Villa".into()),
                status: "active".into(),
                list_price: Some(1_000_000.0),
                featured: true,
                hero_media_id: Some("media-1".into()),
                ..Default::default()
            }])
        }

        async fn property(&self, key: &str) -> DbResult<Option<PublicProperty>> {
            if key == "estate" {
                Ok(Some(PublicProperty {
                    key: "estate".into(),
                    name: "Estate".into(),
                    status: "active".into(),
                    hero_media_id: Some("media-1".into()),
                    media: vec![model::PublicPropertyMedia {
                        id: "media-1".into(),
                        role: "hero".into(),
                        media_type: "image".into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                }))
            } else {
                Ok(None)
            }
        }

        async fn slugs(&self) -> DbResult<Vec<String>> {
            Ok(vec!["estate".into()])
        }

        async fn similar(&self, key: &str, limit: i64) -> DbResult<Vec<PublicListing>> {
            if key == "estate" && limit > 0 {
                Ok(vec![])
            } else {
                Ok(vec![])
            }
        }

        async fn listing_copy(&self) -> DbResult<Vec<PublicListingCopy>> {
            Ok(vec![PublicListingCopy {
                slug: "estate".into(),
                tagline: "Two pools.".into(),
            }])
        }
    }

    /// The anonymous public website, as `resolve_public_guest_context` builds it: a system actor and no principal.
    fn public_website() -> ServiceContext {
        ServiceContext {
            actor: ServiceActor {
                id: Some("public-website".into()),
                kind: ServiceActorKind::System,
            },
            correlation_id: "public-listing-copy-test".into(),
            causation_id: None,
            principal: None,
        }
    }

    #[tokio::test]
    async fn the_public_website_may_read_published_copy_under_the_real_policy() {
        let infrastructure = ServiceInfrastructure::new(
            Arc::new(
                CasbinAuthorizationPort::new()
                    .await
                    .expect("the policy loads"),
            ),
            Arc::new(CapturingAuditPort::default()),
            Arc::new(CapturingDomainEventPort::default()),
        );
        let service = PublicListingService::new(FakeRepository, infrastructure);
        let copy = service
            .listing_copy(&public_website())
            .await
            .expect("a published read is published");
        assert_eq!(copy[0].tagline, "Two pools.");
    }
    /// A repository whose only meaningful answer is the object pointer the direct-URL resolver reads.
    struct ResolverRepository {
        location: Option<db::MediaObjectLocation>,
        fail: bool,
    }

    impl ResolverRepository {
        fn pointing_at(bucket: &str, visibility: &str, status: &str) -> Self {
            Self {
                location: Some(db::MediaObjectLocation {
                    bucket: bucket.to_owned(),
                    key: "media/abc/card.jpg".to_owned(),
                    status: status.to_owned(),
                    visibility: visibility.to_owned(),
                }),
                fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                location: None,
                fail: true,
            }
        }
    }

    fn unconfigured<T>(method: &'static str) -> DbResult<T> {
        Err(db::DbFailure::configuration(
            "test.public_listings.media_public_url",
            format!("{method} is outside the direct-URL resolver subject"),
        ))
    }

    #[async_trait]
    impl PublicListingRepository for ResolverRepository {
        async fn listings(&self) -> DbResult<Vec<PublicListing>> {
            unconfigured("listings")
        }

        async fn property(&self, _key: &str) -> DbResult<Option<PublicProperty>> {
            unconfigured("property")
        }

        async fn similar(&self, _key: &str, _limit: i64) -> DbResult<Vec<PublicListing>> {
            unconfigured("similar")
        }

        async fn slugs(&self) -> DbResult<Vec<String>> {
            unconfigured("slugs")
        }

        async fn listing_copy(&self) -> DbResult<Vec<PublicListingCopy>> {
            unconfigured("listing_copy")
        }

        async fn media_object_location(
            &self,
            _id: &str,
            _size: &str,
        ) -> DbResult<Option<db::MediaObjectLocation>> {
            if self.fail {
                return unconfigured("media_object_location");
            }
            Ok(self.location.clone())
        }
    }

    /// A store that only has to answer the one question this resolver asks: the public URL for an object.
    struct PublicStore;

    #[async_trait]
    impl AbstractObjectStoreService for PublicStore {
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

    async fn resolver_service(
        repository: ResolverRepository,
        with_store: bool,
    ) -> PublicListingService<ResolverRepository> {
        let infrastructure = ServiceInfrastructure::new(
            Arc::new(
                CasbinAuthorizationPort::new()
                    .await
                    .expect("the policy loads"),
            ),
            Arc::new(CapturingAuditPort::default()),
            Arc::new(CapturingDomainEventPort::default()),
        );
        let service = PublicListingService::new(repository, infrastructure);
        if with_store {
            service.with_object_store(Arc::new(PublicStore))
        } else {
            service
        }
    }

    /// The direct resolver returns the same derivative choice `media_read` serves, as a final CDN URL, without a
    /// per-image authorize/audit pass.
    #[tokio::test]
    async fn the_direct_resolver_returns_the_public_object_url() {
        let service = resolver_service(
            ResolverRepository::pointing_at("property_media_public", "public", "VERIFIED"),
            true,
        )
        .await;
        let url = service
            .media_public_url("media-abc", "card")
            .await
            .expect("the pointer resolves")
            .expect("a verified public object has a URL");
        assert_eq!(url, "https://cdn.example/media/abc/card.jpg");
    }

    /// The same public+VERIFIED gate as `media_read`: every other shape is the indistinguishable `None`.
    #[tokio::test]
    async fn the_direct_resolver_refuses_everything_that_is_not_a_public_verified_object() {
        for repository in [
            ResolverRepository::pointing_at("property_media_private", "private", "VERIFIED"),
            ResolverRepository::pointing_at("property_media_public", "public", "MIGRATION_PENDING"),
            ResolverRepository::pointing_at("vault", "vault", "VERIFIED"),
            ResolverRepository {
                location: None,
                fail: false,
            },
        ] {
            let service = resolver_service(repository, true).await;
            assert_eq!(
                service.media_public_url("media-abc", "card").await.unwrap(),
                None,
                "only a VERIFIED public object in the public bucket may become a direct URL"
            );
        }
    }

    #[tokio::test]
    async fn the_direct_resolver_needs_the_object_store_and_surfaces_repository_failures() {
        let service = resolver_service(ResolverRepository::failing(), true).await;
        assert_eq!(
            service
                .media_public_url("media-abc", "card")
                .await
                .expect_err("a repository failure is an error, not a missing photograph")
                .code(),
            "DATABASE"
        );

        let service = resolver_service(ResolverRepository::failing(), false).await;
        assert_eq!(
            service
                .media_public_url("media-abc", "card")
                .await
                .expect_err("no store")
                .code(),
            "MEDIA_OBJECT_STORE_NOT_CONFIGURED"
        );
    }
}
