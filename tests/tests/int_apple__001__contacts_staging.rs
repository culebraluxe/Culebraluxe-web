//! INT.APPLE — contacts staging (TST-INT-APPLE-001).
//!
//! Contract: an Apple Contacts export is **staged**, never promoted. One call to the production
//! `LandingDao::load_apple_contacts` (`db/src/landing.rs:310-321`) hands the export to the database function
//! `apple_contacts_load` (`db/migrations/254_apple_contacts_load_project.sql:291`), and that boundary must leave
//! exactly these facts behind — no more:
//!
//! - one **batch receipt** (`integration_intake_batch`, `db/migrations/072_integration_staged_contact_profile.sql:34`)
//!   whose counters balance: `input = valid + error` and `valid = new + replay + changed`;
//! - one **durable inbox receipt per contact identity** (`integration_inbox`, written at
//!   `db/migrations/254_apple_contacts_load_project.sql:411`), keyed on `(source, source_account,
//!   external_event_id)` so a replay reads the SAME receipt back instead of minting a second one;
//! - one **immutable staged revision per changed profile** (`integration_staged_contact_profile`,
//!   `db/migrations/072_integration_staged_contact_profile.sql:76`, written at
//!   `db/migrations/254_apple_contacts_load_project.sql:521`): revision 1 for a first sighting, revision 2 linked
//!   by `supersedes_profile_id` when the profile bytes change — an earlier revision is never overwritten;
//! - **snapshot membership for every contact of THIS export, replays included**
//!   (`db/migrations/254_apple_contacts_load_project.sql:551`), which is what makes "who is in the current
//!   snapshot" a fact of the export rather than a guess read back off staged rows.
//!
//! Staging stops at that boundary. `l_person`/`l_property` (the current-state projection, `apple_contacts_project`)
//! and the canonical `person`/`person_identity` spine (the promotion, `db/migrations/253_apple_contacts_promote.sql`)
//! must stay untouched: dirty Apple data may only reach the warehouse through those later hops, so a load that
//! wrote them would be a one-step import of unreviewed data. That negative is asserted twice — at the first load and
//! again at the end.
//!
//! The refusals are the other half of the contract. Each one is a case where staging must NOT happen, and each is
//! followed by a read-back proving nothing was written (the function is one transaction: "a failure leaves NO batch,
//! NO inbox receipt and NO staged revision behind"):
//!
//! - the same `exportId` with different bytes is a **conflict**, not a replay — `exitCode 1`, and the batch row is
//!   marked `conflict` rather than quietly overwritten (`db/migrations/254_apple_contacts_load_project.sql:393-409`);
//! - two contacts sharing one Apple `sourceId` are refused before a batch exists
//!   (`db/migrations/254_apple_contacts_load_project.sql:363`);
//! - an export declaring another `sourceSystem` is refused (`db/migrations/254_apple_contacts_load_project.sql:336`);
//! - an empty `source_account` is refused while the source holds more than one account
//!   (`db/migrations/254_apple_contacts_load_project.sql:324`) — identity is never guessed.
//!
//! BOUNDARY: the production DAO is the subject; the fake is the fixture export itself. Raw SQL below is read-back
//! verification of what that one production call wrote (the harness reserves raw SQL for tests whose subject *is*
//! the database contract — `tests/src/lib.rs:4-6`), and no business rule is re-implemented here. The fixture is the
//! artifact the exporter ships, byte-shape for byte-shape (`contact-export/Sources/contact-export/contact_export.swift:4-45`:
//! `sourceLabel`, `givenName`, `postalAddresses`, `schemaVersion: 1`), because the staging function reads the EXPORT's
//! field names — a fixture in a different shape would prove a contract nobody runs.
//!
//! LEVEL: the packet declares **L1 Component / ExternalFixtureHarness** — the component under test is the staging
//! boundary and the external fixture is the export artifact. The staging rules themselves are owned by a database
//! function, so the component is executed against the DEV target the harness resolves and refuses PRODUCTION before
//! any socket is opened (`tests/src/database.rs:68-75`). Every row this test creates carries its own run tag and is
//! deleted at the end, and a zero-leftover assertion is part of the proof, so DEV is left exactly as it was found.
//! Deterministic: fixed fixture, no clock, no network, no live provider, no PRODUCTION connection.
//!
//! Greenfield Rust: this is not a port of any TypeScript test. The retired `scripts/load-apple-contacts.ts` is
//! reference only (`docs/agent/BROKEN-TS-INVENTORY.md:91`).
//!
//! Run with:
//!   set -a; . ./.env.local; set +a   # DATABASE_URL_DEV must reach a disposable DEV branch
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test int_apple__001__contacts_staging -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the staging boundary is a database
//! function and the harness will never open a PRODUCTION one.

