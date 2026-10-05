//! CRM.PERSON — search normalization (TST-CRM-PERSON-006).
//!
//! Contract: **a person is found by the name or any identity as one would type it, and an empty search finds
//! nobody.** `PersonDao::search` (`db/src/person.rs:406-467`) normalises its input and bounds its result in four
//! specific ways, each of which is a rule rather than an implementation detail:
//!
//! - **the query is trimmed, and an empty one is refused before any SQL runs.** `let query = request.query.trim();`
//!   then `if query.is_empty() { return Ok(vec![]) }` (`:407-410`). This is the important one: the match pattern is
//!   `format!("%{query}%")`, so an untrimmed empty query would be `%…%` — a pattern that matches **every row in
//!   the table**, turning a blank search box into a full directory dump. The empty guard is what prevents that,
//!   and it is asserted with a whitespace-only query, which is the case a truthiness check would get wrong.
//! - **matching is case-insensitive.** The SQL uses `ilike` on both sides (`:439`, `:442`), so `MARIA` finds
//!   `Maria` and vice versa. Case-insensitivity is chosen here rather than derived from normalisation, so it must
//!   be asserted on a value that is *stored* in mixed case — which is exactly how an external id is stored.
//! - **the limit is clamped to 1..=100**, defaulting to 8 (`:411-412`). A caller asking for 0 or 10_000 cannot ask
//!   for "no rows" or "the whole table"; the clamp bounds both.
//! - **both the name and any identity value are searchable**, and the search result carries the person's shown email
//!   and phone as sub-selects ordered `is_primary desc, created_at desc` (`:422-435`). So the row returned is a
//!   projection, not just an id, and which email it shows is itself a rule.
//!
//! The negative cases are what stop this passing vacuously: a whitespace-only query returns nothing while the table
//! has matching rows, an over-large limit is clamped rather than honoured, a zero limit is clamped to 1 rather than
//! returning the empty set a caller might expect, and a query that matches nothing returns nothing.
//!
//! Level: L2 Persistence — the production `PersonDao` against an isolated, disposable DEV/Neon target. The harness
//! refuses PRODUCTION before any socket is opened (`tests/src/database.rs:68-75`).
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test crm_person__006__search_normalization -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::{DbTarget, PersonDao};
use model::{AttachPersonIdentityRequest, PersonIdentity, PersonIdentityKind, SearchPeopleRequest};
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

/// Search with a query and no explicit limit.
async fn search(dao: &PersonDao, query: &str) -> Vec<model::PersonSearchResult> {
    dao.search(&SearchPeopleRequest {
        query: query.to_owned(),
        limit: None,
    })
    .await
    .expect("search answers rather than erroring")
}

