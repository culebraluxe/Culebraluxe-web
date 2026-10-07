//! PROPERTY.schema_parity — DB writable columns vs OPS editable fields (TST-PROPERTY-SCHEMA-PARITY-001).
//!
//! Contract: the property write path is one unbroken 1:1 chain, read from its own metadata at test time —
//! OPS FieldSpec keys (camelCase) → `SavePropertyAdminBody` JSON names → `SavePropertyAdminRequest` fields →
//! `admin_save` SET columns (+ the stellar upsert for the MLS extension). Every OPS control writes a field the
//! server accepts; every accepted field is forwarded field-for-field by `apply_property_admin_save`; every
//! request field lands on a same-named column; every written column comes from a same-named request field —
//! except the documented derivations: `is_active_listing`/`is_published` mirror `status` (STATUS IS THE ONLY
//! SWITCH), `archived_at` folds `status = 'archived'`/`archived`, and `updated_at` is `now()`. Provenance and
//! enrichment (`source_*`, `regrid_*`) are writable nowhere on this path: assignment and import processes own
//! those writes.
//!
//! Level: L3 Composition — the UI metadata, route body, service request and DAO SQL composed as one contract.
//! Filesystem reads only; no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_schema_parity__001__property_db_writable_columns_and_ops_editable_fields_maintain_exact_1_1_parity_from_metadata

use std::collections::BTreeSet;
use test_harness::source;

/// True for a provenance/enrichment field: assignment and import processes own these writes, never OPS.
fn is_provenance(field: &str) -> bool {
    field.starts_with("source_") || field.starts_with("regrid_") || field == "catastro_source"
}

