//! The public site's listing copy, as a service: the taglines of published listings.
//!
//! AUTHORIZED AS `property.public.read`, the named published action (security/entitlements.rs): anyone may run it,
//! queries only. It is not `property.read`, so this door reaches published copy and nothing else.

use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, PublicListingDao};
use domain::{PublicListing, PublicListingCopy};
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait PublicListingRepository: Send {
    async fn listings(&mut self) -> DbResult<Vec<PublicListing>>;
    async fn listing_copy(&mut self) -> DbResult<Vec<PublicListingCopy>>;
}

#[async_trait]
impl PublicListingRepository for PublicListingDao {
    async fn listings(&mut self) -> DbResult<Vec<PublicListing>> {
        db::retrying_read!(PublicListingDao::listings(self))
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
                name: "Estate".into(),
                property_type: Some("Villa".into()),
                status: "active".into(),
                list_price: Some(1_000_000.0),
                featured: true,
            }])
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
