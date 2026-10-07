//! PROPERTY.PUBLIC — similar listings (TST-PROPERTY-PUBLIC-006).
//!
//! Contract: **the strip under a Property page shows other listings the site is already showing, and it cannot be
//! asked for a report.** Two rules, and the second is enforced in the service rather than in the query.
//!
//! - **Same visibility rule as the inventory.** A similar listing is a listing the site is showing
//!   (`db/src/public_listing.rs:500-502`). The definition was deliberately kept simple — same `property_type`, most
//!   expensive first — precisely because the previous definition lived in TypeScript with its own filters and
//!   drifted from the inventory rule. A Property that is not published cannot appear in its own strip.
//! - **The limit is clamped, not trusted.** `PublicListingService::similar` clamps the caller's limit to
//!   `1..=24` (`web/src/public_listings.rs:154`) with the reason stated in the code: "A limit from a caller is a
//!   request, not an instruction: the page shows a strip, so it can never ask for a report." Zero becomes one
//!   (an empty strip is a bug report), and a thousand becomes twenty-four.
//!
//! The clamp is the part this test can drive for real, because it is the service's own decision and not the query's.
//! It is asserted at both ends and in between, and against the repository rather than against a return value, so a
//! clamp applied after the read would fail.
//!
//! Level: L3 Composition — the real `PublicListingService` with the real `CasbinAuthorizationPort`, the database
//! faked at the `PublicListingRepository` adapter boundary.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_public__006__similar_listings

use std::sync::{Arc, Mutex};

use db::{DbFailure, DbResult};
use model::{PublicListing, PublicListingCopy, PublicProperty};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, ServiceActor, ServiceActorKind, ServiceContext,
    ServiceInfrastructure,
};
use test_harness::source;
use web::public_listings::{PublicListingRepository, PublicListingService};

const HARNESS: &str = "PROPERTY.PUBLIC/006";

/// The clamp `PublicListingService::similar` applies: a strip, never a report.
const STRIP_MIN: i64 = 1;
const STRIP_MAX: i64 = 24;

