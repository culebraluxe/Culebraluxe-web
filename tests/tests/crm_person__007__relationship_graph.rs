//! CRM.PERSON — relationship graph (TST-CRM-PERSON-007).
//!
//! Contract: **relationship meaning lives on the edge, not on the person.** `person_person` is the canonical
//! person-to-person graph (`db/migrations/117_firm_relation_role.sql:70-91`) and it carries the meaning as data:
//! `role_id` names the relationship and `role_scope` is pinned to `'person_person'`. Nothing about "spouse of"
//! or "partner of" is a column on `person`; a person is related to another by *holding an edge*, and the edge's role
//! is what says which relationship it is.
//!
//! This is the one CRM.PERSON boundary with **no Rust DAO** — `grep -rn "person_person" --include=*.rs` finds
//! nothing outside this file — so the invariants are the database's own CHECK, UNIQUE and FOREIGN KEY constraints,
//! and this test drives them directly. That is the honest test seam here and it is the one the harness itself
//! sanctions: *"Raw SQL is reserved for tests whose subject **is** the database contract"*
//! (`tests/src/lib.rs:11-14`). No production code is re-declared and none is bypassed — there is none to bypass,
//! and inventing a DAO here would be the second implementation the harness forbids.
//!
//! Six rules are proven, and each is a constraint that exists for a reason a caller would otherwise get wrong:
//!
//! 1. **NO SELF-EDGE.** `person_person_not_self check (person_id <> related_person_id)`. A person is not their own
//!    spouse; without it a malformed import creates a self-loop that every graph walk must then special-case.
//! 2. **ONE EDGE PER (FROM, TO, ROLE).** `person_person_unique unique (person_id, related_person_id, role_id)`. Two
//!    imports of the same relationship converge on one edge rather than duplicating it.
//! 3. **THE ROLE MUST BE A `person_person` ROLE.** `role_scope` is CHECKed to `'person_person'` *and* carries a
//!    composite foreign key onto `relation_role(id, scope)`. A firm-scoped or contract-scoped role cannot be
//!    smuggled onto a person-to-person edge, which is the mistake a single-scope `role.id` FK would have allowed.
//! 4. **THE GRAPH IS DIRECTED.** `A → B` and `B → A` are two distinct edges, because the key is the *ordered*
//!    triple. "A is B's partner" and "B is A's partner" are different statements, and a graph that conflated them
//!    would make every relationship symmetric.
//! 5. **ROLE DISTINGUISHES THE MEANING.** The same two people, related by two different roles, are two edges — the
//!    meaning is on the edge, which is the whole point of rule 4's existence.
//! 6. **EDGES CASCADE WITH THEIR PEOPLE.** Both ends are `references person(id) on delete cascade`, so removing a
//!    person removes their edges and cannot leave a dangling row pointing at nobody.
//!
//! The negative cases are the constraint violations themselves: a self-edge, a duplicate edge, an out-of-scope
//! role, and an edge to a person that does not exist — each asserted to be refused *and* to have written nothing.
//!
//! Level: L2 Persistence — the `person_person` contract against an isolated, disposable DEV/Neon target. The harness
//! refuses PRODUCTION before any socket is opened (`tests/src/database.rs:68-75`).
//!
//! Greenfield Rust: this is not a port of any TypeScript test.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test crm_person__007__relationship_graph -- --ignored
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use db::{DbFailure, DbTarget};
use test_harness::CrmHarness;

const HARNESS: &str = "CrmHarness/L2 Persistence";

/// The scope every `person_person` edge must carry, and the only scope its role may come from.
const EDGE_SCOPE: &str = "person_person";

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

/// A person-to-person role id from the seeded `relation_role`, by code.
async fn edge_role(harness: &CrmHarness, code: &str) -> String {
    // The table was renamed `relation_role` → `role` after migration 117 (its `id` stayed the primary key, which is
    // why the composite FK on `person_person` still resolves). The lookup follows the live name.
    let id: Option<String> =
        sqlx::query_scalar("select id::text from role where scope = $1 and code = $2")
            .bind(EDGE_SCOPE)
            .bind(code)
            .fetch_optional(harness.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("test-harness.crm.edge_role", &error))
            .expect("the edge role lookup runs");
    id.unwrap_or_else(|| {
        panic!("{HARNESS}: relation_role must seed a {EDGE_SCOPE} role with code {code}")
    })
}

