//! SEC.ROLE — Internal Guest has read-only portal access and cannot mutate portal data
//! (TST-SEC-ROLE-002).
//!
//! Contract: an INTERNAL account holding the guest role may query what it has been explicitly granted — and nothing
//! else. Read-only is a property of the grants held, enforced action-by-action and kind-by-kind: a read grant
//! authorizes a query for that action only, never a command, never another action, and the absence of grants
//! authorizes nothing. In production this is one decision in one place: `CasbinAuthorizationPort::authorize`
//! (`web/src/security/entitlements.rs`), driven here through the public `services::AuthorizationPort` with an
//! in-memory enforcer seeded from the production action catalog (no database, no network).
//!
//! Four facts are pinned:
//!
//! - **A granted read reads.** `person.read` held as a grant authorizes the `person.read` query.
//! - **A read grant never writes.** The same grant authorizes no command — not `person.write`, not even `person.read`
//!   as a command: action AND kind must both match the grant.
//! - **A grant never travels.** `person.read` authorizes no other action's query.
//! - **No grant, no access.** The same internal guest with an empty grant list is denied the query too: read-only by
//!   grant, never by role name.
//!
//! Level: L3 Composition — the real authorizer, the real catalog-seeded policy, the real principal shape.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_role__002__internal_guest_has_read_only_portal_access_and_cannot_mutate_portal_data

use services::{
    AuthorizationPort, AuthorizationRequest, OperationKind, ServiceActor, ServiceActorKind,
    ServicePrincipal,
};
use web::security::CasbinAuthorizationPort;

/// An INTERNAL account with the guest role: inside the portal door, holding `grants` and nothing else.
fn internal_guest(grants: &[&str]) -> (ServiceActor, ServicePrincipal) {
    (
        ServiceActor {
            id: Some("guest-internal-1".into()),
            kind: ServiceActorKind::User,
        },
        ServicePrincipal {
            app_user_id: "guest-internal-1".into(),
            level: "GUEST".into(),
            role_codes: vec!["internal_guest".into()],
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-ROLE-002).
async fn sec_role_002__internal_guest_has_read_only_portal_access_and_cannot_mutate_portal_data() {
    let auth = CasbinAuthorizationPort::new()
        .await
        .expect("the authorizer seeds from the production catalog, in memory");

    // 1. A GRANTED READ READS. The internal guest holding `person.read` queries `person.read`: allowed, by grant.
    let (actor, principal) = internal_guest(&["person.read"]);
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
    assert!(read.allowed, "a granted read must read");
    assert_eq!(read.policy_id, "entitlement:role-grant");

    // 2. A READ GRANT NEVER WRITES. The same grant authorizes no command: neither the write action nor the read
    //    action performed as a command. Either allowance would be a mutation through a read-only grant.
    for (action, operation) in [
        ("person.write", "person.update"),
        ("person.read", "person.snapshot"),
    ] {
        let (actor, principal) = internal_guest(&["person.read"]);
        let mutation = auth
            .authorize(request(
                action,
                "person",
                operation,
                OperationKind::Command,
                actor,
                principal,
            ))
            .await
            .expect("in-memory authorization cannot fail");
        assert!(
            !mutation.allowed,
            "a read grant must not authorize the command {action}"
        );
    }

    // 3. A GRANT NEVER TRAVELS. `person.read` authorizes no other action's query: the grant names its action.
    let (actor, principal) = internal_guest(&["person.read"]);
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

    // 4. NO GRANT, NO ACCESS. The same internal guest with an empty grant list is denied even the read: read-only
    //    comes from the grants held, never from the role name. A role-name-based read would survive a grant wipe.
    let (actor, principal) = internal_guest(&[]);
    let ungranted = auth
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
    assert!(
        !ungranted.allowed,
        "an internal guest with no grants must not read"
    );
}
