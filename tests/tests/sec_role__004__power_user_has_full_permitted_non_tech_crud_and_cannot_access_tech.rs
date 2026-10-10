//! SEC.ROLE — Power User has full permitted non-TECH CRUD and cannot access TECH (TST-SEC-ROLE-004).
//!
//! Contract: a BUSINESS_POWER_USER with grants does the full non-TECH job — reads, writes, and the floored
//! high-value commands (`contract.execute`, whose floor the power user meets) — while the TECH domain stays closed:
//! any action in the `tech` domain, or starting with `tech.`, is denied BEFORE the grant check, so even a held TECH
//! grant cannot open it. In production this is one decision in one place: `CasbinAuthorizationPort::authorize`
//! (`web/src/security/entitlements.rs`), driven here through the public `services::AuthorizationPort` with an
//! in-memory enforcer seeded from the production action catalog (no database, no network).
//!
//! Four facts are pinned:
//!
//! - **Full non-TECH CRUD.** Granted reads, writes, and `contract.execute` are all allowed: the floor is met and the
//!   grants match.
//! - **TECH is denied by domain.** `tech.access` as a query and as a command are both refused with the domain rule.
//! - **A held TECH grant cannot override the domain.** The denial precedes the grant check, so an accidental TECH
//!   grant escalates nothing.
//! - **The floor still binds sideways.** `luxesign.issue` needs the grant as well as the level here (unlike the
//!   USER story, where the floor alone decides): level without grant is still denied, by the grant check.
//!
//! Level: L3 Composition — the real authorizer, the real catalog-seeded policy, the real principal shape.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_role__004__power_user_has_full_permitted_non_tech_crud_and_cannot_access_tech

use services::{
    AuthorizationPort, AuthorizationRequest, OperationKind, ServiceActor, ServiceActorKind,
    ServicePrincipal,
};
use web::security::CasbinAuthorizationPort;

/// A BUSINESS_POWER_USER holding `grants` and nothing else.
fn power_user(grants: &[&str]) -> (ServiceActor, ServicePrincipal) {
    (
        ServiceActor {
            id: Some("power-1".into()),
            kind: ServiceActorKind::User,
        },
        ServicePrincipal {
            app_user_id: "power-1".into(),
            level: "BUSINESS_POWER_USER".into(),
            role_codes: vec!["business_power_user".into()],
            account_type: "internal".into(),
            entitlement_codes: grants.iter().map(|grant| grant.to_string()).collect(),
        },
    )
}

fn request(
    action: &'static str,
    domain: &'static str,
    operation: &'static str,
    kind: OperationKind,
    actor: ServiceActor,
    principal: ServicePrincipal,
) -> AuthorizationRequest {
    AuthorizationRequest {
        domain,
        action,
        operation,
        kind,
        actor,
        principal: Some(principal),
    }
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-ROLE-004).
async fn sec_role_004__power_user_has_full_permitted_non_tech_crud_and_cannot_access_tech() {
    let auth = CasbinAuthorizationPort::new()
        .await
        .expect("the authorizer seeds from the production catalog, in memory");

    // 1. FULL NON-TECH CRUD. Granted read, granted write, and the floored high-value command whose floor the power
    //    user meets: all allowed.
    for (action, domain, operation, kind) in [
        (
            "person.read",
            "person",
            "person.snapshot",
            OperationKind::Query,
        ),
        (
            "person.write",
            "person",
            "person.update",
            OperationKind::Command,
        ),
        (
            "contract.execute",
            "contract",
            "contract.execute",
            OperationKind::Command,
        ),
    ] {
        let (actor, principal) = power_user(&["person.read", "person.write", "contract.execute"]);
        let decision = auth
            .authorize(request(action, domain, operation, kind, actor, principal))
            .await
            .expect("in-memory authorization cannot fail");
        assert!(
            decision.allowed,
            "a power user holding the grant must be allowed {action}"
        );
    }

    // 2. TECH IS DENIED BY DOMAIN — both kinds. The TECH surface is not a grant away: the domain rule refuses it
    //    before any grant is consulted.
    for kind in [OperationKind::Query, OperationKind::Command] {
        let (actor, principal) = power_user(&["person.read", "person.write"]);
        let tech = auth
            .authorize(request(
                "tech.access",
                "tech",
                "tech.cockpit",
                kind,
                actor,
                principal,
            ))
            .await
            .expect("in-memory authorization cannot fail");
        assert!(
            !tech.allowed,
            "a power user must not access TECH, as {kind:?}"
        );
        assert_eq!(tech.policy_id, "domain:tech");
    }

    // 3. A HELD TECH GRANT CANNOT OVERRIDE THE DOMAIN. The denial precedes the grant check, so even naming
    //    `tech.access` in the grant list changes nothing: an accidental TECH grant escalates nothing.
    let (actor, principal) = power_user(&["tech.access"]);
    let granted_tech = auth
        .authorize(request(
            "tech.access",
            "tech",
            "tech.cockpit",
            OperationKind::Query,
            actor,
            principal,
        ))
        .await
        .expect("in-memory authorization cannot fail");
    assert!(
        !granted_tech.allowed,
        "holding a TECH grant must not open TECH for a power user"
    );
    assert_eq!(granted_tech.policy_id, "domain:tech");

    // 4. THE FLOOR STILL BINDS SIDEWAYS. `luxesign.issue` needs the grant as well as the level: a power user at
    //    the right level but without the grant is denied by the grant check, not waved through by the level.
    let (actor, principal) = power_user(&["person.read"]);
    let ungranted_issue = auth
        .authorize(request(
            "luxesign.issue",
            "luxesign",
            "luxesign.issue",
            OperationKind::Command,
            actor,
            principal,
        ))
        .await
        .expect("in-memory authorization cannot fail");
    assert!(
        !ungranted_issue.allowed,
        "a power user without the grant must not issue, floor met or not"
    );
    assert_eq!(ungranted_issue.policy_id, "entitlement:role-grant");
}
