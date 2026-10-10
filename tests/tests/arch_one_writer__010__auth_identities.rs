//! ARCH.ONE_WRITER — auth identities (TST-ARCH-ONE-WRITER-010).
//!
//! Contract: an auth identity is always a known (provider, subject) pair, never
//! an anonymous or self-declared principal. Request handling translates the
//! authenticated provider subject through the startup-warmed identity map
//! (`web/src/security/identity_cache.rs`, bounded at 1024 entries, keyed by the
//! pair); a miss may use the database for a newly provisioned identity, then
//! joins the map. Actions are decided by derivation, not declaration:
//! `policy_domain_for` derives the domain from the action, and `catalog_action`
//! only ever returns an action the catalog knows.
//!
//! Level: L0 Pure — pure calls and file reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_one_writer__010__auth_identities

use test_harness::source;
use web::security::{catalog_action, policy_domain_for};

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-ONE-WRITER-010); the file and the assay use it.
fn arch_one_writer_010__auth_identities() {
    // The domain is derived from the action: a caller cannot relabel itself out of a floor.
    assert_eq!(policy_domain_for("security.users.read"), "security");
    assert_eq!(policy_domain_for("contract.execute"), "contract");
    assert_eq!(policy_domain_for("tech"), "tech");
    assert_eq!(policy_domain_for("luxesign.finalize"), "luxesign");
    assert_eq!(policy_domain_for("something.unknown"), "application");

    // The catalog only ever returns actions it knows; anything else is refused, not defaulted.
    let known = catalog_action("accounting.read");
    assert_eq!(known, Some(("accounting.read", "query")));
    assert_eq!(catalog_action("no.such.action"), None);
    assert_eq!(catalog_action(""), None);

    // The identity map is bounded and keyed by the pair: no anonymous principals, no unbounded growth.
    let cache = source::read(&source::workspace_root().join("web/src/security/identity_cache.rs"));
    assert!(
        cache.contains("const MAX_ENTRIES: usize = 1024;"),
        "the identity map is bounded so a process cannot accumulate principals without limit"
    );
    assert!(
        cache.contains("Mutex<HashMap<(String, String), SecurityPrincipal>>"),
        "identities are keyed by the (provider, subject) pair"
    );
    assert!(
        cache.contains("Only known active identities are held."),
        "the map holds known identities; misses rejoin through the database"
    );
    assert!(
        cache.contains("pub fn get(&self, provider: &str, provider_subject: &str)"),
        "lookup takes both halves of the pair — there is no anonymous lookup"
    );

    // Negative controls: derivation covers the special prefixes and nothing else is special.
    assert_eq!(policy_domain_for("signer.consent"), "signer");
    assert_eq!(policy_domain_for("email.send"), "email");
    assert_eq!(policy_domain_for(""), "application");
}
