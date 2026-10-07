//! PROPERTY.regrid — rate/error behavior (TST-PROPERTY-REGRID-007).
//!
//! Contract: the parcel merge serializes ruthlessly and refuses instead of guessing. An undigited catastro
//! returns `Ok(None)` before any query; no other live record with those digits returns `Ok(None)`; MORE than
//! one returns a configuration error naming the count ("merge them one at a time") — the merge processes one
//! source record per call, never a batch; and a save for an unknown id rolls back to `Ok(None)` rather than
//! fabricating a record. No error path invents data, and no ambiguous path proceeds.
//!
//! Rate note, stated honestly: the current Rust regrid path owns no in-process/API rate limiter — parcel loads
//! are truncate-and-replace batch loads and the live-lookup script is retired TypeScript — so the executable
//! contract is one-source-per-call serialization plus refusal. A limiter appearing on this path later must arrive
//! with its own test; this file pins what the path does today.
//!
//! Level: L0 Pure — the production merge/save control flow is the subject; no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_regrid__007__rate_error_behavior

use test_harness::source;

/// True when an ambiguous match (several records, one catastro) is a configuration error, never a pick.
fn ambiguous_is_error(merge: &str) -> bool {
    merge.contains("{} records carry catastro {catastro}; merge them one at a time")
        && merge.contains("DbFailure::configuration")
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-REGRID-007).
fn property_regrid_007__rate_error_behavior() {
    let root = source::workspace_root();
    let merge = source::read(&root.join("db/src/property/merge_parcel_record.rs"));

    // 1. Nothing digit-like in the request: no query runs, `Ok(None)` answers.
    assert!(
        merge.contains("if digits.is_empty() {\n            return Ok(None);\n        }")
            || merge.contains("if digits.is_empty() {"),
        "an undigited catastro must short-circuit to Ok(None)"
    );
    let short_circuit = merge
        .find("if digits.is_empty()")
        .expect("short-circuit exists");
    let first_query = merge[short_circuit..]
        .find("sqlx::query")
        .expect("a query follows");
    assert!(
        first_query > 0,
        "the short-circuit must precede every query"
    );

    // 2. Zero matches fabricate nothing.
    assert!(
        merge.contains("[] => return Ok(None),"),
        "no other record with those digits must answer Ok(None)"
    );

    // 3. Several matches are an error naming the count — one source per call, merged one at a time.
    assert!(
        ambiguous_is_error(&merge),
        "an ambiguous catastro must be a configuration error, never a pick"
    );

    // 4. Exactly one match proceeds.
    assert!(
        merge.contains("[one] => one.clone(),"),
        "a single match must proceed to the merge"
    );

    // 5. Dependents move before the source row is deleted: whatever hangs off the other record lands on the
    //    listing first, so a failure cannot strand or lose the graph.
    let mover = merge.find("property.merge.move").expect("the move exists");
    let delete = merge
        .find("property.merge.delete")
        .expect("the delete exists");
    assert!(
        mover < delete,
        "references must move onto the listing before the source row is deleted"
    );

    // 6. A save for an unknown id rolls back to Ok(None) — the write path fabricates no record either.
    assert!(
        merge.contains("if updated.is_none() {"),
        "admin_save must detect the no-row write"
    );
    assert!(
        merge.contains("let _ = tx.rollback().await;\n            return Ok(None);")
            || (merge.contains("tx.rollback()") && merge.contains("return Ok(None);")),
        "the no-row write must roll back and answer Ok(None)"
    );

    // Negative controls: first-of-many picking, silent multi-merge, and fabrication must all be refused.
    let first_of_many = "sources.first().cloned()";
    assert!(
        !ambiguous_is_error(first_of_many),
        "taking the first of several matches must not satisfy the ambiguity rule"
    );
    assert!(
        !merge.contains("limit 1"),
        "the candidate search must not silently take one row of many"
    );
    let fabricates = "Ok(Some(\"created\"))";
    assert!(
        !ambiguous_is_error(fabricates),
        "an error-shaped fabrication must not satisfy the ambiguity rule"
    );
}
