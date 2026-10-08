//! CRM.PERSON — merge duplicates (TST-CRM-PERSON-002).
//!
//! Contract: **`merge_person(golden, duplicate)`** (migration `db/migrations/228_person_merge.sql`) folds two records
//! of one person into one, inside the caller's transaction — all of it or none:
//!
//! 1. every field the golden record lacks (null, or empty text) is filled from the duplicate, and the golden
//!    record's own values always win (`:33-44`);
//! 2. everything pointing at the duplicate moves to the golden record — every foreign key to `person`, found from the
//!    catalog so a table added later is included, plus the three references kept as text — and a row that would break
//!    a unique or check rule on the way is **dropped** instead (`:47-61`);
//! 3. a person linked to themself by the merge is unlinked, and the duplicate row is deleted (`:63-66`);
//!
//! and it answers `{"moved": n, "dropped": n}`.
//!
//! There is no Rust caller of this function yet: the Records screen's "Merge into this person" and the duplicate
//! cleanup call it as SQL (`db/loads/person_merge_duplicates.sql`). So the boundary under test *is* the function, and
//! this test reaches it through the harness seam that runs exactly that statement (`CrmHarness::merge_person`,
//! `tests/src/crm.rs`) on a disposable DEV target — asserting committed truth where the merge commits, and asserting
//! the **rollback** where the caller owns the transaction.
//!
//! The negative cases are the refusals the function owes its caller: a person cannot be merged into themself, and
//! both people must exist — and after each refusal the person that was there is still there.
//!
//! What this does not cover, stated so nobody reads more into a green run: the other callers' behaviour (the Records
//! screen's button and the cleanup load) is their own code, not this function's; only the function is proven here.
//!
//! Level: L2 Persistence — the production SQL function against an isolated, disposable DEV/Neon target.
//! `CrmHarness` refuses PRODUCTION before any socket is opened (`tests/src/database.rs:68-75`). Every fixture person
//! is named under a unique run marker and deleted at the end; a zero-leftover count is asserted, so DEV is left as it
//! was found. Raw SQL here is fixture setup, teardown and read-back only.
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test crm_person__002__merge_duplicates -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::DbTarget;
use model::{PersonIdentity, PersonIdentityKind};
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
    );
}

fn identity(kind: PersonIdentityKind, value: &str) -> PersonIdentity {
    PersonIdentity {
        kind,
        value: value.to_owned(),
        source_system: None,
        is_primary: false,
    }
}

/// Ten digits no other run holds, so a fixture phone is this run's alone.
///
/// The value is typed the way a person would type it — `+1 (787) 555-0199`-shaped — and normalised by the DAO under
/// test; the digits come from a fresh UUID so a run killed before its teardown can never collide with the next one.
fn unique_phone() -> String {
    let bytes = uuid::Uuid::new_v4().into_bytes();
    let digits: String = bytes
        .iter()
        .take(10)
        .map(|byte| char::from(b'0' + byte % 10))
        .collect();
    format!("+1 (787) {}-{}", &digits[0..3], &digits[3..7])
}

/// The note the duplicate carries and the golden record lacks — the field the merge fills from the duplicate.
async fn set_notes(harness: &CrmHarness, person_id: &str, notes: &str) {
    sqlx::query("update person set notes = $2 where id = $1::uuid")
        .bind(person_id)
        .bind(notes)
        .execute(harness.pool())
        .await
        .expect("proof: a fixture note must be writable on DEV");
}

/// Point one person at another, through the role the relation table requires — the pair a merge has to resolve.
async fn link_people(harness: &CrmHarness, person_id: &str, related_person_id: &str) {
    let inserted = sqlx::query(
        "insert into person_person (person_id, related_person_id, role_id)
         select $1::uuid, $2::uuid, id from role where scope = 'person_person' order by id limit 1",
    )
    .bind(person_id)
    .bind(related_person_id)
    .execute(harness.pool())
    .await
    .expect("proof: a fixture relation must be writable on DEV")
    .rows_affected();
    assert_eq!(
        inserted, 1,
        "{HARNESS}: the relation fixture must exist, or the self-link case would vanish silently"
    );
}

