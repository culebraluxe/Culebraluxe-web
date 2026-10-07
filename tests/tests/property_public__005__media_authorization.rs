//! PROPERTY.PUBLIC — media authorization (TST-PROPERTY-PUBLIC-005).
//!
//! Contract: **a photograph of a published Property is servable to an anonymous visitor, and the answer to "may
//! this visitor have these bytes" is `None` — never an error, and never a distinction.** The gate is
//! `PublicListingDao::media_bytes` (`db/src/public_listing.rs:610-651`) and it has two properties that are easy to
//! get wrong and expensive to leak.
//!
//! **The gate is evaluated on the ORIGINAL, not on the copy.** Every photograph gets derivatives — a `web` copy and
//! a `thumb` — and a copy has no `property_media` link of its own. So the query walks
//! `join media root on root.id = coalesce(m.derivative_of, m.id)` and asks whether the ROOT is linked to an
//! unpublished Property. Asking about the copy would answer "no links, therefore fine" for every derivative of
//! every unpublished Property, and the public site would serve all of them (:602-604). This is the single most
//! important line in the batch.
//!
//! **"No such media" and "not published" are the same answer.** Both return `None` (:608-609), because an
//! anonymous visitor must not be able to tell them apart — a distinguishable "exists but is hidden" is an oracle
//! for which listings exist. Media with no Property link at all stays reachable: it is not listing media (:605).
//!
//! The size variant is a presentation choice, not a permission: `card` and `thumb` serve that derivative when the
//! photograph has one, anything else serves the `web` copy and then the original (:610-615). Asking for an unknown
//! size must widen to the full image, never fail.
//!
//! Level: L3 Composition — the real `PublicListingService` with the real `CasbinAuthorizationPort`, the database
//! faked at the `PublicListingRepository` adapter boundary. The gate SQL is pinned structurally below.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_public__005__media_authorization

use std::sync::{Arc, Mutex};

use db::{DbFailure, DbResult};
use model::{PublicListing, PublicListingCopy, PublicProperty};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, ServiceActor, ServiceActorKind, ServiceContext,
    ServiceInfrastructure,
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

/// Serves bytes only for ids the fake was told are servable, records every `(id, size)` it was asked for, and can
/// be told to fail. A refusal and a miss are both `Ok(None)` — exactly as production returns them — so the test can
/// prove the two are indistinguishable to the caller.
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
        self.asked.lock().expect("the fake repository is never poisoned").clone()
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

    async fn media_bytes(&self, id: &str, size: &str) -> DbResult<Option<(String, Vec<u8>)>> {
        self.asked
            .lock()
            .expect("the fake repository is never poisoned")
            .push((id.to_owned(), size.to_owned()));
        if self.fail {
            return Err(DbFailure::configuration(
                "test.property_public.005",
                "the media read failed",
            ));
        }
        Ok(self
            .serves(id)
            .then(|| ("image/jpeg".to_owned(), vec![0xFF, 0xD8, 0xFF, 0xE0])))
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
}