use db::{Database, DbTarget, LandingDao};
use serde_json::Value;
use test_harness::TestDatabase;

/// The harness and level this proof reports itself under, carried in every failure message.
const HARNESS: &str = "ExternalFixtureHarness/L1 Component";

/// The one source this contract stages.
const SOURCE: &str = "apple_contacts";

/// The export's own timestamp. It is weeks before any run clock, so a receipt that carries it proves staging
/// records WHEN THE SNAPSHOT WAS TAKEN rather than when this test ran.
const EXPORTED_AT: &str = "2026-09-22T19:33:12.308Z";

/// The fixture contacts' identities. They are `.test` addresses, so nothing else on DEV can own them and the
/// "never promoted" assertion is scoped rather than a count of somebody else's rows.
const DANA_EMAIL: &str = "dana.staging@example.test";
const MARCUS_EMAIL: &str = "marcus.staging@example.test";

/// The artifact digests the caller declares for each fixture export. `file_sha256` is whatever the caller says it
/// is — the CLI computes it from the export file's bytes (`cli/src/apple_contacts.rs:281-286`) and the database
/// only compares it for batch identity (same id + same bytes = replay, different bytes = conflict), so three
/// distinct digests are what make both halves of that rule exerciseable.
const SHA_FIRST: &str = "6f1a2b3c4d5e6f708192a3b4c5d6e7f86f1a2b3c4d5e6f708192a3b4c5d6e7f8";
const SHA_SECOND: &str = "9a8b7c6d5e4f302112233445566778899a8b7c6d5e4f30211223344556677889";
const SHA_THIRD: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0";
const SHA_FOURTH: &str = "aa11bb22cc33dd44ee55ff66aa77bb88aa11bb22cc33dd44ee55ff66aa77bb88";
const SHA_FIFTH: &str = "5566778899aabbccddeeff00112233445566778899aabbccddeeff0011223344";
const SHA_SIXTH: &str = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
const SHA_SEVENTH: &str = "fedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321";

/// The three contacts of the fixture export.
///
/// `second_phone` is the one field the caller can vary, because the whole revision rule turns on a single byte of
/// the profile changing. The third contact carries neither an email nor a phone: Apple ships those, and staging must
/// keep them (the inbox receipt falls back to the source id) rather than dropping a person nobody can address yet.
fn contacts(second_phone: &str) -> Value {
    serde_json::json!([
        {
            "sourceId": "TST:ABPerson-001",
            "namePrefix": "", "givenName": "Dana", "middleName": "", "familyName": "Rhodes",
            "nameSuffix": "", "nickname": "",
            "organization": "", "department": "", "jobTitle": "", "note": "",
            "emails": [{"sourceLabel": "home", "value": DANA_EMAIL}],
            "phones": [{"sourceLabel": "home", "value": "787-555-0101"}],
            "postalAddresses": [{
                "sourceLabel": "_$!<Home>!$_", "street": "1 Test Lane", "city": "San Juan",
                "state": "PR", "postalCode": "00901", "country": "USA", "isoCountryCode": "us"
            }]
        },
        {
            "sourceId": "TST:ABPerson-002",
            "namePrefix": "", "givenName": "Marcus", "middleName": "", "familyName": "Vega",
            "nameSuffix": "", "nickname": "",
            "organization": "", "department": "", "jobTitle": "", "note": "",
            "emails": [{"sourceLabel": "work", "value": MARCUS_EMAIL}],
            "phones": [{"sourceLabel": "work", "value": second_phone}],
            "postalAddresses": []
        },
        {
            "sourceId": "TST:ABPerson-003",
            "namePrefix": "", "givenName": "", "middleName": "", "familyName": "",
            "nameSuffix": "", "nickname": "",
            "organization": "CulebraLuxe Test Fixture", "department": "", "jobTitle": "", "note": "",
            "emails": [], "phones": [], "postalAddresses": []
        }
    ])
}

/// The one contact staged under the second source account — enough to make that account real, so an unnamed account
/// later has two to choose from and must refuse instead of guessing.
fn account_b_contacts() -> Value {
    serde_json::json!([
        {
            "sourceId": "TST:ABPerson-101",
            "namePrefix": "", "givenName": "Iris", "middleName": "", "familyName": "Test",
            "nameSuffix": "", "nickname": "",
            "organization": "", "department": "", "jobTitle": "", "note": "",
            "emails": [{"sourceLabel": "home", "value": "iris.staging@example.test"}],
            "phones": [], "postalAddresses": []
        }
    ])
}

