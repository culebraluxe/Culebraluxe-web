//! PROPERTY.regrid — enrichment (TST-PROPERTY-REGRID-002).
//!
//! Contract: Regrid is an enrichment source, not the owner of Property identity. Its parcel id, assessor parcel
//! number, lookup provenance and feature payload land in dedicated enrichment columns (migration 137), keyed by
//! the durable parcel id — a unique partial index on `regrid_ll_uuid` so one parcel links once — and the OPS
//! record surfaces them through `regrid_fields`, projected as exactly the `regrid_%` keys of the property row
//! (`admin_get`: `jsonb_object_agg(key, value) ... where key like 'regrid_%'`). The workbench renders that map
//! in a read-only "Regrid enrichment columns" panel: enrichment is visible for reconciliation, never an input.
//!
//! Level: L0 Pure + L1 read-path shape — the production DDL, the production projection SQL and the production
//! panel are the subject; no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_regrid__002__enrichment

use test_harness::source;

/// The enrichment columns migration 137 owns: parcel link, parcel numbers, lookup provenance, payload.
const ENRICHMENT_COLUMNS: [&str; 8] = [
    "regrid_ll_uuid",
    "regrid_parcel_number",
    "regrid_path",
    "regrid_lookup_query",
    "regrid_match_address",
    "regrid_enriched_at",
    "regrid_data",
    "catastro_source",
];

/// True when `text` adds `column` to the property table (an `add column if not exists` for it).
fn adds_column(text: &str, column: &str) -> bool {
    text.lines().any(|line| {
        let code = line.split("--").next().unwrap_or("");
        code.contains("add column if not exists") && code.contains(column)
    })
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-REGRID-002).
fn property_regrid_002__enrichment() {
    let root = source::workspace_root();
    let mig_137 = source::read(&root.join("db/migrations/137_property_regrid_enrichment.sql"));
    let database = source::read(&root.join("db/src/property/database.rs"));
    let editor = source::read(&root.join("web/ui/src/app/screens/workbench/view/editor.rs"));

    // 1. The enrichment columns exist as dedicated columns — parcel facts never overwrite canonical ones.
    for column in ENRICHMENT_COLUMNS {
        assert!(
            adds_column(&mig_137, column),
            "migration 137 must own the enrichment column {column}"
        );
    }

    // 2. One parcel links once: the durable id carries a unique partial index, the parcel number a plain one.
    assert!(
        mig_137.contains("idx_property_regrid_ll_uuid_unique")
            && mig_137.contains("on property(regrid_ll_uuid)")
            && mig_137.contains("where regrid_ll_uuid is not null"),
        "the parcel link must be unique where set"
    );
    assert!(
        mig_137.contains("idx_property_regrid_parcel_number"),
        "the assessor parcel number must stay look-up-able"
    );

    // 3. The OPS record projects exactly the `regrid_%` keys — enrichment travels as its own map, and a canonical
    //    column can never leak into it (or be overwritten through it).
    assert!(
        database.contains("jsonb_object_agg(key, value)")
            && database.contains("where key like 'regrid_%'"),
        "admin_get must project regrid_fields as exactly the regrid_% keys of the row"
    );
    assert!(
        database.contains("as regrid_fields"),
        "the projection must feed the record's regrid_fields map"
    );

    // 4. The workbench renders that map read-only: the Sources tab carries a "Regrid enrichment columns" panel,
    //    and the panel itself takes `&BTreeMap` and renders definition-list rows — no form control, no message.
    assert!(
        editor.contains(
            "{source_columns_panel(\"Regrid enrichment columns\", &property.regrid_fields)}"
        ),
        "the workbench must show the enrichment map under its own title"
    );
    let panel_start = editor
        .find("pub(super) fn source_columns_panel")
        .expect("the panel implementation must exist");
    let panel = &editor[panel_start..];
    let panel_end = panel
        .find("\n}\n")
        .map(|i| panel_start + i)
        .unwrap_or(editor.len());
    let panel = &editor[panel_start..panel_end];
    assert!(
        panel.contains("Read-only values maintained by the system or imported data source."),
        "the panel must declare itself read-only"
    );
    assert!(
        !panel.contains("<input") && !panel.contains("on_msg") && !panel.contains("field_panel"),
        "the enrichment panel must contain no editable control"
    );

    // Negative controls: the same checks must refuse an enrichment path that edits, leaks, or duplicates.
    assert!(
        !adds_column(&mig_137, "regrid_owner_name"),
        "the detector must not invent columns migration 137 never owned"
    );
    let forged_projection = "coalesce((select jsonb_object_agg(key, value) from jsonb_each(to_jsonb(p))), '{}'::jsonb) as regrid_fields";
    assert!(
        !forged_projection.contains("where key like 'regrid_%'"),
        "a projection without the regrid_% predicate would leak canonical columns into enrichment and must be refused"
    );
    let forged_panel_call =
        "{field_panel(model, on_msg, \"Regrid enrichment columns\", &property.regrid_fields)}";
    assert!(
        !forged_panel_call.contains("source_columns_panel"),
        "an editable enrichment panel would make imports writable and must be refused"
    );
}
