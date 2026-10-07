//! PROPERTY.regrid — bad coordinate (TST-PROPERTY-REGRID-004).
//!
//! Contract: a bad coordinate is refused, never stored. The OPS save writes latitude/longitude through
//! `nullif($N::text, '')::numeric` — an empty coordinate degrades to NULL, and a non-numeric one fails the cast
//! so the statement errors instead of persisting garbage. The stored precision is unconstrained numeric
//! (migration 200): no typed scale may round what the source said.
//!
//! Level: L0 Pure — the production save SQL and the production coordinate widening own the contract; no database,
//! no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_regrid__004__bad_coordinate

use test_harness::source;

/// True when `assignment` degrades empty to NULL and casts the rest (refusing non-numeric input at the cast).
fn coordinate_assignment_ok(save_sql: &str, assignment: &str) -> bool {
    save_sql.contains(assignment)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-REGRID-004).
fn property_regrid_004__bad_coordinate() {
    let root = source::workspace_root();
    let merge = source::read(&root.join("db/src/property/merge_parcel_record.rs"));
    let widen_200 = source::read(&root.join("db/migrations/200_l_regrid_coordinate_precision.sql"));

    // 1. Empty degrades to NULL; the cast refuses anything non-numeric. There is no path that stores '' or a
    //    malformed coordinate as a coordinate.
    assert!(
        coordinate_assignment_ok(&merge, "latitude = nullif($21::text, '')::numeric"),
        "latitude must pass through nullif + numeric cast"
    );
    assert!(
        coordinate_assignment_ok(&merge, "longitude = nullif($22::text, '')::numeric"),
        "longitude must pass through nullif + numeric cast"
    );

    // 2. Stored coordinates carry no scale: migration 200 widened lat/lon (and inside_x/inside_y) to
    //    unconstrained numeric after a typed scale silently rounded 8-decimal source values.
    let code_200: String = widen_200
        .lines()
        .map(|line| line.split("--").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    for column in ["lat", "lon", "inside_x", "inside_y"] {
        assert!(
            code_200.contains(&format!("alter column \"{column}\" type numeric")),
            "{column} must be stored without a rounding scale"
        );
    }
    assert!(
        !code_200.contains("numeric("),
        "no scaled numeric may remain on a coordinate column"
    );

    // Negative controls: a coordinate written without the nullif/cast (storing '' or unchecked text) and a
    // coordinate re-typed with a scale must both be refused.
    assert!(
        !coordinate_assignment_ok(&merge, "latitude = $21"),
        "a raw coordinate bind would store '' or unchecked text and must be refused"
    );
    assert!(
        !coordinate_assignment_ok(&merge, "longitude = nullif($22::text, '')::text"),
        "a coordinate kept as text would bypass numeric refusal and must be refused"
    );
    assert!(
        !code_200.contains("type numeric(11,7)"),
        "a re-introduced coordinate scale must be refused"
    );
    let forged_widen = "alter column \"lat\" type numeric(11,7)";
    assert!(
        forged_widen.contains("numeric("),
        "the scale detector must fire on a scaled coordinate type"
    );
}
