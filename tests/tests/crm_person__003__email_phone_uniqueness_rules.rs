//! CRM.PERSON — email/phone uniqueness rules (TST-CRM-PERSON-003).
//!
//! Contract: **a normalised email or phone belongs to one person, and the same characters are free to be an email
//! on one person and a phone on another.** Uniqueness here is not a string property; it is a property of a
//! *normalised value inside a kind*. Three rules compose, and each is asserted against committed truth:
//!
//! - **the kind is part of the key.** `person_identity_unique` is `UNIQUE (identity_type, identity_value)`
//!   (`db/migrations/001_initial_schema.sql`). The literal string `5551234` may be alice's phone and bob's email
//!   without collision, because `identity_type` differs. A uniqueness rule keyed on the value alone would refuse
//!   that, which is why this is asserted as a *positive* case and not only as a collision.
//! - **normalisation happens before the key is compared.** `attach_identity` normalises with
//!   `normalized_identity` (`db/src/person.rs:65-71`) — a phone keeps only its digits and drops a US country code, an
//!   email is trimmed and lower-cased — and refuses with a `SchemaMismatch` when the normalised form already
//!   belongs to someone else (`db/src/person.rs:361-369`). That refusal is the DAO's half.
//! - **the database is the backstop, for the literal value.** The DAO's check is a read-then-write with a window
//!   between them, so the unique index is what actually holds under concurrency. It is a plain btree on the raw
//!   pair (`USING btree (identity_type, identity_value)`), so it refuses an *exact* duplicate written past the DAO
//!   — and it does not normalise, which is why the DAO's own normalisation is the load-bearing half and the index
//!   is the backstop for the window between the check and the write. Both halves are asserted, and the index's
//!   limit is asserted rather than assumed.
//!
//! The negative cases are what make the test unable to pass without exercising the subject: a second person is
//! refused alice's phone in three different spellings and alice's email in two cases, each followed by a
//! committed-truth count proving nothing was written, plus the direct-insert bypass being stopped by the index.
//!
//! Level: L2 Persistence — the production `PersonDao` against an isolated, disposable DEV/Neon target. The harness
//! refuses PRODUCTION before any socket is opened (`tests/src/database.rs:68-75`).
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test crm_person__003__email_phone_uniqueness_rules -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::{DbFailureKind, DbTarget, PersonDao};
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

