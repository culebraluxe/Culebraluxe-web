mod identity_cache;
mod entitlements;
pub use entitlements::CasbinAuthorizationPort;

use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, SecurityDao};
use domain::{
    resolve_security_level,
    security::{RoleEntitlements, SecurityUserRoles},
    ActingUser, SecurityIdentityResolution, SecurityPrincipal,
};
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait SecurityRepository: Send {
    async fn resolve_provider_subject(
        &mut self,
        provider: &str,
        provider_subject: &str,
    ) -> DbResult<Option<String>>;
    async fn get_principal(&mut self, app_user_id: &str) -> DbResult<Option<ActingUser>>;
    async fn list_role_entitlements(&mut self) -> DbResult<Vec<RoleEntitlements>>;
    async fn set_role_entitlement(&mut self, role_code: &str, action: &str, granted: bool) -> DbResult<bool>;
    async fn list_security_users(&mut self) -> DbResult<Vec<SecurityUserRoles>>;
    async fn set_user_primary_role(&mut self, app_user_id: &str, role_code: &str) -> DbResult<bool>;
}

#[async_trait]
impl SecurityRepository for SecurityDao {
    async fn resolve_provider_subject(
        &mut self,
        provider: &str,
        provider_subject: &str,
    ) -> DbResult<Option<String>> {
        SecurityDao::resolve_provider_subject(self, provider, provider_subject).await
    }

    async fn get_principal(&mut self, app_user_id: &str) -> DbResult<Option<ActingUser>> {
        SecurityDao::get_principal(self, app_user_id).await
    }
    async fn list_role_entitlements(&mut self) -> DbResult<Vec<RoleEntitlements>> {
        SecurityDao::list_role_entitlements(self).await
    }
    async fn set_role_entitlement(&mut self, role_code: &str, action: &str, granted: bool) -> DbResult<bool> {
        SecurityDao::set_role_entitlement(self, role_code, action, granted).await
    }

    async fn list_security_users(&mut self) -> DbResult<Vec<SecurityUserRoles>> {
        SecurityDao::list_security_users(self).await
    }

    async fn set_user_primary_role(&mut self, app_user_id: &str, role_code: &str) -> DbResult<bool> {
        SecurityDao::set_user_primary_role(self, app_user_id, role_code).await
    }
}

