//! PROPERTY.PUBLIC — public inventory (TST-PROPERTY-PUBLIC-003).
//!
//! Contract: **the public inventory is the whole published set, in the order the reader should see it, and it is
//! read at request time.** Two properties follow.
//!
//! - **It is one query, not N+1.** `listings()` is a single statement that carries each card's fields and resolves
//!   its hero through a lateral join (`db/src/public_listing.rs:157-214`). The hero is part of the row on purpose:
//!   "a card without a picture is a card nobody clicks", and a missing hero must not become a second query per card.
//! - **Featured first, then by name.** `order by coalesce(p.featured, false) desc, p.name asc` (:206). The order is
//!   the merchandising decision, so the service must hand it back untouched rather than re-sorting.
//!
//! The service's own contribution is deliberately narrow and is asserted as such: it authorizes the published read,
//! audits the outcome, and returns the repository's rows **verbatim**. Visibility is decided once, in the DAO — "One
//! fact has ONE writer" — so the service must not filter, re-sort or re-derive anything. A service that dropped a
//! row would make the published set depend on which door a visitor arrived through.
//!
//! Level: L3 Composition — the real `PublicListingService` with the real `CasbinAuthorizationPort`, the database
//! faked at the `PublicListingRepository` adapter boundary.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_public__003__public_inventory

use std::sync::Arc;

use db::{DbFailure, DbResult};
use model::{PublicListing, PublicListingCopy, PublicProperty};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, ServiceActor, ServiceActorKind, ServiceContext,
    ServiceInfrastructure,
};
use test_harness::source;
use web::public_listings::{PublicListingRepository, PublicListingService};

const HARNESS: &str = "PROPERTY.PUBLIC/003";