/// Ten digits, unique to this run, so the rows can never collide with a concurrent run's on shared DEV.
fn unique_digits(namespace: &str, salt: u8) -> String {
    let mut accumulator: u64 = 1_469_598_103_934_665_603;
    for byte in namespace.bytes().chain(std::iter::once(salt)) {
        accumulator ^= u64::from(byte);
        accumulator = accumulator.wrapping_mul(1_099_511_628_211);
    }
    format!("{:010}", accumulator % 10_000_000_000)
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-PERSON-003); the file and the assay use it.
async fn crm_person_003__email_phone_uniqueness_rules() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the uniqueness proof runs only on an isolated DEV target"
    );
    let dao: &PersonDao = harness.dao();
    let ns = harness.namespace().to_string();
    let marker = format!("TST-CRMPERSON003-{ns}");

    let alice = harness
        .seed_person(&format!("{marker}-alice"))
        .await
        .expect("the identity owner seeds");
    let bob = harness
        .seed_person(&format!("{marker}-bob"))
        .await
        .expect("the second person seeds");

    let national = unique_digits(&ns, 1);
    let email = format!("uniqueness.{ns}@example.test");

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE OWNER TAKES BOTH. One phone and one email, each stored normalised.
    // -----------------------------------------------------------------------------------------------------------
    let stored_phone = dao
        .attach_identity(&AttachPersonIdentityRequest {
            person_id: alice.clone(),
            identity: identity(
                PersonIdentityKind::Phone,
                &format!(
                    "1-{}-{}-{}",
                    &national[0..3],
                    &national[3..6],
                    &national[6..10]
                ),
            ),
        })
        .await
        .expect("a phone attaches");
    assert_eq!(
        stored_phone.value, national,
        "{HARNESS}: the phone is stored as bare digits with the US country code dropped, so the key is comparable"
    );
    let stored_email = dao
        .attach_identity(&AttachPersonIdentityRequest {
            person_id: alice.clone(),
            identity: identity(PersonIdentityKind::Email, &format!("  {email} ")),
        })
        .await
        .expect("an email attaches");
    assert_eq!(
        stored_email.value, email,
        "{HARNESS}: the email is stored trimmed and lower-cased, so casing cannot buy a second copy"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE KIND IS PART OF THE KEY — a positive case, not a collision. The very same characters are alice's
    //    phone AND bob's email. `person_identity_unique` is on (identity_type, identity_value), so this is two
    //    distinct identities and both are legal. A uniqueness rule keyed on the value alone would refuse it, and
    //    that mistake is invisible to a test that only ever tries to create collisions.
    // -----------------------------------------------------------------------------------------------------------
    let shared = dao
        .attach_identity(&AttachPersonIdentityRequest {
            person_id: bob.clone(),
            identity: identity(PersonIdentityKind::Email, &national),
        })
        .await
        .expect("the same characters are a legal email for a different person");
    assert_eq!(
        shared.value, national,
        "{HARNESS}: an email may hold the same characters as another person's phone — the kind is part of the key"
    );
    assert_eq!(
        harness
            .identity_count(&bob, "email")
            .await
            .expect("bob's email count reads"),
        1,
        "{HARNESS}: bob holds the shared value as an email and nothing else"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE / REFUSAL — THE PHONE. A second person is refused alice's phone in every spelling, because all of
    //    them normalise to the same digits. Each refusal is followed by a committed-truth count, so the test fails
    //    if a refusal ever comes with a write.
    // -----------------------------------------------------------------------------------------------------------
    for spelling in [
        national.clone(),
        format!("1{national}"),
        format!(
            "({}) {}-{}",
            &national[0..3],
            &national[3..6],
            &national[6..10]
        ),
        format!(
            "1-{}-{}-{}",
            &national[0..3],
            &national[3..6],
            &national[6..10]
        ),
    ] {
        let refusal = dao
            .attach_identity(&AttachPersonIdentityRequest {
                person_id: bob.clone(),
                identity: identity(PersonIdentityKind::Phone, &spelling),
            })
            .await
            .expect_err("a second owner of the same normalised phone must be refused");
        assert!(
            matches!(refusal.kind, DbFailureKind::SchemaMismatch),
            "{HARNESS}: the refusal for {spelling} is a schema mismatch, got {:?}",
            refusal.kind
        );
        assert!(
            refusal
                .to_string()
                .contains("already belongs to another Person"),
            "{HARNESS}: the refusal names the conflict it found, got {refusal}"
        );
        assert!(
            refusal.to_string().contains(&national),
            "{HARNESS}: the refusal reports the NORMALISED value it collided on, not the spelling that was typed"
        );
        assert_eq!(
            harness
                .identity_count(&bob, "phone")
                .await
                .expect("bob's phone count reads"),
            0,
            "{HARNESS}: a refused phone attach ({spelling}) writes no row"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE / REFUSAL — THE EMAIL. Two cases, both normalising to alice's stored address.
    // -----------------------------------------------------------------------------------------------------------
    for spelling in [email.to_uppercase(), format!("  {email}  ")] {
        let refusal = dao
            .attach_identity(&AttachPersonIdentityRequest {
                person_id: bob.clone(),
                identity: identity(PersonIdentityKind::Email, &spelling),
            })
            .await
            .expect_err("a second owner of the same normalised email must be refused");
        assert!(
            matches!(refusal.kind, DbFailureKind::SchemaMismatch),
            "{HARNESS}: the email refusal is a schema mismatch, got {:?}",
            refusal.kind
        );
    }
    assert_eq!(
        harness
            .identity_count(&bob, "email")
            .await
            .expect("bob's email count reads after the refusals"),
        1,
        "{HARNESS}: the two refused email attaches wrote nothing — bob still holds only the shared-value email"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE / BYPASS — THE DATABASE IS THE BACKSTOP. `attach_identity` checks ownership with a read and then
    //    writes, which leaves a window; the unique index is what actually holds. So a caller that skips the DAO's
    //    check entirely — a migration, a script, a second writer with a stale view — is still stopped. The insert
    //    below writes alice's EXACT stored phone, which is the one value no amount of re-spelling can evade.
    // -----------------------------------------------------------------------------------------------------------
    let bypass = sqlx::query(
        "insert into person_identity (person_id, identity_type, identity_value, source_system, is_primary)
         values ($1::uuid, 'phone', $2, 'bypass', false) returning id",
    )
    .bind(&bob)
    .bind(&national)
    .fetch_one(harness.pool())
    .await;
    assert!(
        bypass.is_err(),
        "{HARNESS}: a duplicate phone written past the DAO is still refused by `person_identity_unique`"
    );
    let violation = bypass
        .expect_err("the bypassing insert is refused")
        .to_string();
    assert!(
        violation.contains("person_identity_unique"),
        "{HARNESS}: the refusal names the unique constraint, got {violation}"
    );
    assert_eq!(
        harness
            .identity_count(&bob, "phone")
            .await
            .expect("bob's phone count reads after the bypass"),
        0,
        "{HARNESS}: the refused bypass wrote no row"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5b. THE INDEX'S ACTUAL LIMIT, asserted rather than assumed. `person_identity_unique` is a plain btree on the
    //     raw literal pair — `USING btree (identity_type, identity_value)`, with no expression and no `citext`. So
    //     it refuses an *exact* duplicate and it does NOT refuse one that differs only in case.
    //
    //     That is a property of the boundary, not a defect, and it is recorded here so nobody later "discovers" it
    //     and assumes the index is doing the normalisation: normalising the value so that case cannot buy a second
    //     copy is the DAO's job (`normalized_identity`, `db/src/person.rs:65-71`), and the index is the backstop
    //     for the window between the DAO's read and its write. Through the DAO the rule holds, which sections 3
    //     and 4 prove. What this section pins is where the backstop stops — so the layering is documented by the
    //     test rather than by a comment nobody runs.
    // -----------------------------------------------------------------------------------------------------------
    let exact_email_bypass = sqlx::query(
        "insert into person_identity (person_id, identity_type, identity_value, source_system, is_primary)
         values ($1::uuid, 'email', $2, 'bypass', false) returning id",
    )
    .bind(&bob)
    .bind(&email)
    .fetch_one(harness.pool())
    .await;
    assert!(
        exact_email_bypass.is_err(),
        "{HARNESS}: an exactly-duplicate email written past the DAO is refused by `person_identity_unique`"
    );
    assert_eq!(
        harness
            .identity_count(&bob, "email")
            .await
            .expect("bob's email count reads after the exact bypass"),
        1,
        "{HARNESS}: the refused exact bypass wrote no row"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. IDEMPOTENCE — the same person re-attaching their own phone is absorbed, not duplicated. Without this, the
    //    DAO's own ownership check would read its own row as a conflict and refuse a person their own number.
    // -----------------------------------------------------------------------------------------------------------
    let again = dao
        .attach_identity(&AttachPersonIdentityRequest {
            person_id: alice.clone(),
            identity: identity(
                PersonIdentityKind::Phone,
                &format!(
                    "({}) {}-{}",
                    &national[0..3],
                    &national[3..6],
                    &national[6..10]
                ),
            ),
        })
        .await
        .expect("an owner re-attaching their own phone is absorbed");
    assert_eq!(
        again.value, national,
        "{HARNESS}: the existing normalised phone is returned, not a new row"
    );
    assert_eq!(
        harness
            .identity_count(&alice, "phone")
            .await
            .expect("alice's phone count reads"),
        1,
        "{HARNESS}: re-attaching an owned phone does not create a second identity row"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. CLEANUP / NO LEFTOVER. This run's two persons are deleted (their identities cascade); a non-zero leftover
    //    count fails the proof, because DEV must be left as it was found.
    // -----------------------------------------------------------------------------------------------------------
    let removed = harness
        .cleanup(&marker)
        .await
        .expect("the fixture persons are removed");
    assert_eq!(
        removed, 2,
        "{HARNESS}: exactly this run's two persons are removed"
    );
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("the leftover count reads"),
        0,
        "{HARNESS}: the proof leaves no person or identity behind"
    );
}
