//! CRM.PERSON — merge duplicates (TST-CRM-PERSON-002).
//!
//! Contract: **two persons that are duplicates of each other must resolve to one
//! canonical person — and must never be merged silently.** The production boundary
//! (`PersonDao`, `db/src/person.rs`) honours exactly one half of that contract today:
//!
//! - **No silent merge, ever.** `attach_identity` (`:321-369`) refuses a cross-person
//!   attach with a `SchemaMismatch` (`identity already belongs to another Person`), and
//!   `set_contact` (`:235-319`) returns the other owner's display name and changes
//!   nothing on conflict. An identity can never be ripped from one person and sewn
//!   onto another behind the caller's back — both refusals are asserted here, with the
//!   committed counts read back from the pool to prove nothing moved.
//! - **No explicit merge either.** There is no merge operation anywhere in the
//!   workspace: no `merge` method on `PersonDao`, no service, no route. Two duplicate
//!   rows therefore coexist forever — the requirement "merge duplicates" is not met by
//!   the production boundary, and the final assertion states the contract as it should
//!   hold and is deliberately left failing, so the record is executable. A weakened
//!   assertion that merely documented "duplicates persist" would let the missing
//!   capability survive unnoticed.
//!
//! The negative cases are what stop the passing half passing vacuously: the refused
//! attach writes no row for the second person, the refused `set_contact` leaves both
//! persons' identities exactly as they were, and the lookup still resolves the address
//! to its one true owner.
//!
//! Level: L2 Persistence — the production `PersonDao` against an isolated, disposable
//! DEV/Neon target. The harness refuses PRODUCTION before any socket is opened
//! (`tests/src/database.rs:68-75`).
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test crm_person__002__merge_duplicates -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs
//! a disposable DEV database and the harness will never open a PRODUCTION one.

use db::{DbFailure, DbFailureKind, DbTarget, PersonDao};
use model::{AttachPersonIdentityRequest, PersonIdentity, PersonIdentityKind};
use test_harness::CrmHarness;

const HARNESS: &str = "CrmHarness/L2 Persistence";

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
///
/// This is infrastructure, not the contract: `CrmHarness` still refuses PRODUCTION before any socket is opened.
async fn connect_dev() -> CrmHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match CrmHarness::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; CrmHarness refuses PROD: {}",
        last.unwrap_or_default()
    )
}

