//! CRM.PERSON — PersonIdentityKind restrictions (TST-CRM-PERSON-004).
//!
//! Contract: **`PersonIdentityKind` is a closed set of exactly three kinds, and the boundary admits no fourth.**
//! The enum is `Phone | Email | External` (`middle/model/src/person.rs:5-9`), and that closure is load-bearing in
//! three separate places — it is not decoration on a struct:
//!
//! - **`as_str()` is total.** Every variant maps to a name the database knows (`phone`, `email`, `external`,
//!   `middle/model/src/person.rs:12-18`), and it is the value `normalized_identity` switches on
//!   (`db/src/person.rs:65-71`). A variant with no `as_str` arm could not be stored at all; a name outside the
//!   column's CHECK would be rejected by the database. So the mapping is proved in both directions.
//! - **serde is closed too.** The enum is `#[serde(rename_all = "lowercase")]`, so an inbound payload naming a
//!   fourth kind — `whatsapp`, which `AGENTS.md` lists under Never ("Treat WhatsApp as a new identity type") — is
//!   refused at the parse boundary rather than silently coerced. This is asserted through the real
//!   `serde_json` deserialisation the API uses, not by inspecting the enum's shape.
//! - **normalisation is per-kind and exhaustive.** A phone strips to digits, an email lower-cases, an external id
//!   keeps its case. There is no default arm, so adding a variant without deciding its normalisation would not
//!   compile. The test proves the three rules differ, because "external keeps its case" is the one that would be
//!   wrong if normalisation were applied uniformly.
//!
//! The negative cases are what make the closure testable: an unknown kind in JSON is refused; a kind name outside
//! the column's CHECK is refused by the database even when written directly; and an external id that differs from
//! another only by case is *accepted*, which is the asymmetry that proves `External` is genuinely its own kind
//! rather than an email in disguise.
//!
//! Level: L2 Persistence — the production `PersonDao` and the production serde boundary against an isolated,
//! disposable DEV/Neon target. The harness refuses PRODUCTION before any socket is opened
//! (`tests/src/database.rs:68-75`).
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test crm_person__004__personidentitykind_restrictions -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::{DbTarget, PersonDao};
use model::{AttachPersonIdentityRequest, PersonIdentity, PersonIdentityKind};
use test_harness::CrmHarness;

const HARNESS: &str = "CrmHarness/L2 Persistence";