/// camelCase OPS key → snake_case column/field name (`listPrice` → `list_price`).
fn snake(key: &str) -> String {
    let mut out = String::new();
    for ch in key.chars() {
        if ch.is_ascii_uppercase() {
            out.push('_');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Field names of `pub struct NAME` / `pub(super) struct NAME` in `text`.
fn struct_fields(text: &str, name: &str) -> BTreeSet<String> {
    let start = text
        .find(&format!("struct {name}"))
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

/// `key: "..."` values inside `text`.
fn field_keys(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter_map(|line| {
            let (_, rest) = line.split_once("key:")?;
            let rest = rest.trim().trim_end_matches(',').trim();
            let key = rest.strip_prefix('"')?.strip_suffix('"')?;
            Some(key.to_owned())
        })
        .collect()
}

/// The `const NAME = &[FieldSpec] = &[ ... ];` block in `text`.
fn const_block<'a>(text: &'a str, name: &str) -> &'a str {
    let start = text
        .find(&format!("const {name}"))
        .unwrap_or_else(|| panic!("const {name} must exist"));
    let tail = &text[start..];
    let end = tail
        .find("\n];")
        .unwrap_or_else(|| panic!("const {name} must terminate"));
    &tail[..end]
}

/// `SET` targets of the `admin_save` UPDATE (`name = ...` before `where id =`).
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

/// Columns of the `insert into property_stellar_listing ( ... ) values` list.
fn stellar_columns(text: &str) -> BTreeSet<String> {
    let start = text
        .find("insert into property_stellar_listing (")
        .expect("the stellar upsert must exist");
    let tail = &text[start..];
    let end = tail
        .find(") values")
        .expect("the column list must terminate");
    let list = &tail["insert into property_stellar_listing (".len()..end];
    list.split(',')
        .map(|c| c.trim().to_owned())
        .filter(|c| !c.is_empty())
        .collect()
}

/// `body.field` occurrences inside `apply_property_admin_save` — what the bridge actually forwards.
fn forwarded_fields(text: &str) -> BTreeSet<String> {
    let start = text
        .find("pub(in super::super) async fn apply_property_admin_save")
        .expect("the save bridge must exist");
    let tail = &text[start..];
    let end = tail.find("\n}\n").unwrap_or(tail.len());
    let body = &tail[..end];
    let mut out = BTreeSet::new();
    let mut rest = body;
    while let Some(i) = rest.find("body.") {
        rest = &rest[i + "body.".len()..];
        let len = rest
            .find(|c: char| !(c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit()))
            .unwrap_or(rest.len());
        if len > 0 {
            out.insert(rest[..len].to_owned());
        }
    }
    out
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-SCHEMA-PARITY-001).
fn property_schema_parity_001__property_db_writable_columns_and_ops_editable_fields_maintain_exact_1_1_parity_from_metadata(
) {
    let root = source::workspace_root();
    let bodies = source::read(&root.join("web/src/api/routes/bodies.rs"));
    let bridge = source::read(&root.join("web/src/api/routes/people_properties.rs"));
    let model = source::read(&root.join("middle/model/src/property.rs"));
    let dao = source::read(&root.join("db/src/property/merge_parcel_record.rs"));
    let property_fields =
        source::read(&root.join("web/ui/src/app/screens/workbench/view/property_fields.rs"));
    let listing_fields =
        source::read(&root.join("web/ui/src/app/screens/workbench/view/listing_fields.rs"));

    // Metadata, parsed — each side must have been found, or a silently empty set would prove a false parity.
    let body_fields = struct_fields(&bodies, "SavePropertyAdminBody");
    let request_fields = struct_fields(&model, "SavePropertyAdminRequest");
    let stellar_fields = struct_fields(&model, "PropertyStellarDetails");
    assert!(
        body_fields.len() >= 80,
        "the save body must have been parsed ({} fields)",
        body_fields.len()
    );
    assert!(
        request_fields.contains("property_id"),
        "the request must have been parsed"
    );
    assert_eq!(
        stellar_fields.len(),
        15,
        "the MLS extension must have its 15 fields"
    );

    // 1. OPS controls → server body: every property FieldSpec key and every website key is an accepted body
    //    field; every MLS key is an accepted stellar field. An OPS control that writes nowhere would pass the
    //    UI and die (or worse, silently drop) at the server.
    let mut ui_property_keys = field_keys(&property_fields);
    ui_property_keys.extend(field_keys(const_block(&listing_fields, "WEBSITE_FIELDS")));
    assert!(
        ui_property_keys.len() >= 60,
        "the OPS property keys must have been parsed"
    );
    let unmapped: Vec<String> = ui_property_keys
        .iter()
        .filter(|k| !body_fields.contains(&snake(k)))
        .cloned()
        .collect();
    assert!(
        unmapped.is_empty(),
        "every OPS property control must map to a server body field: {unmapped:?}"
    );
    let mls_keys = field_keys(const_block(&listing_fields, "MLS_FIELDS"));
    assert_eq!(
        mls_keys.len(),
        15,
        "the MLS section must declare its 15 keys"
    );
    let unmapped_mls: Vec<String> = mls_keys
        .iter()
        .filter(|k| !stellar_fields.contains(&snake(k)))
        .cloned()
        .collect();
    assert!(
        unmapped_mls.is_empty(),
        "every MLS control must map to a stellar field: {unmapped_mls:?}"
    );

    // 2. Server body → service request: exact 1:1 modulo the id, which arrives in the URL path, not the body.
    let mut expected_request: BTreeSet<String> = body_fields.clone();
    expected_request.insert("property_id".to_owned());
    expected_request.insert("stellar".to_owned());
    assert_eq!(
        request_fields, expected_request,
        "the service request must mirror the server body plus the path id"
    );

    // 3. The bridge forwards field-for-field: no accepted field is dropped and none is synthesized.
    let forwarded = forwarded_fields(&bridge);
    let mut expected_forwarded = request_fields.clone();
    expected_forwarded.remove("property_id");
    assert_eq!(
        forwarded, expected_forwarded,
        "apply_property_admin_save must forward every request field from the body"
    );

    // 4. Service request → DB columns: exact 1:1 modulo the documented derivations.
    let targets = save_targets(&dao);
    assert!(
        targets.len() >= 80,
        "the save targets must have been parsed"
    );
    let mut expected_targets: BTreeSet<String> = request_fields.clone();
    expected_targets.remove("property_id");
    expected_targets.remove("stellar");
    // `archived` folds into `archived_at` through the case expression pinned below — it is not a column.
    expected_targets.remove("archived");
    expected_targets.insert("archived_at".to_owned());
    expected_targets.insert("updated_at".to_owned());
    assert_eq!(
        targets, expected_targets,
        "admin_save must write exactly the request fields plus archived_at/updated_at"
    );
    // The derivations: status is the only switch, archiving folds through it, updated_at is now().
    assert!(
        dao.contains("is_active_listing = ($4 in ('active', 'under_contract', 'sold'))"),
        "is_active_listing must mirror status, not the shadowed request field"
    );
    assert!(
        dao.contains("is_published = ($4 in ('active', 'under_contract', 'sold'))"),
        "is_published must mirror status, not the shadowed request field"
    );
    assert!(
        dao.contains("archived_at = case when $4 = 'archived' or $46"),
        "archived_at must fold status/archived, never a direct write"
    );

    // 5. MLS extension: stellar fields → stellar columns 1:1 modulo the property key.
    let columns = stellar_columns(&dao);
    let mut expected_columns: BTreeSet<String> = stellar_fields.clone();
    expected_columns.insert("property_id".to_owned());
    assert_eq!(
        columns, expected_columns,
        "the stellar upsert must carry exactly the extension fields plus the key"
    );
    for column in &stellar_fields {
        assert!(
            dao.contains(&format!("{column} = excluded.{column}")),
            "the stellar upsert must refresh {column} on conflict"
        );
    }

    // 6. Provenance and enrichment are writable nowhere on this path.
    for (side, set) in [
        ("request", &request_fields),
        ("body", &body_fields),
        ("save targets", &targets),
    ] {
        let owned: Vec<&String> = set.iter().filter(|f| is_provenance(f)).collect();
        assert!(
            owned.is_empty(),
            "{side} must own no provenance/enrichment writes: {owned:?}"
        );
    }
    assert!(
        !targets.contains("id") && !targets.contains("created_at"),
        "identity/audit columns are never save targets"
    );

    // Negative controls: each link's detector must refuse a smuggled or dropped field.
    assert!(
        !body_fields.contains(&snake("regridOwnerName")),
        "an enrichment key must have no body field to land on"
    );
    let mut forged_body = body_fields.clone();
    forged_body.insert("regrid_owner_name".to_owned());
    let mut forged_expected = forged_body.clone();
    forged_expected.insert("property_id".to_owned());
    forged_expected.insert("stellar".to_owned());
    assert_ne!(
        request_fields, forged_expected,
        "the body/request detector must catch a smuggled enrichment field"
    );
    let mut forged_targets = targets.clone();
    forged_targets.insert("catastro_source".to_owned());
    assert_ne!(
        forged_targets, expected_targets,
        "the request/column detector must catch a smuggled provenance write"
    );
    let mut dropped = expected_targets.clone();
    dropped.remove("view_description");
    assert_ne!(
        targets, dropped,
        "the detector must catch a dropped writable column"
    );
}