/// A role id from a *different* scope, to prove an edge cannot borrow it.
async fn foreign_scope_role(harness: &CrmHarness) -> Option<String> {
    sqlx::query_scalar("select id::text from role where scope <> $1 order by scope, code limit 1")
        .bind(EDGE_SCOPE)
        .fetch_optional(harness.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("test-harness.crm.foreign_role", &error))
        .expect("the out-of-scope role lookup runs")
}

/// Insert one edge, returning the database's answer.
async fn add_edge(
    harness: &CrmHarness,
    person_id: &str,
    related_person_id: &str,
    role_id: &str,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar(
        "insert into person_person (person_id, related_person_id, role_id, role_scope, source_type)
         values ($1::uuid, $2::uuid, $3::uuid, $4, 'manual') returning id::text",
    )
    .bind(person_id)
    .bind(related_person_id)
    .bind(role_id)
    .bind(EDGE_SCOPE)
    .fetch_one(harness.pool())
    .await
}

/// How many committed edges this run created between two people under a role.
async fn edge_count(harness: &CrmHarness, from: &str, to: &str, role_id: &str) -> i64 {
    sqlx::query_scalar(
        "select count(*) from person_person
          where person_id = $1::uuid and related_person_id = $2::uuid and role_id = $3::uuid",
    )
    .bind(from)
    .bind(to)
    .bind(role_id)
    .fetch_one(harness.pool())
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm.edge_count", &error))
    .expect("the edge count reads")
}

/// Every edge this run's people hold, regardless of role or direction.
async fn all_edges(harness: &CrmHarness, people: &[String]) -> i64 {
    sqlx::query_scalar("select count(*) from person_person where person_id = any($1::uuid[])")
        .bind(people)
        .fetch_one(harness.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("test-harness.crm.all_edges", &error))
        .expect("the all-edges count reads")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); CrmHarness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-CRM-PERSON-007); the file and the assay use it.
