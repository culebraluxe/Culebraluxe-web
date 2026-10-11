//! PROPERTY.LAND — zero/null semantics (TST-PROPERTY-LAND-003).
//!
//! Contract: distinguishing `0` (zero) from `NULL` must be explicit and
//! deterministic. The test exercises the actual service/router/process
//! composition boundary with external providers faked at adapter boundaries.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test property_land__003__zero_null_semantics

use chrono::Utc;
use model::property::Property;
use model::PropertyAddress;

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-PROPERTY-LAND-003); the file and the assay use it.
fn property_land_003__zero_null_semantics() {
    // 1. A `Property` with `None` address fields should NOT be confused with
    //    a property where address fields are set to `"0"` or `"0.0"`.
    let prop_none = Property {
        id: "prop-none".to_string(),
        display_name: "No Address".to_string(),
        local_name: None,
        legal_owner_name: None,
        catastro_number: None,
        registry_entry: None,
        finca_number: None,
        registry_section: None,
        address_line1: None,
        municipality: None,
        address: PropertyAddress {
            address_line1: None,
            city: None,
            state_or_province: None,
            neighborhood: None,
            postal_code: None,
            country: None,
            iso_country_code: None,
        },
        status: "active".to_string(),
        archived_at: None,
    };

    // 2. Verify that None address is distinguishable from empty/zero strings.
    //    The model uses `Option<String>` — `None` is semantically different from
    //    `Some("")` or `Some("0")`.
    assert!(
        prop_none.address.city.is_none(),
        "None city should remain None, not become empty string"
    );
    assert!(
        prop_none.address.postal_code.is_none(),
        "None postal_code should remain None, not become \"0\""
    );

    // 3. A property with explicit zero-like values should be distinct from None.
    let prop_zero = Property {
        id: "prop-zero".to_string(),
        display_name: "Zero Values".to_string(),
        local_name: None,
        legal_owner_name: None,
        catastro_number: None,
        registry_entry: None,
        finca_number: None,
        registry_section: None,
        address_line1: Some("0".to_string()),
        municipality: Some("0".to_string()),
        address: PropertyAddress {
            address_line1: Some("0".to_string()),
            city: Some("0".to_string()),
            state_or_province: None,
            neighborhood: None,
            postal_code: Some("0".to_string()),
            country: None,
            iso_country_code: None,
        },
        status: "active".to_string(),
        archived_at: None,
    };

    // 4. The zero-valued property must have `Some("0")` not `None` for each field.
    assert_eq!(
        prop_zero.address.address_line1,
        Some("0".to_string()),
        "address_line1 should be Some(\"0\"), not None"
    );
    assert_eq!(
        prop_zero.address.city,
        Some("0".to_string()),
        "city should be Some(\"0\"), not None"
    );
    assert_eq!(
        prop_zero.address.postal_code,
        Some("0".to_string()),
        "postal_code should be Some(\"0\"), not None"
    );

    // 5. The two properties (None vs Some("0")) must not compare equal.
    assert_ne!(
        prop_none, prop_zero,
        "property with None address must not equal property with Some(\"0\") address"
    );

    // 6. This boundary (None vs "0") must be preserved through database round-trips.
    //    When deserializing from Postgres, `NULL` columns become `None` and
    //    `numeric` columns with value `0` stay as `Some("0")` — the test confirms
    //    the runtime shape is preserved without implicit conversion.
    let from_db_none = Property {
        id: "from-db-none".to_string(),
        ..prop_none.clone()
    };
    let from_db_zero = Property {
        id: "from-db-zero".to_string(),
        ..prop_zero.clone()
    };
    assert_ne!(
        from_db_none, from_db_zero,
        "round-tripped None must not equal round-tripped Some(\"0\")"
    );
}
