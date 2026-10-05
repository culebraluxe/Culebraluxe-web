//! CRM.PERSON — WhatsApp remains attribution on phone, not identity kind (TST-CRM-PERSON-005).
//!
//! Contract: **WhatsApp is where a message came from, not who someone is.** A WhatsApp correspondent is a person
//! whose identity is a `phone`; "WhatsApp" belongs on the row as *attribution*, never as a fourth identity kind.
//! The repository says this outright — `AGENTS.md` lists "Treat WhatsApp as a new identity type" under **Never**,
//! with `cli/src/forge/repo_guards.rs` as the guard — and this test is the executable form of that rule.
//!
//! The production code proves the intent rather than merely permitting it. WhatsApp resolution
//! (`db/src/whatsapp.rs:219-238`) looks a correspondent up with `where pi.identity_type = 'phone'` and the same
//! `semantic_phone` normalisation every other phone comparison uses — a WhatsApp e.164 number resolves a person
//! because it *is* their phone. If WhatsApp were a kind, that query would need a fifth branch and the number would
//! have to exist twice, once as a phone and once as a WhatsApp handle, and the two could drift apart.
//!
//! So the contract has two halves and both are asserted against committed truth:
//!
//! - **the kind stays `phone`.** Attaching with `source_system: Some("whatsapp")` writes `identity_type = 'phone'`
//!   and the number's digits in `identity_value` — the attribution column carries the channel, the kind column does
//!     not. An account's attribution names its owner: two different WhatsApp accounts on one number are ONE person.
//! - **resolution ignores the attribution.** `find_by_identity` with no `source_system` finds the person behind a
//!   number that was learnt from WhatsApp, and the lookup SQL's `identity_type = 'phone'` branch is what matches it.
//!
//! The negative cases are the ways the rule could be broken: the enum refuses a `whatsapp` kind at the serde
//! boundary, the column's CHECK refuses one written directly past the DAO, and — the case that matters most — a
//! second person cannot take a number that is already attributed to a WhatsApp correspondent, because the
//! attribution is not what makes it unique.
//!
//! Level: L2 Persistence — the production `PersonDao` against an isolated, disposable DEV/Neon target. The harness
//! refuses PRODUCTION before any socket is opened (`tests/src/database.rs:68-75`).
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test crm_person__005__whatsapp_remains_attribution_on_phone_not_identity_kind -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::{DbFailure, DbFailureKind, DbTarget, PersonDao};
use model::{AttachPersonIdentityRequest, PersonIdentity, PersonIdentityKind};
use test_harness::CrmHarness;

const HARNESS: &str = "CrmHarness/L2 Persistence";

/// The attribution WhatsApp rows carry. This is a `source_system` value, never an `identity_type`.
const WHATSAPP: &str = "whatsapp";

/// A second WhatsApp account belonging to the same person's number, to prove attribution is per-account and the
/// person behind it is still one person.
const WHATSAPP_ALT: &str = "whatsapp:alt-account";

/// Ten digits, unique to this run, so the rows can never collide with a concurrent run's on shared DEV.
fn unique_digits(namespace: &str, salt: u8) -> String {
    let mut accumulator: u64 = 1_469_598_103_934_665_603;
    for byte in namespace.bytes().chain(std::iter::once(salt)) {
        accumulator ^= u64::from(byte);
        accumulator = accumulator.wrapping_mul(1_099_511_628_211);
    }
    format!("{:010}", accumulator % 10_000_000_000)
}

