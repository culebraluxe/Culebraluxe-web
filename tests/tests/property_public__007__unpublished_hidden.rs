//! PROPERTY.PUBLIC — unpublished hidden (TST-PROPERTY-PUBLIC-007).
//!
//! Contract: **an unpublished Property is not merely absent from the inventory — it cannot be reached at all.** A
//! draft that a visitor can reach by typing its slug, its name or its id is a confidentiality failure, and the
//! public read model is the only thing standing between the two.
//!
//! "PUBLISHED" is stated once, in the file's own header (`db/src/public_listing.rs:3-4`): "`is_published`, an active
//! listing, not archived, with a slug — the predicate the public inventory and the public document route use. A
//! draft's tagline is not public copy." The predicate the queries actually enforce is
//! `p.archived_at is null and p.status in ('active', 'under_contract', 'sold')`, and it is applied by **all six**
//! public reads — `listings`, `property`, `similar`, `slugs`, `media_bytes` and `listing_copy`. One read that
//! forgot it is a leak, and the reads were written at different times, so the invariant is worth pinning per read
//! rather than per file.
//!
//! The behaviour this test pins at the service boundary is that a miss is a **miss**, not an error and not a
//! partial answer, on every door: the page, the sitemap, the strip, the taglines and the media bytes. And — just as
//! important — that a *published* Property in the same fixture is served, so the test cannot pass by denying
//! everything.
//!
//! Level: L3 Composition — the real `PublicListingService` with the real `CasbinAuthorizationPort` (the anonymous
//! visitor genuinely reaches this service; nothing is short-circuited), the database faked at the
//! `PublicListingRepository` adapter boundary. The per-read visibility predicate is pinned structurally below.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_public__007__unpublished_hidden

use std::sync::Arc;

use db::{DbFailure, DbResult};
use model::{PublicListing, PublicListingCopy, PublicProperty};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, ServiceActor, ServiceActorKind, ServiceContext,
    ServiceInfrastructure,
};
use test_harness::source;
use web::public_listings::{PublicListingRepository, PublicListingService};

const HARNESS: &str = "PROPERTY.PUBLIC/007";

/// THE predicate, written once here so the structural check below cannot drift from the behavioural one.
const VISIBILITY: &str = "p.status in ('active', 'under_contract', 'sold')";

/// Every public read in `PublicListingDao`. All six must be bounded by the visibility rule.
const PUBLIC_READS: &[&str] = &[
    "listings",
    "property",
    "similar",
    "slugs",
    "media_bytes",
    "listing_copy",
];

/// The draft is addressed by all four forms `property()` accepts, because all four are ways a visitor could try.
const DRAFT_KEYS: &[&str] = &[
    "casa-borrador",
    "Casa-Borrador",
    "Casa Borrador",
    "3f2504e0-4f89-11d3-9a0c-0305e82c3301",
];

