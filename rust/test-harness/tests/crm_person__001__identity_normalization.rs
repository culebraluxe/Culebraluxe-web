//! CRM.PERSON — identity normalization (TST-CRM-PERSON-001).
//!
//! Contract: a person's email, phone and external identities are compared and stored **normalised**, by exactly
//! three rules and no others (`normalized_identity`, `db/src/person.rs:65-71`):
//!
//! - a **phone** keeps only its ASCII digits, and drops a leading US country code (`1`) when the result is eleven
//!   digits (`semantic_phone`, `db/src/person.rs:53-63`), so `+1 (787) 555-1234`, `787-555-1234` and
//!   `17875551234` are one identity;
//! - an **email** is trimmed and lower-cased (`:68`);
//! - an **external** id is trimmed, its case preserved (`:69`).
//!
//! The same normalisation is applied to the *stored* row and to the *lookup* value — `find_by_identity`
//! (`db/src/person.rs:104-152`) normalises the argument with `normalized_identity` and its SQL
//! (`:122-131`) normalises the stored `identity_value` the same way, so a query typed one way finds a row written
//! another. `attach_identity` (`:321-404`) and `set_contact` (`:235-319`) refuse an identity whose *normalised*
//! form already belongs to another person: normalisation is what makes that uniqueness enforceable, because two
//! different spellings of one number are one row under `person_identity_unique`
//! (`db/migrations/001_initial_schema.sql:113-114`).
//!
//! The contract is proven through the production boundary, not a copy of it: the real `PersonDao` is asked to
//! `attach_identity`, `find_by_identity` and `set_contact`, and every result is read back from the pool the DAO
//! wrote to. Raw SQL here is fixture setup and teardown only.
//!
//! The negative case is the one that matters: if normalisation were not applied on the lookup, a person could be
//! handed an identity that a differently-spelled duplicate already owns — a second owner for one address — and this
//! test fails on the `SchemaMismatch` it expects. Two refusals are asserted (`attach_identity` and `set_contact`),
//! each followed by a committed-truth count showing that nothing was written.
//!
//! Level: L2 Persistence — the production `PersonDao` against an isolated, disposable DEV/Neon target. The harness
//! refuses PRODUCTION before any socket is opened (`rust/test-harness/src/database.rs:68-75`). Fixture persons are
//! named under a unique run marker and deleted at the end; a zero-leftover count is asserted, so DEV is left as it
//! was found, and a rolled-back probe proves the DAO reads committed truth.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test crm_person__001__identity_normalization -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::{DbFailure, DbFailureKind, DbTarget, PersonDao};
use model::{AttachPersonIdentityRequest, PersonIdentity, PersonIdentityKind};
use sqlx::PgConnection;
use test_harness::CrmHarness;

/// The harness name and level, carried in every assertion message so a failure names its boundary.
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
    );
}

/// A phone or email value typed as one person might type it.
fn identity(kind: PersonIdentityKind, value: &str) -> PersonIdentity {
    PersonIdentity {
        kind,
        value: value.to_owned(),
        source_system: None,
        is_primary: false,
    }
}

/// Ten digits, unique to this run, so the contract's rows can never collide with a concurrent run's on shared DEV.
///
/// A phone identity is stored as its bare digits, so the run marker lives in the digits themselves. The FNV-1a
/// fold over the namespace is deterministic and namespaced; the width is ten digits, which is not eleven and so is
/// left untouched by the country-code rule until a test deliberately prefixes it with `1`.
fn unique_digits(namespace: &str, salt: u8) -> String {
    let mut accumulator: u64 = 1_469_598_103_934_665_603;
    for byte in namespace.bytes().chain(std::iter::once(salt)) {
        accumulator ^= u64::from(byte);
        accumulator = accumulator.wrapping_mul(1_099_511_628_211);
    }
    format!("{:010}", accumulator % 10_000_000_000)
}

