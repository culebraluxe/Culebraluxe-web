//! PROPERTY.regrid — CSV parsing (TST-PROPERTY-REGRID-001).
//!
//! Contract: the Regrid parcel export lands in `"l_Regrid"` with the source's own field names and types derived
//! from the fields — identifiers are TEXT so leading zeros survive (`szip` 00775 stays 00775, never 775),
//! measures are unconstrained NUMERIC so nothing is rounded, the two JSON-carrying address columns stay TEXT
//! because one source row is malformed JSON in the source itself (typed `jsonb` the load would reject that row),
//! `qoz` is boolean, the `ll_*` ids are uuid, and coordinates are unconstrained numeric (migration 200 widened
//! the `numeric(11,7)` that silently rounded 8-decimal `inside_x`/`inside_y` values).
//!
//! Level: L0 Pure — the production DDL is the artifact that owns every CSV parsing decision in the current Rust
//! estate (the loader itself is retired TypeScript, reference-only; there is no Rust CSV importer to drive, and
//! inventing one would be a second implementation). The assertions read `db/migrations/199_*` and `200_*` as
//! text, so a column typed so the load would corrupt or reject a row fails this test.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_regrid__001__csv_parsing

use std::collections::BTreeMap;
use test_harness::source;

/// SQL code without `--` comments: the migration prose names the old `numeric(11,7)` defect, and prose must not
/// satisfy (or break) a rule about the schema itself.
fn sql_code(text: &str) -> String {
    text.lines()
        .map(|line| line.split("--").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `"column" type` definitions in a `CREATE TABLE` body: column name → declared type token.
fn column_defs(create_table: &str) -> BTreeMap<String, String> {
    let mut defs = BTreeMap::new();
    for line in create_table.lines() {
        let trimmed = line.trim().trim_end_matches(',');
        let Some(rest) = trimmed.strip_prefix('"') else {
            continue;
        };
        let Some(end) = rest.find('"') else { continue };
        let (name, after) = (&rest[..end], rest[end + 1..].trim());
        if name.is_empty()
            || name.contains(|c: char| !(c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit()))
        {
            continue;
        }
        let Some(ty) = after.split_whitespace().next() else {
            continue;
        };
        if matches!(
            ty,
            "text" | "numeric" | "bigint" | "boolean" | "date" | "uuid" | "timestamptz"
        ) || ty.starts_with("numeric(")
        {
            defs.insert(name.to_owned(), ty.to_owned());
        }
    }
    defs
}

/// The landing-table rule for one column: the declared type must be exactly the expected one.
fn landing_column_ok(defs: &BTreeMap<String, String>, column: &str, expected: &str) -> bool {
    defs.get(column).is_some_and(|ty| ty == expected)
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-REGRID-001).
fn property_regrid_001__csv_parsing() {
    let root = source::workspace_root();
    let landing_199 = source::read(&root.join("db/migrations/199_l_regrid_landing.sql"));
    let widen_200 = source::read(&root.join("db/migrations/200_l_regrid_coordinate_precision.sql"));

    // The subject is the real landing table: 138 source fields, so a reader that parsed nothing (or the wrong
    // file) cannot satisfy what follows.
    let defs = column_defs(&landing_199);
    assert_eq!(
        defs.len(),
        138,
        "the landing table mirrors the export's 138 fields; parsed {}",
        defs.len()
    );

    // IDENTIFIERS ARE TEXT, NOT NUMBERS: all-digits in the export, leading zeros meaningful. As integers 00775
    // becomes 775 and the row silently stops joining anything.
    for column in [
        "geoid",
        "parcelnumb_no_formatting",
        "szip",
        "szip5",
        "census_zcta",
        "census_tract",
        "census_block",
        "qoz_tract",
        "direccion_fisica_szip",
        "parcelnumb",
        "num_catastro",
        "catastro",
    ] {
        assert!(
            landing_column_ok(&defs, column, "text"),
            "{column} must land as text so leading zeros survive"
        );
    }

    // MEASURES ARE NUMERIC, unconstrained: areas, values and taxes keep exact precision — a landing table never
    // rounds what it was given.
    for column in [
        "improvval",
        "landval",
        "parval",
        "saleprice",
        "cabida",
        "taxable",
        "ll_gisacre",
        "ll_gissqft",
    ] {
        assert!(
            landing_column_ok(&defs, column, "numeric"),
            "{column} must land as unconstrained numeric"
        );
    }

    // THE JSON-CARRYING COLUMNS ARE TEXT: one `original_address` value is malformed JSON in the source itself
    // (unescaped inner quotes). Typed jsonb the load would reject that row; as text the landing table keeps what
    // the source said, and the consumer parses it explicitly.
    for column in ["original_address", "original_mailing_address"] {
        assert!(
            landing_column_ok(&defs, column, "text"),
            "{column} must stay text so the malformed source row is kept, not rejected"
        );
    }

    // The remaining typed decisions the export profile fixed.
    assert!(landing_column_ok(&defs, "qoz", "boolean"));
    assert!(landing_column_ok(&defs, "ll_uuid", "uuid"));
    assert!(landing_column_ok(&defs, "ll_stack_uuid", "uuid"));
    assert!(landing_column_ok(&defs, "ogc_fid", "bigint"));
    assert!(
        landing_199.contains("\"ogc_fid\""),
        "the export's own unique row id is present"
    );

    // Migration 200: coordinates are unconstrained numeric — a scale is a promise the SOURCE makes, not one an
    // importer may impose. The old `numeric(11,7)` rounded 8-decimal inside_x/inside_y values.
    let code_200 = sql_code(&widen_200);
    for column in ["inside_x", "inside_y", "lat", "lon"] {
        assert!(
            code_200.contains(&format!("alter column \"{column}\" type numeric")),
            "{column} must be widened to unconstrained numeric"
        );
    }
    assert!(
        !code_200.contains("numeric(11,7)")
            && !code_200.contains("numeric(10,7)")
            && !code_200.to_lowercase().contains("numeric("),
        "no typed scale may remain on a coordinate: the class of defect is the type"
    );

    // Negative controls: the detector must refuse the exact corruptions this contract exists to prevent — while
    // staying quiet on the real definitions.
    let mut forged = defs.clone();
    forged.insert("szip".to_owned(), "integer".to_owned());
    assert!(
        !landing_column_ok(&forged, "szip", "text"),
        "an integer zip would silently drop leading zeros and must be refused"
    );
    forged.insert("original_address".to_owned(), "jsonb".to_owned());
    assert!(
        !landing_column_ok(&forged, "original_address", "text"),
        "a jsonb address column would reject the malformed source row and must be refused"
    );
    forged.insert("inside_x".to_owned(), "numeric(11,7)".to_owned());
    assert!(
        !landing_column_ok(&forged, "inside_x", "numeric"),
        "a scaled coordinate would round source precision and must be refused"
    );
    assert!(
        !landing_column_ok(&defs, "no_such_column", "text"),
        "an unknown column must not satisfy the rule"
    );
    assert!(
        landing_column_ok(&defs, "szip", "text"),
        "the real definition passes the same detector"
    );
}