fn unreachable_media<T>(method: &'static str) -> DbResult<T> {
    Err(DbFailure::configuration(
        "test.property_public.005",
        format!("{method} is outside the media-authorization subject"),
    ))
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-PUBLIC-005); the file and the assay use it.
async fn property_public_005__media_authorization() {
    // `media-1` is a published Property's photograph. `media-hidden` stands for a photograph of an unpublished
    // Property; `media-absent` for an id that does not exist. The fake refuses both of the latter the same way.
    let repository = FakePublicListingRepository::default().serving("media-1");
    let service = PublicListingService::new(repository.clone(), infrastructure().await);
    let ctx = public_website();

    // ---- 1. A published Property's photograph is servable to an anonymous visitor. ----
    let bytes = service
        .media_bytes("media-1", "web", &ctx)
        .await
        .expect("a published photograph is a published read");
    let (mime, payload) = bytes.expect("media-1 is a published Property's photograph");
    assert_eq!(
        mime, "image/jpeg",
        "{HARNESS}: the servable bytes carry the media's own MIME, which is what the response Content-Type is"
    );
    assert!(
        !payload.is_empty(),
        "{HARNESS}: the servable bytes are the bytes, not a placeholder"
    );

    // ---- 2. NEGATIVE: a hidden photograph and an absent id are the SAME answer. ----
    // An anonymous visitor must not be able to tell "this listing exists but is not published" from "there is no
    // such media". A distinguishable answer is an oracle for which listings exist.
    let hidden = service.media_bytes("media-hidden", "web", &ctx).await;
    let absent = service.media_bytes("media-absent", "web", &ctx).await;
    match (&hidden, &absent) {
        (Ok(None), Ok(None)) => {}
        (Ok(None), Ok(Some(_))) | (Ok(Some(_)), Ok(None)) => panic!(
            "{HARNESS}: a hidden photograph and an absent id must be indistinguishable — one answering \
             `None` and the other bytes tells an anonymous visitor that the hidden listing exists"
        ),
        (Ok(Some(_)), Ok(Some(_))) => panic!(
            "{HARNESS}: neither a hidden photograph nor an absent id may be servable, but both returned bytes"
        ),
        (Err(error), _) | (_, Err(error)) => panic!(
            "{HARNESS}: a refusal is `None`, not an error; an error would distinguish 'hidden' from 'absent' \
             and from a fault alike. Got {error:?}"
        ),
    }

    // ---- 3. The size variant is a presentation choice and is forwarded verbatim. ----
    // `card` and `thumb` serve that derivative; anything else serves the `web` copy and then the original. An
    // unknown size must WIDEN to the full image, never fail — a card asking for an unknown size must still get a
    // picture.
    for size in ["card", "thumb", "web", "original", "", "full"] {
        let before = repository.asked().len();
        let answer = service.media_bytes("media-1", size, &ctx).await;
        assert_eq!(
            &repository.asked()[before],
            &("media-1".to_owned(), size.to_owned()),
            "{HARNESS}: the service must forward the requested size verbatim — the DAO, not the service, owns \
             which derivative that selects"
        );
        assert!(
            answer
                .expect("a published photograph is a published read")
                .expect("a published photograph is served at every size")
                .1
                .len()
                > 0,
            "{HARNESS}: size {size:?} must still yield the photograph; an unknown variant widens rather than \
             failing"
        );
    }

    // ---- 4. NEGATIVE: a failed media read is an error, never bytes and never a silent refusal. ----
    // Swallowing it would serve a broken image; answering `None` would make an outage look like a hidden
    // listing. It has to be visible.
    let failing = PublicListingService::new(
        FakePublicListingRepository::failing(),
        infrastructure().await,
    );
    match failing.media_bytes("media-1", "web", &ctx).await {
        Ok(Some((_, leaked))) => panic!(
            "{HARNESS}: a failed read must not serve bytes; it returned {leaked:?}"
        ),
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
        "{HARNESS}: the visibility subquery must join on `root.id`, not on `m.id` — this is what makes the \
         root-based gate above actually bite"
    );
    // And the root must be judged against the same visibility rule as the inventory.
    assert!(
        code.contains("p.archived_at is null") && code.contains("p.status in ('active', 'under_contract', 'sold')"),
        "{HARNESS}: the media gate must carry the same visibility rule as every other public read"
    );
    // The derivative choice: the requested variant, then `web`, then the original.
    assert!(
        code.contains("let size = if matches!(size, \"card\" | \"thumb\")"),
        "{HARNESS}: `card` and `thumb` are the recognized variants"
    );
    assert!(
        code.contains("coalesce(copy.file_data, m.file_data)") && code.contains("coalesce(copy.mime_type, m.mime_type)"),
        "{HARNESS}: an absent derivative must fall back to the original's bytes and MIME, so an unknown or \
         missing variant still serves the photograph"
    );
    // And the query returns ONE row, never an error, so the two refusals stay indistinguishable.
    assert!(
        code.contains("limit 1") && code.contains("fetch_optional"),
        "{HARNESS}: the media read returns at most one row and no error — 'absent' and 'not published' must \
         remain one answer"
    );
}