/// How many committed relations point one person at another.
async fn relation_count(harness: &CrmHarness, person_id: &str, related_person_id: &str) -> i64 {
    sqlx::query_scalar(
        "select count(*) from person_person
          where person_id = $1::uuid and related_person_id = $2::uuid",
    )
    .bind(person_id)
    .bind(related_person_id)
    .fetch_one(harness.pool())
    .await
    .expect("proof: the relation table must be readable on DEV")
}

/// What the merge answered, read from its own jsonb.
fn counts(answer: &str) -> (i64, i64) {
    let value: serde_json::Value = serde_json::from_str(answer)
        .expect("proof: merge_person answers its own {\"moved\", \"dropped\"} jsonb");
    (
        value["moved"]
            .as_i64()
            .expect("proof: `moved` must be a number"),
        value["dropped"]
            .as_i64()
            .expect("proof: `dropped` must be a number"),
    )
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
        "{HARNESS}: the merge proof runs only on an isolated DEV target"
    );
    let marker = format!("TST-CRM-PERSON-002-{}-", harness.namespace());

    // 1. Two records of one person. The golden record has a name and an email; the duplicate has the phone, a note the
    //    golden record lacks, and a relation pointing back at the golden record.
    let golden_name = format!("{marker}Golden Record");
    let duplicate_name = format!("{marker}Duplicate Record");
    let golden = harness
        .seed_person(&golden_name)
        .await
        .expect("proof: a person fixture row must be insertable on DEV");
    let duplicate = harness
        .seed_person(&duplicate_name)
        .await
        .expect("proof: a person fixture row must be insertable on DEV");
    let email = harness
        .attach(
            &golden,
            identity(
                PersonIdentityKind::Email,
                &format!("{marker}golden@tst-harness.invalid"),
            ),
        )
        .await
        .expect("proof: the golden record's identity must attach through the production DAO");
    let phone = harness
        .attach(
            &duplicate,
            identity(PersonIdentityKind::Phone, &unique_phone()),
        )
        .await
        .expect("proof: the duplicate's identity must attach through the production DAO");
    set_notes(&harness, &duplicate, "met at the open house").await;
    link_people(&harness, &duplicate, &golden).await;

    // 2. The committed merge: everything pointing at the duplicate moves, the golden record's own values win, what it
    //    lacked is filled, and the duplicate is gone.
    let answer = harness
        .merge_person(&golden, &duplicate)
        .await
        .expect("proof: merge_person must fold a duplicate into a golden record");
    let (moved, dropped) = counts(&answer);
    assert_eq!(
        moved, 1,
        "{HARNESS}: the duplicate's one identity must be moved to the golden record, not copied and not dropped"
    );
    assert_eq!(
        dropped, 1,
        "{HARNESS}: the relation pointing at the golden record would have pointed it at itself — that row is dropped"
    );
    assert!(
        !harness
            .person_exists(&duplicate)
            .await
            .expect("proof: the person table must be readable"),
        "{HARNESS}: the duplicate record must be gone after the merge"
    );
    assert!(
        harness
            .person_exists(&golden)
            .await
            .expect("proof: the person table must be readable"),
        "{HARNESS}: the golden record must survive the merge"
    );

    let (name, notes) = harness
        .person_name_and_notes(&golden)
        .await
        .expect("proof: the golden record must be readable")
        .expect("proof: the golden record must still exist");
    assert_eq!(
        name, golden_name,
        "{HARNESS}: the golden record's own display_name wins — a merge never renames the record it keeps"
    );
    assert_eq!(
        notes.as_deref(),
        Some("met at the open house"),
        "{HARNESS}: the field the golden record lacked is filled from the duplicate"
    );
    assert_eq!(
        harness
            .identity_values(&golden, "email")
            .await
            .expect("proof: the identity table must be readable"),
        vec![email.value.clone()],
        "{HARNESS}: the golden record keeps its own identity"
    );
    assert_eq!(
        harness
            .identity_values(&golden, "phone")
            .await
            .expect("proof: the identity table must be readable"),
        vec![phone.value.clone()],
        "{HARNESS}: and gains the duplicate's phone — one person, one record, both ways to reach them"
    );
    assert_eq!(
        relation_count(&harness, &golden, &golden).await,
        0,
        "{HARNESS}: nobody is their own relation after the merge"
    );

    // 3. The rollback: the merge runs inside the caller's transaction, so a caller that rolls back leaves nothing
    //    behind — which is what makes an abandoned merge safe.
    let golden_two = harness
        .seed_person(&format!("{marker}Golden Two"))
        .await
        .expect("proof: a person fixture row must be insertable on DEV");
    let duplicate_two = harness
        .seed_person(&format!("{marker}Duplicate Two"))
        .await
        .expect("proof: a person fixture row must be insertable on DEV");

    let mut transaction = harness
        .database()
        .begin()
        .await
        .expect("proof: the harness must be able to open a transaction on DEV");
    harness
        .merge_person_on(transaction.connection(), &golden_two, &duplicate_two)
        .await
        .expect("proof: the merge must run on the caller's connection");
    let gone_inside: bool =
        sqlx::query_scalar("select not exists (select 1 from person where id = $1::uuid)")
            .bind(&duplicate_two)
            .fetch_one(transaction.connection())
            .await
            .expect("proof: the caller's transaction must be readable");
    assert!(
        gone_inside,
        "{HARNESS}: inside the caller's transaction the duplicate is already gone — the merge happened"
    );
    transaction
        .rollback()
        .await
        .expect("proof: the harness transaction must roll back");
    assert!(
        harness
            .person_exists(&duplicate_two)
            .await
            .expect("proof: the person table must be readable"),
        "{HARNESS}: and after the rollback the duplicate is back — the merge is all of it or none of it"
    );
    assert!(
        harness
            .person_exists(&golden_two)
            .await
            .expect("proof: the person table must be readable"),
        "{HARNESS}: the golden record is unchanged by the rollback too"
    );

    // 4. The refusals the function owes its callers. Neither may write anything.
    let self_merge = harness
        .merge_person(&golden_two, &golden_two)
        .await
        .expect_err("proof: a person cannot be merged into themself");
    assert!(
        self_merge.to_string().contains("cannot be merged into themself"),
        "{HARNESS}: the self-merge must name what it refused, not fail silently with a database error: {self_merge}"
    );
    assert!(
        harness
            .person_exists(&golden_two)
            .await
            .expect("proof: the person table must be readable"),
        "{HARNESS}: a refused self-merge leaves the person exactly where it was"
    );

    let missing = harness
        .merge_person(&golden_two, &uuid::Uuid::new_v4().to_string())
        .await
        .expect_err("proof: both people must exist, so an unknown duplicate is refused");
    assert!(
        missing.to_string().contains("both people must exist"),
        "{HARNESS}: the missing-person refusal must name its own rule: {missing}"
    );
    assert!(
        harness
            .person_exists(&golden_two)
            .await
            .expect("proof: the person table must be readable"),
        "{HARNESS}: and the golden record survives the refused merge"
    );

    // 5. Teardown: the merged duplicate is already gone, so the three that remain are the two golden records and the
    //    duplicate the rollback restored — and none of them stays behind.
    let removed = harness
        .cleanup(&marker)
        .await
        .expect("proof: the fixture rows must be deletable");
    assert_eq!(
        removed, 3,
        "{HARNESS}: teardown must remove exactly the persons this run left behind"
    );
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("proof: the leftover count must be readable"),
        0,
        "{HARNESS}: DEV must be left as it was found — zero fixture rows remain"
    );
}
