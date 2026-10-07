//! PROPERTY.PUBLIC — property detail (TST-PROPERTY-PUBLIC-004).
//!
//! Contract: **one Property page, assembled in one place, with the hero resolved once so no surface has to remember
//! the rule.** `PublicListingDao::property` (`db/src/public_listing.rs:222-496`) is the whole page: it resolves the
//! Property by any identifier that names it, then reads its FULL media list and derives the three word lists the
//! screen draws.
//!
//! The parts worth a contract:
//!
//! - **The full media list, not just the hero.** Photographs, Mux videos and documents are all `media` rows, and
//!   the page sorts them into a gallery, a video strip and a document list (`db/src/public_listing.rs:308-310`).
//!   Returning only the hero would make the conditional video and document panels impossible.
//! - **The hero is resolved here** — the marked hero first, otherwise the first photograph (:386-390) — so no
//!   surface re-implements it and two surfaces cannot disagree.
//! - **The derived word lists are the surface's contract.** `view_type`, `amenities` and `lifestyle_tags` are
//!   labels, not the eight stored booleans (:392-450), and every surface wants the same words.
//! - **A key that resolves to nothing answers `None`, not an error** — documented at
//!   `web/src/api/routes/public.rs:194-195`.
//!
//! Level: L3 Composition — the real `PublicListingService` with the real `CasbinAuthorizationPort`, the database
//! faked at the `PublicListingRepository` adapter boundary. The assembly SQL is pinned structurally below.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_public__004__property_detail

use std::sync::Arc;

use db::{DbFailure, DbResult};
use model::{PublicListing, PublicListingCopy, PublicProperty, PublicPropertyMedia};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, ServiceActor, ServiceActorKind, ServiceContext,
    ServiceInfrastructure,
};
use test_harness::source;
use web::public_listings::{PublicListingRepository, PublicListingService};

const HARNESS: &str = "PROPERTY.PUBLIC/004";

