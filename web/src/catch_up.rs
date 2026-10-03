use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{CatchUpDao, DbResult};
use model::CatchUpSnapshot;
use services::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait CatchUpRepository: Send {
    async fn snapshot(&self) -> DbResult<CatchUpSnapshot>;
    async fn handle(&self, person_id: &str, reason_code: &str) -> DbResult<()>;
    async fn snooze(&self, person_id: &str, reason_code: &str, days: i32) -> DbResult<()>;
}

#[async_trait]
impl CatchUpRepository for CatchUpDao {
    async fn snapshot(&self) -> DbResult<CatchUpSnapshot> {
        CatchUpDao::snapshot(self).await
    }

    async fn handle(&self, person_id: &str, reason_code: &str) -> DbResult<()> {
        CatchUpDao::handle(self, person_id, reason_code).await
    }

    async fn snooze(&self, person_id: &str, reason_code: &str, days: i32) -> DbResult<()> {
        CatchUpDao::snooze(self, person_id, reason_code, days).await
    }
}

pub struct CatchUpService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: CatchUpRepository> CatchUpService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn snapshot(
        &self,
        context: &ServiceContext,
    ) -> Result<CatchUpSnapshot, CoreServiceError> {
        const OP: &str = "catchUp.snapshot";
        let decision = authorize(
            &self.runtime,
            "catch-up",
            "portal.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.snapshot().await.map_err(Into::into);
        audit_result(&self.runtime, "catch-up", OP, context, decision, &result).await?;
        result
    }

    pub async fn handle(
        &self,
        person_id: &str,
        reason_code: &str,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        const OP: &str = "catchUp.handle";
        let decision = authorize(
            &self.runtime,
            "catch-up",
            "crm.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            let person_id = required(person_id, "CATCH_UP_PERSON_REQUIRED", "Person is required.")?;
            let reason_code = required(
                reason_code,
                "CATCH_UP_REASON_REQUIRED",
                "Catch-Up reason is required.",
            )?;
            self.repository
                .handle(person_id, reason_code)
                .await
                .map_err(Into::into)
        }
        .await;
        audit_result(&self.runtime, "catch-up", OP, context, decision, &result).await?;
        result
    }

    pub async fn snooze(
        &self,
        person_id: &str,
        reason_code: &str,
        days: i32,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        const OP: &str = "catchUp.snooze";
        let decision = authorize(
            &self.runtime,
            "catch-up",
            "crm.write",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = async {
            let person_id = required(person_id, "CATCH_UP_PERSON_REQUIRED", "Person is required.")?;
            let reason_code = required(
                reason_code,
                "CATCH_UP_REASON_REQUIRED",
                "Catch-Up reason is required.",
            )?;
            if !(1..=30).contains(&days) {
                return Err(CoreServiceError::business(
                    "CATCH_UP_SNOOZE_INVALID",
                    "Snooze must be between 1 and 30 days.",
                ));
            }
            self.repository
                .snooze(person_id, reason_code, days)
                .await
                .map_err(Into::into)
        }
        .await;
        audit_result(&self.runtime, "catch-up", OP, context, decision, &result).await?;
        result
    }
}

fn required<'a>(
    value: &'a str,
    code: &'static str,
    message: &'static str,
) -> Result<&'a str, CoreServiceError> {
    let value = value.trim();
    if value.is_empty() {
        Err(CoreServiceError::business(code, message))
    } else {
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use services::{
        CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceActor,
        ServiceActorKind, ServicePrincipal,
    };
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Fake {
        actions: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl CatchUpRepository for Arc<Fake> {
        async fn snapshot(&self) -> DbResult<CatchUpSnapshot> {
            Ok(CatchUpSnapshot::default())
        }
        async fn handle(&self, person_id: &str, reason_code: &str) -> DbResult<()> {
            self.actions
                .lock()
                .unwrap()
                .push(format!("handle:{person_id}:{reason_code}"));
            Ok(())
        }
        async fn snooze(&self, person_id: &str, reason_code: &str, days: i32) -> DbResult<()> {
            self.actions
                .lock()
                .unwrap()
                .push(format!("snooze:{person_id}:{reason_code}:{days}"));
            Ok(())
        }
    }

    /// A context that may COMMAND (the tests below exercise `handle`/`snooze`, both
    /// `OperationKind::Command`). The default authorization port reads a missing principal as GUEST
    /// and refuses commands (`default:guest.command-deny`), so a harness that commands must carry a
    /// principal — see `a_command_without_a_principal_is_forbidden_...` for the refusal pinned.
    fn context(
        actor_kind: ServiceActorKind,
        principal: Option<ServicePrincipal>,
    ) -> ServiceContext {
        ServiceContext {
            actor: ServiceActor {
                id: Some("tester".into()),
                kind: actor_kind,
            },
            correlation_id: "catch-up-test".into(),
            causation_id: None,
            principal,
        }
    }

    fn business_power_user() -> ServicePrincipal {
        ServicePrincipal {
            app_user_id: "tester".into(),
            level: "BUSINESS_POWER_USER".into(),
            role_codes: vec![],
            account_type: "internal".into(),
            entitlement_codes: vec![],
        }
    }

    fn service() -> (CatchUpService<Arc<Fake>>, Arc<Fake>) {
        let fake = Arc::new(Fake::default());
        let infrastructure = ServiceInfrastructure::new(
            Arc::new(DefaultAuthorizationPort),
            Arc::new(CapturingAuditPort::default()),
            Arc::new(CapturingDomainEventPort::default()),
        );
        (CatchUpService::new(fake.clone(), infrastructure), fake)
    }

    fn harness() -> (CatchUpService<Arc<Fake>>, Arc<Fake>, ServiceContext) {
        let (service, fake) = service();
        (
            service,
            fake,
            context(ServiceActorKind::User, Some(business_power_user())),
        )
    }

    #[tokio::test]
    async fn snooze_is_bounded_and_handle_is_a_repository_command() {
        let (service, fake, context) = harness();
        service.handle("p1", "quiet", &context).await.unwrap();
        service.snooze("p2", "inbound", 3, &context).await.unwrap();
        assert_eq!(
            fake.actions.lock().unwrap().as_slice(),
            ["handle:p1:quiet", "snooze:p2:inbound:3"]
        );
        let error = service
            .snooze("p2", "inbound", 31, &context)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Snooze"));
    }

    /// The other half of the same rule, and the reason this harness now carries a principal: a
    /// command with no principal is GUEST, GUEST cannot command, and the refusal must happen
    /// BEFORE the repository — `default:guest.command-deny`. If this test ever fails by passing
    /// (the command went through), the deny was loosened, not the test fixed.
    #[tokio::test]
    async fn a_command_without_a_principal_is_forbidden_and_never_reaches_the_repository() {
        let (service, fake) = service();
        let context = context(ServiceActorKind::System, None);

        let error = service.handle("p1", "quiet", &context).await.unwrap_err();
        assert_eq!(error.code(), "FORBIDDEN");

        let error = service
            .snooze("p1", "quiet", 3, &context)
            .await
            .unwrap_err();
        assert_eq!(error.code(), "FORBIDDEN");

        assert!(fake.actions.lock().unwrap().is_empty());
    }
}
