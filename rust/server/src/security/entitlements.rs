use async_trait::async_trait;
use casbin::{CoreApi, DefaultModel, Enforcer, MemoryAdapter, MgmtApi};
use service::{
    AuthorizationDecision, AuthorizationPort, AuthorizationRequest, OperationKind,
    ServiceActorKind, ServicePortError,
};

// The catalog is declared by `security` (its parent) so both this adapter and the API layer's authorize endpoint
// can name an action from the same list: the adapter seeds its policies from it, and the endpoint refuses any action
// it does not contain. One list, so a decision can never be asked about an action that does not exist.
use super::entitlement_catalog as catalog;

const MODEL: &str = r#"
[request_definition]
r = sub, obj, act
[policy_definition]
p = sub, obj, act
[policy_effect]
e = some(where (p_eft == allow))
[matchers]
m = r.sub == p.sub && r.obj == p.obj && r.act == p.act
"#;

/// The actions that are PUBLISHED: they may be run by anyone, queries only.
///
/// The rule this replaces was the open-stub default "GUEST may query", which is a hole rather than a decision — an
/// anonymous caller was allowed `deal.read`, `firm.read` and `security.listRoleEntitlements` too. The site publishes
/// ACTIVE LISTINGS; it does not publish the firm's deals. So publication is a NAMED list.
///
/// WHY AN ACTION AND NOT AN OPERATION. The decision endpoint derives an operation from the action, so a list of
/// operation names could never match here. It also has to be an action of its OWN rather than the internal
/// `property.read`: public listings and private property detail are both reads, and sharing one action would let the
/// public surface reach the detail — the guarantee would live in a code comment instead of in the policy.
///
/// WHY "ANYONE" RATHER THAN "THE PUBLIC ACTOR". An action on this list is published, so being signed in does not
/// remove the right to read it: an internal picker may read what an anonymous visitor may read. The public DOOR still
/// exists, because a visitor has no session and the identified door requires one.
const PUBLIC_READ_ACTIONS: &[&str] = &["property.public.read"];

/// The COMMANDS only the public website may run, as `(operation, action)` pairs, and nobody else — not even ROOT.
///
/// Each is a narrow thing the site asks the server to do for a visitor who has no principal: the request names only
/// what the server needs, and the server decides the rest from the database. Both halves must match, so a caller
/// cannot borrow one action for a different operation, and every action here is RESERVED: no role can be granted it.
///
/// - A website lead's emails (server/src/website_leads.rs): the command carries only the submission id.
/// - Visitor sign-in (server/src/visitor.rs): send an email code, check one, and resolve a signed-in visitor. A visitor
///   is an EXTERNAL GUEST held in its own tables; nothing here reaches `app_user`, so none of it can open the portal.
const PUBLIC_WEBSITE_COMMANDS: &[(&str, &str)] = &[
    ("website.notifyLead", "website.lead.notify"),
    ("visitor.requestSignInCode", "visitor.signin.request"),
    ("visitor.verifySignInCode", "visitor.signin.verify"),
    ("visitor.resolveSession", "visitor.session.resolve"),
];

/// Evaluates the grants supplied by the active user's DB roles against an explicit
/// Casbin action catalog. The database remains the source of truth for grants.
pub struct CasbinAuthorizationPort {
    enforcer: Enforcer,
}

impl CasbinAuthorizationPort {
    pub async fn new() -> Result<Self, casbin::Error> {
        let model = DefaultModel::from_str(MODEL).await?;
        let mut enforcer = Enforcer::new(model, MemoryAdapter::default()).await?;
        for &(action, kind) in catalog::ACTIONS {
            enforcer
                .add_policy(vec![action.to_owned(), action.to_owned(), kind.to_owned()])
                .await?;
        }
        Ok(Self { enforcer })
    }
}

