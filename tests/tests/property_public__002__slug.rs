//! PROPERTY.PUBLIC — slug (TST-PROPERTY-PUBLIC-002).
//!
//! Contract: **a Property is addressed on the public site by its slug, and the slug is the site's own identity for
//! it — not a display string that may change.** Three things depend on that identity and are pinned here:
//!
//! - **The inventory key.** `listings()` publishes `coalesce(p.slug, p.id::text) as row_key`
//!   (`db/src/public_listing.rs:162`), so a card links to `/property/<row_key>` and a Property without a slug falls
//!   back to its immutable id rather than to nothing.
//! - **The sitemap.** `slugs()` returns `p.slug` for every servable Property, and a row with no slug is ABSENT
//!   rather than null (`db/src/public_listing.rs:583-596`) — a sitemap entry with no URL is not an entry.
//! - **Resolution.** `property()` resolves a key by slug, by name in any case, by the dashed form of the name, or
//!   by id (`db/src/public_listing.rs:288-293`), so a hand-typed name still reaches the right page.
//!
//! The service's own contribution is deliberately small and is asserted exactly: it forwards the key **verbatim**.
//! `public_property` trims at the router (`web/src/api/routes/public.rs:196`, `query.key.trim()`), and the service
//! does not re-normalize, lowercase or validate. That is the architecture — the router is the one normalizer, so
//! two callers cannot disagree about what a key means — and a service that quietly lowercased here would break the
//! router's contract without anyone noticing.
//!
//! Level: L3 Composition — the real `PublicListingService` with the real `CasbinAuthorizationPort` (the published
//! action `property.public.read`), the database faked at the `PublicListingRepository` adapter boundary. The
//! resolution SQL is pinned structurally at the end rather than re-declared here.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_public__002__slug

use std::sync::{Arc, Mutex};

use db::{DbFailure, DbResult};
use model::{PublicListing, PublicListingCopy, PublicProperty};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, ServiceActor, ServiceActorKind, ServiceContext,
    ServiceInfrastructure,
};
use test_harness::source;
use web::public_listings::{PublicListingRepository, PublicListingService};

const HARNESS: &str = "PROPERTY.PUBLIC/002";

