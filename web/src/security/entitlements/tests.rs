// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::*;
use services::{ServiceActor, ServicePrincipal};

fn request(action: &'static str, kind: OperationKind, grants: &[&str]) -> AuthorizationRequest {
    AuthorizationRequest {
        domain: "person",
        action,
        operation: "person.sample",
        kind,
        actor: ServiceActor {
            id: Some("u1".into()),
            kind: ServiceActorKind::User,
        },
        principal: Some(ServicePrincipal {
            app_user_id: "u1".into(),
            level: "USER".into(),
            role_codes: vec!["user".into()],
            account_type: "internal".into(),
            entitlement_codes: grants.iter().map(|s| s.to_string()).collect(),
        }),
    }
}

#[tokio::test]
async fn the_contract_floor_and_the_guest_default_hold_where_they_are_enforced() {
    let auth = CasbinAuthorizationPort::new().await.unwrap();

    // THE RULES MOVED HERE, SO THEIR TESTS DID TOO. These two assertions used to live in the TypeScript
    // suite, testing the TypeScript copy of the rules; that copy is gone (the port asks this one now), so
    // leaving the test behind would have deleted the only coverage of a rule that still guards a command.

    // contract.execute is high-value and near-irreversible: BUSINESS_POWER_USER is the floor, and holding the
    // grant does not lower it.
    let mut execute = request(
        "contract.execute",
        OperationKind::Command,
        &["contract.execute"],
    );
    execute.domain = "contract";
    execute.operation = "contract.execute";
    execute.principal.as_mut().unwrap().level = "USER".into();
    assert!(
        !auth.authorize(execute.clone()).await.unwrap().allowed,
        "USER must not execute a contract, even holding the grant"
    );
    execute.principal.as_mut().unwrap().level = "BUSINESS_POWER_USER".into();
    assert!(
        auth.authorize(execute).await.unwrap().allowed,
        "BUSINESS_POWER_USER is the floor for contract.execute"
    );

    let mut issue = request(
        "luxesign.issue",
        OperationKind::Command,
        &["luxesign.issue"],
    );
    issue.domain = "luxesign";
    issue.operation = "luxesign.issue";
    issue.principal.as_mut().unwrap().level = "USER".into();
    assert!(
        !auth.authorize(issue.clone()).await.unwrap().allowed,
        "USER must not issue a document even if a grant is accidentally present"
    );
    issue.principal.as_mut().unwrap().level = "BUSINESS_POWER_USER".into();
    assert!(
        auth.authorize(issue).await.unwrap().allowed,
        "BUSINESS_POWER_USER is the floor for luxesign.issue"
    );

    // A missing principal (GUEST) never commands, whatever the action.
    let mut guest = request("contract.write", OperationKind::Command, &[]);
    guest.principal = None;
    assert!(
        !auth.authorize(guest).await.unwrap().allowed,
        "a missing principal must not command"
    );
}

#[tokio::test]
async fn action_and_kind_must_both_match_a_role_grant() {
    let auth = CasbinAuthorizationPort::new().await.unwrap();
    assert!(
        auth.authorize(request(
            "person.read",
            OperationKind::Query,
            &["person.read"]
        ))
        .await
        .unwrap()
        .allowed
    );
    assert!(
        !auth
            .authorize(request(
                "person.write",
                OperationKind::Command,
                &["person.read"]
            ))
            .await
            .unwrap()
            .allowed
    );
    assert!(
        !auth
            .authorize(request(
                "person.read",
                OperationKind::Command,
                &["person.read"]
            ))
            .await
            .unwrap()
            .allowed
    );
    assert!(
        !auth
            .authorize(request(
                "future.read",
                OperationKind::Query,
                &["future.read"]
            ))
            .await
            .unwrap()
            .allowed
    );
}