/// An identity as a person would type it.
fn identity(kind: PersonIdentityKind, value: &str) -> PersonIdentity {
    PersonIdentity {
        kind,
        value: value.to_owned(),
        source_system: None,
        is_primary: false,
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); CrmHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-PERSON-002); the file and the assay use it.
async fn crm_person_002__merge_duplicates() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the merge-duplicates proof runs only on an isolated DEV target"
    );
    let dao: &PersonDao = harness.dao();
    let ns = harness.namespace().to_string();
    let token = format!("dupe{}-{}", &ns.replace('-', ""), uuid::Uuid::new_v4());

    // The duplicate pair: two persons, same display name, where the second one is about
    // to claim the first one's address. The token makes the pair this run's alone on
    // shared DEV.
    let owner_name = format!("Ana Dupe-{token}");
    let owner = harness
        .seed_person(&owner_name)
        .await
        .expect("the owning person seeds");
    let dupe_name = format!("Ana Dupe-{token}");
    let dupe = harness
        .seed_person(&dupe_name)
        .await
        .expect("the duplicate person seeds");
    let address = format!("ana.dupe.{token}@example.test");
    harness
        .attach(&owner, identity(PersonIdentityKind::Email, &address))
        .await
        .expect("the owner's address attaches");

    // -----------------------------------------------------------------------------------------------------------
    // 1. NEGATIVE / REFUSAL — the duplicate cannot take the address by attach. The DAO
    //    refuses with a schema mismatch naming the conflict, and the refused attach
    //    writes no row: the duplicate still holds no email.
    // -----------------------------------------------------------------------------------------------------------
    let refusal = dao
        .attach_identity(&AttachPersonIdentityRequest {
            person_id: dupe.clone(),
            identity: identity(PersonIdentityKind::Email, &address),
        })
        .await
        .expect_err("a duplicate claiming another person's address must be refused");
    assert!(
        matches!(refusal.kind, DbFailureKind::SchemaMismatch),
        "{HARNESS}: the cross-person refusal is a schema mismatch, got {:?}",
        refusal.kind
    );
    assert!(
        refusal
            .to_string()
            .contains("already belongs to another Person"),
        "{HARNESS}: the refusal names the conflict it found, got {refusal}"
    );
    assert_eq!(
        harness
            .identity_count(&dupe, "email")
            .await
            .expect("the duplicate's email count reads"),
        0,
        "{HARNESS}: the refused attach writes no row for the duplicate"
    );
    assert_eq!(
        harness
            .identity_count(&owner, "email")
            .await
            .expect("the owner's email count reads"),
        1,
        "{HARNESS}: the owner's address stays exactly where it was"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. NEGATIVE / REFUSAL — the duplicate cannot take the address by set_contact
    //    either. The DAO returns the other owner's display name and changes nothing:
    //    the duplicate still holds no email and the owner still holds one.
    // -----------------------------------------------------------------------------------------------------------
    let conflict = dao
        .set_contact(&dupe, "email", &address)
        .await
        .expect("a conflicting set_contact answers rather than erroring");
    assert_eq!(
        conflict.as_deref(),
        Some(owner_name.as_str()),
        "{HARNESS}: the conflict names the true owner, got {conflict:?}"
    );
    assert_eq!(
        harness
            .identity_count(&dupe, "email")
            .await
            .expect("the duplicate's email count re-reads"),
        0,
        "{HARNESS}: the refused set_contact writes no row for the duplicate"
    );
    assert_eq!(
        harness
            .identity_count(&owner, "email")
            .await
            .expect("the owner's email count re-reads"),
        1,
        "{HARNESS}: the refused set_contact moves nothing from the owner"
    );

    // The address still resolves to its one true owner — the lookup never splits the
    // difference between the pair.
    let found = harness
        .find(&identity(PersonIdentityKind::Email, &address))
        .await
        .expect("the lookup answers rather than erroring")
        .expect("the address still resolves");
    assert_eq!(
        found.id, owner,
        "{HARNESS}: the contested address resolves to the owner, not the duplicate"
    );

    // How many persons of the duplicate pair exist: measured here against the committed
    // truth, asserted at the very end so a test which fails on this defect still leaves
    // DEV exactly as it found it. A failing assertion must not be the reason a
    // disposable database keeps its fixtures.
    let pair_remaining: i64 = sqlx::query_scalar(
        "select count(*) from person where display_name = $1",
    )
    .bind(&owner_name)
    .fetch_one(harness.pool())
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm_person002.pair", &error))
    .expect("the duplicate-pair count reads");

    // -----------------------------------------------------------------------------------------------------------
    // 3. CLEANUP — both persons of the pair are removed (identities cascade), asserted
    //    against this run's token so a concurrent run's fixtures are never touched.
    // -----------------------------------------------------------------------------------------------------------
    let removed = sqlx::query("delete from person where display_name = $1")
        .bind(&owner_name)
        .execute(harness.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("test-harness.crm_person002.cleanup", &error))
        .expect("this run's duplicate pair is removed")
        .rows_affected();
    assert_eq!(
        removed, 2,
        "{HARNESS}: exactly this run's duplicate pair is removed, and no other run's"
    );
    let pair_left: i64 = sqlx::query_scalar(
        "select count(*) from person where display_name = $1",
    )
    .bind(&owner_name)
    .fetch_one(harness.pool())
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm_person002.residue", &error))
    .expect("the pair leftover count reads");
    assert_eq!(
        pair_left, 0,
        "{HARNESS}: the proof leaves no person of the pair behind"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. THE DISCOVERY, asserted last so the fixtures above are already swept.
    //
    //    No production operation merges the duplicate pair: there is no `merge` method
    //    on `PersonDao`, no service, no route — a workspace-wide search for a
    //    person/client merge seam finds only property-parcel and workflow-evidence
    //    merges, neither of which can unify two persons. The pair therefore persists
    //    as TWO persons where the contract demands ONE canonical one.
    //
    //    This is a real defect in production code, not a wrong assertion: the fix is a
    //    duplicate-resolution operation on the person boundary, and it is out of scope
    //    for this test to build it. The assertion states the contract as it should hold
    //    and is deliberately left failing, so the record is executable — a weakened
    //    assertion that merely documented "duplicates persist" would let the missing
    //    capability survive.
    // -----------------------------------------------------------------------------------------------------------
    assert_eq!(
        pair_remaining, 1,
        "{HARNESS}: a duplicate pair merges into ONE canonical person. The production boundary \
         refused the silent merge (sections 1-2) but offers no explicit one, so the pair persisted \
         as {pair_remaining} persons. Production defect, recorded as evidence."
    );
}
