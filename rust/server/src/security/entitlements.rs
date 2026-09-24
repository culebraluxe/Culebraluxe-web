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
        let (allowed, policy_id) = if bootstrap || public {
            (true, "system:explicit")
        } else if request.action == "security.identity.resolve"
            || request.action == "vault.publicListingDocument.read"
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
        req.operation = "security.getPrincipal";
        assert!(!auth.authorize(req.clone()).await.unwrap().allowed);
        req.actor.id = Some("public-website".into());
        req.action = "vault.publicListingDocument.read";
        req.operation = "vault.publicListingDocumentBytes";
        assert!(auth.authorize(req.clone()).await.unwrap().allowed);
        req.action = "vault.read";
        assert!(!auth.authorize(req).await.unwrap().allowed);
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