/// The payload `apple_contacts_load` receives: the export's own envelope facts plus its contacts, untouched.
/// `fileSha256` is the digest the CLI adds from the file's bytes; everything else is as the exporter wrote it.
fn export(export_id: &str, file_sha256: &str, contacts: Value) -> Value {
    serde_json::json!({
        "schemaVersion": 1,
        "sourceSystem": SOURCE,
        "exportId": export_id,
        "exportedAt": EXPORTED_AT,
        "fileSha256": file_sha256,
        "contacts": contacts,
    })
}

// ---------------------------------------------------------------------------------------------
// Read-back helpers. They bind every parameter (never interpolate) and name the boundary they
// failed on, because they exist to observe what the ONE production call above wrote.
// ---------------------------------------------------------------------------------------------

/// A count read back from the database. `params` are bound in order to `$1..$n`; `sql` is a literal, because a
/// runtime-built statement would be the hand-escaped SQL this suite refuses to write.
async fn count(db: &Database, sql: &'static str, params: &[&str]) -> Result<i64, String> {
    let mut query = sqlx::query_scalar::<_, i64>(sql);
    for param in params {
        query = query.bind(*param);
    }
    query
        .fetch_one(db.pool())
        .await
        .map_err(|error| format!("{HARNESS}: read-back `{sql}` failed: {error}"))
}

/// A single text value read back from the database, `None` when no row answers.
async fn text_value(
    db: &Database,
    sql: &'static str,
    params: &[&str],
) -> Result<Option<String>, String> {
    let mut query = sqlx::query_scalar::<_, String>(sql);
    for param in params {
        query = query.bind(*param);
    }
    query
        .fetch_optional(db.pool())
        .await
        .map_err(|error| format!("{HARNESS}: read-back `{sql}` failed: {error}"))
}

/// The staged revisions this run owns.
async fn staged(db: &Database, accounts: &[&str]) -> Result<i64, String> {
    count(
        db,
        "select count(*) from integration_staged_contact_profile where source_account in ($1,$2)",
        accounts,
    )
    .await
}

/// The durable inbox receipts this run owns.
async fn receipts(db: &Database, accounts: &[&str]) -> Result<i64, String> {
    count(
        db,
        "select count(*) from integration_inbox where source_account in ($1,$2)",
        accounts,
    )
    .await
}

/// The snapshot memberships this run owns.
async fn members(db: &Database, accounts: &[&str]) -> Result<i64, String> {
    count(
        db,
        "select count(*) from integration_source_snapshot_member where source_account in ($1,$2)",
        accounts,
    )
    .await
}

/// One batch receipt: `(input, valid, new, replay, changed, error, load_status)`, or `None` when there is no receipt.
type BatchCounters = (i32, i32, i32, i32, i32, i32, String);

async fn batch_counters(db: &Database, export_id: &str) -> Result<Option<BatchCounters>, String> {
    sqlx::query_as::<_, BatchCounters>(
        "select input_count, valid_count, new_profile_count, replay_count,
                changed_revision_count, error_count, load_status
           from integration_intake_batch
          where source = 'apple_contacts' and external_batch_id = $1",
    )
    .bind(export_id)
    .fetch_optional(db.pool())
    .await
    .map_err(|error| format!("{HARNESS}: the batch receipt for {export_id} does not read: {error}"))
}

/// Run one statement whose result the caller does not need (teardown).
async fn run(db: &Database, sql: &'static str, params: &[&str]) -> Result<(), String> {
    let mut query = sqlx::query(sql);
    for param in params {
        query = query.bind(*param);
    }
    query
        .execute(db.pool())
        .await
        .map_err(|error| format!("{HARNESS}: `{sql}` failed: {error}"))?;
    Ok(())
}

/// A field of the tally `apple_contacts_load` returns, reported when the boundary stops returning it.
fn want<'a>(tally: &'a Value, key: &str) -> Result<&'a Value, String> {
    tally
        .get(key)
        .ok_or_else(|| format!("{HARNESS}: the tally carries `{key}` (got {tally})"))
}

/// A `totals.*` counter of the tally.
fn total(tally: &Value, key: &str) -> Result<i64, String> {
    want(want(tally, "totals")?, key)?
        .as_i64()
        .ok_or_else(|| format!("{HARNESS}: `totals.{key}` is an integer (got {tally})"))
}