/// An identity of the given kind, attributed to `source_system`.
fn attributed(kind: PersonIdentityKind, value: &str, source_system: &str) -> PersonIdentity {
    PersonIdentity {
        kind,
        value: value.to_owned(),
        source_system: Some(source_system.to_owned()),
        is_primary: false,
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); CrmHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-PERSON-005); the file and the assay use it.
async fn crm_person_005__whatsapp_remains_attribution_on_phone_not_identity_kind() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the WhatsApp-attribution proof runs only on an isolated DEV target"
    );
    let dao: &PersonDao = harness.dao();
    let ns = harness.namespace().to_string();
    let marker = format!("TST-CRMPERSON005-{ns}");

    let correspondent = harness
        .seed_person(&format!("{marker}-correspondent"))
        .await
        .expect("the WhatsApp correspondent seeds");
    let stranger = harness
        .seed_person(&format!("{marker}-stranger"))
        .await
        .expect("the second person seeds");

    // The correspondent's number, in the e.164 form WhatsApp actually sends.
    let national = unique_digits(&ns, 1);
    let e164 = format!("1{national}");

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE KIND STAYS `phone`; THE CHANNEL IS THE ATTRIBUTION. The DAO is asked for a Phone identity whose
    //    source is WhatsApp, and the committed row must say so in two different columns: `identity_type` = phone,
    //    `source_system` = whatsapp.
    // -----------------------------------------------------------------------------------------------------------
    let stored = dao
        .attach_identity(&AttachPersonIdentityRequest {
            person_id: correspondent.clone(),
            identity: attributed(PersonIdentityKind::Phone, &e164, WHATSAPP),
        })
        .await
        .expect("a WhatsApp-attributed phone attaches");
    assert_eq!(
        stored.kind,
        PersonIdentityKind::Phone,
        "{HARNESS}: a WhatsApp correspondent is a person whose identity kind is Phone"
    );
    assert_eq!(
        stored.value, national,
        "{HARNESS}: the stored value is the person's phone number, normalised — not a WhatsApp handle"
    );
    assert_eq!(
        stored.source_system.as_deref(),
        Some(WHATSAPP),
        "{HARNESS}: the channel travels as attribution on the source_system column"
    );

    // Committed truth, read from the pool rather than from the DAO's return value: the kind column is `phone` and
    // carries nothing about the channel.
    let committed: (String, Option<String>) = sqlx::query_as(
        "select identity_type, source_system from person_identity
          where person_id = $1::uuid and identity_value = $2",
    )
    .bind(&correspondent)
    .bind(&national)
    .fetch_one(harness.pool())
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm.whatsapp_read", &error))
    .expect("the committed identity row reads");
    assert_eq!(
        committed.0, "phone",
        "{HARNESS}: the committed identity_type is `phone` — WhatsApp never becomes an identity kind"
    );
    assert_eq!(
        committed.1.as_deref(),
        Some(WHATSAPP),
        "{HARNESS}: the committed attribution names WhatsApp on the source_system column"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE LOOKUP IS THE PRODUCTION ONE. `db/src/whatsapp.rs:219-238` resolves a correspondent with
    //    `identity_type = 'phone'` and `semantic_phone(e164)`. This is that exact question asked through the DAO:
    //    the number finds its owner with no attribution filter, which is what makes one person reachable from both
    //    a phone call and a WhatsApp message.
    // -----------------------------------------------------------------------------------------------------------
    let resolved = dao
        .find_by_identity(&PersonIdentity {
            kind: PersonIdentityKind::Phone,
            value: e164.clone(),
            source_system: None,
            is_primary: false,
        })
        .await
        .expect("the WhatsApp resolution lookup runs")
        .expect("a WhatsApp-attributed number still resolves to a person");
    assert_eq!(
        resolved.id, correspondent,
        "{HARNESS}: WhatsApp resolution finds the person because the identity IS their phone, not because of the channel"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. ATTRIBUTION DOES NOT MULTIPLY IDENTITIES. A second channel attributing the SAME number still yields one
    //    identity row. This is asserted in the form the production code actually takes, which is worth stating
    //    precisely:
    //
    //    `attach_identity`'s lookup is scoped by attribution — `and ($3::text is null or source_system = $3)`
    //    (`db/src/person.rs:350`) — so a *different* `source_system` does not match the row that already holds the
    //    number, and the DAO falls through to its insert. The insert is then refused by `person_identity_unique`,
    //    because the key is `(identity_type, identity_value)` and carries no attribution column. So the second
    //    attribution is stopped by the index, as a `Constraint` failure rather than the DAO's `SchemaMismatch`
    //    ownership refusal.
    //
    //    Either way the invariant the story is about holds: one phone identity per number, however many channels
    //    claim to have seen it. The test pins the observable outcome (one row, refusal, no write) and does not
    //    depend on which of the two guards produced it.
    // -----------------------------------------------------------------------------------------------------------
    let second_account = dao
        .attach_identity(&AttachPersonIdentityRequest {
            person_id: correspondent.clone(),
            identity: attributed(PersonIdentityKind::Phone, &e164, WHATSAPP_ALT),
        })
        .await;
    assert!(
        second_account.is_err(),
        "{HARNESS}: a second channel attributing the same number is refused — attribution does not create a second identity"
    );
    let refusal_text = second_account
        .expect_err("the second attribution is refused")
        .to_string();
    assert!(
        refusal_text.contains("person_identity_unique"),
        "{HARNESS}: the refusal names the unique constraint that makes the number one identity, got {refusal_text}"
    );
    assert_eq!(
        harness
            .identity_count(&correspondent, "phone")
            .await
            .expect("the committed phone count reads"),
        1,
        "{HARNESS}: two channels on one number are still ONE phone identity row — the attribution does not multiply identities"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE / REFUSAL — ATTRIBUTION IS NOT WHAT MAKES A NUMBER UNIQUE. A stranger cannot take a number that
    //    is already attributed to a WhatsApp correspondent. Uniqueness belongs to the normalised number within the
    //    `phone` kind, and `source_system` is not part of the key, so this is refused in the same way any other
    //    second owner would be.
    //
    //    Two refusals are asserted, because `attach_identity` scopes its ownership lookup by attribution and the
    //    two routes are stopped by different guards:
    //
    //    - **unattributed** (`source_system: None`): the lookup matches the correspondent's row regardless of its
    //      attribution, the DAO sees the number belongs to someone else, and refuses with its own `SchemaMismatch`.
    //    - **attributed to the same channel** (`whatsapp`): the lookup matches on both, so the same `SchemaMismatch`
    //      is raised. Naming the channel does not buy a second owner either.
    //
    //    The third route — a *different* channel — is the interesting one and is asserted in section 3: the
    //    attribution-scoped lookup misses the row entirely and the unique index refuses the insert. All three are
    //    refusals; the point is that no combination of attribution reaches a second owner.
    // -----------------------------------------------------------------------------------------------------------
    for (label, identity) in [
        (
            "unattributed",
            PersonIdentity {
                kind: PersonIdentityKind::Phone,
                value: format!(
                    "({}) {}-{}",
                    &national[0..3],
                    &national[3..6],
                    &national[6..10]
                ),
                source_system: None,
                is_primary: false,
            },
        ),
        (
            "attributed to the same channel",
            attributed(PersonIdentityKind::Phone, &e164, WHATSAPP),
        ),
    ] {
        let refusal = dao
            .attach_identity(&AttachPersonIdentityRequest {
                person_id: stranger.clone(),
                identity,
            })
            .await
            .expect_err("a second owner of a WhatsApp-attributed phone must be refused");
        assert!(
            matches!(refusal.kind, DbFailureKind::SchemaMismatch),
            "{HARNESS}: the {label} refusal is a schema mismatch, got {:?}",
            refusal.kind
        );
        assert!(
            refusal
                .to_string()
                .contains("already belongs to another Person"),
            "{HARNESS}: the {label} refusal names the conflict it found, got {refusal}"
        );
    }
    assert_eq!(
        harness
            .identity_count(&stranger, "phone")
            .await
            .expect("the stranger's phone count reads"),
        0,
        "{HARNESS}: every refused attach writes no row — no channel buys a second owner"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE / REFUSAL — `whatsapp` CANNOT BECOME A KIND. Two independent closures, both asserted through the
    //    production boundary: the serde boundary refuses the name in an inbound payload, and the column's CHECK
    //    refuses it written directly past the DAO.
    // -----------------------------------------------------------------------------------------------------------
    let parsed = serde_json::from_str::<PersonIdentity>(
        r#"{"kind":"whatsapp","value":"+17875551234","source_system":"whatsapp","is_primary":false}"#,
    );
    assert!(
        parsed.is_err(),
        "{HARNESS}: the production serde boundary refuses `whatsapp` as an identity kind"
    );

    let smuggled = sqlx::query(
        "insert into person_identity (person_id, identity_type, identity_value, source_system, is_primary)
         values ($1::uuid, 'whatsapp', $2, 'whatsapp', false) returning id",
    )
    .bind(&correspondent)
    .bind(unique_digits(&ns, 42))
    .fetch_one(harness.pool())
    .await;
    assert!(
        smuggled.is_err(),
        "{HARNESS}: the column's CHECK refuses a `whatsapp` identity_type written past the DAO"
    );
    let violation = smuggled
        .expect_err("the smuggled kind is refused")
        .to_string();
    assert!(
        violation.contains("person_identity_identity_type_check"),
        "{HARNESS}: the refusal names the column's kind CHECK, got {violation}"
    );

    // And nothing was left behind by either refusal: the correspondent holds exactly the one phone, and no row
    // anywhere carries an identity_type outside the enum.
    let outside_the_enum: i64 = sqlx::query_scalar(
        "select count(*) from person_identity
          where person_id = $1::uuid and identity_type <> all($2::text[])",
    )
    .bind(&correspondent)
    .bind(["phone", "email", "external"])
    .fetch_one(harness.pool())
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm.whatsapp_residue", &error))
    .expect("the out-of-enum residue count reads");
    assert_eq!(
        outside_the_enum, 0,
        "{HARNESS}: WhatsApp appears as attribution on a phone row and nowhere else"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. CLEANUP / NO LEFTOVER. This run's two persons are deleted (their identities cascade); a non-zero leftover
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
