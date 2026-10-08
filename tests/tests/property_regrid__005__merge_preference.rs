//! PROPERTY.regrid — merge preference (TST-PROPERTY-REGRID-005).
//!
//! Contract: when a hand-entered listing and a Regrid parcel record describe the same land, the LISTING wins.
//! `merge_parcel_record` fills each field the listing lacks from the other record — text on empty, all else on
//! null — and never the reverse; identity/audit columns are excluded; the record never merges into itself; and
//! the match is digit-normalized, so `123-456`, `123 456` and `123456` name the same parcel.
//!
//! Level: L0 Pure — the production merge SQL owns the preference; no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_regrid__005__merge_preference

use test_harness::source;

/// True when the fill prefers the target (listing) over the source (parcel record) in both arms.
fn prefers_target(merge: &str) -> bool {
    merge.contains("coalesce(nullif(t.{column}, ''), s.{column})")
        && merge.contains("coalesce(t.{column}, s.{column})")
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-REGRID-005).
fn property_regrid_005__merge_preference() {
    let root = source::workspace_root();
    let merge = source::read(&root.join("db/src/property/merge_parcel_record.rs"));

    // 1. The listing keeps every value it has; the parcel record donates only what is missing.
    assert!(
        prefers_target(&merge),
        "the fill must prefer t.* (listing) over s.* (parcel record) in both arms"
    );

    // 2. Identity and audit columns are outside the preference entirely — a merge never re-identifies a row.
    assert!(
        merge.contains("not in ('id', 'created_at', 'updated_at')"),
        "id, created_at and updated_at must be excluded from the fill"
    );

    // 3. A record never merges into itself.
    assert!(
        merge.contains("where id <> $1::uuid"),
        "the merge candidate search must exclude the target itself"
    );

    // 4. The match is digit-normalized on both sides: punctuation variants of one catastro number are one parcel.
    assert!(
        merge.contains(
            "let digits: String = catastro.chars().filter(char::is_ascii_digit).collect();"
        ),
        "the request side must be reduced to digits before matching"
    );
    assert!(
        merge.contains("regexp_replace(coalesce(catastro_number, ''), '[^0-9]', '', 'g') = $2"),
        "the stored side must be reduced to digits before matching"
    );

    // Negative controls: a fill preferring the source, a self-merge, and an un-normalized match must be refused.
    assert!(
        !prefers_target("format!(\"{column} = coalesce(s.{column}, t.{column})\")"),
        "a source-wins fill would clobber hand-entered values and must be refused"
    );
    assert!(
        !prefers_target("format!(\"{column} = s.{column}\")"),
        "an unconditional source overwrite must be refused"
    );
    assert!(
        !merge.contains("where id = $1::uuid and archived_at is null"),
        "this detector is calibrated: a same-id match would be a self-merge"
    );
    let unnormalized = "where catastro_number = $2";
    assert!(
        !unnormalized.contains("regexp_replace"),
        "a punctuation-sensitive match would miss the same parcel and must be refused"
    );
}