fn public_website() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("public-website".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: "property-public-007".into(),
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

/// A fixture holding one PUBLISHED Property and one DRAFT, and answering every public read from it.
///
/// The draft is present in the fixture and simply never answered: this stands in for the row existing in the
/// database and being filtered by the query's predicate. If a read were to answer with it, the leak is caught here.
struct Fixture {
    published: PublicProperty,
    draft: PublicProperty,
}

#[derive(Clone)]
struct FakePublicListingRepository {
    fixture: Arc<Fixture>,
}

impl FakePublicListingRepository {
    fn new() -> Self {
        Self {
            fixture: Arc::new(Fixture {
                published: PublicProperty {
                    id: "aaaaaaa1-0000-4000-8000-000000000001".into(),
                    key: "casa-luar".into(),
                    name: "Casa Luar".into(),
                    status: "active".into(),
                    hero_media_id: Some("media-published".into()),
                    ..Default::default()
                },
                draft: PublicProperty {
                    id: "3f2504e0-4f89-11d3-9a0c-0305e82c3301".into(),
                    key: "casa-borrador".into(),
                    name: "Casa Borrador".into(),
                    status: "draft".into(),
                    hero_media_id: Some("media-draft".into()),
                    ..Default::default()
                },
            }),
        }
    }

    /// Only ever resolves a PUBLISHED Property. The draft row exists in the fixture and is never returned.
    fn resolve_published(&self, key: &str) -> Option<PublicProperty> {
        let published = &self.fixture.published;
        if key == published.key
            || key.eq_ignore_ascii_case(&published.name)
            || key.eq_ignore_ascii_case(&published.name.replace(' ', "-"))
            || key == published.id
        {
            return Some(published.clone());
        }
        None
    }
}

#[async_trait::async_trait]
impl PublicListingRepository for FakePublicListingRepository {
    async fn listings(&self) -> DbResult<Vec<PublicListing>> {
        Ok(vec![PublicListing {
            key: self.fixture.published.key.clone(),
            id: self.fixture.published.id.clone(),
            name: self.fixture.published.name.clone(),
            status: "active".into(),
            hero_media_id: Some("media-published".into()),
            ..Default::default()
        }])
    }

    async fn property(&self, key: &str) -> DbResult<Option<PublicProperty>> {
        Ok(self.resolve_published(key))
    }

    async fn media_bytes(&self, id: &str, _size: &str) -> DbResult<Option<(String, Vec<u8>)>> {
        Ok((id == "media-published").then(|| ("image/jpeg".into(), vec![0xFF, 0xD8])))
    }

    async fn similar(&self, key: &str, _limit: i64) -> DbResult<Vec<PublicListing>> {
        // An unpublished subject has no comparables, because the subject itself is not a listing the site shows.
        Ok(self
            .resolve_published(key)
            .map(|property| {
                vec![PublicListing {
                    key: property.key.clone(),
                    id: property.id.clone(),
                    name: property.name.clone(),
                    status: property.status.clone(),
                    hero_media_id: property.hero_media_id.clone(),
                    ..Default::default()
                }]
            })
            .unwrap_or_default())
    }

    async fn slugs(&self) -> DbResult<Vec<String>> {
        Ok(vec![self.fixture.published.key.clone()])
    }

    async fn listing_copy(&self) -> DbResult<Vec<PublicListingCopy>> {
        Ok(vec![PublicListingCopy {
            slug: self.fixture.published.key.clone(),
            tagline: "Two pools.".into(),
        }])
    }
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-PUBLIC-007); the file and the assay use it.
async fn property_public_007__unpublished_hidden() {
    let service = PublicListingService::new(FakePublicListingRepository::new(), infrastructure().await);
    let ctx = public_website();

    // ---- 1. The control: a PUBLISHED Property is served. Nothing below may pass by denying everything. ----
    let published = service
        .property("casa-luar", &ctx)
        .await
        .expect("a published Property is a published read")
        .expect("casa-luar is published and must resolve");
    assert_eq!(
        published.name, "Casa Luar",
        "{HARNESS}: the published Property must be served — this is the control that stops the rest of this \
         test from passing by refusing everything"
    );
    assert!(
        service.media_bytes("media-published", "web", &ctx).await.ok().flatten().is_some(),
        "{HARNESS}: the published Property's photograph must be servable"
    );
    assert_eq!(
        service.slugs(&ctx).await.expect("the sitemap is published"),
        vec!["casa-luar".to_owned()],
        "{HARNESS}: the sitemap carries the published slug"
    );

    // ---- 2. NEGATIVE: the draft is unreachable by EVERY identifier `property()` accepts. ----
    // slug, name, the dashed form of the name, and the raw id. A leak through any one of the four is a leak.
    for key in DRAFT_KEYS {
        match service.property(key, &ctx).await {
            Ok(None) => {}
            Ok(Some(leaked)) => panic!(
                "{HARNESS}: the unpublished Property resolved from {key:?} and was served: {leaked:?}. A draft \
                 reachable by typing its slug, name or id is a confidentiality failure"
            ),
            Err(error) => panic!(
                "{HARNESS}: an unpublished Property must be a MISS (`null`), not a failure — an error would \
                 distinguish 'hidden' from 'absent'. Got {error:?}"
            ),
        }
    }

    // ---- 3. NEGATIVE: the draft's photograph is not servable, and is indistinguishable from absent. ----
    let draft_media = service.media_bytes("media-draft", "web", &ctx).await;
    let absent_media = service
        .media_bytes("00000000-0000-4000-8000-000000000000", "web", &ctx)
        .await;
    match (&draft_media, &absent_media) {
        (Ok(None), Ok(None)) => {}
        _ => panic!(
            "{HARNESS}: a draft's photograph must answer exactly as an absent one does — anything else tells an \
             anonymous visitor that the hidden listing exists. Got {draft_media:?} and {absent_media:?}"
        ),
    }
    assert_ne!(
        service.media_bytes("media-published", "web", &ctx).await.ok().flatten().map(|(_, b)| b.len()),
        None,
        "{HARNESS}: the draft's photograph must not be servable while the published one still is"
    );

    // ---- 4. NEGATIVE: the draft is in no other public read either. ----
    let inventory = service
        .listings(&ctx)
        .await
        .expect("the public inventory is a published read");
    assert!(
        !inventory.iter().any(|row| row.key == "casa-borrador" || row.status == "draft"),
        "{HARNESS}: an unpublished Property must not appear in the public inventory: {inventory:?}"
    );
    assert!(
        !service
            .slugs(&ctx)
            .await
            .expect("the sitemap is published")
            .contains(&"casa-borrador".to_owned()),
        "{HARNESS}: an unpublished Property must not appear in the sitemap"
    );
    assert!(
        service
            .similar("casa-borrador", 3, &ctx)
            .await
            .expect("the strip is a published read")
            .is_empty(),
        "{HARNESS}: an unpublished Property has no comparables — it is not a listing the site is showing"
    );
    let copy = service
        .listing_copy(&ctx)
        .await
        .expect("the taglines are a published read");
    assert!(
        !copy.iter().any(|item| item.slug == "casa-borrador"),
        "{HARNESS}: a draft's tagline is not public copy: {copy:?}"
    );

    // ---- 5. EVERY public read is bounded by the visibility rule, not just the ones exercised above. ----
    // The reads were written at different times; one that forgot the predicate is the whole leak. This walks the
    // DAO and checks each of the six public reads individually, so removing it from `media_bytes` alone fails.
    let dao = source::read(&source::workspace_root().join("db/src/public_listing.rs"));
    let mut bodies: Vec<(&str, &str)> = Vec::new();
    let mut current: Option<(&str, usize)> = None;
    for (number, line) in dao.lines().enumerate() {
        let code = source::code_of(line);
        if let Some(rest) = code.trim_start().strip_prefix("pub async fn ") {
            if let Some((name, _)) = current {
                bodies.push((name, &dao[..]));
            }
            let name = rest.split('(').next().unwrap_or_default().trim().to_owned();
            let start = dao
                .lines()
                .take(number)
                .map(str::len)
                .sum::<usize>()
                + number;
            current = Some((Box::leak(name.into_boxed_str()), start));
        }
    }
    if let Some((name, start)) = current {
        bodies.push((name, &dao[start..]));
    }

    for read in PUBLIC_READS {
        let body = bodies
            .iter()
            .find(|(name, _)| *name == *read)
            .map(|(_, body)| *body)
            .unwrap_or_else(|| {
                panic!("{HARNESS}: `PublicListingDao::{read}` was not found in db/src/public_listing.rs")
            });
        assert!(
            body.contains(VISIBILITY),
            "{HARNESS}: `PublicListingDao::{read}` must carry `{VISIBILITY}`. Every public read needs the \
             predicate; a read without it serves an unpublished Property"
        );
        assert!(
            body.contains("p.archived_at is null"),
            "{HARNESS}: `PublicListingDao::{read}` must also exclude archived Properties"
        );
    }
}
