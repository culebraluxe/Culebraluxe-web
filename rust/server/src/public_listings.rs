//! The public site's listing copy, as a service: the taglines of published listings.
//!
//! AUTHORIZED AS `property.public.read`, the named published action (security/entitlements.rs): anyone may run it,
//! queries only. It is not `property.read`, so this door reaches published copy and nothing else.

use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, PublicListingDao};
use domain::{PublicListing, PublicListingCopy, PublicProperty};
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait PublicListingRepository: Send {
    async fn listings(&mut self) -> DbResult<Vec<PublicListing>>;
    /// `None` means no such Property — a genuine not-found, not a failure.
    async fn property(&mut self, key: &str) -> DbResult<Option<PublicProperty>>;
    /// `None` means no such media, or media that is not published. The two are indistinguishable on purpose.
    async fn media_bytes(&mut self, id: &str) -> DbResult<Option<(String, Vec<u8>)>>;
    /// Listings like this one, by the same visibility rule as the inventory.
    async fn similar(&mut self, key: &str, limit: i64) -> DbResult<Vec<PublicListing>>;
    /// Every slug the site can serve, for the sitemap.
    async fn slugs(&mut self) -> DbResult<Vec<String>>;
    async fn listing_copy(&mut self) -> DbResult<Vec<PublicListingCopy>>;
}

#[async_trait]
impl PublicListingRepository for PublicListingDao {
    async fn listings(&mut self) -> DbResult<Vec<PublicListing>> {
        db::retrying_read!(PublicListingDao::listings(self))
    }

    async fn property(&mut self, key: &str) -> DbResult<Option<PublicProperty>> {
        let key = key.to_owned();
        db::retrying_read!(PublicListingDao::property(self, &key))
    }

    async fn media_bytes(&mut self, id: &str) -> DbResult<Option<(String, Vec<u8>)>> {
        let id = id.to_owned();
        db::retrying_read!(PublicListingDao::media_bytes(self, &id))
    }

    async fn similar(&mut self, key: &str, limit: i64) -> DbResult<Vec<PublicListing>> {
        let key = key.to_owned();
        db::retrying_read!(PublicListingDao::similar(self, &key, limit))
    }

    async fn slugs(&mut self) -> DbResult<Vec<String>> {
        db::retrying_read!(PublicListingDao::slugs(self))
    }

    async fn listing_copy(&mut self) -> DbResult<Vec<PublicListingCopy>> {
        db::retrying_read!(PublicListingDao::listing_copy(self))
    }
}

pub struct PublicListingService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: PublicListingRepository> PublicListingService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn listings(
        &mut self,
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
        &mut self,
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

    /// One media asset's servable bytes, for the anonymous public site.
    pub async fn media_bytes(
        &mut self,
        id: &str,
        context: &ServiceContext,
    ) -> Result<Option<(String, Vec<u8>)>, CoreServiceError> {
        const OP: &str = "property.publicMediaBytes";
        let decision = authorize(
            &self.runtime,
            "property",
            "property.public.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.media_bytes(id).await.map_err(Into::into);
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    /// Listings like this one, for the property page's strip.
    pub async fn similar(
        &mut self,
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
        let result = self.repository.similar(key, limit).await.map_err(Into::into);
        audit_result(&self.runtime, "property", OP, context, decision, &result).await?;
        result
    }

    /// Every slug the site can serve, for the sitemap.
    pub async fn slugs(&mut self, context: &ServiceContext) -> Result<Vec<String>, CoreServiceError> {
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
        &mut self,
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
    use service::{CapturingAuditPort, CapturingDomainEventPort, ServiceActor, ServiceActorKind};
    use std::sync::Arc;

    #[derive(Default)]
    struct FakeRepository;

    #[async_trait]
    impl PublicListingRepository for FakeRepository {
        async fn listings(&mut self) -> DbResult<Vec<PublicListing>> {
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

        async fn property(&mut self, key: &str) -> DbResult<Option<PublicProperty>> {
            if key == "estate" {
                Ok(Some(PublicProperty {
                    key: "estate".into(),
                    name: "Estate".into(),
                    status: "active".into(),
                    hero_media_id: Some("media-1".into()),
                    media: vec![domain::PublicPropertyMedia {
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

        async fn media_bytes(&mut self, id: &str) -> DbResult<Option<(String, Vec<u8>)>> {
            if id == "media-1" {
                Ok(Some(("image/jpeg".into(), vec![1, 2, 3])))
            } else {
                Ok(None)
            }
        }

        async fn slugs(&mut self) -> DbResult<Vec<String>> {
            Ok(vec!["estate".into()])
        }

        async fn similar(&mut self, key: &str, limit: i64) -> DbResult<Vec<PublicListing>> {
            if key == "estate" && limit > 0 {
                Ok(vec![])
            } else {
                Ok(vec![])
            }
        }

        async fn listing_copy(&mut self) -> DbResult<Vec<PublicListingCopy>> {
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
        let mut service = PublicListingService::new(FakeRepository, infrastructure);
        let copy = service
            .listing_copy(&public_website())
            .await
            .expect("a published read is published");
        assert_eq!(copy[0].tagline, "Two pools.");
    }
}
