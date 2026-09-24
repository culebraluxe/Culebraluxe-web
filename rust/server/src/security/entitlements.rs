use async_trait::async_trait;
use casbin::{CoreApi, DefaultModel, Enforcer, MgmtApi, MemoryAdapter};
use service::{
    AuthorizationDecision, AuthorizationPort, AuthorizationRequest, OperationKind,
    ServiceActorKind, ServicePortError,
};

mod catalog {
    include!("entitlement_catalog.rs");
}

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
        let system = request.principal.is_none()
            && request.actor.kind == ServiceActorKind::System;
        let bootstrap = system
            && request.actor.id.as_deref() == Some("authjs-edge")
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
            } else if principal.role_codes.iter().any(|role| role == "root" || role == "owner") {
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
                    if self.enforcer.enforce((code.as_str(), request.action, kind))
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
            reason: if allowed { "allowed" } else { "no matching entitlement" }.into(),
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
            actor: ServiceActor { id: Some("u1".into()), kind: ServiceActorKind::User },
            principal: Some(ServicePrincipal {
                app_user_id: "u1".into(), level: "USER".into(),
                role_codes: vec!["user".into()], account_type: "internal".into(),
                entitlement_codes: grants.iter().map(|s| s.to_string()).collect(),
            }),
        }
    }

    #[tokio::test]
    async fn action_and_kind_must_both_match_a_role_grant() {
        let auth = CasbinAuthorizationPort::new().await.unwrap();
        assert!(auth.authorize(request("person.read", OperationKind::Query, &["person.read"])).await.unwrap().allowed);
        assert!(!auth.authorize(request("person.write", OperationKind::Command, &["person.read"])).await.unwrap().allowed);
        assert!(!auth.authorize(request("person.read", OperationKind::Command, &["person.read"])).await.unwrap().allowed);
        assert!(!auth.authorize(request("future.read", OperationKind::Query, &["future.read"])).await.unwrap().allowed);
    }

    #[tokio::test]
    async fn only_explicit_system_bootstrap_and_public_document_are_open() {
        let auth = CasbinAuthorizationPort::new().await.unwrap();
        let mut req = request("security.identity.resolve", OperationKind::Query, &[]);
        req.principal = None;
        req.actor = ServiceActor { id: Some("authjs-edge".into()), kind: ServiceActorKind::System };
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
    async fn external_account_cannot_use_an_internal_grant() {
        let auth = CasbinAuthorizationPort::new().await.unwrap();
        let mut req = request("person.read", OperationKind::Query, &["person.read"]);
        req.principal.as_mut().unwrap().account_type = "external".into();
        assert!(!auth.authorize(req).await.unwrap().allowed);
    }
}