#[tokio::test]
async fn only_explicit_system_bootstrap_and_public_document_are_open() {
    let auth = CasbinAuthorizationPort::new().await.unwrap();
    let mut req = request("security.identity.resolve", OperationKind::Query, &[]);
    req.principal = None;
    req.actor = ServiceActor {
        id: Some("authjs-edge".into()),
        kind: ServiceActorKind::System,
    };
    req.operation = "security.resolveIdentity";
    assert!(auth.authorize(req.clone()).await.unwrap().allowed);

    // The OTHER way the edge asks: through the kernel, for a session it holds. The identified door mints a USER
    // actor, so a policy that only recognised the edge actor refused every sign-in — the resolution failed closed
    // and the portal answered "not authorized". Both arrivals are the edge, and the internal bridge key (which the
    // browser never has) is what makes that true.
    let mut sign_in = request("security.identity.resolve", OperationKind::Query, &[]);
    sign_in.actor = ServiceActor {
        id: Some("user-1".into()),
        kind: ServiceActorKind::User,
    };
    sign_in.operation = "security.resolveIdentity";
    let decision = auth.authorize(sign_in.clone()).await.unwrap();
    assert!(
        decision.allowed,
        "a signed-in session may find out who it is"
    );
    assert_eq!(decision.policy_id, "rule:identity.resolve.edge");

    // But not as anything else: no session, no resolution — the reserved branch still holds.
    sign_in.actor = ServiceActor {
        id: None,
        kind: ServiceActorKind::System,
    };
    assert!(!auth.authorize(sign_in).await.unwrap().allowed);

    req.operation = "security.getPrincipal";
    assert!(!auth.authorize(req.clone()).await.unwrap().allowed);
    req.actor.id = Some("public-website".into());
    req.action = "vault.publicListingDocument.read";
    req.operation = "vault.publicListingDocumentBytes";
    assert!(auth.authorize(req.clone()).await.unwrap().allowed);
    req.action = "vault.read";
    assert!(!auth.authorize(req.clone()).await.unwrap().allowed);

    // Native document signing system actors are narrow and operation-specific.
    let mut luxesign = request("signature.write", OperationKind::Command, &[]);
    luxesign.principal = None;
    luxesign.actor = ServiceActor {
        id: Some(crate::luxesign::LUXESIGN_SERVICE_ACTOR.into()),
        kind: ServiceActorKind::System,
    };
    luxesign.domain = "signature";
    luxesign.operation = "signature.prepare";
    assert!(auth.authorize(luxesign.clone()).await.unwrap().allowed);
    luxesign.operation = "signature.send";
    assert!(
        !auth.authorize(luxesign.clone()).await.unwrap().allowed,
        "native orchestration must not inherit raw provider send authority"
    );

    let mut signer = request("signer.act", OperationKind::Command, &[]);
    signer.principal = None;
    signer.actor = ServiceActor {
        id: Some(crate::signer::LUXESIGN_EDGE_ACTOR.into()),
        kind: ServiceActorKind::System,
    };
    signer.domain = "signer";
    signer.operation = "signer.complete";
    assert!(auth.authorize(signer.clone()).await.unwrap().allowed);
    signer.operation = "signer.issueAccess";
    signer.action = "signer.access.issue";
    assert!(
        !auth.authorize(signer).await.unwrap().allowed,
        "public signing edge may never mint signer capabilities"
    );

    // The recipient-bound actor the edge runs mutations as: the five signer commands and nothing else.
    let mut recipient = request("signer.act", OperationKind::Command, &[]);
    recipient.principal = None;
    recipient.actor = ServiceActor {
        id: Some("signature-recipient:7d1c0a52-0000-4000-8000-000000000001".into()),
        kind: ServiceActorKind::System,
    };
    recipient.domain = "signer";
    for operation in [
        "signer.open",
        "signer.acceptConsent",
        "signer.completeField",
        "signer.complete",
        "signer.decline",
    ] {
        recipient.operation = operation;
        assert!(
            auth.authorize(recipient.clone()).await.unwrap().allowed,
            "a signing recipient may {operation}"
        );
    }
    recipient.operation = "signer.issueAccess";
    recipient.action = "signer.access.issue";
    assert!(
        !auth.authorize(recipient.clone()).await.unwrap().allowed,
        "a signing recipient may never mint signer capabilities"
    );
    recipient.operation = "signer.complete";
    recipient.action = "signer.act";
    recipient.domain = "luxesign";
    assert!(
        !auth.authorize(recipient.clone()).await.unwrap().allowed,
        "the recipient-bound actor is a signer actor, not a luxesign one"
    );
    recipient.domain = "signer";
    // The signer's document door: the recipient-bound actor, its own reserved action, a Query — nothing wider.
    let mut document = request("vault.signingDocument.read", OperationKind::Query, &[]);
    document.principal = None;
    document.actor = ServiceActor {
        id: Some("signature-recipient:7d1c0a52-0000-4000-8000-000000000001".into()),
        kind: ServiceActorKind::System,
    };
    document.domain = "vault";
    document.operation = "vault.signingDocumentBytes";
    assert!(
        auth.authorize(document.clone()).await.unwrap().allowed,
        "a signing recipient may read the document it is asked to sign"
    );
    let mut wider = document.clone();
    wider.operation = "vault.mediaBytes";
    wider.action = "vault.read";
    assert!(
        !auth.authorize(wider).await.unwrap().allowed,
        "a signing recipient may never open a Vault document of its own choosing"
    );
    let mut as_command = document.clone();
    as_command.kind = OperationKind::Command;
    assert!(
        !auth.authorize(as_command).await.unwrap().allowed,
        "the signing document door is a query"
    );
    let mut other_actor = document.clone();
    other_actor.actor.id = Some(crate::signer::LUXESIGN_EDGE_ACTOR.into());
    assert!(
        !auth.authorize(other_actor).await.unwrap().allowed,
        "the shared edge actor has no recipient to read for, so it has no document door"
    );
    let mut human = request(
        "vault.signingDocument.read",
        OperationKind::Query,
        &["vault.read"],
    );
    human.domain = "vault";
    human.operation = "vault.signingDocumentBytes";
    assert!(
        !auth.authorize(human).await.unwrap().allowed,
        "the signing document action is reserved: no role can grant it to a person"
    );

    recipient.actor.id = Some("signature-recipient:".into());
    assert!(
        !auth.authorize(recipient.clone()).await.unwrap().allowed,
        "a recipient actor must name a recipient"
    );
    recipient.actor.id = Some("signature-recipient-impostor".into());
    assert!(
        !auth.authorize(recipient).await.unwrap().allowed,
        "only the exact recipient-bound prefix is admitted"
    );

    let mut delivery = request("email.deliver", OperationKind::Command, &[]);
    delivery.principal = None;
    delivery.actor = ServiceActor {
        id: Some(crate::email::EMAIL_DELIVERY_ACTOR.into()),
        kind: ServiceActorKind::System,
    };
    delivery.domain = "email";
    delivery.operation = "email.deliver";
    assert!(auth.authorize(delivery.clone()).await.unwrap().allowed);
    delivery.operation = "email.queue";
    delivery.action = "email.queue";
    assert!(
        !auth.authorize(delivery).await.unwrap().allowed,
        "delivery worker may not manufacture email"
    );

    // A website lead's emails: the public website may command exactly this, and nobody else may.
    req.action = "website.lead.notify";
    req.operation = "website.notifyLead";
    req.kind = OperationKind::Command;
    assert!(auth.authorize(req.clone()).await.unwrap().allowed);
    req.operation = "website.notifyAnything";
    assert!(
        !auth.authorize(req.clone()).await.unwrap().allowed,
        "only the named operation"
    );
    req.operation = "website.notifyLead";
    req.actor.id = Some("authjs-edge".into());
    assert!(
        !auth.authorize(req.clone()).await.unwrap().allowed,
        "only the public website"
    );
    let mut granted = request(
        "website.lead.notify",
        OperationKind::Command,
        &["website.lead.notify"],
    );
    granted.operation = "website.notifyLead";
    assert!(
        !auth.authorize(granted).await.unwrap().allowed,
        "reserved: no role can grant it"
    );

    // ---- PUBLISHED actions (PUBLIC_READ_ACTIONS) ---------------------------------------------------------
    // The public site goes through the service kernel with no principal, so "may it read this" is decided here.
    // The rule this replaces was the open-stub default "GUEST may query", which would have allowed ALL of the
    // negatives below to an anonymous caller.
    let mut public_read = request("property.public.read", OperationKind::Query, &[]);
    public_read.principal = None;
    public_read.actor = ServiceActor {
        id: Some("public-website".into()),
        kind: ServiceActorKind::System,
    };
    assert!(
        auth.authorize(public_read.clone()).await.unwrap().allowed,
        "published listings may be read with no principal at all"
    );

    // Signed in, published data: still readable. Publication is not a privilege that sign-in removes.
    public_read.actor = ServiceActor {
        id: Some("u1".into()),
        kind: ServiceActorKind::User,
    };
    assert!(
        auth.authorize(public_read.clone()).await.unwrap().allowed,
        "an internal reader may read what an anonymous visitor may read"
    );

    // NOT PUBLISHED: the internal read action, which is how private property detail is reached.
    public_read.action = "property.read";
    assert!(
        !auth.authorize(public_read.clone()).await.unwrap().allowed,
        "private property detail is not published"
    );

    // NOT PUBLISHED: other people's data.
    for action in ["deal.read", "firm.read", model::security::ROLE_MANAGE] {
        public_read.action = action;
        assert!(
            !auth.authorize(public_read.clone()).await.unwrap().allowed,
            "{action} is not published"
        );
    }

    // PUBLICATION IS A READ: the same published action cannot be COMMANDED.
    public_read.action = "property.public.read";
    public_read.kind = OperationKind::Command;
    assert!(
        !auth.authorize(public_read).await.unwrap().allowed,
        "the site reads its listings; it does not write them"
    );
}

