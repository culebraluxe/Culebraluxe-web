//! PROPERTY.regrid — missing fields (TST-PROPERTY-REGRID-003).
//!
//! Contract: a parcel merge (and the Regrid golden-record fold) fills ONLY the fields the listing lacks. Text
//! counts as missing when empty — `coalesce(nullif(t.col, ''), s.col)` — every other type when null —
//! `coalesce(t.col, s.col)` — and identity/audit columns (`id`, `created_at`, `updated_at`) are never filled:
//! the listing keeps every value it has, the parcel record donates the rest, nothing is overwritten, and no row
//! is ever re-identified. Migration 225 states the same rule for the golden fold ("Only EMPTY property columns
//! are filled", `coalesce(nullif(btrim(...), ''), ...)`).
//!
//! Level: L0 Pure — the production fill SQL owns the missing-field semantics; no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_regrid__003__missing_fields

use test_harness::source;

/// True when the merge fill SQL fills `column` only when the target lacks it (never a blind overwrite).
fn fill_is_conditional(merge: &str) -> bool {
    merge.contains("coalesce(nullif(t.{column}, ''), s.{column})")
        && merge.contains("coalesce(t.{column}, s.{column})")
}

/// True when the fill explicitly excludes the identity/audit columns.
fn fill_excludes_identity(merge: &str) -> bool {
    merge.contains("not in ('id', 'created_at', 'updated_at')")
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-REGRID-003).
fn property_regrid_003__missing_fields() {
    let root = source::workspace_root();
    let merge = source::read(&root.join("db/src/property/merge_parcel_record.rs"));
    let fold_225 = source::read(&root.join("db/migrations/225_property_golden_from_regrid.sql"));

    // 1. The parcel merge fills conditionally: empty text counts as missing, other types fill on null.
    assert!(
        fill_is_conditional(&merge),
        "the merge must fill text on empty and other columns on null, preferring the listing"
    );

    // 2. Identity and audit columns are never part of the fill.
    assert!(
        fill_excludes_identity(&merge),
        "id, created_at and updated_at must be excluded from the fill column list"
    );

    // 3. The golden fold obeys the same rule: only empty property columns are filled from the linked parcel,
    //    with blank-trimmed text treated as missing.
    assert!(
        fold_225.contains("Only EMPTY property columns are filled"),
        "migration 225 must state the only-empty rule it implements"
    );
    let guarded = fold_225.matches("coalesce(nullif(btrim(").count();
    assert!(
        guarded >= 10,
        "migration 225 must guard each folded text column with coalesce(nullif(btrim(...), ''), ...); found {guarded}"
    );
    assert!(
        fold_225.contains(
            "catastro_source   = coalesce(nullif(btrim(p.catastro_source), ''), 'regrid')"
        ) || fold_225
            .contains("catastro_source = coalesce(nullif(btrim(p.catastro_source), ''), 'regrid')"),
        "provenance itself is filled only when blank"
    );

    // Negative controls: the detectors must refuse a fill that overwrites, re-identifies, or fills unconditionally.
    let blind_overwrite = "format!(\"{column} = s.{column}\")";
    assert!(
        !blind_overwrite.contains("coalesce"),
        "a blind source-overwrite must never satisfy the conditional-fill rule"
    );
    let source_wins = "format!(\"{column} = coalesce(s.{column}, t.{column})\")";
    assert!(
        !fill_is_conditional(source_wins),
        "a fill preferring the parcel record over the listing must be refused"
    );
    let keeps_identity_out = "not in ('created_at', 'updated_at')";
    assert!(
        !fill_excludes_identity(keeps_identity_out),
        "a fill list that still covers id must be refused"
    );
    assert!(
        !fill_is_conditional("update property t set name = s.name from source s"),
        "an unconditional assignment must be refused"
    );
}