/// The anonymous public website, exactly as `web/src/api/context.rs:132` builds it: a system actor, no principal.
/// `property.public.read` is a published action (`web/src/security/entitlements.rs:235-236`), so this context is
/// admitted for the query — the service is reachable by an anonymous visitor, which is the point.
fn public_website() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("public-website".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: "property-public-002".into(),
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

/// A Property that resolves, keyed by slug.
fn listed(slug: &str, name: &str) -> PublicProperty {
    PublicProperty {
        id: "property-1".into(),
        key: slug.into(),
        name: name.into(),
        status: "active".into(),
        hero_media_id: Some("media-1".into()),
        ..Default::default()
    }
}

/// Records every key the service asked about, and answers `property()` from a fixed table.
#[derive(Clone, Default)]
struct RecordingPublicListingRepository {
    keys: Arc<Mutex<Vec<String>>>,
    limits: Arc<Mutex<Vec<i64>>>,
    slugs: Arc<Mutex<Vec<String>>>,
    resolvable: Arc<Mutex<Vec<(String, PublicProperty)>>>,
    fail: bool,
}

impl RecordingPublicListingRepository {
    fn new() -> Self {
        Self::default()
    }

    fn failing() -> Self {
        Self {
            fail: true,
            ..Self::default()
        }
    }

    fn resolving(self, key: &str, property: PublicProperty) -> Self {
        self.resolvable
            .lock()
            .expect("the fake repository is never poisoned")
            .push((key.to_owned(), property));
        self
    }

    fn with_slugs(self, slugs: &[&str]) -> Self {
        *self.slugs.lock().expect("the fake repository is never poisoned") =
            slugs.iter().map(|slug| slug.to_string()).collect();
        self
    }

    fn keys(&self) -> Vec<String> {
        self.keys.lock().expect("the fake repository is never poisoned").clone()
    }

    fn limits(&self) -> Vec<i64> {
        self.limits.lock().expect("the fake repository is never poisoned").clone()
    }

    fn slugs_read(&self) -> Vec<String> {
        self.slugs.lock().expect("the fake repository is never poisoned").clone()
    }

    fn calls(&self) -> usize {
        self.keys().len() + self.slugs_read().len()
    }
}

#[async_trait::async_trait]
impl PublicListingRepository for RecordingPublicListingRepository {
    async fn listings(&self) -> DbResult<Vec<PublicListing>> {
        if self.fail {
            return Err(DbFailure::configuration(
                "test.property_public.002",
                "the inventory read failed",
            ));
        }
        Ok(Vec::new())
    }

    async fn property(&self, key: &str) -> DbResult<Option<PublicProperty>> {
        self.keys
            .lock()
            .expect("the fake repository is never poisoned")
            .push(key.to_owned());
        if self.fail {
            return Err(DbFailure::configuration(
                "test.property_public.002",
                "the property read failed",
            ));
        }
        Ok(self
            .resolvable
            .lock()
            .expect("the fake repository is never poisoned")
            .iter()
            .find(|(resolvable, _)| resolvable == key)
            .map(|(_, property)| property.clone()))
    }

    async fn media_bytes(&self, _id: &str, _size: &str) -> DbResult<Option<(String, Vec<u8>)>> {
        unreachable_public("media_bytes")
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
        Ok(Vec::new())
    }

    async fn slugs(&self) -> DbResult<Vec<String>> {
        if self.fail {
            return Err(DbFailure::configuration(
                "test.property_public.002",
                "the sitemap read failed",
            ));
        }
        Ok(self.slugs_read())
    }

    async fn listing_copy(&self) -> DbResult<Vec<PublicListingCopy>> {
        unreachable_public("listing_copy")
    }
}

fn unreachable_public<T>(method: &'static str) -> DbResult<T> {
    Err(DbFailure::configuration(
        "test.property_public.002",
        format!("{method} is outside the slug subject"),
    ))
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-PUBLIC-002); the file and the assay use it.
async fn property_public_002__slug() {
    // ---- 1. The sitemap is exactly the servable slugs, in the order the repository gave them. ----
    let repository = RecordingPublicListingRepository::new()
        .with_slugs(&["casa-luar", "casa-brava"])
        .resolving("casa-luar", listed("casa-luar", "Casa Luar"));
    let service = PublicListingService::new(repository.clone(), infrastructure().await);
    let ctx = public_website();

    let slugs = service.slugs(&ctx).await.expect("the sitemap is published");
    assert_eq!(
        slugs,
        vec!["casa-luar".to_owned(), "casa-brava".to_owned()],
        "{HARNESS}: the sitemap is the list of servable slugs, in the repository's order"
    );

    // ---- 2. A Property resolves by its slug. ----
    let found = service
        .property("casa-luar", &ctx)
        .await
        .expect("a published slug resolves")
        .expect("casa-luar is a servable Property");
    assert_eq!(
        found.key, "casa-luar",
        "{HARNESS}: the resolved Property carries the slug it was addressed by as its key"
    );
    assert_eq!(
        found.name, "Casa Luar",
        "{HARNESS}: resolution yields the Property, not an echo of the key"
    );

    // ---- 3. The service forwards the key verbatim; the router is the one normalizer. ----
    // `public_property` trims (`web/src/api/routes/public.rs:196`), so the service receives an already-clean key.
    // These three are what would break if the service began normalizing, trimming or lower-casing on its own:
    // the query, the dashed-name form and a padded key.
    for key in ["Casa-Luar", "casa-luar", "  casa-luar  "] {
        let before = repository.keys().len();
        let _ = service.property(key, &ctx).await;
        assert_eq!(
            &repository.keys()[before],
            key,
            "{HARNESS}: the service must forward {key:?} to the repository exactly as given — the router is \
             the single normalizer, and a service that re-normalized would make two callers disagree"
        );
    }

    // ---- 4. NEGATIVE: an unknown key is a miss, not an error and not a fabricated row. ----
    // `public_property` documents it: "A key that resolves to nothing answers `null`, not an error"
    // (`web/src/api/routes/public.rs:194-195`). A 404 for a real-but-hidden Property and a 200 with a stub for an
    // unknown one would both be wrong; `None` is the honest answer for both.
    match service.property("no-such-listing", &ctx).await {
        Ok(None) => {}
        Ok(Some(leaked)) => panic!(
            "{HARNESS}: an unknown slug must resolve to nothing, not to a fabricated Property: {leaked:?}"
        ),
        Err(error) => panic!(
            "{HARNESS}: an unknown slug is a miss, not a failure — the browser would see a 500 for a typo. \
             Got {error:?}"
        ),
    }

    // ---- 5. NEGATIVE: a failed read is an error, never a miss. ----
    // `Ok(None)` is the answer to "no such Property". If a database fault also answered `None`, a Property page
    // would answer "not found" during an outage — indistinguishable from being unpublished.
    let failing = PublicListingService::new(
        RecordingPublicListingRepository::failing(),
        infrastructure().await,
    );
    match failing.property("casa-luar", &ctx).await {
        Ok(None) => panic!(
            "{HARNESS}: a failed read must surface as an error; answering `None` would tell the browser the \
             Property does not exist, which is what an unpublished Property answers"
        ),
        Ok(Some(_)) | Err(_) => {}
    }
    match failing.slugs(&ctx).await {
        Ok(empty) => panic!(
            "{HARNESS}: a failed sitemap read must surface as an error, not as an empty sitemap — an empty \
             sitemap tells Google the site has no listings. Got {empty:?}"
        ),
        Err(_) => {}
    }

    // ---- 6. The resolution rule itself, pinned where it lives. ----
    let dao = source::read(&source::workspace_root().join("db/src/public_listing.rs"));
    let code: String = dao
        .lines()
        .map(source::code_of)
        .collect::<Vec<_>>()
        .join("\n");

    // Four ways to name a Property, so a hand-typed name still reaches the page.
    for resolver in [
        "p.slug = $1",
        "or lower(p.name) = lower($1)",
        "or lower(replace($1, '-', ' ')) = lower(p.name)",
        "or p.id::text = $1",
    ] {
        assert!(
            code.contains(resolver),
            "{HARNESS}: a Property must keep resolving by {resolver:?} — dropping a form makes a shared link \
             404 after a rename"
        );
    }
    // The identity: the slug, falling back to the immutable id rather than to null.
    assert!(
        code.contains("coalesce(p.slug, p.id::text) as row_key"),
        "{HARNESS}: the published key must be the slug, falling back to the immutable id"
    );
    // The sitemap omits rows with no slug instead of publishing an entry with no URL.
    assert!(
        code.contains("and p.slug is not null"),
        "{HARNESS}: the sitemap must omit Properties that have no slug — a sitemap entry with no URL is not an entry"
    );
    // And the resolver is bounded by the same visibility rule as the inventory.
    assert!(
        code.contains("p.archived_at is null") && code.contains("p.status in ('active', 'under_contract', 'sold')"),
        "{HARNESS}: resolving a Property must carry the visibility rule, or an unpublished Property is \
         reachable by typing its slug"
    );
}
