//! PROPERTY.PUBLIC — publish eligibility (TST-PROPERTY-PUBLIC-001).
//!
//! Contract: **a listing becomes publishable only when its copy, its slug and its photographs are all present, and
//! the readiness answer is never guessed.** `PublishingDao::project` (`db/src/publishing.rs:105-168`) derives the
//! answer from the Property's own columns and, when something is absent, NAMES it:
//!
//! ```text
//! let copy_ready = present(public_remarks) && present(short_description) && present(seo_title) && present(seo_description);
//! let media_ready = image_count > 0 && hero_count > 0;
//! let website_ready = present(slug) && copy_ready && media_ready;
//! ```
//!
//! Three things make that worth a contract rather than a look:
//!
//! - **The name is required, not the truth.** A `website_ready` listing with no hero is a live page with a blank
//!   masthead, so `media_ready` demands `hero_count > 0`, not merely "has photographs".
//! - **The Stellar package is a different, stricter gate** than the website (`stellar_package_ready`, :115-121):
//!   it additionally requires the legal and MLS facts. One flag cannot stand for both.
//! - **A missing input is named, never inferred.** `missing` (:123-144) carries the twelve labels a listing needs,
//!   so the operator is told what to supply instead of being told "not ready".
//!
//! The eligibility is also a door, not just a computation: `PublishingService::snapshot` authorizes under
//! `property.read` (`web/src/publishing.rs:39`) — the **private** action, not `property.public.read`. Under the real
//! Casbin policy an anonymous website visitor is refused that action ("private property detail is not published",
//! `web/src/security/entitlements.rs:719-722`), so a draft's readiness is never readable by the public. This test
//! drives the real policy and asserts that refusal rather than assuming it.
//!
//! Level: L3 Composition — the real `PublishingService` with the real `CasbinAuthorizationPort`, the database faked
//! at the `PublishingRepository` adapter boundary. The readiness projection itself is a private function inside
//! `db/src/publishing.rs`, so it is pinned structurally at the end of this test rather than re-declared here.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_public__001__publish_eligibility

use std::sync::{Arc, Mutex};

use db::{DbFailure, DbResult};
use model::{PublishingListing, PublishingSnapshot};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, ServiceActor, ServiceActorKind, ServiceContext,
    ServiceInfrastructure, ServicePrincipal,
};
use test_harness::source;
use web::publishing::{PublishingRepository, PublishingService};

const HARNESS: &str = "PROPERTY.PUBLIC/001";

/// The anonymous public website, exactly as `web/src/api/context.rs:132` builds it: a system actor, no principal.
fn public_website() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("public-website".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: "property-public-001-public".into(),
        causation_id: None,
        principal: None,
    }
}

/// An internal reader holding the private `property.read` entitlement.
///
/// The real policy resolves a principal by its grants: an internal account whose `entitlement_codes` name the
/// action it wants (`web/src/security/entitlements.rs:327-338`), or whose role is `root`/`owner`. A level string
/// alone grants nothing, so the code is named explicitly rather than borrowed from the catalog.
fn internal_reader() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("u1".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "property-public-001-internal".into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "u1".into(),
            level: "BUSINESS_POWER_USER".into(),
            role_codes: vec![],
            account_type: model::security::INTERNAL_ACCOUNT.into(),
            entitlement_codes: vec!["property.read".into()],
        }),
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

/// A listing that is ready for the website, and one that is ready for the Stellar package — the two are different
/// gates and a test that used the same listing for both could not tell them apart.
fn ready_listing() -> PublishingListing {
    PublishingListing {
        property_id: "property-1".into(),
        name: "Casa Luar".into(),
        status: "active".into(),
        slug: Some("casa-luar".into()),
        location: Some("Punta Cana".into()),
        is_active_listing: true,
        is_published: true,
        list_price: Some("1200000".into()),
        property_type: Some("residential".into()),
        image_count: 4,
        video_count: 0,
        has_hero: true,
        copy_ready: true,
        media_ready: true,
        website_ready: true,
        facebook_ready: true,
        stellar_package_ready: true,
        listing_type: Some("Exclusive Right".into()),
        agent_mls_id: Some("MLS1".into()),
        missing: vec![],
    }
}

