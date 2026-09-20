use crate::{OperationKind, ServiceActor, ServicePortError, ServicePrincipal};
use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationRequest {
    pub domain: &'static str,
    pub action: &'static str,
    pub operation: &'static str,
    pub kind: OperationKind,
    pub actor: ServiceActor,
    pub principal: Option<ServicePrincipal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationDecision {
    pub allowed: bool,
    pub reason: String,
    pub policy_id: String,
    pub mode: &'static str,
}

#[async_trait]
pub trait AuthorizationPort: Send + Sync {
    async fn authorize(
        &self,
        request: AuthorizationRequest,
    ) -> Result<AuthorizationDecision, ServicePortError>;
}

/// First Rust authorization adapter. It mirrors the current TypeScript kernel's
/// kind-based default: ROOT always passes, queries are readable by default,
/// GUEST/missing principals cannot command, and authenticated principals may
/// command unless a more specific policy adapter replaces this port.
#[derive(Debug, Default)]
pub struct DefaultAuthorizationPort;

#[async_trait]
impl AuthorizationPort for DefaultAuthorizationPort {
    async fn authorize(
        &self,
        request: AuthorizationRequest,
    ) -> Result<AuthorizationDecision, ServicePortError> {
        let level = request
            .principal
            .as_ref()
            .map(|principal| principal.level.as_str())
            .unwrap_or("GUEST");

        if level == "ROOT" {
            return Ok(AuthorizationDecision {
                allowed: true,
                reason: "ROOT superuser".into(),
                policy_id: "authorization:root".into(),
                mode: "enforced",
            });
        }

        if request.domain == "contract" && request.operation == "contract.execute" {
            let rank = match level {
                "ROOT" => 3,
                "BUSINESS_POWER_USER" => 2,
                "USER" => 1,
                _ => 0,
            };
            let allowed = rank >= 2;
            return Ok(AuthorizationDecision {
                allowed,
                reason: if allowed {
                    format!("rule:contract.execute: {level} meets required BUSINESS_POWER_USER")
                } else {
                    format!("rule:contract.execute: {level} is below required BUSINESS_POWER_USER")
                },
                policy_id: "rule:contract.execute".into(),
                mode: "enforced",
            });
        }

        if request.kind == OperationKind::Query {
            return Ok(AuthorizationDecision {
                allowed: true,
                reason: if level == "GUEST" {
                    "default: GUEST query allowed".into()
                } else {
                    "default: query allowed".into()
                },
                policy_id: "default:query".into(),
                mode: "enforced",
            });
        }

        if level == "GUEST" {
            return Ok(AuthorizationDecision {
                allowed: false,
                reason: "default: GUEST (or missing principal) cannot run commands".into(),
                policy_id: "default:guest.command-deny".into(),
                mode: "enforced",
            });
        }

        Ok(AuthorizationDecision {
            allowed: true,
            reason: format!("default: {level} command allowed"),
            policy_id: "default:command".into(),
            mode: "enforced",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ServiceActorKind, ServiceContext};

    fn request(kind: OperationKind, level: Option<&str>) -> AuthorizationRequest {
        let context = ServiceContext {
            actor: ServiceActor {
                id: None,
                kind: ServiceActorKind::System,
            },
            correlation_id: "test".into(),
            causation_id: None,
            principal: level.map(|value| ServicePrincipal {
                app_user_id: "test-user".into(),
                level: value.into(),
                role_codes: vec![],
            }),
        };

        AuthorizationRequest {
            domain: "project",
            action: "project.write",
            operation: "project.create",
            kind,
            actor: context.actor,
            principal: context.principal,
        }
    }

    #[tokio::test]
    async fn guest_commands_fail_closed() {
        let decision = DefaultAuthorizationPort
            .authorize(request(OperationKind::Command, None))
            .await
            .unwrap();
        assert!(!decision.allowed);
    }

    #[tokio::test]
    async fn guest_queries_are_allowed_like_typescript_kernel() {
        let decision = DefaultAuthorizationPort
            .authorize(request(OperationKind::Query, None))
            .await
            .unwrap();
        assert!(decision.allowed);
    }
}