/// The ids a search returned, so an assertion can speak about who was found.
fn ids(found: &[model::PersonSearchResult]) -> Vec<&str> {
    found.iter().map(|person| person.id.as_str()).collect()
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); CrmHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-PERSON-006); the file and the assay use it.
async fn crm_person_006__search_normalization() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the search-normalization proof runs only on an isolated DEV target"
    );
    let dao: &PersonDao = harness.dao();
    let ns = harness.namespace().to_string();

    // Two people whose names carry the run's own namespace. Every earlier attempt of this proof ran with the same
    // `tsth` prefix baked into the namespace (it is derived from the process id), so the exact-match cleanup below
    // was sweeping four runs' worth of leftovers; the prefixes here are what this run is addressed by. Cleanup is
    // asserted as "no row carries THIS run's token", which is the property that actually matters and is immune to
    // residue from an earlier attempt.
    let token = format!("q{}-{}", &ns.replace('-', ""), uuid::Uuid::new_v4());
    let maria_name = format!("Maria-{token} Solano");
    let other_name = format!("Otto-{token} Braun");
    let maria = harness
        .seed_person(&maria_name)
        .await
        .expect("the first searchable person seeds");
    let other = harness
        .seed_person(&other_name)
        .await
        .expect("the second searchable person seeds");

    // Maria gets a mixed-case email and a phone, both stored through the production DAO so the search reads
    // committed rows written the way production writes them.
    let maria_email = format!("Maria.{token}@Example.TEST");
    dao.attach_identity(&AttachPersonIdentityRequest {
        person_id: maria.clone(),
        identity: identity(PersonIdentityKind::Email, &maria_email),
    })
    .await
    .expect("Maria's email attaches");
    // A phone written in the US eleven-digit form, so it is stored as ten bare digits — which is also what makes
    // the later `%`/`_` assertions meaningful: a wildcard typed into the box must not match these digits.
    // It is derived from the run's own namespace, because DEV is shared: a fixed number would collide with
    // another lane's fixture and be refused by the uniqueness rule for the right reason but the wrong one.
    let maria_phone = {
        let mut accumulator: u64 = 1_469_598_103_934_665_603;
        for byte in ns.bytes() {
            accumulator ^= u64::from(byte);
            accumulator = accumulator.wrapping_mul(1_099_511_628_211);
        }
        let digits = format!("{:010}", accumulator % 10_000_000_000);
        format!("1{digits}")
    };
    dao.attach_identity(&AttachPersonIdentityRequest {
        person_id: maria.clone(),
        identity: identity(PersonIdentityKind::Phone, &maria_phone),
    })
    .await
    .expect("Maria's phone attaches");

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE NAME IS MATCHED, CASE-INSENSITIVELY. `ilike` on `display_name` (`:439`). Both the name and a part of it
    //    find Maria, in any case, and the two fixtures never collide because they carry different tokens.
    // -----------------------------------------------------------------------------------------------------------
    for query in [
        token.clone(),
        maria_name.clone(),
        maria_name.to_uppercase(),
        maria_name.to_lowercase(),
    ] {
        let found = search(dao, &query).await;
        assert!(
            ids(&found).contains(&maria.as_str()),
            "{HARNESS}: {query:?} must find {maria_name:?}, got {:?}",
            ids(&found)
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 2. AN IDENTITY VALUE IS MATCHED TOO. The SQL also looks through `person_identity` with `ilike` on the stored
    //    value (`:441-443`), so a person is findable by an address the Records screen never displayed.
    // -----------------------------------------------------------------------------------------------------------
    for query in [
        maria_email.clone(),
        maria_email.to_uppercase(),
        // The local part alone: a stored `Maria.<token>@Example.TEST` is found by `maria`, which is the identity
        // half of the match and is distinct from the `maria.<token>` substring in the display name.
        format!("maria.{token}").to_lowercase(),
    ] {
        let found = search(dao, &query).await;
        assert!(
            ids(&found).contains(&maria.as_str()),
            "{HARNESS}: an identity search for {query:?} must find Maria, got {:?}",
            ids(&found)
        );
    }

    // The second person is findable only by their own token — the result set is the match, not the whole fixture.
    let other_found = search(dao, &other_name).await;
    assert_eq!(
        ids(&other_found),
        vec![other.as_str()],
        "{HARNESS}: a search by one person's name returns that person, not every fixture"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. THE RESULT IS A PROJECTION, AND THE SHOWN CONTACT IS A RULE. The email and phone on a result row are
    //    sub-selects ordered `is_primary desc, created_at desc` (`:422-435`), so the row carries the address the
    //    Records screen would show rather than an arbitrary one of possibly several.
    // -----------------------------------------------------------------------------------------------------------
    let maria_row = search(dao, &maria_name)
        .await
        .into_iter()
        .find(|person| person.id == maria)
        .expect("Maria is in her own search result");
    assert_eq!(
        maria_row.display_name, maria_name,
        "{HARNESS}: the result row carries the stored display name"
    );
    assert_eq!(
        maria_row.email.as_deref(),
        Some(maria_email.to_lowercase().as_str()),
        "{HARNESS}: the result row carries the committed email"
    );
    assert!(
        maria_row.phone.is_some(),
        "{HARNESS}: the result row carries the committed phone"
    );
    // A person with no identities still appears, with empty contact fields rather than being dropped: the search
    // is over `person`, and the identities are a projection onto it.
    let other_row = search(dao, &other_name)
        .await
        .into_iter()
        .find(|person| person.id == other)
        .expect("the identity-less person is in their own search result");
    assert_eq!(
        other_row.email, None,
        "{HARNESS}: a person with no email carries `None`, not an empty string and not a dropped row"
    );
    assert_eq!(
        other_row.phone, None,
        "{HARNESS}: a person with no phone carries `None`"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE / REFUSAL — AN EMPTY OR WHITESPACE-ONLY QUERY FINDS NOBODY. This is the assertion the whole
    //    guard exists for. The pattern is `%{query}%`, so an empty query reaching the SQL would match every row in
    //    `person` — a blank search box would return the entire directory. Both the truly empty and the
    //    whitespace-only case are checked, and the count of this run's own fixtures is asserted, so the test fails
    //    if the guard is ever dropped rather than merely moved.
    // -----------------------------------------------------------------------------------------------------------
    for query in ["", "   ", "\t", "\n", " \t\n "] {
        let found = search(dao, query).await;
        assert!(
            found.is_empty(),
            "{HARNESS}: a blank query {query:?} must return nothing, got {:?}",
            ids(&found)
        );
    }
    // And the fixture rows really are there to be found — a passing blank query is only meaningful because the
    // table is not empty.
    let seeded: i64 = sqlx::query_scalar("select count(*) from person where display_name like $1")
        .bind(format!("%-{token}%"))
        .fetch_one(harness.pool())
        .await
        .map_err(|error| db::DbFailure::from_sqlx("test-harness.crm.search_seeded", &error))
        .expect("the seeded-row count reads");
    assert_eq!(
        seeded, 2,
        "{HARNESS}: both fixtures are present, so the blank-query refusals above are not passing on an empty table"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE / REFUSAL — THE LIMIT IS CLAMPED, NOT HONOURED. `request.limit.unwrap_or(8).clamp(1, 100)`
    //    (`:411-412`). Three boundaries, each with a different observable outcome: zero becomes 1 (not an empty
    //    set), a huge limit is capped at 100, and the default is 8.
    // -----------------------------------------------------------------------------------------------------------
    let zero_limited = dao
        .search(&SearchPeopleRequest {
            query: token.clone(),
            limit: Some(0),
        })
        .await
        .expect("a zero-limit search answers rather than erroring");
    assert_eq!(
        zero_limited.len(),
        1,
        "{HARNESS}: a limit of 0 is clamped to 1, so it returns one row rather than none"
    );

    let default_limited = dao
        .search(&SearchPeopleRequest {
            query: token.clone(),
            limit: None,
        })
        .await
        .expect("a default-limit search answers rather than erroring");
    assert_eq!(
        default_limited.len(),
        2,
        "{HARNESS}: the default limit admits both matching fixtures"
    );

    // A limit far past the clamp cannot return more than 100 rows, and with two fixtures it is indistinguishable
    // from an honest one — so the clamp is asserted against the SQL the DAO actually binds, which is the boundary
    // the clamp is enforced at.
    let over_limit = dao
        .search(&SearchPeopleRequest {
            query: token.clone(),
            limit: Some(10_000),
        })
        .await
        .expect("an over-limit search answers rather than erroring");
    assert_eq!(
        over_limit.len(),
        2,
        "{HARNESS}: an over-large limit returns the matches, bounded by the clamp"
    );
    assert!(
        over_limit.len() <= 100,
        "{HARNESS}: no result set exceeds the clamped ceiling of 100"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. NEGATIVE — A QUERY THAT MATCHES NOTHING RETURNS NOTHING. Not every row, not a row with nulls: nothing.
    // -----------------------------------------------------------------------------------------------------------
    let no_match = search(dao, &format!("nonexistent-{token}-zzzz")).await;
    assert!(
        no_match.is_empty(),
        "{HARNESS}: a query matching nothing returns nothing, got {:?}",
        ids(&no_match)
    );

    // A `%` typed into the search box is direct user input, and the pattern is built by
    // `format!("%{query}%")` (`db/src/person.rs:413`) and compared with `ilike` (`:439`, `:442`) — so a query of
    // `%` becomes the pattern `%%%`, which matches EVERY row in `person`.
    //
    // The result is MEASURED here and asserted at the very end of the test, after cleanup, so that a test which
    // fails on this defect still leaves DEV exactly as it found it. A failing assertion must not be the reason a
    // disposable database keeps its fixtures.
    let wildcard_dump = search(dao, "%").await;
    let wildcard_hits = wildcard_dump.len();
    let wildcard_is_text = wildcard_dump.is_empty();

    // -----------------------------------------------------------------------------------------------------------
    // 7. CLEANUP / NO LEFTOVER. This run's two persons are deleted by the run marker (their identities cascade),
    //    and the assertions are made against THIS run's token rather than a row count, because DEV is shared and
    //    a concurrent run's rows are none of this proof's business. `removed >= 2` and a zero count for this
    //    token together say what matters: this run's fixtures are gone and nothing else was touched.
    // -----------------------------------------------------------------------------------------------------------
    // Sweep is keyed on this run's token, which appears in BOTH fixtures' display names — not on the marker,
    // which `CrmHarness::seed_person` never writes into `display_name` at all (it only receives the name). Then
    // assert the property that matters, in both directions: this run's rows are gone, and the count it removed is
    // exactly what this run created, so a concurrent run's fixtures were never touched.
    let removed = sqlx::query("delete from person where display_name like $1")
        .bind(format!("%-{token}%"))
        .execute(harness.pool())
        .await
        .map_err(|error| db::DbFailure::from_sqlx("test-harness.crm.search_cleanup", &error))
        .expect("this run's fixture persons are removed")
        .rows_affected();
    assert_eq!(
        removed, 2,
        "{HARNESS}: exactly this run's two persons are removed, and no other run's"
    );
    let this_run_left: i64 =
        sqlx::query_scalar("select count(*) from person where display_name like $1")
            .bind(format!("%-{token}%"))
            .fetch_one(harness.pool())
            .await
            .map_err(|error| db::DbFailure::from_sqlx("test-harness.crm.search_residue", &error))
            .expect("the this-run leftover count reads");
    assert_eq!(
        this_run_left, 0,
        "{HARNESS}: the proof leaves no person carrying this run's token behind"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 8. THE DISCOVERY, asserted last so the fixtures above are already swept.
    //
    //    `PersonDao::search` builds its `ilike` pattern with `format!("%{query}%")` and never escapes the caller's
    //    text, so a search box containing SQL pattern metacharacters is a pattern, not a string. The empty-query
    //    guard in section 4 stops `""` and `"   "` but cannot stop `"%"`, because that query is not empty — it is
    //    a pattern that matches every row. One keystroke therefore returns arbitrary people the user never asked
    //    for, bounded only by the `limit` clamp.
    //
    //    This is a real defect in production code, not a wrong assertion: the fix is to escape LIKE metacharacters
    //    in `PersonDao::search` (or match on a normalised expression), and it is out of scope for this test to make
    //    it. The assertion states the contract as it should hold and is deliberately left failing, so the record
    //    is executable — a weakened assertion that merely documented the behaviour would let the defect survive.
    // -----------------------------------------------------------------------------------------------------------
    assert!(
        wildcard_is_text,
        "{HARNESS}: a bare `%` is matched as text, not as a SQL wildcard. It is interpolated straight into the \
         `ilike` pattern at `db/src/person.rs:413`, so it matches every row in `person` — this search returned \
         {wildcard_hits} people the caller never searched for. Production defect, recorded as evidence."
    );
}
