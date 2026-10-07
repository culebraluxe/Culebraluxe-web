//! PROPERTY.regrid — source attribution (TST-PROPERTY-REGRID-006).
//!
//! Contract: every property carries its provenance, and provenance is never relabelled. `admin_get` builds
//! `source_metadata` from exactly the system/provenance columns (owner id, timestamps, source triplet,
//! sync stamps, `catastro_source`, legacy measures); the Regrid fold records `catastro_source = 'regrid'` only
//! when it fills a previously blank value — existing manual/imported values are never relabelled; and the OPS
//! write path owns NONE of it — `SavePropertyAdminRequest` has no `source_*`/`regrid_*` field and `admin_save`
//! assigns no `source_*`/`regrid_*` column, so assignment and import processes own those writes alone.
//!
//! Level: L0 Pure — the production projection, fold and write-path shape own the attribution; no database.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_regrid__006__source_attribution

use std::collections::BTreeSet;
use test_harness::source;

/// True for a provenance/enrichment field: assignment and import processes own these writes, never OPS.
fn is_provenance(field: &str) -> bool {
    field.starts_with("source_") || field.starts_with("regrid_") || field == "catastro_source"
}

/// The provenance keys `admin_get` must build into `source_metadata` — and no others.
const PROVENANCE_KEYS: [&str; 12] = [
    "id",
    "listing_user_id",
    "created_at",
    "updated_at",
    "source_type",
    "source_provider",
    "source_listing_key",
    "source_modified_at",
    "last_synced_at",
    "catastro_source",
    "lot_size",
    "lot_size_units",
];

/// Field names of `pub struct NAME { ... }` in `text` (struct body to the first closing brace at column 0).
fn struct_fields(text: &str, name: &str) -> BTreeSet<String> {
    let start = text
        .find(&format!("pub struct {name}"))
        .unwrap_or_else(|| panic!("struct {name} must exist"));
    let body = &text[start..];
    let end = body
        .find("\n}")
        .unwrap_or_else(|| panic!("struct {name} must terminate"));
    body[..end]
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let rest = trimmed
                .strip_prefix("pub(super) ")
                .or_else(|| trimmed.strip_prefix("pub(crate) "))
                .or_else(|| trimmed.strip_prefix("pub "))?;
            let field = rest.split(':').next()?.trim();
            if field.is_empty() || field.contains(' ') {
                return None;
            }
            Some(field.to_owned())
        })
        .collect()
}

/// `SET` targets of the `admin_save` UPDATE in `text` (`name = ...` assignments before `where id =`).
fn save_targets(text: &str) -> BTreeSet<String> {
    let save = text
        .find("pub async fn admin_save")
        .expect("admin_save must exist");
    let update = text[save..]
        .find("update property\n")
        .map(|i| save + i)
        .expect("admin_save must update property");
    let end = text[update..]
        .find("where id = $1::uuid")
        .map(|i| update + i)
        .expect("the save must be keyed by id");
    text[update..end]
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim().trim_end_matches(',');
            let (name, _) = trimmed.split_once('=')?;
            let name = name.trim();
            if name.is_empty()
                || name
                    .contains(|c: char| !(c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit()))
            {
                return None;
            }
            Some(name.to_owned())
        })
        .collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-REGRID-006).
fn property_regrid_006__source_attribution() {
    let root = source::workspace_root();
    let database = source::read(&root.join("db/src/property/database.rs"));
    let merge = source::read(&root.join("db/src/property/merge_parcel_record.rs"));
    let model = source::read(&root.join("middle/model/src/property.rs"));
    let fold_225 = source::read(&root.join("db/migrations/225_property_golden_from_regrid.sql"));

    // 1. The attribution map carries exactly the provenance keys — a canonical fact cannot hide in it, and no
    //    provenance fact is missing from it.
    for key in PROVENANCE_KEYS {
        assert!(
            database.contains(&format!("'{key}', p.")),
            "source_metadata must record {key}"
        );
    }
    assert!(
        !database.contains("'regrid_', p.") && !database.contains("'name', p.name"),
        "source_metadata must not absorb enrichment or canonical columns"
    );

    // 2. The Regrid fold labels the catastro source only when it fills a blank — never over a manual value.
    assert!(
        fold_225.contains(
            "catastro_source   = coalesce(nullif(btrim(p.catastro_source), ''), 'regrid')"
        ) || fold_225
            .contains("catastro_source = coalesce(nullif(btrim(p.catastro_source), ''), 'regrid')"),
        "catastro_source must be set conditionally on blank, never unconditionally"
    );
    assert!(
        !fold_225.contains("catastro_source   = 'regrid'")
            && !fold_225.contains("catastro_source = 'regrid'"),
        "an unconditional relabel would rewrite manual provenance and must be absent"
    );

    // 3. The OPS write path owns no provenance: the request carries no source_*/regrid_* field and the save
    //    assigns no source_*/regrid_* column. System and imported columns are visible in OPS without becoming
    //    writable input.
    let request_fields = struct_fields(&model, "SavePropertyAdminRequest");
    assert!(
        request_fields.contains("property_id"),
        "the request struct must have been parsed (property_id found)"
    );
    let owned: Vec<&String> = request_fields.iter().filter(|f| is_provenance(f)).collect();
    assert!(
        owned.is_empty(),
        "OPS must not write provenance: unexpected request fields {owned:?}"
    );
    let targets = save_targets(&merge);
    assert!(
        targets.contains("name"),
        "the save targets must have been parsed (name found)"
    );
    let written: Vec<&String> = targets.iter().filter(|t| is_provenance(t)).collect();
    assert!(
        written.is_empty(),
        "admin_save must not assign provenance columns: {written:?}"
    );

    // Negative controls: an unconditional relabel and a writable provenance field must both be refused.
    let forged_fold = "catastro_source = 'regrid',";
    assert!(
        !forged_fold.contains("coalesce(nullif(btrim(p.catastro_source), '')"),
        "the conditional-label detector must refuse an unconditional relabel"
    );
    let mut forged_request = request_fields.clone();
    forged_request.insert("catastro_source".to_owned());
    let forged_owned: Vec<&String> = forged_request.iter().filter(|f| is_provenance(f)).collect();
    assert_eq!(
        forged_owned,
        vec!["catastro_source"],
        "the writable-provenance detector must catch a smuggled provenance field"
    );
}
