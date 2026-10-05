//! ARCH.ONE_WRITER — role assignments (TST-ARCH-ONE-WRITER-011).
//!
//! Contract: role assignment rows have a closed writer set of two. The security
//! DAO (`db/src/security.rs`) assigns roles to application users, and guest
//! provisioning (`db/src/guest.rs`) assigns the guest's initial role at sign-in.
//! Entitlement evaluation then reads grants from those database roles — the
//! database remains the source of truth — so a third file inserting assignments
//! is a second granter nobody audits.
//!
//! Level: L0 Pure — filesystem reads only, no database, no network.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test arch_one_writer__011__role_assignments

use test_harness::source;

/// True when a code line inserts a role assignment row.
fn inserts_assignment(line: &str) -> bool {
    source::code_of(line)
        .to_lowercase()
        .contains("insert into app_user_role")
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-ARCH-ONE-WRITER-011); the file and the assay use it.
fn arch_one_writer_011__role_assignments() {
    let workspace = source::sources_under(&source::workspace_root());
    let mut holders: Vec<String> = Vec::new();
    for path in &workspace {
        let relative = source::relative(path);
        if relative.contains("/tests/") || relative.contains("/target/") {
            continue;
        }
        if source::read(path).lines().any(inserts_assignment) {
            holders.push(relative);
        }
    }
    holders.sort();
    assert_eq!(
        holders,
        vec!["db/src/guest.rs", "db/src/security.rs"],
        "role assignments have two writers (security DAO, guest provisioning); a third is an unaudited granter"
    );

    // Both writers assign through the same columns: user plus role, nothing implicit.
    for relative in &holders {
        let text = source::read(&source::workspace_root().join(relative));
        assert!(
            text.contains("insert into app_user_role (app_user_id, role_id)"),
            "{relative} must assign an explicit user and an explicit role"
        );
    }

    // The evaluation side reads grants from the database roles rather than inventing them.
    let entitlements =
        source::read(&source::workspace_root().join("web/src/security/entitlements.rs"));
    assert!(
        entitlements.contains("The database remains the source of truth for grants."),
        "entitlement evaluation treats the database as the source of truth for grants"
    );

    // Negative controls: the detector fires on the insert and stays quiet on reads and prose.
    assert!(inserts_assignment(
        "insert into app_user_role (app_user_id, role_id) values ($1, $2)"
    ));
    assert!(!inserts_assignment(
        "select r.code from app_user_role aur join security_role r on r.id = aur.role_id"
    ));
    assert!(!inserts_assignment(
        "// guest provisioning inserts into app_user_role for us"
    ));
}