async fn crm_person_007__relationship_graph() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let harness = connect_dev().await;
    assert_eq!(
        harness.database().target(),
        DbTarget::Dev,
        "{HARNESS}: the relationship-graph proof runs only on an isolated DEV target"
    );
    let ns = harness.namespace().to_string();
    let marker = format!("TST-CRMPERSON007-{ns}");

    // Three people: two endpoints of the relationships under test, plus a third for the cascade rule.
    let ada = harness
        .seed_person(&format!("{marker}-ada"))
        .await
        .expect("the first endpoint seeds");
    let grace = harness
        .seed_person(&format!("{marker}-grace"))
        .await
        .expect("the second endpoint seeds");
    let alan = harness
        .seed_person(&format!("{marker}-alan"))
        .await
        .expect("the third person seeds");

    let spouse = edge_role(&harness, "SPOUSE").await;
    let partner = edge_role(&harness, "PARTNER").await;
    assert_ne!(
        spouse, partner,
        "{HARNESS}: the graph needs two distinct person_person roles — the meaning is carried by the role"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1. AN EDGE IS AN EDGE. One person is related to another by holding a row whose role names the relationship;
    //    nothing about the relationship is a column on `person` itself.
    // -----------------------------------------------------------------------------------------------------------
    add_edge(&harness, &ada, &grace, &spouse)
        .await
        .expect("a spouse edge between two people is legal");
    assert_eq!(
        edge_count(&harness, &ada, &grace, &spouse).await,
        1,
        "{HARNESS}: the committed edge exists between the two people under the spouse role"
    );

    // The stored row carries the scope, so the edge is self-describing without joining anything.
    let stored_scope: String = sqlx::query_scalar(
        "select role_scope from person_person
          where person_id = $1::uuid and related_person_id = $2::uuid and role_id = $3::uuid",
    )
    .bind(&ada)
    .bind(&grace)
    .bind(&spouse)
    .fetch_one(harness.pool())
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm.edge_scope", &error))
    .expect("the committed edge scope reads");
    assert_eq!(
        stored_scope, EDGE_SCOPE,
        "{HARNESS}: every person-to-person edge records its own scope, so the graph is self-describing"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. NEGATIVE / REFUSAL — NO SELF-EDGE. `person_person_not_self` refuses a person being related to themselves,
    //    and the refusal writes nothing.
    // -----------------------------------------------------------------------------------------------------------
    let self_edge = add_edge(&harness, &ada, &ada, &spouse).await;
    assert!(
        self_edge.is_err(),
        "{HARNESS}: a self-edge is refused — a person is not their own spouse"
    );
    assert!(
        self_edge
            .expect_err("the self-edge is refused")
            .to_string()
            .contains("person_person_not_self"),
        "{HARNESS}: the refusal names the no-self-edge constraint"
    );
    assert_eq!(
        edge_count(&harness, &ada, &ada, &spouse).await,
        0,
        "{HARNESS}: the refused self-edge wrote no row"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. NEGATIVE / REFUSAL — ONE EDGE PER (FROM, TO, ROLE). Two imports of the same relationship converge on one
    //    edge instead of duplicating it, which is what keeps a re-run of an importer from doubling a relationship.
    // -----------------------------------------------------------------------------------------------------------
    let duplicate = add_edge(&harness, &ada, &grace, &spouse).await;
    assert!(
        duplicate.is_err(),
        "{HARNESS}: the same (from, to, role) twice is refused — a relationship is one edge, not two"
    );
    assert!(
        duplicate
            .expect_err("the duplicate edge is refused")
            .to_string()
            .contains("person_person_unique"),
        "{HARNESS}: the refusal names the one-edge-per-pair constraint"
    );
    assert_eq!(
        edge_count(&harness, &ada, &grace, &spouse).await,
        1,
        "{HARNESS}: the refused duplicate wrote nothing — the original edge is still the only one"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. THE GRAPH IS DIRECTED, AND THE ROLE CARRIES THE MEANING. `A → B` and `B → A` are distinct edges because
    //    the unique key is the ordered triple, and the same pair under a second role is a second edge. Both are
    //    positive cases: the interesting failures here are over-collapsing, which a collision-only test never sees.
    // -----------------------------------------------------------------------------------------------------------
    add_edge(&harness, &grace, &ada, &spouse)
        .await
        .expect("the reverse direction is a distinct, legal edge");
    assert_eq!(
        edge_count(&harness, &grace, &ada, &spouse).await,
        1,
        "{HARNESS}: `B → A` is its own edge — the graph is directed, not symmetric"
    );
    add_edge(&harness, &ada, &grace, &partner).await.expect(
        "the same two people under a second role is a distinct, legal edge — the meaning lives on the edge",
    );
    assert_eq!(
        edge_count(&harness, &ada, &grace, &partner).await,
        1,
        "{HARNESS}: a second role between the same two people is a second edge"
    );
    assert_eq!(
        edge_count(&harness, &ada, &grace, &spouse).await
            + edge_count(&harness, &grace, &ada, &spouse).await
            + edge_count(&harness, &ada, &grace, &partner).await,
        3,
        "{HARNESS}: three distinct edges exist — two directions plus a second role — so no edge was collapsed"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. NEGATIVE / FAULT — AN EDGE TO A PERSON THAT DOES NOT EXIST IS REFUSED. Both ends are foreign keys, so a
    //    graph cannot grow a node that is not a person.
    // -----------------------------------------------------------------------------------------------------------
    let dangling = sqlx::query(
        "insert into person_person (person_id, related_person_id, role_id, role_scope, source_type)
         values ($1::uuid, gen_random_uuid(), $2::uuid, $3, 'manual') returning id",
    )
    .bind(&ada)
    .bind(&spouse)
    .bind(EDGE_SCOPE)
    .fetch_one(harness.pool())
    .await;
    assert!(
        dangling.is_err(),
        "{HARNESS}: an edge to a person that does not exist is refused — the graph has no dangling nodes"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 6. NEGATIVE / FAULT — THE ROLE MUST BE A `person_person` ROLE. The composite foreign key
    //    `person_person_role_scope_fk (role_id, role_scope) references relation_role(id, scope)` is what forbids
    //    borrowing a firm-scoped or contract-scoped role onto a person-to-person edge — the mistake a plain FK on
    //    `role.id` would have permitted, since `relation_role.id` is unique on its own.
    // -----------------------------------------------------------------------------------------------------------
    if let Some(foreign_role) = foreign_scope_role(&harness).await {
        let out_of_scope = sqlx::query(
            "insert into person_person (person_id, related_person_id, role_id, role_scope, source_type)
             values ($1::uuid, $2::uuid, $3::uuid, $4, 'manual') returning id",
        )
        .bind(&alan)
        .bind(&ada)
        .bind(&foreign_role)
        .bind(EDGE_SCOPE)
        .fetch_one(harness.pool())
        .await;
        assert!(
            out_of_scope.is_err(),
            "{HARNESS}: an edge cannot borrow a role from another scope, even while declaring `person_person`"
        );

        // And the scope column itself is pinned: an edge that declares a foreign scope is refused outright.
        let wrong_scope = sqlx::query(
            "insert into person_person (person_id, related_person_id, role_id, role_scope, source_type)
             values ($1::uuid, $2::uuid, $3::uuid, 'person_firm', 'manual') returning id",
        )
        .bind(&alan)
        .bind(&ada)
        .bind(&spouse)
        .fetch_one(harness.pool())
        .await;
        assert!(
            wrong_scope.is_err(),
            "{HARNESS}: an edge declaring a scope other than `person_person` is refused"
        );
        assert!(
            wrong_scope
                .expect_err("the wrong-scope edge is refused")
                .to_string()
                .contains("person_person_role_scope_check"),
            "{HARNESS}: the wrong-scope refusal names the scope CHECK"
        );
    }

    // -----------------------------------------------------------------------------------------------------------
    // 7. NEGATIVE / FAULT — NO REFUSAL ABOVE LEFT A ROW. Every rejected write must have written nothing, or the
    //    graph would contain edges no rule sanctioned.
    // -----------------------------------------------------------------------------------------------------------
    let alan_edges: i64 =
        sqlx::query_scalar("select count(*) from person_person where person_id = $1::uuid")
            .bind(&alan)
            .fetch_one(harness.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("test-harness.crm.alan_edges", &error))
            .expect("Alan's edge count reads");
    assert_eq!(
        alan_edges, 0,
        "{HARNESS}: Alan holds no edge — every refused write above landed nothing"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 8. EDGES CASCADE WITH THEIR PEOPLE. Both ends are `references person(id) on delete cascade`, so removing a
    //    person removes their edges rather than leaving a row pointing at nobody. This is asserted by counting the
    //    run's edges after one endpoint is removed, so the cascade is observed rather than assumed.
    // -----------------------------------------------------------------------------------------------------------
    let people = [ada.clone(), grace.clone(), alan.clone()];
    assert_eq!(
        all_edges(&harness, &people).await,
        3,
        "{HARNESS}: the three sanctioned edges are the only ones this run holds"
    );

    // Grace sits on all three edges — as the source of `grace → ada` and as the target of the two `ada → grace`
    // rows — so removing her must take every one of them. Counting only the `person_id` side would understate the
    // cascade and miss a dangling edge on the `related_person_id` side, so both sides are counted.
    let touching: i64 = sqlx::query_scalar(
        "select count(*) from person_person
          where person_id = $1::uuid or related_person_id = $1::uuid",
    )
    .bind(&grace)
    .fetch_one(harness.pool())
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm.touching", &error))
    .expect("the touching-edge count reads");
    assert_eq!(
        touching, 3,
        "{HARNESS}: all three edges touch Grace on one side or the other, so the cascade has three rows to remove"
    );

    sqlx::query("delete from person where id = $1::uuid")
        .bind(&grace)
        .execute(harness.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("test-harness.crm.cascade_delete", &error))
        .expect("the endpoint is removed");
    assert_eq!(
        all_edges(&harness, &people).await,
        0,
        "{HARNESS}: removing a person removed every edge that touched them, on both sides of the edge"
    );
    let dangling_after: i64 = sqlx::query_scalar(
        "select count(*) from person_person pp
          where pp.related_person_id = $1::uuid or pp.person_id = $1::uuid",
    )
    .bind(&grace)
    .fetch_one(harness.pool())
    .await
    .map_err(|error| DbFailure::from_sqlx("test-harness.crm.dangling", &error))
    .expect("the dangling-edge count reads");
    assert_eq!(
        dangling_after, 0,
        "{HARNESS}: no edge still references the removed person"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 9. CLEANUP / NO LEFTOVER. The remaining people are deleted (their edges cascade); a non-zero leftover count
    //    fails the proof, because DEV must be left as it was found.
    // -----------------------------------------------------------------------------------------------------------
    let removed = harness
        .cleanup(&marker)
        .await
        .expect("the fixture people are removed");
    assert_eq!(
        removed, 2,
        "{HARNESS}: this run's remaining two people are removed (the third went in the cascade check)"
    );
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("the leftover count reads"),
        0,
        "{HARNESS}: the proof leaves no person or edge behind"
    );
}