/// Insert one person and one identity inside `conn`'s transaction, so the caller can see them and then roll back.
async fn probe_insert_identity(
    conn: &mut PgConnection,
    display_name: &str,
    kind: &str,
    value: &str,
) -> Result<(), DbFailure> {
    let person_id: String = sqlx::query_scalar(
        "insert into person (display_name, role, status) values ($1, 'buyer', 'new') returning id::text",
    )
    .bind(display_name)
    .fetch_one(&mut *conn)
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm.probe_person", &error))?;
    sqlx::query(
        "insert into person_identity (person_id, identity_type, identity_value, source_system, is_primary)
         values ($1::uuid, $2, $3, 'probe', false)",
    )
    .bind(&person_id)
    .bind(kind)
    .bind(value)
    .execute(&mut *conn)
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm.probe_identity", &error))?;
    Ok(())
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); CrmHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-PERSON-001); the file and the assay use it.
async fn crm_person_001__identity_normalization() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the identity proof runs only on an isolated DEV target"
    );
    let dao: &PersonDao = harness.dao();
    let ns = harness.namespace().to_string();
    let marker = format!("TST-CRMPERSON001-{ns}");

    // Two people: the owner of the identities, and a second person who must not be handed a spelling of the same one.
    let alice = harness
        .seed_person(&format!("{marker}-alice"))
        .await
        .expect("the first person seeds");
    let bob = harness
        .seed_person(&format!("{marker}-bob"))
        .await
        .expect("the second person seeds");

    // The run's unique phone, in the three spellings the contract must fold into one identity. Ten bare digits,
    // an eleven-digit US form (leading `1`), and a formatted form.
    let national = unique_digits(&ns, 1);
    let e164 = format!("1{national}");
    let formatted = format!(
        "+1 ({}) {}-{}",
        &national[0..3],
        &national[3..6],
        &national[6..10]
    );
    let dashed = format!(
        "{}-{}-{}",
        &national[0..3],
        &national[3..6],
        &national[6..10]
    );
    // The run's unique email, typed with padding and mixed case; and an external id, padded, with case to preserve.
    let email_normalized = format!("dana.{ns}@example.test");
    let email_typed = format!("  Dana.{ns}@Example.TEST  ");
    let external_normalized = format!("HubSpot-{ns}-Alpha");
    let external_typed = format!("  {external_normalized}  ");

    // -----------------------------------------------------------------------------------------------------------
    // 1. PHONE — attach stores the digits with the country code removed, and nothing else.
    // -----------------------------------------------------------------------------------------------------------
    let stored = dao
        .attach_identity(&AttachPersonIdentityRequest {
            person_id: alice.clone(),
            identity: identity(PersonIdentityKind::Phone, &formatted),
        })
        .await
        .expect("a phone attaches");
    assert_eq!(
        stored.value, national,
        "{HARNESS}: an attached phone is stored as bare digits with the US country code removed"
    );
    assert_eq!(
        harness
            .identity_values(&alice, "phone")
            .await
            .expect("the committed phone reads back"),
        vec![national.clone()],
        "{HARNESS}: committed truth is the normalised phone, not the spelling that was typed"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. LOOKUP IS NORMALISATION-INSENSITIVE TO SPELLING. Every way of typing this one number finds alice.
    // -----------------------------------------------------------------------------------------------------------
    for spelling in [&national, &e164, &formatted, &dashed] {
        let found = dao
            .find_by_identity(&identity(PersonIdentityKind::Phone, spelling))
            .await
            .expect("the phone lookup runs")
            .unwrap_or_else(|| {
                panic!("{HARNESS}: {spelling} must resolve to the person who owns it")
            });
        assert_eq!(
            found.id, alice,
            "{HARNESS}: {spelling} is the same identity as {national}"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 3. EMAIL — trimmed, lower-cased on store; lookup matches any case and padding.
    // -----------------------------------------------------------------------------------------------------------
    let stored_email = dao
        .attach_identity(&AttachPersonIdentityRequest {
            person_id: alice.clone(),
            identity: identity(PersonIdentityKind::Email, &email_typed),
        })
        .await
        .expect("an email attaches");
    assert_eq!(
        stored_email.value, email_normalized,
        "{HARNESS}: an attached email is stored trimmed and lower-cased"
    );
    assert_eq!(
        harness
            .identity_values(&alice, "email")
            .await
            .expect("the committed email reads back"),
        vec![email_normalized.clone()],
        "{HARNESS}: committed truth is the normalised email"
    );
    for spelling in [
        email_normalized.to_uppercase(),
        format!("  {email_normalized}  "),
        email_normalized.clone(),
    ] {
        let found = dao
            .find_by_identity(&identity(PersonIdentityKind::Email, &spelling))
            .await
            .expect("the email lookup runs")
            .unwrap_or_else(|| panic!("{HARNESS}: {spelling} must resolve to its owner"));
        assert_eq!(
            found.id, alice,
            "{HARNESS}: {spelling} normalises to the stored email"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 4. EXTERNAL — trimmed, case PRESERVED. This is the rule that distinguishes the third kind: normalising it
    //    like an email would make two distinct external ids collide, and it does not.
    // -----------------------------------------------------------------------------------------------------------
    let stored_external = dao
        .attach_identity(&AttachPersonIdentityRequest {
            person_id: alice.clone(),
            identity: identity(PersonIdentityKind::External, &external_typed),
        })
        .await
        .expect("an external id attaches");
    assert_eq!(
        stored_external.value, external_normalized,
        "{HARNESS}: an attached external id is trimmed but keeps its case"
    );
    let found = dao
        .find_by_identity(&identity(
            PersonIdentityKind::External,
            &external_normalized,
        ))
        .await
        .expect("the external lookup runs")
        .expect("the external id resolves");
    assert_eq!(found.id, alice, "{HARNESS}: the external id resolves");

    // -----------------------------------------------------------------------------------------------------------
    // 5. IDEMPOTENCE — attaching a differently-spelled copy of an identity this person already owns returns the
    //    existing row and writes no second one. Normalisation is why the copy is recognised as a copy.
    // -----------------------------------------------------------------------------------------------------------
    let again = dao
        .attach_identity(&AttachPersonIdentityRequest {
            person_id: alice.clone(),
            identity: identity(PersonIdentityKind::Phone, &dashed),
        })
        .await
        .expect("a second spelling of an owned phone is absorbed");
    assert_eq!(
        again.value, national,
        "{HARNESS}: the existing normalised phone is returned"
    );
    assert_eq!(
        harness
            .identity_count(&alice, "phone")
            .await
            .expect("the phone count reads"),
        1,
        "{HARNESS}: a differently-spelled duplicate does not create a second identity row"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. NEGATIVE / REFUSAL — a second person cannot take the same NORMALISED identity, however it is spelled.
    //    This is the assertion the contract exists for: the phone below normalises to alice's, so the attach must
    //    fail rather than silently create a second owner. The email below is the same case on the email rule.
    // -----------------------------------------------------------------------------------------------------------
    let phone_refusal = dao
        .attach_identity(&AttachPersonIdentityRequest {
            person_id: bob.clone(),
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
        .expect_err("a second owner of the same normalised phone must be refused");
    assert!(
        matches!(phone_refusal.kind, DbFailureKind::SchemaMismatch),
        "{HARNESS}: the phone refusal is a schema mismatch, got {:?}",
        phone_refusal.kind
    );
    assert!(
        phone_refusal
            .to_string()
            .contains("already belongs to another Person"),
        "{HARNESS}: the phone refusal names the conflict, got {phone_refusal}"
    );
    assert!(
        phone_refusal.to_string().contains(&national),
        "{HARNESS}: the refusal reports the NORMALISED value it collided on"
    );

    let email_refusal = dao
        .attach_identity(&AttachPersonIdentityRequest {
            person_id: bob.clone(),
            identity: identity(PersonIdentityKind::Email, &email_normalized.to_uppercase()),
        })
        .await
        .expect_err("a second owner of the same normalised email must be refused");
    assert!(
        matches!(email_refusal.kind, DbFailureKind::SchemaMismatch),
        "{HARNESS}: the email refusal is a schema mismatch, got {:?}",
        email_refusal.kind
    );

    // Committed truth: the refusals wrote NOTHING for bob, on either kind.
    assert_eq!(
        harness
            .identity_count(&bob, "phone")
            .await
            .expect("bob's phone count reads"),
        0,
        "{HARNESS}: a refused phone attach writes no row"
    );
    assert_eq!(
        harness
            .identity_count(&bob, "email")
            .await
            .expect("bob's email count reads"),
        0,
        "{HARNESS}: a refused email attach writes no row"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 7. THE SECOND PRODUCTION SEAM — `set_contact` normalises the value it compares, so it too refuses to hand
    //    bob an address another person owns, and reports the owner it found instead of overwriting.
    // -----------------------------------------------------------------------------------------------------------
    let owner = dao
        .set_contact(&bob, "email", &email_normalized.to_uppercase())
        .await
        .expect("set_contact answers rather than erroring")
        .expect("the address is owned by someone else, so set_contact reports the owner");
    assert!(
        owner.contains("alice"),
        "{HARNESS}: set_contact reports the existing owner's name, got {owner}"
    );
    assert_eq!(
        harness
            .identity_count(&bob, "email")
            .await
            .expect("bob's email count reads after set_contact"),
        0,
        "{HARNESS}: a refused set_contact writes no identity for the other person"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 8. COMMITTED TRUTH / ROLLBACK — the DAO reads committed rows only. A person and identity inserted inside a
    //    transaction the harness can only roll back are visible inside it and gone from the pool afterwards.
    // -----------------------------------------------------------------------------------------------------------
    let probe_display = format!("{marker}-probe");
    let probe_value = unique_digits(&ns, 7);
    let probe_display_for_body = probe_display.clone();
    let visible_inside = harness
        .database()
        .with_rollback(move |conn| {
            Box::pin(async move {
                probe_insert_identity(conn, &probe_display_for_body, "phone", &probe_value).await?;
                let count: i64 =
                    sqlx::query_scalar("select count(*) from person where display_name = $1")
                        .bind(&probe_display_for_body)
                        .fetch_one(&mut *conn)
                        .await
                        .map_err(|error| {
                            DbFailure::from_sqlx("test-harness.crm.probe_read", &error)
                        })?;
                Ok(count)
            })
        })
        .await
        .expect("the rolled-back probe runs");
    assert_eq!(
        visible_inside, 1,
        "{HARNESS}: the probe row is visible inside its own transaction"
    );
    let committed_probe: i64 =
        sqlx::query_scalar("select count(*) from person where display_name = $1")
            .bind(&probe_display)
            .fetch_one(harness.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("test-harness.crm.probe_leftover", &error))
            .expect("the post-rollback count reads");
    assert_eq!(
        committed_probe, 0,
        "{HARNESS}: a rolled-back transaction commits nothing — the DAO reads committed truth"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 9. CLEANUP / NO LEFTOVER. This run's two persons are deleted (their identities cascade); a non-zero leftover
    //    count is a failed rollback and fails the proof, because DEV must be left as it was found.
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
