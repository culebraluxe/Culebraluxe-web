//! SEC.ROLE — ROOT has all authorized capabilities including TECH (TST-SEC-ROLE-005).
//!
//! Contract: the ROOT role is the whole capability set — TECH, the root-only administration actions, and ordinary
//! CRUD — with NO grants held at all. The role branch precedes every other principal check (root-only, TECH domain,
//! level floors, grant match), so ROOT's reach comes from the role itself and survives a grant wipe. The two
//! root-only actions (`security.entitlement.manage`, `security.role.manage`, the one list in
//! `middle/model/src/security.rs`) stay refused for every non-root role — including `owner`, which shares ROOT's
//! reach everywhere else — so the administration boundary has exactly one keyholder. In production this is one
//! decision in one place: `CasbinAuthorizationPort::authorize` (`web/src/security/entitlements.rs`), driven here
//! through the public `services::AuthorizationPort` with an in-memory enforcer seeded from the production action
//! catalog (no database, no network).
//!
//! Four facts are pinned:
//!
//! - **ROOT reaches everything with empty grants.** TECH, both root-only actions, and ordinary CRUD are allowed for
//!   a root principal holding no grants: the role decides, not the grant list.
//! - **The root-only actions have one keyholder.** `owner` and `business_power_user` are refused them, by the
//!   root-only rule rather than by a missing grant.
//! - **Owner is root-like but not root.** `owner` reaches TECH and ordinary CRUD (the shared role branch) while
//!   missing the root-only actions: the boundary between them is exactly the two administration actions.
//! - **ROOT is not anonymous.** The reach belongs to the root ROLE on an internal principal, not to the absence of
//!   one: a missing principal is still denied the same actions.
//!
//! Level: L3 Composition — the real authorizer, the real catalog-seeded policy, the real principal shape.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_role__005__root_has_all_authorized_capabilities_including_tech

use services::{
    AuthorizationPort, AuthorizationRequest, OperationKind, ServiceActor, ServiceActorKind,
    ServicePrincipal,
};
use web::security::CasbinAuthorizationPort;

/// An internal principal with `roles`, holding NO grants: whatever is allowed here comes from the role alone.
fn principal_with_roles(roles: &[&str], level: &str) -> (ServiceActor, ServicePrincipal) {
    (
        ServiceActor {
            id: Some("root-1".into()),
            kind: ServiceActorKind::User,
        },
        ServicePrincipal {
            app_user_id: "root-1".into(),
            level: level.into(),
            role_codes: roles.iter().map(|role| role.to_string()).collect(),
            account_type: "internal".into(),
            entitlement_codes: vec![],
        },
    )
}

fn request(
    action: &'static str,
    domain: &'static str,
    operation: &'static str,
    kind: OperationKind,
    actor: ServiceActor,
    principal: Option<ServicePrincipal>,
) -> AuthorizationRequest {
    AuthorizationRequest {
        domain,
        action,
        operation,
        kind,
        actor,
        principal,
    }
}

#[tokio::test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-ROLE-005).
async fn sec_role_005__root_has_all_authorized_capabilities_including_tech() {
    let auth = CasbinAuthorizationPort::new()
        .await
        .expect("the authorizer seeds from the production catalog, in memory");

    // 1. ROOT REACHES EVERYTHING WITH EMPTY GRANTS. TECH, both root-only administration actions, and ordinary CRUD:
    //    allowed with no grants held, so the reach is the role's, not the grant list's.
    for (action, domain, operation, kind, policy) in [
        (
            "tech.access",
            "tech",
            "tech.cockpit",
            OperationKind::Query,
            "role:root",
        ),
        (
            "security.role.manage",
            "security",
            "security.assignRole",
            OperationKind::Command,
            "rule:security.manage.root",
        ),
        (
            "security.entitlement.manage",
            "security",
            "security.grantEntitlement",
            OperationKind::Command,
            "rule:security.manage.root",
        ),
        (
            "person.write",
            "person",
            "person.update",
            OperationKind::Command,
            "role:root",
        ),
    ] {
        let (actor, principal) = principal_with_roles(&["root"], "ROOT");
        let decision = auth
            .authorize(request(
                action,
                domain,
                operation,
                kind,
                actor,
                Some(principal),
            ))
            .await
            .expect("in-memory authorization cannot fail");
        assert!(
            decision.allowed,
            "ROOT with no grants must still be allowed {action}"
        );
        assert_eq!(decision.policy_id, policy, "wrong rule decided {action}");
    }

    // 2. THE ROOT-ONLY ACTIONS HAVE ONE KEYHOLDER. `owner` — root-like everywhere else — and the power user are
    //    refused the administration actions by the root-only rule, not by a missing grant (they hold none either,
    //    and grants are not consulted on this branch).
    for roles in [["owner"], ["business_power_user"]] {
        let (actor, principal) = principal_with_roles(&roles, "ROOT");
        let decision = auth
            .authorize(request(
                "security.role.manage",
                "security",
                "security.assignRole",
                OperationKind::Command,
                actor,
                Some(principal),
            ))
            .await
            .expect("in-memory authorization cannot fail");
        assert!(!decision.allowed, "role {} must not manage roles", roles[0]);
        assert_eq!(decision.policy_id, "rule:security.manage.root");
    }

    // 3. OWNER IS ROOT-LIKE BUT NOT ROOT. The shared role branch reaches TECH and ordinary CRUD for `owner` too —
    //    the boundary between owner and root is exactly the two administration actions pinned above.
    for (action, domain, operation, kind) in [
        ("tech.access", "tech", "tech.cockpit", OperationKind::Query),
        (
            "person.write",
            "person",
            "person.update",
            OperationKind::Command,
        ),
    ] {
        let (actor, principal) = principal_with_roles(&["owner"], "ROOT");
        let decision = auth
            .authorize(request(
                action,
                domain,
                operation,
                kind,
                actor,
                Some(principal),
            ))
            .await
            .expect("in-memory authorization cannot fail");
        assert!(
            decision.allowed,
            "owner reaches {action} through the shared role branch"
        );
        assert_eq!(decision.policy_id, "role:root");
    }

    // 4. ROOT IS NOT ANONYMOUS. The reach belongs to the root role on an internal principal: with no principal at
    //    all, the same actions are denied — so an absent identity can never inherit ROOT's reach.
    let anonymous = ServiceActor {
        id: None,
        kind: ServiceActorKind::User,
    };
    let decision = auth
        .authorize(request(
            "tech.access",
            "tech",
            "tech.cockpit",
            OperationKind::Query,
            anonymous,
            None,
        ))
        .await
        .expect("in-memory authorization cannot fail");
    assert!(
        !decision.allowed,
        "no principal must never inherit ROOT's reach"
    );
}