fn public_website() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("public-website".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: "property-public-004".into(),
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

fn media(id: &str, role: &str, media_type: &str, sort_order: i32) -> PublicPropertyMedia {
    PublicPropertyMedia {
        id: id.into(),
        role: role.into(),
        media_type: media_type.into(),
        sort_order,
        mime_type: Some("image/jpeg".into()),
        ..Default::default()
    }
}

/// A Property page with a gallery, a hero, a video and a document — the four things the cockpit renders.
fn full_page() -> PublicProperty {
    PublicProperty {
        id: "property-1".into(),
        key: "casa-luar".into(),
        name: "Casa Luar".into(),
        status: "active".into(),
        property_type: Some("residential".into()),
        list_price: Some(1_200_000.0),
        bedrooms: Some(4),
        hero_media_id: Some("media-hero".into()),
        media: vec![
            media("media-hero", "hero", "image", 0),
            media("media-gallery-1", "gallery", "image", 1),
            media("media-gallery-2", "gallery", "image", 2),
            media("media-video", "video", "video", 0),
            media("media-doc", "document", "document", 0),
        ],
        video_count: 1,
        ..Default::default()
    }
}

#[derive(Clone, Default)]
struct FakePublicListingRepository {
    page: Arc<Option<PublicProperty>>,
    fail: bool,
    reads: Arc<std::sync::atomic::AtomicUsize>,
}

impl FakePublicListingRepository {
    fn with(page: PublicProperty) -> Self {
        Self {
            page: Arc::new(Some(page)),
            fail: false,
            reads: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    fn missing() -> Self {
        Self {
            page: Arc::new(None),
            fail: false,
            reads: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    fn failing() -> Self {
        Self {
            page: Arc::new(None),
            fail: true,
            reads: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    fn reads(&self) -> usize {
        self.reads.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl PublicListingRepository for FakePublicListingRepository {
    async fn listings(&self) -> DbResult<Vec<PublicListing>> {
        unreachable_detail("listings")
    }

    async fn property(&self, _key: &str) -> DbResult<Option<PublicProperty>> {
        self.reads
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.fail {
            return Err(DbFailure::configuration(
                "test.property_public.004",
                "the property detail read failed",
            ));
        }
        Ok(self.page.as_ref().clone())
    }

    async fn media_bytes(&self, _id: &str, _size: &str) -> DbResult<Option<(String, Vec<u8>)>> {
        unreachable_detail("media_bytes")
    }

    async fn similar(&self, _key: &str, _limit: i64) -> DbResult<Vec<PublicListing>> {
        unreachable_detail("similar")
    }

    async fn slugs(&self) -> DbResult<Vec<String>> {
        unreachable_detail("slugs")
    }

    async fn listing_copy(&self) -> DbResult<Vec<PublicListingCopy>> {
        unreachable_detail("listing_copy")
    }
}

fn unreachable_detail<T>(method: &'static str) -> DbResult<T> {
    Err(DbFailure::configuration(
        "test.property_public.004",
        format!("{method} is outside the property-detail subject"),
    ))
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-PUBLIC-004); the file and the assay use it.
async fn property_public_004__property_detail() {
    let page = full_page();
    let repository = FakePublicListingRepository::with(page.clone());
    let service = PublicListingService::new(repository.clone(), infrastructure().await);
    let ctx = public_website();

    // ---- 1. The whole page arrives in one read. ----
    let found = service
        .property("casa-luar", &ctx)
        .await
        .expect("a published Property resolves")
        .expect("casa-luar is servable");
    assert_eq!(repository.reads(), 1, "{HARNESS}: one key, one read");
    assert_eq!(found.name, "Casa Luar");
    assert_eq!(found.list_price, Some(1_200_000.0));

    // ---- 2. The FULL media list comes back, not just the hero. ----
    // Photographs, a Mux video and a document are all `media` rows, and the page sorts them into a gallery, a
    // video strip and a document list. Returning only the hero would make the conditional panels impossible.
    assert_eq!(
        found.media.len(),
        5,
        "{HARNESS}: the page carries the full media list, not just the hero"
    );
    let roles: Vec<&str> = found.media.iter().map(|item| item.role.as_str()).collect();
    for expected in ["hero", "gallery", "video", "document"] {
        assert!(
            roles.contains(&expected),
            "{HARNESS}: a {expected:?} asset must reach the page — it is what makes the {expected} panel \
             conditional rather than always-empty. Got {roles:?}"
        );
    }
    assert_eq!(
        found.video_count, 1,
        "{HARNESS}: the page reports its video count, which is what the conditional video panel keys off"
    );

    // ---- 3. The hero is resolved once, here, and it is an IMAGE. ----
    assert_eq!(
        found.hero_media_id.as_deref(),
        Some("media-hero"),
        "{HARNESS}: the marked hero is the page's hero; resolving it here is what stops two surfaces from \
         disagreeing about which picture leads"
    );
    assert!(
        found
            .media
            .iter()
            .find(|item| item.id == found.hero_media_id.as_deref().unwrap_or_default())
            .is_some_and(|item| item.media_type == "image"),
        "{HARNESS}: the hero is always a photograph, never a video or a document"
    );

    // ---- 4. The service hands the page back unchanged. ----
    assert_eq!(
        found, page,
        "{HARNESS}: the service assembles nothing of its own — it returns the repository's page so that what \
         is stored is what is shown"
    );

    // ---- 5. NEGATIVE: a key that resolves to nothing answers `None`, not an error. ----
    let missing = PublicListingService::new(
        FakePublicListingRepository::missing(),
        infrastructure().await,
    );
    match missing.property("no-such-listing", &ctx).await {
        Ok(None) => {}
        Ok(Some(leaked)) => panic!(
            "{HARNESS}: an unknown key must answer `null`, not a fabricated Property: {leaked:?}"
        ),
        Err(error) => panic!(
            "{HARNESS}: an unknown key is a miss, not a failure; a 500 for a mistyped URL is wrong. Got {error:?}"
        ),
    }

    // ---- 6. NEGATIVE: a failed detail read is an error, never a miss. ----
    // `None` is the answer to "no such Property". If a database fault also answered `None`, a live listing page
    // would answer "not found" during an outage.
    let failing = PublicListingService::new(
        FakePublicListingRepository::failing(),
        infrastructure().await,
    );
    match failing.property("casa-luar", &ctx).await {
        Ok(None) => panic!(
            "{HARNESS}: a failed read must surface as an error; answering `None` makes a live listing look \
             unpublished"
        ),
        Ok(Some(_)) | Err(_) => {}
    }

    // ---- 7. The page assembly itself, pinned where it lives. ----
    let dao = source::read(&source::workspace_root().join("db/src/public_listing.rs"));
    let code: String = dao
        .lines()
        .map(source::code_of)
        .collect::<Vec<_>>()
        .join("\n");

    // The hero rule, stated once: the marked hero first, otherwise the first photograph.
    assert!(
        code.contains(".find(|item| item.media_type == \"image\" && item.role == \"hero\")"),
        "{HARNESS}: the page must prefer the MARKED hero"
    );
    assert!(
        code.contains(".or_else(|| media.iter().find(|item| item.media_type == \"image\"))"),
        "{HARNESS}: a listing whose hero was never flagged must still have a picture — the first photograph"
    );
    // Videos count by media type OR by role, because a Mux asset is attached either way.
    assert!(
        code.contains("vm.media_type = 'video' or vpm.role in ('video', 'short')"),
        "{HARNESS}: the video count must accept both a video asset and a video role, or a conditionally-video \
         listing would report zero videos and hide its player"
    );
    // The media read is the full list, ordered hero first.
    assert!(
        code.contains(
            "order by case when pm.role = 'hero' then 0 else 1 end, pm.sort_order, m.created_at"
        ),
        "{HARNESS}: the page's media must be ordered hero-first then by sort order"
    );
    // The page is bounded by the visibility rule like every other public read.
    assert!(
        code.contains("p.archived_at is null") && code.contains("p.status in ('active', 'under_contract', 'sold')"),
        "{HARNESS}: the detail read must carry the visibility rule, or a draft is reachable by typing its slug"
    );
}