#[tokio::test]
async fn guest_sign_in_is_reserved_to_its_doors() {
    let auth = CasbinAuthorizationPort::new().await.unwrap();
    for (actor, operation, action) in [
        (
            "public-website",
            "security.requestGuestCode",
            "security.guestCode.request",
        ),
        (
            "public-website",
            "security.verifyGuestCode",
            "security.guestCode.verify",
        ),
        (
            services::AUTHJS_EDGE_ACTOR,
            "security.provisionGuest",
            "security.guest.provision",
        ),
    ] {
        let mut req = request(action, OperationKind::Command, &[]);
        req.principal = None;
        req.operation = operation;
        req.actor = ServiceActor {
            id: Some(actor.into()),
            kind: ServiceActorKind::System,
        };
        assert!(
            auth.authorize(req.clone()).await.unwrap().allowed,
            "{action}"
        );

        req.operation = "security.somethingElse";
        assert!(
            !auth.authorize(req.clone()).await.unwrap().allowed,
            "{action} borrowed"
        );
        req.operation = operation;

        req.actor.id = Some("someone-else".into());
        assert!(
            !auth.authorize(req).await.unwrap().allowed,
            "{action} other actor"
        );

        // Reserved: neither a grant nor ROOT opens it to a signed-in user.
        let mut granted = request(action, OperationKind::Command, &[action]);
        granted.operation = operation;
        granted.principal.as_mut().unwrap().role_codes = vec!["root".into()];
        assert!(
            !auth.authorize(granted).await.unwrap().allowed,
            "{action} granted"
        );
    }
}