#[async_trait]
impl AuthorizationPort for CasbinAuthorizationPort {
    async fn authorize(
        &self,
        request: AuthorizationRequest,
    ) -> Result<AuthorizationDecision, ServicePortError> {
        let system = request.principal.is_none() && request.actor.kind == ServiceActorKind::System;
        let bootstrap = system
            && request.actor.id.as_deref() == Some(service::AUTHJS_EDGE_ACTOR)
            && request.operation == "security.resolveIdentity"
            && request.action == "security.identity.resolve"
            && request.kind == OperationKind::Query;
        let public = system
            && request.actor.id.as_deref() == Some("public-website")
            && request.operation == "vault.publicListingDocumentBytes"
            && request.action == "vault.publicListingDocument.read"
            && request.kind == OperationKind::Query;
        // THE PUBLIC WEBSITE'S OWN COMMANDS (lead emails, visitor sign-in). See PUBLIC_WEBSITE_COMMANDS.
        let website_command = system
            && request.actor.id.as_deref() == Some("public-website")
            && request.kind == OperationKind::Command
            && PUBLIC_WEBSITE_COMMANDS.contains(&(request.operation, request.action));
        // PUBLISHED ACTIONS: anyone may run them, and only as a QUERY. Placed before the principal check because a
        // visitor has no principal at all — that is what "published" means. See PUBLIC_READ_ACTIONS.
        let published =
            request.kind == OperationKind::Query && PUBLIC_READ_ACTIONS.contains(&request.action);
        // THE EDGE MAY RESOLVE AN IDENTITY FOR A SESSION IT HOLDS.
        //
        // The kernel's login operation is authorized like every other operation — but this is the step that
        // ESTABLISHES who is calling, so the question is circular if it is asked about the caller. Rust grants
        // `security.identity.resolve` to the Auth.js edge actor, and the identified door cannot BE that actor: it
        // mints the actor from the session's own identity. So every signed-in user was refused the right to find out
        // that they had signed in: the resolution failed closed and the portal answered "not authorized".
        //
        // A request that arrives with a resolved USER is the edge speaking for that user's session — and the only
        // way to reach either door is the internal bridge key, a server-side credential the browser never holds. So
        // this is the edge's question, answered for the edge, without opening the action to anonymous callers: the
        // `system:reserved` branch below still refuses it for the public door and for anything without a session.
        let identity_resolution = request.action == "security.identity.resolve"
            && request.kind == OperationKind::Query
            && request.actor.kind == ServiceActorKind::User;
        let (allowed, policy_id) = if bootstrap || public || website_command || published {
            (true, "system:explicit")
        } else if identity_resolution {
            (true, "rule:identity.resolve.edge")
        } else if request.action == "security.identity.resolve"
            || request.action == "vault.publicListingDocument.read"
            || PUBLIC_WEBSITE_COMMANDS
                .iter()
                .any(|&(_, action)| action == request.action)
        {
            (false, "system:reserved")
        } else if let Some(principal) = request.principal.as_ref() {
            if principal.account_type != "internal" {
                (false, "account:external")
            } else if matches!(
                request.action,
                "security.entitlement.manage" | "security.role.manage"
            ) {
                if principal.role_codes.iter().any(|role| role == "root") {
                    (true, "rule:security.manage.root")
                } else {
                    (false, "rule:security.manage.root")
                }
            } else if principal
                .role_codes
                .iter()
                .any(|role| role == "root" || role == "owner")
            {
                (true, "role:root")
            } else if request.domain == "tech" || request.action.starts_with("tech.") {
                (false, "domain:tech")
            } else if request.domain == "contract"
                && request.operation == "contract.execute"
                && principal.level != "BUSINESS_POWER_USER"
            {
                (false, "rule:contract.execute.level")
            } else {
                let kind = match request.kind {
                    OperationKind::Query => "query",
                    OperationKind::Command => "command",
                };
                let mut granted = false;
                for code in &principal.entitlement_codes {
                    if self
                        .enforcer
                        .enforce((code.as_str(), request.action, kind))
                        .map_err(|error| ServicePortError::new(error.to_string()))?
                    {
                        granted = true;
                        break;
                    }
                }
                (granted, "entitlement:role-grant")
            }
        } else {
            (false, "principal:missing")
        };
        Ok(AuthorizationDecision {
            allowed,
            reason: if allowed {
                "allowed"
            } else {
                "no matching entitlement"
            }
            .into(),
            policy_id: policy_id.into(),
            mode: "enforced",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use service::{ServiceActor, ServicePrincipal};

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
        for action in ["deal.read", "firm.read", "security.role.manage"] {
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
    async fn visitor_sign_in_is_the_public_websites_alone() {
        let auth = CasbinAuthorizationPort::new().await.unwrap();
        for (operation, action) in [
            ("visitor.requestSignInCode", "visitor.signin.request"),
            ("visitor.verifySignInCode", "visitor.signin.verify"),
            ("visitor.resolveSession", "visitor.session.resolve"),
        ] {
            let mut req = request(action, OperationKind::Command, &[]);
            req.principal = None;
            req.operation = operation;
            req.actor = ServiceActor {
                id: Some("public-website".into()),
                kind: ServiceActorKind::System,
            };
            assert!(
                auth.authorize(req.clone()).await.unwrap().allowed,
                "{action}"
            );

            // Only as the operation it belongs to.
            req.operation = "website.notifyLead";
            assert!(
                !auth.authorize(req.clone()).await.unwrap().allowed,
                "{action} borrowed"
            );
            req.operation = operation;

            // Only for the public website.
            req.actor.id = Some("authjs-edge".into());
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
    async fn only_root_can_manage_role_entitlements() {
        let auth = CasbinAuthorizationPort::new().await.unwrap();
        let mut req = request(
            "security.entitlement.manage",
            OperationKind::Command,
            &["security.entitlement.manage"],
        );
        req.domain = "security";
        req.operation = "security.setRoleEntitlement";
        req.principal.as_mut().unwrap().role_codes = vec!["owner".into()];
        assert!(!auth.authorize(req.clone()).await.unwrap().allowed);

        req.principal.as_mut().unwrap().role_codes = vec!["root".into()];
        assert!(auth.authorize(req).await.unwrap().allowed);
    }

    #[tokio::test]
    async fn only_root_can_manage_user_roles() {
        let auth = CasbinAuthorizationPort::new().await.unwrap();
        let mut req = request(
            "security.role.manage",
            OperationKind::Command,
            &["security.role.manage"],
        );
        req.domain = "security";
        req.operation = "security.setUserPrimaryRole";
        req.principal.as_mut().unwrap().role_codes = vec!["owner".into()];
        assert!(!auth.authorize(req.clone()).await.unwrap().allowed);

        req.principal.as_mut().unwrap().role_codes = vec!["root".into()];
        assert!(auth.authorize(req).await.unwrap().allowed);
    }

    #[tokio::test]
    async fn external_account_cannot_use_an_internal_grant() {
        let auth = CasbinAuthorizationPort::new().await.unwrap();
        let mut req = request("person.read", OperationKind::Query, &["person.read"]);
        req.principal.as_mut().unwrap().account_type = "external".into();
        assert!(!auth.authorize(req).await.unwrap().allowed);
    }
}
