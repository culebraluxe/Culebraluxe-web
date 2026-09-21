mod identity_cache;

use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, SecurityDao};
use domain::{resolve_security_level, ActingUser, SecurityIdentityResolution, SecurityPrincipal};
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait SecurityRepository: Send {
    async fn resolve_provider_subject(
        &mut self,
        provider: &str,
        provider_subject: &str,
    ) -> DbResult<Option<String>>;
    async fn get_principal(&mut self, app_user_id: &str) -> DbResult<Option<ActingUser>>;
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
            Err(_) | Ok(None) => Ok(SecurityIdentityResolution::Unmapped),
            Ok(Some(app_user_id)) => match self.repository.get_principal(&app_user_id).await {
                Err(_) | Ok(None) => Ok(SecurityIdentityResolution::Inactive),
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
                Err(_) | Ok(None) => Ok(None),
                Ok(Some(acting_user)) => {
                    let level = resolve_security_level(&acting_user.role_codes);
                    Ok(Some(SecurityPrincipal { acting_user, level }))
                }
            };

        audit_result(&self.runtime, "security", OP, context, decision, &result).await?;
        result
    }
}