pub struct SecurityService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: SecurityRepository> SecurityService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn resolve_identity(
        &mut self,
        provider: &str,
        provider_subject: &str,
        context: &ServiceContext,
    ) -> Result<SecurityIdentityResolution, CoreServiceError> {
        const OP: &str = "security.resolveIdentity";
        let decision = authorize(
            &self.runtime,
            "security",
            "security.identity.resolve",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;

        // The cache sits between the authorization decision and the two lookups, so the audit trail records this
        // operation on every request exactly as it did before; only the queries are skipped.
        if let Some(principal) = identity_cache::get(provider, provider_subject) {
            let result: Result<SecurityIdentityResolution, CoreServiceError> =
                Ok(SecurityIdentityResolution::Known(principal));
            audit_result(&self.runtime, "security", OP, context, decision, &result).await?;
            return result;
        }

        let result = match self
            .repository
            .resolve_provider_subject(provider, provider_subject)
            .await
        {
            Err(error) => Err(error.into()),
            Ok(None) => Ok(SecurityIdentityResolution::Unmapped),
            Ok(Some(app_user_id)) => match self.repository.get_principal(&app_user_id).await {
                Err(error) => Err(error.into()),
                Ok(None) => Ok(SecurityIdentityResolution::Inactive),
                Ok(Some(acting_user)) => {
                    let level = resolve_security_level(&acting_user.role_codes);
                    Ok(SecurityIdentityResolution::Known(SecurityPrincipal {
                        acting_user,
                        level,
                    }))
                }
            },
        };

        if let Ok(SecurityIdentityResolution::Known(principal)) = &result {
            identity_cache::put(provider, provider_subject, principal);
        }

        audit_result(&self.runtime, "security", OP, context, decision, &result).await?;
        result
    }

    pub async fn list_role_entitlements(
        &mut self,
        context: &ServiceContext,
    ) -> Result<Vec<RoleEntitlements>, CoreServiceError> {
        const OP: &str = "security.listRoleEntitlements";
        let decision = authorize(
            &self.runtime, "security", "security.principal.read", OP, OperationKind::Query, context,
        ).await?;
        let result: Result<Vec<RoleEntitlements>, CoreServiceError> =
            self.repository.list_role_entitlements().await.map_err(Into::into);
        audit_result(&self.runtime, "security", OP, context, decision, &result).await?;
        result
    }

    pub async fn set_role_entitlement(
        &mut self,
        role_code: &str,
        action: &str,
        granted: bool,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        const OP: &str = "security.setRoleEntitlement";
        let decision = authorize(
            &self.runtime, "security", "security.entitlement.manage", OP, OperationKind::Command, context,
        ).await?;
        let result = match self.repository.set_role_entitlement(role_code, action, granted).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(CoreServiceError::business("ENTITLEMENT_TARGET_UNKNOWN", "An active internal role and action are required.")),
            Err(error) => Err(error.into()),
        };
        audit_result(&self.runtime, "security", OP, context, decision, &result).await?;
        result
    }

    pub async fn list_security_users(
        &mut self,
        context: &ServiceContext,
    ) -> Result<Vec<SecurityUserRoles>, CoreServiceError> {
        const OP: &str = "security.listUsers";
        let decision = authorize(
            &self.runtime,
            "security",
            "security.principal.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result: Result<Vec<SecurityUserRoles>, CoreServiceError> =
            self.repository.list_security_users().await.map_err(Into::into);
        audit_result(&self.runtime, "security", OP, context, decision, &result).await?;
        result
    }

    pub async fn set_user_primary_role(
        &mut self,
        app_user_id: &str,
        role_code: &str,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        const OP: &str = "security.setUserPrimaryRole";
        let decision = authorize(
            &self.runtime,
            "security",
            "security.role.manage",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = match self
            .repository
            .set_user_primary_role(app_user_id, role_code)
            .await
        {
            Ok(true) => Ok(()),
            Ok(false) => Err(CoreServiceError::business(
                "SECURITY_ROLE_TARGET_INVALID",
                "An active internal user and canonical role are required, and the final ROOT cannot be removed.",
            )),
            Err(error) => Err(error.into()),
        };
        audit_result(&self.runtime, "security", OP, context, decision, &result).await?;
        result
    }

    pub async fn get_principal(
        &mut self,
        app_user_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<SecurityPrincipal>, CoreServiceError> {
        const OP: &str = "security.getPrincipal";
        let decision = authorize(
            &self.runtime,
            "security",
            "security.principal.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;

        let result: Result<Option<SecurityPrincipal>, CoreServiceError> =
            match self.repository.get_principal(app_user_id).await {
                Err(error) => Err(error.into()),
                Ok(None) => Ok(None),
                Ok(Some(acting_user)) => {
                    let level = resolve_security_level(&acting_user.role_codes);
                    Ok(Some(SecurityPrincipal { acting_user, level }))
                }
            };

        audit_result(&self.runtime, "security", OP, context, decision, &result).await?;
        result
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use db::{DbFailure, DbFailureKind};
    use service::{
        CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceActor,
        ServiceActorKind,
    };
    use std::sync::Arc;
    use uuid::Uuid;

    fn infrastructure() -> ServiceInfrastructure {
        ServiceInfrastructure::new(
            Arc::new(DefaultAuthorizationPort),
            Arc::new(CapturingAuditPort::default()),
            Arc::new(CapturingDomainEventPort::default()),
        )
    }

    fn context() -> ServiceContext {
        ServiceContext {
            actor: ServiceActor {
                id: Some("authjs-edge".into()),
                kind: ServiceActorKind::System,
            },
            correlation_id: "security-test".into(),
            causation_id: None,
            principal: None,
        }
    }

    fn transient(operation: &'static str) -> DbFailure {
        DbFailure {
            kind: DbFailureKind::DatabaseUnavailable,
            operation,
            incident_id: Uuid::nil(),
            code: None,
            detail: None,
            retryable: true,
        }
    }

    struct IdentityLookupFailure;

    #[async_trait]
    impl SecurityRepository for IdentityLookupFailure {
        async fn resolve_provider_subject(
            &mut self,
            _provider: &str,
            _provider_subject: &str,
        ) -> DbResult<Option<String>> {
            Err(transient("security.resolve_provider_subject"))
        }

        async fn get_principal(&mut self, _app_user_id: &str) -> DbResult<Option<ActingUser>> {
            unreachable!("principal lookup must not run after identity lookup failure")
        }
        async fn list_role_entitlements(&mut self) -> DbResult<Vec<RoleEntitlements>> {
            Ok(Vec::new())
        }
        async fn set_role_entitlement(&mut self, _role_code: &str, _action: &str, _granted: bool) -> DbResult<bool> {
            unreachable!("grant mutation must not run after identity lookup failure")
        }

        async fn list_security_users(&mut self) -> DbResult<Vec<SecurityUserRoles>> {
            Ok(Vec::new())
        }
        async fn set_user_primary_role(&mut self, _app_user_id: &str, _role_code: &str) -> DbResult<bool> {
            unreachable!("role mutation must not run after identity lookup failure")
        }
    }

    struct PrincipalLookupFailure;

    #[async_trait]
    impl SecurityRepository for PrincipalLookupFailure {
        async fn resolve_provider_subject(
            &mut self,
            _provider: &str,
            _provider_subject: &str,
        ) -> DbResult<Option<String>> {
            Ok(Some("user-1".into()))
        }

        async fn get_principal(&mut self, _app_user_id: &str) -> DbResult<Option<ActingUser>> {
            Err(transient("security.get_principal"))
        }
        async fn list_role_entitlements(&mut self) -> DbResult<Vec<RoleEntitlements>> {
            Err(transient("security.list_role_entitlements"))
        }
        async fn set_role_entitlement(&mut self, _role_code: &str, _action: &str, _granted: bool) -> DbResult<bool> {
            Err(transient("security.set_role_entitlement"))
        }

        async fn list_security_users(&mut self) -> DbResult<Vec<SecurityUserRoles>> {
            Err(transient("security.list_security_users"))
        }
        async fn set_user_primary_role(&mut self, _app_user_id: &str, _role_code: &str) -> DbResult<bool> {
            Err(transient("security.set_user_primary_role"))
        }
    }

    #[tokio::test]
    async fn identity_database_failure_is_not_reported_as_unmapped() {
        let mut service = SecurityService::new(IdentityLookupFailure, infrastructure());
        let result = service
            .resolve_identity("test-provider-db-failure", "subject-1", &context())
            .await;

        assert!(matches!(result, Err(CoreServiceError::Database(_))));
    }

    #[tokio::test]
    async fn principal_database_failure_is_not_reported_as_inactive() {
        let mut service = SecurityService::new(PrincipalLookupFailure, infrastructure());
        let result = service
            .resolve_identity("test-provider-principal-failure", "subject-2", &context())
            .await;

        assert!(matches!(result, Err(CoreServiceError::Database(_))));
    }

    #[tokio::test]
    async fn direct_principal_database_failure_is_not_reported_as_missing() {
        let mut service = SecurityService::new(PrincipalLookupFailure, infrastructure());
        let result = service.get_principal("user-1", &context()).await;

        assert!(matches!(result, Err(CoreServiceError::Database(_))));
    }
}