/// A mismatch is a `Result`, not a panic: the caller still has to run CLEANUP before it fails the proof, and a
/// panicking assertion inside the proof would strand fixture rows on DEV.
macro_rules! check_eq {
    ($actual:expr, $expected:expr, $what:expr) => {
        if $actual != $expected {
            return Err(format!(
                "{HARNESS}: {} — expected {:?}, got {:?}",
                $what, $expected, $actual
            ));
        }
    };
}

macro_rules! check {
    ($condition:expr, $what:expr) => {
        if !$condition {
            return Err(format!("{HARNESS}: {}", $what));
        }
    };
}

/// The staging proof: every assertion returns `Err` instead of panicking, so teardown still runs after a failure.
async fn proof(db: &Database, tag: &str, accounts: &[&str]) -> Result<(), String> {
    let account_a = accounts[0];
    let account_b = accounts[1];
    let e1 = format!("{tag}-e1");
    let e2 = format!("{tag}-e2");
    let e3 = format!("{tag}-e3");
    let e4 = format!("{tag}-e4");
    let e5 = format!("{tag}-e5");
    let e6 = format!("{tag}-e6");
    let dao = LandingDao::new(db.clone());

    // ---------------------------------------------------------------------------------------------
    // 1. THE STAGING HAPPENS — one call, and the four facts it owes: batch receipt, one receipt per
    //    contact identity, one staged revision each, snapshot membership. Nothing is promoted.
    // ---------------------------------------------------------------------------------------------
    let first = dao
        .load_apple_contacts(&export(&e1, SHA_FIRST, contacts("787-555-0102")), account_a)
        .await
        .map_err(|error| {
            format!("{HARNESS}: a well-formed contacts export must stage, got {error}")
        })?;
    check_eq!(
        want(&first, "status")?.as_str(),
        Some("loaded"),
        "the load reports `loaded`"
    );
    check_eq!(
        want(&first, "exitCode")?.as_i64(),
        Some(0),
        "a load that staged exits 0"
    );
    check_eq!(
        want(&first, "batchCreated")?.as_bool(),
        Some(true),
        "the first sighting creates the batch receipt"
    );
    check_eq!(
        total(&first, "input")?,
        3,
        "all three contacts of the export are input"
    );
    check_eq!(total(&first, "valid")?, 3, "all three are valid");
    check_eq!(
        total(&first, "new")?,
        3,
        "three first sightings are three new revisions"
    );
    check_eq!(
        total(&first, "replay")?,
        0,
        "nothing is a replay on a first sighting"
    );
    check_eq!(
        total(&first, "changed")?,
        0,
        "nothing is a changed revision on a first sighting"
    );
    check_eq!(total(&first, "error")?, 0, "no contact is dropped");
    check_eq!(
        want(&first, "balanced")?.as_bool(),
        Some(true),
        "input balances to new + replay + changed"
    );
    let quality = want(&first, "dataQuality")?;
    check_eq!(
        want(quality, "withEmail")?.as_i64(),
        Some(2),
        "two of the three contacts carry an email"
    );
    check_eq!(
        want(quality, "withPhone")?.as_i64(),
        Some(2),
        "two of the three carry a phone"
    );
    check_eq!(
        want(quality, "withNeitherEmailNorPhone")?.as_i64(),
        Some(1),
        "the contact with no identity is counted, not dropped"
    );
    check_eq!(
        want(quality, "organizationOrSourceIdDisplayFallback")?.as_i64(),
        Some(1),
        "the display-name fallback is reported honestly"
    );

    check_eq!(
        batch_counters(db, &e1).await?,
        Some((3, 3, 3, 0, 0, 0, "loaded".to_owned())),
        "the batch receipt persists the same balanced counters the tally reported"
    );
    check_eq!(
        count(
            db,
            "select count(*) from integration_inbox where source_account in ($1,$2) and event_type = 'contact.imported'",
            accounts
        )
        .await?,
        3,
        "one durable receipt per contact identity"
    );
    check_eq!(
        count(
            db,
            "select count(*) from integration_inbox where source_account = $1 and subject = $2",
            &[account_a, "Dana Rhodes"]
        )
        .await?,
        1,
        "the receipt carries the display name the export implies"
    );
    let batch_one = text_value(
        db,
        "select id::text from integration_intake_batch where external_batch_id = $1",
        &[&e1],
    )
    .await?
    .ok_or_else(|| format!("{HARNESS}: the batch receipt for the first export exists"))?;
    check_eq!(
        text_value(
            db,
            "select content_reference from integration_inbox where source_account = $1 and external_event_id = $2",
            &[account_a, "TST:ABPerson-001"]
        )
        .await?,
        Some(format!("{batch_one}#contact=TST%3AABPerson-001")),
        "the receipt points at THIS batch and encodes the Apple source id's `:` as %3A"
    );
    check_eq!(
        count(
            db,
            "select count(*) from integration_inbox where source_account = $1
              and occurred_at = observed_at and occurred_at < now() - interval '1 day'",
            &[account_a]
        )
        .await?,
        3,
        "the receipt clock is the export's own timestamp, not the run clock"
    );
    check_eq!(
        staged(db, accounts).await?,
        3,
        "one staged revision per contact"
    );
    check_eq!(
        count(
            db,
            "select count(*) from integration_staged_contact_profile
              where source_account in ($1,$2) and revision = 1 and supersedes_profile_id is null",
            accounts
        )
        .await?,
        3,
        "the first sighting is revision 1 of an unbroken history"
    );
    check_eq!(
        count(
            db,
            "select count(*) from integration_staged_contact_profile
              where source_account in ($1,$2) and reconciliation_status = 'unreviewed' and candidate_person_id is null",
            accounts
        )
        .await?,
        3,
        "staged rows wait for review: a load reconciles nothing and names no candidate person"
    );
    check_eq!(
        members(db, accounts).await?,
        3,
        "every contact of this export is a member of its snapshot"
    );
    check_eq!(
        count(
            db,
            "select count(*) from l_person where source_account in ($1,$2)",
            accounts
        )
        .await?,
        0,
        "staging does not project: l_person belongs to apple_contacts_project"
    );
    check_eq!(
        count(
            db,
            "select count(*) from person_identity where identity_value in ($1,$2)",
            &[DANA_EMAIL, MARCUS_EMAIL]
        )
        .await?,
        0,
        "staging never reaches the canonical person spine"
    );

    // ---------------------------------------------------------------------------------------------
    // 2. THE EXACT REPLAY — same export id, same bytes: the same batch, no second revision, no second
    //    receipt, no second membership row. `payload_fingerprint` is the identity of a staged revision.
    // ---------------------------------------------------------------------------------------------
    let replay = dao
        .load_apple_contacts(&export(&e1, SHA_FIRST, contacts("787-555-0102")), account_a)
        .await
        .map_err(|error| format!("{HARNESS}: an identical re-load must run, got {error}"))?;
    check_eq!(
        want(&replay, "batchCreated")?.as_bool(),
        Some(false),
        "the same batch id is the same batch"
    );
    check_eq!(
        total(&replay, "new")?,
        0,
        "an exact replay creates nothing new"
    );
    check_eq!(
        total(&replay, "replay")?,
        3,
        "all three contacts are recognised as replays"
    );
    check_eq!(
        total(&replay, "changed")?,
        0,
        "nothing changed, so no new revision"
    );
    check_eq!(
        want(&replay, "balanced")?.as_bool(),
        Some(true),
        "the replay balances too"
    );
    check_eq!(
        staged(db, accounts).await?,
        3,
        "an exact replay writes no second revision"
    );
    check_eq!(
        receipts(db, accounts).await?,
        3,
        "a replay reads the same receipts back"
    );
    check_eq!(
        members(db, accounts).await?,
        3,
        "membership of the same batch is not duplicated"
    );
    check_eq!(
        batch_counters(db, &e1).await?,
        Some((3, 3, 0, 3, 0, 0, "loaded".to_owned())),
        "the batch now records three replays and zero new"
    );

    // ---------------------------------------------------------------------------------------------
    // 3. ONE BYTE CHANGES — the next export stages revision 2 linked by supersedes_profile_id, and the
    //    revision it supersedes is still there. History is appended, never rewritten.
    // ---------------------------------------------------------------------------------------------
    let changed = dao
        .load_apple_contacts(
            &export(&e2, SHA_SECOND, contacts("787-555-0999")),
            account_a,
        )
        .await
        .map_err(|error| format!("{HARNESS}: the changed export must stage, got {error}"))?;
    check_eq!(
        want(&changed, "batchCreated")?.as_bool(),
        Some(true),
        "a new export is a new batch"
    );
    check_eq!(
        total(&changed, "input")?,
        3,
        "all three contacts are input again"
    );
    check_eq!(
        total(&changed, "new")?,
        0,
        "no contact is a first sighting now"
    );
    check_eq!(
        total(&changed, "replay")?,
        2,
        "the two untouched contacts are replays"
    );
    check_eq!(
        total(&changed, "changed")?,
        1,
        "the contact whose phone changed is a changed revision"
    );
    check_eq!(
        want(&changed, "balanced")?.as_bool(),
        Some(true),
        "input balances again"
    );
    check_eq!(
        batch_counters(db, &e2).await?,
        Some((3, 3, 0, 2, 1, 0, "loaded".to_owned())),
        "the second batch persists new=0, replay=2, changed=1"
    );
    check_eq!(
        staged(db, accounts).await?,
        4,
        "three revisions plus one successor: the history grows by the change alone"
    );
    check_eq!(
        count(
            db,
            "select count(*) from integration_staged_contact_profile where source_account in ($1,$2) and revision > 1",
            accounts
        )
        .await?,
        1,
        "exactly one successor revision exists"
    );
    check_eq!(
        count(
            db,
            "select count(*) from integration_staged_contact_profile
              where source_account in ($1,$2) and supersedes_profile_id is not null",
            accounts
        )
        .await?,
        1,
        "the successor names the revision it supersedes"
    );
    check_eq!(
        count(
            db,
            "select count(*) from integration_staged_contact_profile
              where source_account in ($1,$2) and supersedes_profile_id is null",
            accounts
        )
        .await?,
        3,
        "the superseded revisions are never overwritten or deleted"
    );
    check_eq!(
        count(
            db,
            "select count(distinct payload_fingerprint) from integration_staged_contact_profile where source_account in ($1,$2)",
            accounts
        )
        .await?,
        4,
        "the changed bytes are a different fingerprint: four staged profiles, four identities"
    );
    check_eq!(
        members(db, accounts).await?,
        6,
        "both exports record their own snapshot membership"
    );
    check_eq!(
        receipts(db, accounts).await?,
        3,
        "a changed contact does not get a second receipt"
    );

    // ---------------------------------------------------------------------------------------------
    // 4. SAME BATCH ID, DIFFERENT BYTES — a truthful conflict, not a replay: non-zero exit, the batch
    //    marked `conflict`, and not one staged row touched.
    // ---------------------------------------------------------------------------------------------
    let conflict = dao
        .load_apple_contacts(&export(&e1, SHA_THIRD, contacts("787-555-0102")), account_a)
        .await
        .map_err(|error| {
            format!("{HARNESS}: a checksum conflict is reported, not raised, got {error}")
        })?;
    check_eq!(
        want(&conflict, "status")?.as_str(),
        Some("conflict"),
        "the batch is a conflict"
    );
    check_eq!(
        want(&conflict, "exitCode")?.as_i64(),
        Some(1),
        "a conflict exits non-zero so the wrapper stops the chain"
    );
    check!(
        want(&conflict, "message")?
            .as_str()
            .unwrap_or_default()
            .contains("different checksum"),
        "the refusal says why it refused"
    );
    check_eq!(
        text_value(
            db,
            "select load_status from integration_intake_batch where external_batch_id = $1",
            &[&e1]
        )
        .await?,
        Some("conflict".to_owned()),
        "the marker is written instead of being swallowed"
    );
    check_eq!(staged(db, accounts).await?, 4, "a conflict stages nothing");
    check_eq!(
        receipts(db, accounts).await?,
        3,
        "a conflict writes no receipt"
    );

    // ---------------------------------------------------------------------------------------------
    // 5. REFUSAL — two contacts sharing one Apple sourceId. The load is one transaction: a refusal
    //    leaves NO batch, NO receipt and NO staged revision, rather than a half-imported export.
    // ---------------------------------------------------------------------------------------------
    let mut duplicated = contacts("787-555-0102");
    let first_contact = duplicated[0].clone();
    duplicated
        .as_array_mut()
        .expect("the fixture is an array")
        .push(first_contact);
    match dao
        .load_apple_contacts(&export(&e3, SHA_FOURTH, duplicated), account_a)
        .await
    {
        Err(refusal) => check!(
            refusal.to_string().contains("apple_contacts_load"),
            format!("the refusal comes from the staging boundary itself, got {refusal}")
        ),
        Ok(tally) => {
            return Err(format!(
                "{HARNESS}: two contacts sharing one sourceId must be refused, got {tally}"
            ))
        }
    }
    check_eq!(
        count(
            db,
            "select count(*) from integration_intake_batch where external_batch_id = $1",
            &[&e3]
        )
        .await?,
        0,
        "a refused export leaves no batch receipt anywhere"
    );
    check_eq!(
        staged(db, accounts).await?,
        4,
        "a refused export stages nothing"
    );
    check_eq!(
        receipts(db, accounts).await?,
        3,
        "a refused export writes no receipt"
    );

    // ---------------------------------------------------------------------------------------------
    // 6. REFUSAL — an export that declares another source system never reaches the ODS.
    // ---------------------------------------------------------------------------------------------
    let mut foreign = export(&e4, SHA_FIFTH, contacts("787-555-0102"));
    foreign["sourceSystem"] = serde_json::json!("apple_messages");
    match dao.load_apple_contacts(&foreign, account_a).await {
        Err(refusal) => check!(
            refusal.to_string().contains("sourceSystem"),
            format!("the refusal names the field it rejected, got {refusal}")
        ),
        Ok(tally) => {
            return Err(format!(
                "{HARNESS}: a foreign sourceSystem must be refused, got {tally}"
            ))
        }
    }
    check_eq!(
        count(
            db,
            "select count(*) from integration_intake_batch where external_batch_id = $1",
            &[&e4]
        )
        .await?,
        0,
        "a foreign export leaves no batch receipt anywhere"
    );
    check_eq!(
        staged(db, accounts).await?,
        4,
        "a foreign export stages nothing"
    );

    // ---------------------------------------------------------------------------------------------
    // 7. REFUSAL — a second account exists, so an unnamed account has a choice to make and must not
    //    make it. Identity is never guessed: `""` would otherwise resolve to whatever account the
    //    database happens to hold (on a shared DEV that is a real one).
    // ---------------------------------------------------------------------------------------------
    let second_account = dao
        .load_apple_contacts(&export(&e5, SHA_SIXTH, account_b_contacts()), account_b)
        .await
        .map_err(|error| {
            format!("{HARNESS}: the second account's export must stage, got {error}")
        })?;
    check_eq!(
        total(&second_account, "new")?,
        1,
        "the second account stages its own contact"
    );
    check_eq!(
        count(
            db,
            "select count(distinct source_account) from integration_intake_batch where source = 'apple_contacts' and source_account in ($1,$2)",
            accounts
        )
        .await?,
        2,
        "the source now holds two named accounts, so an unnamed one is ambiguous"
    );
    let staged_before = staged(db, accounts).await?;
    match dao
        .load_apple_contacts(&export(&e6, SHA_SEVENTH, account_b_contacts()), "")
        .await
    {
        Err(refusal) => check!(
            refusal.to_string().contains("source_account is required"),
            format!("an ambiguous account must be refused, got {refusal}")
        ),
        Ok(tally) => {
            return Err(format!(
                "{HARNESS}: staging guessed an account instead of refusing: {tally}"
            ))
        }
    }
    check_eq!(
        count(
            db,
            "select count(*) from integration_intake_batch where external_batch_id = $1",
            &[&e6]
        )
        .await?,
        0,
        "the refusal writes no batch anywhere, under any account"
    );
    check_eq!(
        staged(db, accounts).await?,
        staged_before,
        "the refusal leaves the staged history untouched"
    );

    // ---------------------------------------------------------------------------------------------
    // 8. STAGING IS STILL NOT PROMOTION — after every load above, the projection and the canonical
    //    person spine are exactly as they were found.
    // ---------------------------------------------------------------------------------------------
    check_eq!(
        count(
            db,
            "select count(*) from l_person where source_account in ($1,$2)",
            accounts
        )
        .await?,
        0,
        "no load projected a row into l_person"
    );
    check_eq!(
        count(
            db,
            "select count(*) from l_property where source_account in ($1,$2)",
            accounts
        )
        .await?,
        0,
        "no load projected an address into l_property"
    );
    check_eq!(
        count(
            db,
            "select count(*) from person_identity where identity_value in ($1,$2)",
            &[DANA_EMAIL, MARCUS_EMAIL]
        )
        .await?,
        0,
        "no load promoted a contact into the canonical person spine"
    );
    check_eq!(
        count(
            db,
            "select count(*) from integration_staged_contact_profile
              where source_account in ($1,$2) and candidate_person_id is not null",
            accounts
        )
        .await?,
        0,
        "no staged row claims a candidate person before reconciliation"
    );
    Ok(())
}

