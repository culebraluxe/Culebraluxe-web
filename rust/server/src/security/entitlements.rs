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
const PUBLIC_READ_ACTIONS: &[&str] = &["property.public.read", "guide.public.read"];

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
        // A WEBSITE LEAD'S EMAILS: the public website asks the server to email the team and the visitor about ONE
        // submission it has just saved. The command carries only the submission id; the server reads what the emails
        // say from the database and sends each lead's emails once (server/src/website_leads.rs).
        let lead_notice = system
            && request.actor.id.as_deref() == Some("public-website")
            && request.operation == "website.notifyLead"
            && request.action == "website.lead.notify"
            && request.kind == OperationKind::Command;
        // POOL DIAGNOSTICS: the internal key's System actor reads two row counts beside the pool metrics
        // (`/v1/diagnostics/db`). One query operation, nothing else.
        let db_diagnostics = system
            && request.actor.id.as_deref() == Some("workflow-engine")
            && request.operation == "support.dbDiagnostics"
            && request.action == "support.diagnostics.read"
            && request.kind == OperationKind::Query;
        let website_intake = system
            && request.actor.id.as_deref() == Some("public-website")
            && request.operation == "website.submitIntake"
            && request.action == "website.intake.submit"
            && request.kind == OperationKind::Command;
        // CRM-26 AGREEMENT EXECUTION: the MQ runtime may execute exactly one canonical Contract command
        // after it has verified immutable document lineage + the agreement_execution marker. This is not a
        // generic "system may command" grant: the actor, domain, operation, action and kind must all match.
        let agreement_execution = system
            && request.actor.id.as_deref() == Some("agreement-execution-worker")
            && request.domain == "contract"
            && request.operation == "contract.execute"
            && request.action == "contract.execute"
            && request.kind == OperationKind::Command;
        // NATIVE DOCUMENT SIGNING: the orchestrator may call exactly the canonical
        // service operations needed to prepare/issue a native envelope. This is service-to-service
        // authority, not a grant to an anonymous caller or to every System actor.
        let document_sign_service = system
            && request.actor.id.as_deref()
                == Some(crate::document_sign::DOCUMENT_SIGN_SERVICE_ACTOR)
            && request.kind == OperationKind::Command
            && matches!(
                (request.domain, request.operation, request.action),
                ("signature", "signature.prepare", "signature.write")
                    | (
                        "signature",
                        "signature.replaceRecipients",
                        "signature.write"
                    )
                    | ("signature", "signature.transition", "signature.write")
                    | ("signer", "signer.issueAccess", "signer.access.issue")
                    | ("email", "email.queue", "email.queue")
            );
        // The public signing edge has no portal principal. Its signed recipient capability is
        // validated by SignerService; Casbin admits only the signer operations named here.
        let document_sign_edge = system
            && request.actor.id.as_deref() == Some(crate::signer::DOCSIGN_EDGE_ACTOR)
            && request.domain == "signer"
            && matches!(
                (request.operation, request.action, request.kind),
                ("signer.session", "signer.read", OperationKind::Query)
                    | ("signer.open", "signer.act", OperationKind::Command)
                    | ("signer.acceptConsent", "signer.act", OperationKind::Command)
                    | ("signer.completeField", "signer.act", OperationKind::Command)
                    | ("signer.complete", "signer.act", OperationKind::Command)
                    | ("signer.decline", "signer.act", OperationKind::Command)
            );
        // Existing MQ owns retries/leases. The worker may deliver exactly one queued email and
        // cannot queue arbitrary messages or call any other application command.
        let email_delivery = system
            && request.actor.id.as_deref() == Some(crate::email::EMAIL_DELIVERY_ACTOR)
            && request.domain == "email"
            && request.operation == "email.deliver"
            && request.action == "email.deliver"
            && request.kind == OperationKind::Command;
        // GUEST SIGN-IN (security/guest.rs). The public website asks for and checks emailed codes for a visitor who
        // has no principal yet; the Auth.js edge provisions the external guest behind an identity it has proved.
        // A guest is an external account, so the principal branch below refuses it every grant.
        let guest_code = system
            && request.actor.id.as_deref() == Some("public-website")
            && request.kind == OperationKind::Command
            && matches!(
                (request.operation, request.action),
                ("security.requestGuestCode", "security.guestCode.request")
                    | ("security.verifyGuestCode", "security.guestCode.verify")
            );
        let guest_provision = system
            && request.actor.id.as_deref() == Some(service::AUTHJS_EDGE_ACTOR)
            && request.operation == "security.provisionGuest"
            && request.action == "security.guest.provision"
            && request.kind == OperationKind::Command;
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
        // CLIENT ROOM: an authenticated external account may read exactly its self-service projection.
        // The route derives the person id from the resolved ActingUser; the browser never supplies the subject.
        let client_room = request.action == "client.room.read"
            && request.domain == "client-room"
            && request.operation == "clientRoom.snapshot"
            && request.kind == OperationKind::Query
            && request.actor.kind == ServiceActorKind::User
            && request
                .principal
                .as_ref()
                .is_some_and(|principal| principal.account_type == "external");
        let explicit = bootstrap
            || public
            || lead_notice
            || db_diagnostics
            || website_intake
            || agreement_execution
            || document_sign_service
            || document_sign_edge
            || email_delivery
            || guest_code
            || guest_provision
            || published;
        let (allowed, policy_id) = if explicit {
            (true, "system:explicit")
        } else if identity_resolution {
            (true, "rule:identity.resolve.edge")
        } else if client_room {
            (true, "rule:client.room.external-self")
        } else if request.action == "security.identity.resolve"
            || request.action == "vault.publicListingDocument.read"
            || request.action == "website.lead.notify"
            || request.action == "website.intake.submit"
            || request.action == "email.deliver"
            || request.action.starts_with("security.guestCode.")
            || request.action == "security.guest.provision"
        {
            (false, "system:reserved")
        } else if let Some(principal) = request.principal.as_ref() {
            if principal.account_type != domain::security::INTERNAL_ACCOUNT {
                (false, "account:external")
            } else if domain::security::is_root_only(request.action) {
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
            } else if request.domain == "document-sign"
                && matches!(request.action, "documentSign.issue" | "documentSign.void")
                && principal.level != "BUSINESS_POWER_USER"
            {
                (false, "rule:document-sign.issue.level")
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

        let mut issue = request(
            "documentSign.issue",
            OperationKind::Command,
            &["documentSign.issue"],
        );
        issue.domain = "document-sign";
        issue.operation = "documentSign.issue";
        issue.principal.as_mut().unwrap().level = "USER".into();
        assert!(
            !auth.authorize(issue.clone()).await.unwrap().allowed,
            "USER must not issue a document even if a grant is accidentally present"
        );
        issue.principal.as_mut().unwrap().level = "BUSINESS_POWER_USER".into();
        assert!(
            auth.authorize(issue).await.unwrap().allowed,
            "BUSINESS_POWER_USER is the floor for documentSign.issue"
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
        let mut docsign = request("signature.write", OperationKind::Command, &[]);
        docsign.principal = None;
        docsign.actor = ServiceActor {
            id: Some(crate::document_sign::DOCUMENT_SIGN_SERVICE_ACTOR.into()),
            kind: ServiceActorKind::System,
        };
        docsign.domain = "signature";
        docsign.operation = "signature.prepare";
        assert!(auth.authorize(docsign.clone()).await.unwrap().allowed);
        docsign.operation = "signature.send";
        assert!(
            !auth.authorize(docsign.clone()).await.unwrap().allowed,
            "native orchestration must not inherit raw provider send authority"
        );

        let mut signer = request("signer.act", OperationKind::Command, &[]);
        signer.principal = None;
        signer.actor = ServiceActor {
            id: Some(crate::signer::DOCSIGN_EDGE_ACTOR.into()),
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
        for action in ["deal.read", "firm.read", domain::security::ROLE_MANAGE] {
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
                service::AUTHJS_EDGE_ACTOR,
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
            domain::security::ENTITLEMENT_MANAGE,
            OperationKind::Command,
            &[domain::security::ENTITLEMENT_MANAGE],
        );
        req.domain = "security";
        req.operation = "security.setRoleEntitlement";
        req.principal.as_mut().unwrap().role_codes = vec!["owner".into()];
        assert!(!auth.authorize(req.clone()).await.unwrap().allowed);

        req.principal.as_mut().unwrap().role_codes = vec!["root".into()];
        assert!(auth.authorize(req).await.unwrap().allowed);
    }

    /// ONE LIST, THREE READERS. `domain::security::ROOT_ONLY_ACTIONS` is the list the portal offers from
    /// (`ScreenCtx::can`), the database guards grants with (`SecurityDao::set_role_entitlement`) and this port enforces.
    /// Every entry must be a catalogued command, refused to a non-root internal user who holds the grant, and allowed
    /// to root — so a code renamed in one place and not another fails here, not in production.
    #[tokio::test]
    async fn every_root_only_action_is_catalogued_and_root_only() {
        let auth = CasbinAuthorizationPort::new().await.unwrap();
        assert!(!domain::security::ROOT_ONLY_ACTIONS.is_empty());
        for action in domain::security::ROOT_ONLY_ACTIONS {
            assert!(
                catalog::ACTIONS.contains(&(action, "command")),
                "{action} must be a catalogued command"
            );
            assert!(domain::security::is_root_only(action));
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
            !domain::security::is_root_only("deal.read"),
            "an ordinary action is not root-only"
        );
    }

    #[tokio::test]
    async fn only_root_can_manage_user_roles() {
        let auth = CasbinAuthorizationPort::new().await.unwrap();
        let mut req = request(
            domain::security::ROLE_MANAGE,
            OperationKind::Command,
            &[domain::security::ROLE_MANAGE],
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
}
