//! SEC.ROLE — External Guest can read allowed public surfaces and cannot access portal surfaces
//! (TST-SEC-ROLE-001).
//!
//! Contract: a caller with no portal identity — anonymous (no principal at all) or an authenticated EXTERNAL account
//! — may read exactly what the site publishes, and nothing else. In production this is one decision in one place:
//! `CasbinAuthorizationPort::authorize` (`web/src/security/entitlements.rs`), driven here through the public
//! `services::AuthorizationPort` with an in-memory enforcer seeded from the production action catalog (no database,
//! no network; external providers are faked by absence — there is no principal to resolve and no grant store to read).
//!
//! Three facts are pinned, each in both directions:
//!
//! - **Published reads are public.** `property.public.read` and `guide.public.read` as queries are allowed with no
//!   principal, and staying signed in as an external account does not remove that right.
//! - **Portal reads are refused.** `person.read` — an ordinary portal query — is denied anonymous (`principal:missing`)
//!   and external (`account:external`): two different refusals, so neither door is the other's fallback.
//! - **Published is query-only.** The same public action as a command is denied: publication is a read, not a grant.
//!
//! The external account's one allowed non-public read — its own client-room projection — is included to pin the full
//! set of allowed external surfaces: without it a wider (or narrower) external surface would pass unnoticed.
//!
//! Level: L3 Composition — the real authorizer, the real catalog-seeded policy, the real principal shape.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_role__001__external_guest_can_read_allowed_public_surfaces_and_cannot_access_portal_surfaces

use services::{
    AuthorizationPort, AuthorizationRequest, OperationKind, ServiceActor, ServiceActorKind,
    ServicePrincipal,
};
use web::security::CasbinAuthorizationPort;

/// An anonymous website visitor: no session, no principal.
fn anonymous() -> ServiceActor {
    ServiceActor {
        id: None,
        kind: ServiceActorKind::User,
    }
}

/// An authenticated EXTERNAL account (a guest/client): identified, but not internal.
fn external_account() -> ServicePrincipal {
    ServicePrincipal {
        app_user_id: "guest-1".into(),
        level: "GUEST".into(),
        role_codes: vec!["guest".into()],
        account_type: "external".into(),
        entitlement_codes: vec![],
    }
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-ROLE-001).
async fn sec_role_001__external_guest_can_read_allowed_public_surfaces_and_cannot_access_portal_surfaces(
) {
    let auth = CasbinAuthorizationPort::new()
        .await
        .expect("the authorizer seeds from the production catalog, in memory");

    // 1. PUBLISHED READS ARE PUBLIC. Anonymous, no grants, no session: the site's published listings and guide read.
    for action in ["property.public.read", "guide.public.read"] {
        let decision = auth
            .authorize(request(
                action,
                "property",
                "property.publicListing",
                OperationKind::Query,
                anonymous(),
                None,
            ))
            .await
            .expect("in-memory authorization cannot fail");
        assert!(decision.allowed, "anonymous must read {action}");
        assert_eq!(decision.policy_id, "system:explicit");
        assert_eq!(decision.mode, "enforced");
    }

    // 2. SIGNING IN AS EXTERNAL DOES NOT REMOVE THE PUBLIC READ. Publication is a property of the action, not of
    //    the caller's anonymity.
    let decision = auth
        .authorize(request(
            "property.public.read",
            "property",
            "property.publicListing",
            OperationKind::Query,
            ServiceActor {
                id: Some("guest-1".into()),
                kind: ServiceActorKind::User,
            },
            Some(external_account()),
        ))
        .await
        .expect("in-memory authorization cannot fail");
    assert!(
        decision.allowed,
        "an external account keeps the published read"
    );

    // 3. PORTAL SURFACES ARE REFUSED — TWICE, FOR TWO REASONS. Anonymous has no principal to decide from; an
    //    external principal is decided and found external. One shared refusal would hide a missing door.
    let anonymous_portal = auth
        .authorize(request(
            "person.read",
            "person",
            "person.snapshot",
            OperationKind::Query,
            anonymous(),
            None,
        ))
        .await
        .expect("in-memory authorization cannot fail");
    assert!(
        !anonymous_portal.allowed,
        "anonymous must not read portal data"
    );
    assert_eq!(anonymous_portal.policy_id, "principal:missing");

    let external_portal = auth
        .authorize(request(
            "person.read",
            "person",
            "person.snapshot",
            OperationKind::Query,
            ServiceActor {
                id: Some("guest-1".into()),
                kind: ServiceActorKind::User,
            },
            Some(external_account()),
        ))
        .await
        .expect("in-memory authorization cannot fail");
    assert!(
        !external_portal.allowed,
        "an external account must not read portal data"
    );
    assert_eq!(external_portal.policy_id, "account:external");

    // 4. PUBLISHED IS QUERY-ONLY. The same public action as a command is not published: a read is not a grant.
    let public_command = auth
        .authorize(request(
            "property.public.read",
            "property",
            "property.publicListing",
            OperationKind::Command,
            anonymous(),
            None,
        ))
        .await
        .expect("in-memory authorization cannot fail");
    assert!(
        !public_command.allowed,
        "a published read must not become a command"
    );

    // 5. THE EXTERNAL ACCOUNT'S ONE NON-PUBLIC READ. An authenticated external account reads exactly its own
    //    client-room projection (the route derives the subject from the session, never from the request). This pins
    //    the allowed external surface at published reads plus the room: a wider external surface fails here.
    let room = auth
        .authorize(request(
            "client.room.read",
            "client-room",
            "clientRoom.snapshot",
            OperationKind::Query,
            ServiceActor {
                id: Some("guest-1".into()),
                kind: ServiceActorKind::User,
            },
            Some(external_account()),
        ))
        .await
        .expect("in-memory authorization cannot fail");
    assert!(room.allowed, "an external account reads its own room");
    assert_eq!(room.policy_id, "rule:client.room.external-self");

    // …while anonymous, with no session to derive a subject from, does not.
    let room_anonymous = auth
        .authorize(request(
            "client.room.read",
            "client-room",
            "clientRoom.snapshot",
            OperationKind::Query,
            anonymous(),
            None,
        ))
        .await
        .expect("in-memory authorization cannot fail");
    assert!(
        !room_anonymous.allowed,
        "without a session there is no room subject to read"
    );
}