#[tokio::test]
async fn agreement_execution_worker_has_one_reserved_contract_command() {
    let auth = CasbinAuthorizationPort::new().await.unwrap();
    let mut req = request("contract.execute", OperationKind::Command, &[]);
    req.domain = "contract";
    req.operation = "contract.execute";
    req.principal = None;
    req.actor = ServiceActor {
        id: Some("agreement-execution-worker".into()),
        kind: ServiceActorKind::System,
    };
    assert!(auth.authorize(req.clone()).await.unwrap().allowed);

    req.operation = "contract.saveDraft";
    assert!(!auth.authorize(req.clone()).await.unwrap().allowed);
    req.operation = "contract.execute";
    req.action = "contract.write";
    assert!(!auth.authorize(req.clone()).await.unwrap().allowed);
    req.action = "contract.execute";
    req.actor.id = Some("another-system".into());
    assert!(!auth.authorize(req).await.unwrap().allowed);
}

#[tokio::test]
async fn only_root_can_manage_role_entitlements() {
    let auth = CasbinAuthorizationPort::new().await.unwrap();
    let mut req = request(
        model::security::ENTITLEMENT_MANAGE,
        OperationKind::Command,
        &[model::security::ENTITLEMENT_MANAGE],
    );
    req.domain = "security";
    req.operation = "security.setRoleEntitlement";
    req.principal.as_mut().unwrap().role_codes = vec!["owner".into()];
    assert!(!auth.authorize(req.clone()).await.unwrap().allowed);

    req.principal.as_mut().unwrap().role_codes = vec!["root".into()];
    assert!(auth.authorize(req).await.unwrap().allowed);
}

