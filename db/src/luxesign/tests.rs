//! The vault-version naming rule, moved out of `luxesign.rs` (2026-10-14).
//!
//! Extracted unchanged; the name of this file is the `#[cfg(test)]` fence for the estate scans.

use super::without_vault_version;

#[test]
fn the_vault_version_is_not_part_of_the_name_people_sign() {
    assert_eq!(
        without_vault_version("Listing Agreement v9"),
        "Listing Agreement"
    );
    assert_eq!(
        without_vault_version("Listing Agreement v12"),
        "Listing Agreement"
    );
    assert_eq!(
        without_vault_version("Listing Agreement"),
        "Listing Agreement"
    );
    assert_eq!(without_vault_version("Offer v2 draft"), "Offer v2 draft");
    assert_eq!(without_vault_version(" v3"), " v3");
}
