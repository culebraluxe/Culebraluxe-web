//! ARCH.BOUNDARY — Security owns role resolution (TST-ARCH-BOUNDARY-009).
//!
//! Contract: turning a user's role codes into a level, and choosing their canonical primary role, is one rule in one
//! place — `middle/model/src/security.rs:83-140`. Nothing else in the system may decide it: a second resolver is
//! how a screen ends up granting access the API refuses, and this port has already paid for one duplicated adjudicator.
//! The two functions are pure, so the rule can be exercised directly, and three properties matter more than the happy
//! path:
//!
//! - **Level is a maximum, and order does not matter.** `["user", "root"]` and `["root", "user"]` are the same actor.
//! - **Unknown roles fail closed.** An unrecognised (or misspelled) role code can only ever be `Guest`; a typo must
//!   never raise an actor, and must never produce a primary role.
//! - **Every canonical code resolves, and nothing but a canonical code does.**
//!
//! Level: L0 Pure — the domain's own functions, no I/O.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_boundary__009__security_owns_role_resolution

use model::security::{
    canonical_primary_role, is_root_only, resolve_security_level, SecurityLevel,
    CANONICAL_INTERNAL_ROLE_CODES, ROOT_ONLY_ACTIONS,
};

/// The role codes an actor holds, as the resolver takes them.
fn roles(codes: &[&str]) -> Vec<String> {
    codes.iter().map(|code| code.to_string()).collect()
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-BOUNDARY-009); the file and the assay use it.
fn arch_boundary_009__security_owns_role_resolution() {
    // 1. No roles at all is a guest, not an error and not an owner.
    assert_eq!(
        resolve_security_level(&roles(&[])),
        SecurityLevel::Guest,
        "an actor with no role is a guest"
    );

    // 2. The level is the maximum of the codes held, whatever order they arrive in.
    assert_eq!(
        resolve_security_level(&roles(&["user"])),
        SecurityLevel::User
    );
    assert_eq!(
        resolve_security_level(&roles(&["root"])),
        SecurityLevel::Root
    );
    assert_eq!(
        resolve_security_level(&roles(&["business_power_user"])),
        SecurityLevel::BusinessPowerUser
    );
    assert_eq!(
        resolve_security_level(&roles(&["user", "root"])),
        resolve_security_level(&roles(&["root", "user"])),
        "the resolver is order-independent"
    );
    assert_eq!(
        resolve_security_level(&roles(&["guest", "user", "owner"])),
        SecurityLevel::Root,
        "an owner holds root in this system"
    );
    assert_eq!(
        resolve_security_level(&roles(&["user", "business_power"])),
        SecurityLevel::BusinessPowerUser,
        "the alias `business_power` resolves to the same level as its canonical code"
    );

    // 3. FAIL CLOSED. A role the resolver does not know cannot raise an actor above a guest, and cannot be a primary
    //    role. This is the case a permissive resolver would get wrong, and it is the one that matters.
    assert_eq!(
        resolve_security_level(&roles(&[" ROOT "])),
        SecurityLevel::Root,
        "case and surrounding whitespace are normalised, so a padded code is the role it spells"
    );
    for unknown in ["owner_typo", "superuser", "admin", ""] {
        assert_eq!(
            resolve_security_level(&roles(&[unknown])),
            SecurityLevel::Guest,
            "an unrecognised role code ({unknown:?}) must fail closed to GUEST"
        );
        assert!(
            canonical_primary_role(&roles(&[unknown])).is_none(),
            "an unrecognised role code ({unknown:?}) must not become a primary role"
        );
    }
    assert_eq!(
        resolve_security_level(&roles(&["client"])),
        SecurityLevel::Guest,
        "an external client role is a guest inside the internal resolver"
    );

    // 4. The canonical primary role is a precedence, and its answer is always one of the five assignable codes.
    assert_eq!(
        canonical_primary_role(&roles(&[])),
        None,
        "no roles, no primary role"
    );
    assert_eq!(canonical_primary_role(&roles(&["ops"])), Some("user"));
    assert_eq!(
        canonical_primary_role(&roles(&["internal_guest", "business_power_user", "owner"])),
        Some("owner"),
        "owner outranks business_power_user, which outranks internal_guest"
    );
    assert_eq!(
        canonical_primary_role(&roles(&["agent", "business_power_user"])),
        Some("business_power_user"),
        "both spellings are the same role"
    );
    for code in CANONICAL_INTERNAL_ROLE_CODES {
        assert_eq!(
            canonical_primary_role(&roles(&[code])),
            Some(*code),
            "every canonical internal role must resolve to itself"
        );
    }
    assert_eq!(
        CANONICAL_INTERNAL_ROLE_CODES.len(),
        5,
        "the five assignable internal roles are the system's model; a sixth is a design decision, not a test edit"
    );

    // 5. Root-only administration: two actions, and nothing else, are reserved to root. A wider set would be a
    //    privilege change made in the one place this test is watching.
    assert_eq!(
        ROOT_ONLY_ACTIONS.len(),
        2,
        "exactly two actions are root-only"
    );
    for action in ROOT_ONLY_ACTIONS {
        assert!(is_root_only(action), "`{action}` must be root-only");
    }
    assert!(
        !is_root_only("property.read") && !is_root_only("") && !is_root_only("security.entitlement"),
        "an ordinary action, an empty string and an action that merely resembles a root action are not root-only"
    );
}