/// ONE LIST, THREE READERS. `model::security::ROOT_ONLY_ACTIONS` is the list the portal offers from
/// (`ScreenCtx::can`), the database guards grants with (`SecurityDao::set_role_entitlement`) and this port enforces.
/// Every entry must be a catalogued command, refused to a non-root internal user who holds the grant, and allowed
/// to root — so a code renamed in one place and not another fails here, not in production.
#[tokio::test]
async fn every_root_only_action_is_catalogued_and_root_only() {
    let auth = CasbinAuthorizationPort::new().await.unwrap();
    assert!(!model::security::ROOT_ONLY_ACTIONS.is_empty());
    for action in model::security::ROOT_ONLY_ACTIONS {
        assert!(
            catalog::ACTIONS.contains(&(action, "command")),
            "{action} must be a catalogued command"
        );
        assert!(model::security::is_root_only(action));
        let mut req = request(action, OperationKind::Command, &[action]);
        req.domain = "security";
        req.principal.as_mut().unwrap().role_codes = vec!["owner".into()];
        assert!(
            !auth.authorize(req.clone()).await.unwrap().allowed,
            "{action}: owner holding the grant is refused"
        );
        req.principal.as_mut().unwrap().role_codes = vec!["root".into()];
        assert!(
            auth.authorize(req).await.unwrap().allowed,
            "{action}: root is allowed"
        );
    }
    assert!(
        !model::security::is_root_only("deal.read"),
        "an ordinary action is not root-only"
    );
}

#[tokio::test]
async fn only_root_can_manage_user_roles() {
    let auth = CasbinAuthorizationPort::new().await.unwrap();
    let mut req = request(
        model::security::ROLE_MANAGE,
        OperationKind::Command,
        &[model::security::ROLE_MANAGE],
    );
    req.domain = "security";
    req.operation = "security.setUserPrimaryRole";
    req.principal.as_mut().unwrap().role_codes = vec!["owner".into()];
    assert!(!auth.authorize(req.clone()).await.unwrap().allowed);

    req.principal.as_mut().unwrap().role_codes = vec!["root".into()];
    assert!(auth.authorize(req).await.unwrap().allowed);
}

#[tokio::test]
async fn external_account_may_read_only_the_client_room_self_projection() {
    let auth = CasbinAuthorizationPort::new().await.unwrap();
    let mut room = request("client.room.read", OperationKind::Query, &[]);
    room.domain = "client-room";
    room.operation = "clientRoom.snapshot";
    room.principal.as_mut().unwrap().account_type = "external".into();
    assert!(auth.authorize(room.clone()).await.unwrap().allowed);

    room.action = "person.read";
    assert!(!auth.authorize(room.clone()).await.unwrap().allowed);
    room.action = "client.room.read";
    room.operation = "clientRoom.someoneElse";
    assert!(!auth.authorize(room).await.unwrap().allowed);
}

#[tokio::test]
async fn external_account_cannot_use_an_internal_grant() {
    let auth = CasbinAuthorizationPort::new().await.unwrap();
    let mut req = request("person.read", OperationKind::Query, &["person.read"]);
    req.principal.as_mut().unwrap().account_type = "external".into();
    assert!(!auth.authorize(req).await.unwrap().allowed);
}

/// The `media.bytes` door: internal media bytes are authorized as
/// `property.read`, so an external account must not be able to hold that
/// action even with a grant naming it — otherwise unpublished media bytes
/// would be one guessed UUID away. (Anonymous callers are already refused
/// by the "not published" assertion above; the public path gates
/// publication separately in `public_listing.media_bytes` SQL.)
#[tokio::test]
async fn external_account_cannot_hold_property_read() {
    let auth = CasbinAuthorizationPort::new().await.unwrap();
    let mut req = request("property.read", OperationKind::Query, &["property.read"]);
    req.principal.as_mut().unwrap().account_type = "external".into();
    assert!(
        !auth.authorize(req).await.unwrap().allowed,
        "external accounts cannot hold property.read, grant or not"
    );
}
