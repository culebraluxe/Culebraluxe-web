use async_trait::async_trait;
use casbin::{CoreApi, DefaultModel, Enforcer, MemoryAdapter, MgmtApi};
use services::{
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
            && request.actor.id.as_deref() == Some(services::AUTHJS_EDGE_ACTOR)
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
        let luxesign_service = system
            && request.actor.id.as_deref() == Some(crate::luxesign::LUXESIGN_SERVICE_ACTOR)
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
        //
        // TWO ACTORS, ONE RULE. The edge reads a session as itself (`luxesign-edge`), then runs every mutation as
        // the RECIPIENT-BOUND actor `signature-recipient:<id>`, derived from the verified token and never from request
        // JSON (`api/routes/signer_edge.rs`), which `SignerService` re-checks against the capability's own recipient. This
        // rule admitted only the shared edge actor, so every signer's open, consent, field and complete was refused
        // `no matching entitlement` over HTTP while the service-level proofs (which run as the edge actor) stayed green.
        // The recipient-bound actor is admitted for exactly the five signer commands; the edge actor keeps all six.
        let signer_command = |operation: &str, action: &str| {
            matches!(
                (operation, action),
                ("signer.open", "signer.act")
                    | ("signer.acceptConsent", "signer.act")
                    | ("signer.completeField", "signer.act")
                    | ("signer.complete", "signer.act")
                    | ("signer.decline", "signer.act")
            )
        };
        let signing_recipient = system
            && request.domain == "signer"
            && request.kind == OperationKind::Command
            && request
                .actor
                .id
                .as_deref()
                .and_then(|id| id.strip_prefix("signature-recipient:"))
                .is_some_and(|recipient| !recipient.trim().is_empty())
            && signer_command(request.operation, request.action);
        // THE SIGNER'S DOCUMENT. The recipient-bound actor may read the one document it is asked to sign — and only
        // through the Vault's own narrow door, which proves in SQL that the recipient belongs to an issued, live
        // envelope. Not `vault.read`: a signing recipient may never open a Vault document of its own choosing.
        let signing_document = system
            && request.domain == "vault"
            && matches!(
                request.operation,
                "vault.signingDocumentBytes" | "vault.signingSignedBytes"
            )
            && request.action == "vault.signingDocument.read"
            && request.kind == OperationKind::Query
            && request
                .actor
                .id
                .as_deref()
                .and_then(|id| id.strip_prefix("signature-recipient:"))
                .is_some_and(|recipient| !recipient.trim().is_empty());
        let luxesign_edge = signing_document
            || signing_recipient
            || (system
                && request.actor.id.as_deref() == Some(crate::signer::LUXESIGN_EDGE_ACTOR)
                && request.domain == "signer"
                && (matches!(
                    (request.operation, request.action, request.kind),
                    ("signer.session", "signer.read", OperationKind::Query)
                ) || (request.kind == OperationKind::Command
                    && signer_command(request.operation, request.action))));
        // THE SIGNING WORKERS. Each is one system actor with one job. The finalizer seals a fully signed envelope: it
        // runs `luxesign.finalize`, and reads that one document's PDF through the Vault (the caller's authority,
        // which for the automatic path is its own). The sweeper expires overdue signers and queues reminders.
        let signing_finalizer = system
            && request.actor.id.as_deref()
                == Some(crate::luxesign::worker::LUXESIGN_FINALIZER_ACTOR)
            && ((request.kind == OperationKind::Command
                && request.domain == "luxesign"
                && request.operation == "luxesign.finalize"
                && request.action == "luxesign.write")
                || (request.kind == OperationKind::Query
                    && request.domain == "vault"
                    && matches!(request.operation, "vault.getDocument" | "vault.mediaBytes")
                    && request.action == "vault.read"));
        let signing_sweeper = system
            && request.actor.id.as_deref() == Some(crate::luxesign::worker::LUXESIGN_SWEEPER_ACTOR)
            && request.kind == OperationKind::Command
            && request.domain == "luxesign"
            && request.operation == "luxesign.sweepDue"
            && request.action == "luxesign.write";
        // THE COMPLETION EMAIL'S FILES. The delivery worker reads the sealed document and its certificate for one
        // envelope, through the Vault's own door, to attach them. Nothing else may use this action.
        let completion_artifacts = system
            && request.actor.id.as_deref() == Some(crate::email::EMAIL_DELIVERY_ACTOR)
            && request.domain == "vault"
            && request.operation == "vault.completionArtifacts"
            && request.action == "vault.completionArtifacts.read"
            && request.kind == OperationKind::Query;
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
            && request.actor.id.as_deref() == Some(services::AUTHJS_EDGE_ACTOR)
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
            || luxesign_service
            || luxesign_edge
            || signing_finalizer
            || signing_sweeper
            || completion_artifacts
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
            || request.action == "vault.signingDocument.read"
            || request.action == "vault.completionArtifacts.read"
            || request.action == "website.lead.notify"
            || request.action == "website.intake.submit"
            || request.action == "email.deliver"
            || request.action.starts_with("security.guestCode.")
            || request.action == "security.guest.provision"
        {
            (false, "system:reserved")
        } else if let Some(principal) = request.principal.as_ref() {
            if principal.account_type != model::security::INTERNAL_ACCOUNT {
                (false, "account:external")
            } else if model::security::is_root_only(request.action) {
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
            } else if request.domain == "luxesign"
                && matches!(request.action, "luxesign.issue" | "luxesign.void")
                && principal.level != "BUSINESS_POWER_USER"
            {
                (false, "rule:luxesign.issue.level")
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
mod tests;