/// Every row this run created, in FK-safe order: successors before the revisions they supersede, staged rows before
/// the receipts and batches they reference, and the batch last because snapshot membership cascades with it.
async fn cleanup(db: &Database, accounts: &[&str]) -> Result<(), String> {
    run(
        db,
        "delete from integration_staged_contact_profile
          where source_account in ($1,$2) and supersedes_profile_id is not null",
        accounts,
    )
    .await?;
    run(
        db,
        "delete from integration_staged_contact_profile where source_account in ($1,$2)",
        accounts,
    )
    .await?;
    run(
        db,
        "delete from integration_inbox where source_account in ($1,$2)",
        accounts,
    )
    .await?;
    run(
        db,
        "delete from integration_intake_batch where source_account in ($1,$2)",
        accounts,
    )
    .await?;
    Ok(())
}

/// Leftovers, in the order the tables were written: staged, receipts, batches, membership, projection (person and
/// property), and the canonical identities of the fixture's two addresses.
async fn leftovers(
    db: &Database,
    accounts: &[&str],
) -> Result<(i64, i64, i64, i64, i64, i64, i64), String> {
    Ok((
        count(
            db,
            "select count(*) from integration_staged_contact_profile where source_account in ($1,$2)",
            accounts,
        )
        .await?,
        count(
            db,
            "select count(*) from integration_inbox where source_account in ($1,$2)",
            accounts,
        )
        .await?,
        count(
            db,
            "select count(*) from integration_intake_batch where source_account in ($1,$2)",
            accounts,
        )
        .await?,
        count(
            db,
            "select count(*) from integration_source_snapshot_member where source_account in ($1,$2)",
            accounts,
        )
        .await?,
        count(
            db,
            "select count(*) from l_person where source_account in ($1,$2)",
            accounts,
        )
        .await?,
        count(
            db,
            "select count(*) from l_property where source_account in ($1,$2)",
            accounts,
        )
        .await?,
        count(
            db,
            "select count(*) from person_identity where identity_value in ($1,$2)",
            &[DANA_EMAIL, MARCUS_EMAIL],
        )
        .await?,
    ))
}