fn public_website() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("public-website".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: "property-public-003".into(),
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

fn listing(key: &str, name: &str, price: Option<f64>, featured: bool) -> PublicListing {
    PublicListing {
        key: key.into(),
        id: format!("id-{key}"),
        name: name.into(),
        status: "active".into(),
        list_price: price,
        featured,
        property_type: Some("residential".into()),
        city: Some("Punta Cana".into()),
        bedrooms: Some(4),
        hero_media_id: Some(format!("hero-{key}")),
        ..Default::default()
    }
}

/// Returns a fixed inventory in a fixed order, or fails.
#[derive(Clone, Default)]
struct FakePublicListingRepository {
    inventory: Arc<Vec<PublicListing>>,
    fail: bool,
}

impl FakePublicListingRepository {
    fn with(inventory: Vec<PublicListing>) -> Self {
        Self {
            inventory: Arc::new(inventory),
            fail: false,
        }
    }

    fn failing() -> Self {
        Self {
            fail: true,
            ..Self::default()
        }
    }
}

#[async_trait::async_trait]
impl PublicListingRepository for FakePublicListingRepository {
    async fn listings(&self) -> DbResult<Vec<PublicListing>> {
        if self.fail {
            return Err(DbFailure::configuration(
                "test.property_public.003",
                "the inventory read failed",
            ));
        }
        Ok(self.inventory.as_ref().clone())
    }

    async fn property(&self, _key: &str) -> DbResult<Option<PublicProperty>> {
        unreachable_inventory("property")
    }

    async fn media_bytes(&self, _id: &str, _size: &str) -> DbResult<Option<(String, Vec<u8>)>> {
        unreachable_inventory("media_bytes")
    }

    async fn similar(&self, _key: &str, _limit: i64) -> DbResult<Vec<PublicListing>> {
        unreachable_inventory("similar")
    }

    async fn slugs(&self) -> DbResult<Vec<String>> {
        unreachable_inventory("slugs")
    }

    async fn listing_copy(&self) -> DbResult<Vec<PublicListingCopy>> {
        unreachable_inventory("listing_copy")
    }
}

fn unreachable_inventory<T>(method: &'static str) -> DbResult<T> {
    Err(DbFailure::configuration(
        "test.property_public.003",
        format!("{method} is outside the public-inventory subject"),
    ))
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-PUBLIC-003); the file and the assay use it.
async fn property_public_003__public_inventory() {
    // The order below is the order the query produced: featured first, then by name.
    let inventory = vec![
        listing("casa-luar", "Casa Luar", Some(1_200_000.0), true),
        listing("casa-brava", "Casa Brava", Some(900_000.0), false),
        listing("casa-alba", "Casa Alba", Some(1_050_000.0), false),
    ];
    let service = PublicListingService::new(
        FakePublicListingRepository::with(inventory.clone()),
        infrastructure().await,
    );
    let ctx = public_website();

    // ---- 1. The anonymous website reads the whole published set. ----
    let found = service
        .listings(&ctx)
        .await
        .expect("the public inventory is a published read");
    assert_eq!(
        found.len(),
        3,
        "{HARNESS}: the inventory is every published listing, not a page of it"
    );

    // ---- 2. The service hands the rows back verbatim — same rows, same order, nothing dropped. ----
    assert_eq!(
        found.iter().map(|row| row.key.as_str()).collect::<Vec<_>>(),
        vec!["casa-luar", "casa-brava", "casa-alba"],
        "{HARNESS}: the repository's order is the merchandising decision (featured first, then by name) and \
         the service must not re-sort it"
    );
    assert_eq!(
        found, inventory,
        "{HARNESS}: the service returns the repository's rows unchanged — visibility is decided once, in the \
         DAO, and a second filter here would make the published set depend on which door a visitor used"
    );
    assert_eq!(
        found[0].hero_media_id.as_deref(),
        Some("hero-casa-luar"),
        "{HARNESS}: a card carries its hero with it — a card without a picture is a card nobody clicks, and \
         the hero must not become a second query per card"
    );

    // ---- 3. A card without a hero is still a row, and its absence is not invented away. ----
    let no_hero = PublicListingService::new(
        FakePublicListingRepository::with(vec![PublicListing {
            key: "casa-sin-hero".into(),
            name: "Casa Sin Hero".into(),
            status: "active".into(),
            hero_media_id: None,
            ..Default::default()
        }]),
        infrastructure().await,
    );
    let bare = no_hero
        .listings(&ctx)
        .await
        .expect("a listing whose hero was never flagged is still published");
    assert_eq!(
        bare.len(),
        1,
        "{HARNESS}: the hero is best-effort. A listing whose hero was never flagged must still appear — \
         hiding it would make a photograph upload a precondition for being listed at all"
    );
    assert!(
        bare[0].hero_media_id.is_none(),
        "{HARNESS}: the missing hero is reported as missing rather than filled with some other asset"
    );

    // ---- 4. NEGATIVE: a failed inventory read is an error, never an empty site. ----
    // This is the failure that would matter most in production. A swallowed error would answer "no listings",
    // and the buyers page would render an empty grid that looks like a deliberate state.
    let failing = PublicListingService::new(
        FakePublicListingRepository::failing(),
        infrastructure().await,
    );
    match failing.listings(&ctx).await {
        Ok(empty) => panic!(
            "{HARNESS}: a failed inventory read must surface as an error, not as an empty inventory — the \
             buyers page would render an empty grid that looks deliberate. Got {empty:?}"
        ),
        Err(_) => {}
    }

    // ---- 5. NEGATIVE: publication is a read, so the inventory must be authorized as a Query. ----
    // The real policy admits a published action only as a Query
    // (`web/src/security/entitlements.rs:235-236`: `kind == Query && PUBLIC_READ_ACTIONS.contains(action)`).
    // A Command here would be refused for every caller, including an internal one.
    let service_source = source::read(&source::workspace_root().join("web/src/public_listings.rs"));
    assert!(
        service_source.contains("\"property.public.read\""),
        "{HARNESS}: the public listing door must authorize the PUBLISHED action `property.public.read`, not \
         the private `property.read` — sharing one action would let the public surface reach private detail"
    );
    assert!(
        !service_source.contains("OperationKind::Command"),
        "{HARNESS}: every public listing operation must be a Query; the policy refuses a published action \
         issued as a Command"
    );

    // ---- 6. The inventory query itself: one statement, ordered, and bounded by the visibility rule. ----
    let dao = source::read(&source::workspace_root().join("db/src/public_listing.rs"));
    let code: String = dao
        .lines()
        .map(source::code_of)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        code.contains("order by coalesce(p.featured, false) desc, p.name asc"),
        "{HARNESS}: the inventory must stay ordered featured-first then by name"
    );
    assert!(
        code.contains("and pm.role in ('hero', 'gallery')"),
        "{HARNESS}: the card hero is resolved from the marked hero or the first photograph — a card's picture \
         must never come from a video or a document"
    );
    assert!(
        code.contains("p.archived_at is null") && code.contains("p.status in ('active', 'under_contract', 'sold')"),
        "{HARNESS}: the inventory must carry the visibility rule"
    );
    assert!(
        code.contains("and p.name is not null"),
        "{HARNESS}: a nameless Property is not a card, and must not appear as an empty one"
    );
}