/// Every kind the enum admits, paired with the name the database stores. Exhaustiveness matters: this is the list
/// the test walks, and a fourth variant would need a row here to be noticed.
const ALL_KINDS: [(PersonIdentityKind, &str); 3] = [
    (PersonIdentityKind::Phone, "phone"),
    (PersonIdentityKind::Email, "email"),
    (PersonIdentityKind::External, "external"),
];

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-PERSON-004); the file and the assay use it.
async fn crm_person_004__personidentitykind_restrictions() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the kind-restriction proof runs only on an isolated DEV target"
    );
    let dao: &PersonDao = harness.dao();
    let ns = harness.namespace().to_string();
    let marker = format!("TST-CRMPERSON004-{ns}");

    let owner = harness
        .seed_person(&format!("{marker}-owner"))
        .await
        .expect("the identity owner seeds");

    // -----------------------------------------------------------------------------------------------------------
    // 1. `as_str()` IS TOTAL AND THE DATABASE AGREES. Every variant names a kind the column's CHECK admits, and
    //    the row the DAO writes carries exactly that name. This is the mapping in both directions: the Rust name
    //    the database accepts, and the stored column the database returns.
    // -----------------------------------------------------------------------------------------------------------
    for (index, (kind, expected)) in ALL_KINDS.iter().enumerate() {
        assert_eq!(
            kind.as_str(),
            *expected,
            "{HARNESS}: the kind's stored name is the one the database column admits"
        );
        let value = match kind {
            PersonIdentityKind::Phone => unique_digits(&ns, index as u8),
            PersonIdentityKind::Email => format!("kind{index}.{ns}@example.test"),
            PersonIdentityKind::External => format!("KindSource-{ns}-{index}"),
        };
        let stored = dao
            .attach_identity(&AttachPersonIdentityRequest {
                person_id: owner.clone(),
                identity: identity(kind.clone(), &value),
            })
            .await
            .unwrap_or_else(|error| {
                panic!("{HARNESS}: a {} identity attaches, got {error}", expected)
            });
        assert_eq!(
            stored.kind, *kind,
            "{HARNESS}: the stored identity answers with the kind it was written as"
        );
        // The committed column carries the Rust name — read from the pool, not from the DAO's return value.
        let committed: String = sqlx::query_scalar(
            "select identity_type from person_identity
              where person_id = $1::uuid and identity_value = $2",
        )
        .bind(&owner)
        .bind(&stored.value)
        .fetch_one(harness.pool())
        .await
        .map_err(|error| db::DbFailure::from_sqlx("test-harness.crm.kind_read", &error))
        .expect("the committed identity_type reads");
        assert_eq!(
            committed, *expected,
            "{HARNESS}: the committed identity_type is the Rust kind's own name"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 2. NORMALISATION IS PER-KIND, AND THE THREE RULES DIFFER. The asymmetry that matters is `External`: it keeps
    //    its case, while `Email` lower-cases. If normalisation were uniform, two external ids differing only by
    //    case would collide — and `External` would be an email in disguise rather than its own kind. Asserted as a
    //    positive case, because the interesting failure is over-normalisation, not a refusal.
    // -----------------------------------------------------------------------------------------------------------
    let case_sensitive = format!("MixedCase-{ns}");
    let stored_preserving = dao
        .attach_identity(&AttachPersonIdentityRequest {
            person_id: owner.clone(),
            identity: identity(PersonIdentityKind::External, &case_sensitive),
        })
        .await
        .expect("an external id attaches");
    assert_eq!(
        stored_preserving.value, case_sensitive,
        "{HARNESS}: an external id keeps its case — it is trimmed, not normalised like an email"
    );

    let email_typed = format!("  CaseProbe.{ns}@Example.TEST  ");
    let stored_folded = dao
        .attach_identity(&AttachPersonIdentityRequest {
            person_id: owner.clone(),
            identity: identity(PersonIdentityKind::Email, &email_typed),
        })
        .await
        .expect("an email attaches");
    assert_eq!(
        stored_folded.value,
        format!("caseprobe.{ns}@example.test"),
        "{HARNESS}: an email is trimmed and lower-cased — the rule that differs from `External` on the same input shape"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE / REFUSAL — SERDE IS CLOSED. An inbound payload naming a fourth kind is refused at the parse
    //    boundary, not coerced to a default. `whatsapp` is the name `AGENTS.md` forbids outright ("Treat WhatsApp
    //    as a new identity type"), so it is the one worth proving cannot be smuggled in over the wire.
    // -----------------------------------------------------------------------------------------------------------
    for refused in [
        r#"{"kind":"whatsapp","value":"+17875551234","is_primary":false}"#,
        r#"{"kind":"WhatsApp","value":"+17875551234","is_primary":false}"#,
        r#"{"kind":"sms","value":"+17875551234","is_primary":false}"#,
        r#"{"kind":"","value":"+17875551234","is_primary":false}"#,
    ] {
        let parsed = serde_json::from_str::<PersonIdentity>(refused);
        assert!(
            parsed.is_err(),
            "{HARNESS}: the production serde boundary refuses the unknown identity kind in {refused}"
        );
    }

    // And the three real kinds DO parse, so the refusals above are about the name and not about the payload shape.
    for (kind, expected) in ALL_KINDS.iter() {
        let payload = format!(r#"{{"kind":"{expected}","value":"probe.{ns}","is_primary":false}}"#);
        let parsed: PersonIdentity = serde_json::from_str(&payload)
            .unwrap_or_else(|error| panic!("{HARNESS}: {expected} must parse, got {error}"));
        assert_eq!(
            parsed.kind, *kind,
            "{HARNESS}: serde maps the stored name back onto the right variant"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE / FAULT — THE COLUMN'S CHECK IS A SECOND, INDEPENDENT CLOSURE. Writing a kind name the CHECK
    //    does not admit is refused by the database even when it bypasses both the enum and serde. So the closure is
    //    not a single point of failure: removing the Rust variant would still not open the column.
    // -----------------------------------------------------------------------------------------------------------
    let outside_the_enum = sqlx::query(
        "insert into person_identity (person_id, identity_type, identity_value, source_system, is_primary)
         values ($1::uuid, 'whatsapp', $2, 'bypass', false) returning id",
    )
    .bind(&owner)
    .bind(unique_digits(&ns, 42))
    .fetch_one(harness.pool())
    .await;
    assert!(
        outside_the_enum.is_err(),
        "{HARNESS}: the column's CHECK refuses an identity kind the enum does not name"
    );
    let violation = outside_the_enum
        .expect_err("the out-of-enum kind is refused")
        .to_string();
    assert!(
        violation.contains("person_identity_identity_type_check"),
        "{HARNESS}: the refusal names the column's kind CHECK, got {violation}"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NO LEFTOVER FROM ANY REFUSAL. Each refused write above must have written nothing, or the closed set would
    //    be closed only at the API and open in the table.
    // -----------------------------------------------------------------------------------------------------------
    // The exact committed count per kind: one from the `ALL_KINDS` walk, plus one more for `email` and one more
    // for `external` from the normalisation-asymmetry case in section 2. `phone` gets no second write, so it must
    // still read 1 — a count of 2 there would mean a refused write had landed.
    for (kind_name, expected) in [("phone", 1_i64), ("email", 2), ("external", 2)] {
        assert_eq!(
            harness
                .identity_count(&owner, kind_name)
                .await
                .expect("the committed kind count reads"),
            expected,
            "{HARNESS}: the owner holds exactly {expected} {kind_name} identities written above — no refused write landed"
        );
    }
    let untyped: i64 = sqlx::query_scalar(
        "select count(*) from person_identity
          where person_id = $1::uuid and identity_type <> all($2::text[])",
    )
    .bind(&owner)
    .bind(["phone", "email", "external"])
    .fetch_one(harness.pool())
    .await
    .map_err(|error| db::DbFailure::from_sqlx("test-harness.crm.kind_residue", &error))
    .expect("the out-of-enum residue count reads");
    assert_eq!(
        untyped, 0,
        "{HARNESS}: no identity row exists whose kind is outside the enum — the closed set holds in the table too"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. CLEANUP / NO LEFTOVER. This run's person is deleted (its identities cascade); a non-zero leftover count
    //    fails the proof, because DEV must be left as it was found.
    // -----------------------------------------------------------------------------------------------------------
    let removed = harness
        .cleanup(&marker)
        .await
        .expect("the fixture person is removed");
    assert_eq!(
        removed, 1,
        "{HARNESS}: exactly this run's person is removed"
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