/// Connect to the disposable DEV branch, tolerating a cold-pool timeout under concurrent test load.
///
/// Infrastructure, not the contract: `TestDatabase` still refuses PRODUCTION before any socket is opened.
async fn connect_dev() -> TestDatabase {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match TestDatabase::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("proof: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "DATABASE_URL_DEV must reach a disposable DEV branch; the harness refuses PROD: {}",
        last.unwrap_or_default()
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); the harness refuses PROD before any socket is opened"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-INT-APPLE-001); the file and the assay use it.
async fn int_apple_001__contacts_staging() {
    // 0. THE BOUNDARY'S TARGET. Resolved and refused by the harness: PRODUCTION never gets a socket.
    let harness = connect_dev().await;
    assert_eq!(
        harness.target(),
        DbTarget::Dev,
        "{HARNESS}: contacts staging is proven on a disposable DEV target"
    );
    let db: Database = harness.database().clone();

    // This run's own marker. Scoped, never shared with another run, and every row below carries it.
    let tag = format!("tst-int-apple-001-{}", uuid::Uuid::new_v4().simple());
    let account_a = format!("{tag}-a");
    let account_b = format!("{tag}-b");
    let accounts = [account_a.as_str(), account_b.as_str()];

    // 1. CLEAN SLATE: a previous run of this same test that died before teardown must not read as evidence.
    cleanup(&db, &accounts)
        .await
        .expect("the marker starts clean");

    // 2. THE PROOF. Every assertion inside returns Err rather than panicking, so step 3 always runs.
    let outcome = proof(&db, &tag, &accounts).await;

    // 3. TEARDOWN, THEN THE ZERO-LEFTOVER PROOF: DEV must be exactly as it was found, or this test is itself the
    //    defect it claims to catch.
    let teardown = cleanup(&db, &accounts).await;
    let remaining = leftovers(&db, &accounts).await;
    match (teardown, remaining) {
        (Err(error), _) => panic!("{HARNESS}: teardown failed, DEV may hold fixture rows: {error}"),
        (Ok(()), Err(error)) => panic!("{HARNESS}: the leftover count does not read: {error}"),
        (Ok(()), Ok(counts)) => assert_eq!(
            counts,
            (0, 0, 0, 0, 0, 0, 0),
            "{HARNESS}: DEV is left as it was found — staged, receipts, batches, membership, projection \
             and canonical identities must all be zero after teardown"
        ),
    }

    if let Err(report) = outcome {
        // The report already carries `HARNESS` in every line it formats.
        panic!("{report}");
    }
}