/// A listing with photographs but no hero — the case `media_ready` exists to refuse.
fn no_hero_listing() -> PublishingListing {
    PublishingListing {
        property_id: "property-2".into(),
        name: "Casa Sin Hero".into(),
        status: "active".into(),
        slug: Some("casa-sin-hero".into()),
        is_active_listing: true,
        copy_ready: true,
        media_ready: false,
        website_ready: false,
        facebook_ready: true,
        stellar_package_ready: false,
        missing: vec!["Hero image".into()],
        ..Default::default()
    }
}

/// Hands back a fixed snapshot and records how many times it was asked, so "was the snapshot read?" is answerable
/// without a database. `fail` makes the read fail, which is how the refusal case below is driven.
#[derive(Clone, Default)]
struct FakePublishingRepository {
    snapshot: Arc<Mutex<Option<PublishingSnapshot>>>,
    fail: bool,
    reads: Arc<Mutex<usize>>,
}

impl FakePublishingRepository {
    fn new(snapshot: PublishingSnapshot) -> Self {
        Self {
            snapshot: Arc::new(Mutex::new(Some(snapshot))),
            fail: false,
            reads: Arc::new(Mutex::new(0)),
        }
    }

    fn failing() -> Self {
        Self {
            snapshot: Arc::new(Mutex::new(None)),
            fail: true,
            reads: Arc::new(Mutex::new(0)),
        }
    }

    fn reads(&self) -> usize {
        *self.reads.lock().expect("the fake repository is never poisoned")
    }
}