fn public_website() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("public-website".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: "property-public-006".into(),
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

fn listing(key: &str, price: f64, property_type: &str) -> PublicListing {
    PublicListing {
        key: key.into(),
        id: format!("id-{key}"),
        name: key.to_uppercase(),
        status: "active".into(),
        list_price: Some(price),
        property_type: Some(property_type.into()),
        hero_media_id: Some(format!("hero-{key}")),
        ..Default::default()
    }
}

#[derive(Clone, Default)]
struct FakePublicListingRepository {
    strip: Arc<Mutex<Vec<PublicListing>>>,
    limits: Arc<Mutex<Vec<i64>>>,
    keys: Arc<Mutex<Vec<String>>>,
    fail: bool,
}

impl FakePublicListingRepository {
    fn with(strip: Vec<PublicListing>) -> Self {
        Self {
            strip: Arc::new(Mutex::new(strip)),
            ..Self::default()
        }
    }

    fn failing() -> Self {
        Self {
            fail: true,
            ..Self::default()
        }
    }

    fn limits(&self) -> Vec<i64> {
        self.limits.lock().expect("the fake repository is never poisoned").clone()
    }

    fn keys(&self) -> Vec<String> {
        self.keys.lock().expect("the fake repository is never poisoned").clone()
    }
}

#[async_trait::async_trait]
impl PublicListingRepository for FakePublicListingRepository {
    async fn listings(&self) -> DbResult<Vec<PublicListing>> {
        unreachable_strip("listings")
    }

    async fn property(&self, _key: &str) -> DbResult<Option<PublicProperty>> {
        unreachable_strip("property")
    }

    async fn media_bytes(&self, _id: &str, _size: &str) -> DbResult<Option<(String, Vec<u8>)>> {
        unreachable_strip("media_bytes")
    }

    async fn similar(&self, key: &str, limit: i64) -> DbResult<Vec<PublicListing>> {
        self.keys
            .lock()
            .expect("the fake repository is never poisoned")
            .push(key.to_owned());
        self.limits
            .lock()
            .expect("the fake repository is never poisoned")
            .push(limit);
        if self.fail {
            return Err(DbFailure::configuration(
                "test.property_public.006",
                "the similar-listings read failed",
            ));
        }
        Ok(self
            .strip
            .lock()
            .expect("the fake repository is never poisoned")
            .iter()
            .take(limit.max(0) as usize)
            .cloned()
            .collect())
    }

    async fn slugs(&self) -> DbResult<Vec<String>> {
        unreachable_strip("slugs")
    }

    async fn listing_copy(&self) -> DbResult<Vec<PublicListingCopy>> {
        unreachable_strip("listing_copy")
    }
}

fn unreachable_strip<T>(method: &'static str) -> DbResult<T> {
    Err(DbFailure::configuration(
        "test.property_public.006",
        format!("{method} is outside the similar-listings subject"),
    ))
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-PUBLIC-006); the file and the assay use it.
async fn property_public_006__similar_listings() {
    let strip = vec![
        listing("casa-brava", 900_000.0, "residential"),
        listing("casa-alba", 1_050_000.0, "residential"),
    ];
    let repository = FakePublicListingRepository::with(strip.clone());
    let service = PublicListingService::new(repository.clone(), infrastructure().await);
    let ctx = public_website();

    // ---- 1. The ordinary strip. ----
    let found = service
        .similar("casa-luar", 3, &ctx)
        .await
        .expect("similar listings are a published read");
    assert_eq!(
        found.len(),
        2,
        "{HARNESS}: the strip returns what the repository produced"
    );
    assert_eq!(
        found, strip,
        "{HARNESS}: the service returns the strip unchanged — 'similar' is the repository's single definition, \
         and a second filter here is how the strip drifts from the inventory rule again"
    );
    assert_eq!(
        &repository.keys()[0],
        "casa-luar",
        "{HARNESS}: the subject key is forwarded to the repository, so the strip is resolved against the same \
         Property the page is showing"
    );

    // ---- 2. NEGATIVE: the caller's limit is clamped, not trusted. ----
    // "A limit from a caller is a request, not an instruction: the page shows a strip, so it can never ask for a
    // report" (`web/src/public_listings.rs:152-153`). Asserted at the repository, so a clamp applied after the
    // read — which would still be too late to bound the query — fails this test.
    let calls_before = repository.limits().len();
    for (asked, expected) in [
        (0_i64, STRIP_MIN),
        (-1, STRIP_MIN),
        (-999, STRIP_MIN),
        (1, 1),
        (3, 3),
        (STRIP_MAX, STRIP_MAX),
        (25, STRIP_MAX),
        (1_000, STRIP_MAX),
        (i64::MAX, STRIP_MAX),
    ] {
        let before = repository.limits().len();
        let _ = service.similar("casa-luar", asked, &ctx).await;
        assert_eq!(
            repository.limits()[before],
            expected,
            "{HARNESS}: a caller asking for {asked} must reach the query as {expected} — the strip is bounded \
             to {STRIP_MIN}..={STRIP_MAX} so it can never become a report"
        );
    }
    assert_eq!(
        repository.limits().len() - calls_before,
        9,
        "{HARNESS}: every clamp case reached the repository exactly once"
    );

    // ---- 3. A limit of zero is a strip of one, never an empty strip. ----
    let one = service
        .similar("casa-luar", 0, &ctx)
        .await
        .expect("a clamped limit is not a failure");
    assert!(
        !one.is_empty(),
        "{HARNESS}: a caller asking for nothing gets one comparable, not an empty strip — an empty strip reads \
         as 'no similar listings exist'"
    );

    // ---- 4. NEGATIVE: a failed strip read is an error, never an empty strip. ----
    // Answering `Ok(vec![])` would be indistinguishable from "this Property has no comparables", which is a
    // claim about the firm's inventory and would be false during an outage.
    let failing = PublicListingService::new(
        FakePublicListingRepository::failing(),
        infrastructure().await,
    );
    match failing.similar("casa-luar", 3, &ctx).await {
        Ok(empty) => panic!(
            "{HARNESS}: a failed strip read must surface as an error, not as an empty strip — an empty strip \
             claims the firm has no comparables. Got {empty:?}"
        ),
        Err(_) => {}
    }

    // ---- 5. The definition of "similar", and the visibility rule it inherits. ----
    let dao = source::read(&source::workspace_root().join("db/src/public_listing.rs"));
    let code: String = dao
        .lines()
        .map(source::code_of)
        .collect::<Vec<_>>()
        .join("\n");

    // A listing is never its own comparable.
    assert!(
        code.contains("and p.id <> subject.id"),
        "{HARNESS}: the strip must exclude the subject — a Property page recommending itself is a bug a \
         `limit`-only test would never catch"
    );
    // Same kind of Property, and the same visibility rule as the inventory. These two together are what stopped
    // the strip from drifting away from the published set.
    assert!(
        code.contains("and (subject.property_type is null or p.property_type = subject.property_type)"),
        "{HARNESS}: a comparable must share the subject's property type"
    );
    assert!(
        code.contains("p.archived_at is null") && code.contains("p.status in ('active', 'under_contract', 'sold')"),
        "{HARNESS}: the strip must carry the SAME visibility rule as the inventory — a similar listing is a \
         listing the site is showing"
    );
    assert!(
        code.contains("order by p.list_price desc nulls last, p.name asc"),
        "{HARNESS}: comparables stay ordered most expensive first, then by name"
    );
}
