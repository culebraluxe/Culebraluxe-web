//! SEC.ROLE — USER is read-only by default except explicitly granted CRUD entitlements
//! (TST-SEC-ROLE-003).
//!
//! Contract: the ordinary internal USER starts with nothing — every query and every command is denied until a grant
//! names it — and each explicitly granted entitlement opens exactly what it says: the named action in the named kind.
//! On top of grants sit two level floors the grant cannot lower: `contract.execute` and `luxesign.issue` demand
//! BUSINESS_POWER_USER even when the grant is held. In production this is one decision in one place:
//! `CasbinAuthorizationPort::authorize` (`web/src/security/entitlements.rs`), driven here through the public
//! `services::AuthorizationPort` with an in-memory enforcer seeded from the production action catalog (no database,
//! no network).
//!
//! Four facts are pinned:
//!
//! - **Default deny.** A USER with no grants is denied the read and the write alike: read-only is the default because
//!   nothing is granted, not because writes are specially refused.
//! - **Explicit grants open exactly their action and kind.** `person.read` opens the query; `person.write` opens the
//!   command; neither opens anything else.
//! - **A grant never travels.** `person.read` authorizes no other action.
//! - **Grants do not lower floors.** `contract.execute` and `luxesign.issue` stay refused for a USER holding the
//!   grant: the level floor is checked before the grant, so an accidental grant cannot escalate.
//!
//! Level: L3 Composition — the real authorizer, the real catalog-seeded policy, the real principal shape.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_role__003__user_is_read_only_by_default_except_explicitly_granted_crud_entitlements

use services::{
    AuthorizationPort, AuthorizationRequest, OperationKind, ServiceActor, ServiceActorKind,
    ServicePrincipal,
};
use web::security::CasbinAuthorizationPort;

/// An ordinary internal USER holding `grants` and nothing else.
fn user(grants: &[&str]) -> (ServiceActor, ServicePrincipal) {
    (
        ServiceActor {
            id: Some("user-1".into()),
            kind: ServiceActorKind::User,
        },
        ServicePrincipal {
            app_user_id: "user-1".into(),
            level: "USER".into(),
            role_codes: vec!["user".into()],
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-ROLE-003).
async fn sec_role_003__user_is_read_only_by_default_except_explicitly_granted_crud_entitlements() {
    let auth = CasbinAuthorizationPort::new()
        .await
        .expect("the authorizer seeds from the production catalog, in memory");

    // 1. DEFAULT DENY. No grants: the read and the write are both refused. The default is deny-all, and read-only
    //    holds because nothing is granted — not because writes are refused while reads pass.
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
    ] {
        let (actor, principal) = user(&[]);
        let decision = auth
            .authorize(request(action, domain, operation, kind, actor, principal))
            .await
            .expect("in-memory authorization cannot fail");
        assert!(
            !decision.allowed,
            "a USER with no grants must be denied {action}"
        );
        assert_eq!(decision.policy_id, "entitlement:role-grant");
    }

    // 2. EXPLICIT GRANTS OPEN EXACTLY THEIR ACTION AND KIND. `person.read` opens the query and nothing else;
    //    adding `person.write` opens the command too. Each grant is one action in one kind.
    let (actor, principal) = user(&["person.read"]);
    let read = auth
        .authorize(request(
            "person.read",
            "person",
            "person.snapshot",
            OperationKind::Query,
            actor,
            principal,
        ))
        .await
        .expect("in-memory authorization cannot fail");
    assert!(read.allowed, "an explicitly granted read must read");

    let (actor, principal) = user(&["person.read"]);
    let write_without_grant = auth
        .authorize(request(
            "person.write",
            "person",
            "person.update",
            OperationKind::Command,
            actor,
            principal,
        ))
        .await
        .expect("in-memory authorization cannot fail");
    assert!(
        !write_without_grant.allowed,
        "a read grant must not open the write"
    );

    let (actor, principal) = user(&["person.read", "person.write"]);
    let write_with_grant = auth
        .authorize(request(
            "person.write",
            "person",
            "person.update",
            OperationKind::Command,
            actor,
            principal,
        ))
        .await
        .expect("in-memory authorization cannot fail");
    assert!(
        write_with_grant.allowed,
        "an explicitly granted write must write"
    );

    // 3. A GRANT NEVER TRAVELS. `person.read` authorizes no other action's query.
    let (actor, principal) = user(&["person.read"]);
    let other = auth
        .authorize(request(
            "deal.read",
            "deal",
            "deal.snapshot",
            OperationKind::Query,
            actor,
            principal,
        ))
        .await
        .expect("in-memory authorization cannot fail");
    assert!(
        !other.allowed,
        "a grant for one action must not read another"
    );

    // 4. GRANTS DO NOT LOWER FLOORS. `contract.execute` and `luxesign.issue` demand BUSINESS_POWER_USER: a USER
    //    holding the grant is still refused, by the floor rule rather than by the grant check — so an accidental
    //    grant cannot escalate a high-value, near-irreversible command.
    for (action, domain, operation) in [
        ("contract.execute", "contract", "contract.execute"),
        ("luxesign.issue", "luxesign", "luxesign.issue"),
    ] {
        let (actor, principal) = user(&[action]);
        let floored = auth
            .authorize(request(
                action,
                domain,
                operation,
                OperationKind::Command,
                actor,
                principal,
            ))
            .await
            .expect("in-memory authorization cannot fail");
        assert!(
            !floored.allowed,
            "a USER must not {action}, even holding the grant"
        );
        assert!(
            floored.policy_id.ends_with(".level"),
            "the refusal must come from the level floor, not the grant check: {}",
            floored.policy_id
        );
    }
}