#[async_trait::async_trait]
impl PublishingRepository for FakePublishingRepository {
    async fn snapshot(&self) -> DbResult<PublishingSnapshot> {
        *self.reads.lock().expect("the fake repository is never poisoned") += 1;
        if self.fail {
            return Err(DbFailure::configuration(
                "test.property_public.001",
                "the publishing read failed",
            ));
        }
        Ok(self
            .snapshot
            .lock()
            .expect("the fake repository is never poisoned")
            .clone()
            .unwrap_or_default())
    }
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-PUBLIC-001); the file and the assay use it.
async fn property_public_001__publish_eligibility() {
    let ready = ready_listing();
    let no_hero = no_hero_listing();
    let snapshot = PublishingSnapshot {
        listings: vec![ready.clone(), no_hero.clone()],
        ready_count: 1,
        live_count: 1,
    };
    let repository = FakePublishingRepository::new(snapshot.clone());
    let service = PublishingService::new(repository.clone(), infrastructure().await);

    // ---- 1. The eligibility snapshot is readable by an internal actor, verbatim. --------------------
    let found = service
        .snapshot(&internal_reader())
        .await
        .expect("an internal reader may read the publishing readiness snapshot");
    assert_eq!(
        found.listings.len(),
        2,
        "{HARNESS}: the snapshot carries every candidate listing, ready or not — the operator needs to see \
         the ones that are not ready in order to fix them"
    );
    assert_eq!(
        found.ready_count, 1,
        "{HARNESS}: exactly one of the two listings is website-ready; the other has no hero"
    );
    assert_eq!(
        found.live_count, 1,
        "{HARNESS}: readiness and liveness are different counts — a ready listing that is not published is not live"
    );
    assert_eq!(
        found.listings[1].missing,
        vec!["Hero image".to_owned()],
        "{HARNESS}: a listing that is not ready NAMES what is missing rather than reporting a bare 'not ready'"
    );
    assert!(
        !found.listings[1].website_ready && found.listings[1].facebook_ready,
        "{HARNESS}: photographs without a hero are not website-ready even though the Facebook gate is met — \
         the two gates are deliberately different"
    );

    // ---- 2. NEGATIVE: the public website is refused. -----------------------------------------------
    // `publishing.snapshot` authorizes under `property.read`, the private action. Under the real policy an
    // anonymous visitor has no principal and is refused, so a draft's readiness — which reveals exactly which
    // listings are unfinished — cannot be read by the public.
    let public_reads_before = repository.reads();
    let refusal = service.snapshot(&public_website()).await;
    match refusal {
        Ok(leaked) => panic!(
            "{HARNESS}: the anonymous public website must NOT be able to read the publishing readiness \
             snapshot; it received {leaked:?}"
        ),
        Err(_) => {}
    }
    assert_eq!(
        repository.reads(),
        public_reads_before,
        "{HARNESS}: the refusal happens at authorization, before the repository is reached — a refused \
         reader must not even cause the query"
    );

    // ---- 3. NEGATIVE: a failed read is an error, never an empty snapshot. --------------------------
    // An operator looking at "0 ready of 0" after a database fault would conclude the firm has no listings.
    let failing = PublishingService::new(FakePublishingRepository::failing(), infrastructure().await);
    match failing.snapshot(&internal_reader()).await {
        Ok(empty) => panic!(
            "{HARNESS}: a failed read must surface as an error, not as an empty snapshot; it returned \
             {empty:?} — '0 ready' would read as 'the firm has no listings'"
        ),
        Err(_) => {}
    }

    // ---- 4. NEGATIVE: publication is a READ. The service must not issue a Command. ------------------
    // The policy refuses `property.public.read` as a Command ("the site reads its listings; it does not write
    // them", `web/src/security/entitlements.rs:733-740`). A snapshot is a query, so this service must never
    // present itself as a write — asserted structurally below.
    let repository_source = source::read(
        &source::workspace_root().join("web/src/publishing.rs"),
    );
    assert!(
        repository_source.contains("OperationKind::Query"),
        "{HARNESS}: `publishing.snapshot` must authorize as a Query; a Command here would be refused by the \
         real policy for the public read action"
    );
    assert!(
        !repository_source.contains("OperationKind::Command"),
        "{HARNESS}: reading readiness must not be issued as a Command"
    );

    // ---- 5. The eligibility projection itself, pinned where it actually lives. ----------------------
    // `project()` is private to `db/src/publishing.rs` and composes in SQL, so it cannot be called from here
    // without re-declaring it — which would test the copy rather than the production code. What is pinned
    // instead is the rule each flag is built from, so removing or weakening a gate fails this test.
    let dao = source::read(&source::workspace_root().join("db/src/publishing.rs"));
    let code: String = dao
        .lines()
        .map(source::code_of)
        .collect::<Vec<_>>()
        .join("\n");

    // A hero, not merely a photograph: a live page with a blank masthead is not publishable.
    assert!(
        code.contains("let media_ready = row.image_count > 0 && row.hero_count > 0;"),
        "{HARNESS}: media readiness demands a HERO image, not just any photograph"
    );
    assert!(
        code.contains("let website_ready = present(row.slug.as_deref()) && copy_ready && media_ready;"),
        "{HARNESS}: website readiness needs a slug, the copy and the hero together"
    );
    // The Stellar package is a stricter gate than the website.
    assert!(
        code.contains("let stellar_package_ready = present(row.list_price.as_deref())"),
        "{HARNESS}: the Stellar package gate must remain its own predicate, requiring the list price"
    );
    for required in [
        "present(row.catastro_number.as_deref())",
        "present(row.legal_owner_name.as_deref())",
        "present(row.listing_type.as_deref())",
        "present(row.agent_mls_id.as_deref())",
    ] {
        assert!(
            code.contains(required),
            "{HARNESS}: the Stellar package gate must keep requiring {required} — dropping a legal or MLS \
             fact would ship a package that cannot be filed"
        );
    }
    // A missing input is named, never inferred.
    for label in [
        "\"List price\"",
        "\"Property type\"",
        "\"Catastro\"",
        "\"Legal owner\"",
        "\"Public remarks\"",
        "\"Short description\"",
        "\"SEO title\"",
        "\"SEO description\"",
        "\"Hero image\"",
        "\"Listing photos\"",
        "\"MLS listing type\"",
        "\"Agent MLS ID\"",
    ] {
        assert!(
            code.contains(label),
            "{HARNESS}: the readiness report must keep naming {label}, so an operator is told what to supply"
        );
    }
    // Archived rows are not candidates at all.
    assert!(
        code.contains("p.archived_at is null"),
        "{HARNESS}: an archived Property is not a publishing candidate"
    );
}
