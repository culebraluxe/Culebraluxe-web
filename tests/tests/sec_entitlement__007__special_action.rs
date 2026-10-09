//! SEC.ENTITLEMENT — Special action: ROOT-only entitlement management (TST-SEC-ENTITLEMENT-007).
//!
//! Contract: `security.entitlement.manage` and `security.role.manage` are ROOT-only actions;
//! holding the grant is not sufficient for non-ROOT users.
//!
//! Level: L2 Persistence — exercises the service against a real (DEV) database via the
//! harness's `TestDatabase`, then rolls back.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_entitlement__007__special_action

use db::SecurityDao;
use model::security;
use services::{
    CapturingAuditPort, CapturingDomainEventPort, ServiceActor, ServiceActorKind, ServiceContext,
    ServiceInfrastructure, ServicePrincipal,
};
use std::sync::Arc;
use test_harness::database::TestDatabase;
use web::security::{CasbinAuthorizationPort, SecurityService};

const HARNESS: &str = "TestDatabase/L2 Persistence";

async fn test_infrastructure(audit: Arc<CapturingAuditPort>) -> ServiceInfrastructure {
    let auth_port = Arc::new(
        CasbinAuthorizationPort::new()
            .await
            .expect("{HARNESS}: CasbinAuthorizationPort builds"),
    );
    ServiceInfrastructure::new(
        auth_port,
        audit,
        Arc::new(CapturingDomainEventPort::default()),
    )
}

fn root_context(correlation_id: &str) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("root-test-user".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: correlation_id.into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "root-test-user".into(),
            level: "ROOT".into(),
            role_codes: vec!["root".into()],
            account_type: "internal".into(),
            entitlement_codes: vec![
                "security.principal.read".into(),
                security::ENTITLEMENT_MANAGE.into(),
            ],
        }),
    }
}

fn owner_context(correlation_id: &str) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("owner-test-user".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: correlation_id.into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "owner-test-user".into(),
            level: "ROOT".into(),
            role_codes: vec!["owner".into()],
            account_type: "internal".into(),
            entitlement_codes: vec![
                "security.principal.read".into(),
                security::ENTITLEMENT_MANAGE.into(),
                security::ROLE_MANAGE.into(),
            ],
        }),
    }
}

fn business_power_user_context(correlation_id: &str) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("bpu-test-user".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: correlation_id.into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "bpu-test-user".into(),
            level: "BUSINESS_POWER_USER".into(),
            role_codes: vec!["business_power_user".into()],
            account_type: "internal".into(),
            entitlement_codes: vec![
                "security.principal.read".into(),
                security::ENTITLEMENT_MANAGE.into(),
                security::ROLE_MANAGE.into(),
            ],
        }),
    }
}

#[tokio::test]
#[allow(non_snake_case)]
async fn sec_entitlement_007__special_action() {
    let db = TestDatabase::connect_from_env()
        .await
        .expect("{HARNESS}: connect to DEV database");
    let dao = SecurityDao::new(db.database().clone());
    let audit = Arc::new(CapturingAuditPort::default());
    let infrastructure = test_infrastructure(audit.clone()).await;
    let service = SecurityService::new(dao, infrastructure);

    let role_code = "test_role_special";
    let action = "property.read";

    // Seed role
    sqlx::query("insert into security_role (code, name, active, account_type) values ($1, $2, true, 'internal') on conflict (code) do nothing")
        .bind(role_code).bind("Test Role Special").execute(db.database().pool()).await.expect("{HARNESS}: insert role");

    // 1. ROOT can manage entitlements
    let root_ctx = root_context("sec-ent-007-root");
    let root_grant = service
        .set_role_entitlement(role_code, action, true, &root_ctx)
        .await;
    assert!(
        root_grant.is_ok(),
        "{HARNESS}: ROOT can grant: {:?}",
        root_grant.err()
    );

    let root_revoke = service
        .set_role_entitlement(role_code, action, false, &root_ctx)
        .await;
    assert!(
        root_revoke.is_ok(),
        "{HARNESS}: ROOT can revoke: {:?}",
        root_revoke.err()
    );

    // 2. OWNER (ROOT-level for other purposes) CANNOT manage entitlements (requires "root" role code)
    let owner_ctx = owner_context("sec-ent-007-owner");
    let owner_grant = service
        .set_role_entitlement(role_code, action, true, &owner_ctx)
        .await;
    assert!(
        owner_grant.is_err(),
        "{HARNESS}: OWNER cannot manage entitlements"
    );
    assert!(
        matches!(
            owner_grant.unwrap_err(),
            web::service_support::CoreServiceError::Runtime(
                services::ServiceRuntimeError::Forbidden { .. }
            )
        ),
        "{HARNESS}: OWNER error is FORBIDDEN"
    );

    // 3. BUSINESS_POWER_USER with grant CANNOT manage entitlements
    let bpu_ctx = business_power_user_context("sec-ent-007-bpu");
    let bpu_grant = service
        .set_role_entitlement(role_code, action, true, &bpu_ctx)
        .await;
    assert!(
        bpu_grant.is_err(),
        "{HARNESS}: BUSINESS_POWER_USER with grant cannot manage entitlements"
    );
    assert!(
        matches!(
            bpu_grant.unwrap_err(),
            web::service_support::CoreServiceError::Runtime(
                services::ServiceRuntimeError::Forbidden { .. }
            )
        ),
        "{HARNESS}: BPU error is FORBIDDEN"
    );

    // 4. ROOT can manage ROOT-only actions (ENTITLEMENT_MANAGE, ROLE_MANAGE)
    let root_manage = service
        .set_role_entitlement("root", security::ENTITLEMENT_MANAGE, true, &root_ctx)
        .await;
    assert!(
        root_manage.is_ok(),
        "{HARNESS}: ROOT can grant ENTITLEMENT_MANAGE to root role"
    );

    // 5. NEGATIVE: non-root cannot grant ROOT-only actions to any role
    let bpu_root_action = service
        .set_role_entitlement(role_code, security::ENTITLEMENT_MANAGE, true, &bpu_ctx)
        .await;
    assert!(
        bpu_root_action.is_err(),
        "{HARNESS}: BPU cannot grant ROOT-only action"
    );
    assert!(
        matches!(
            bpu_root_action.unwrap_err(),
            web::service_support::CoreServiceError::Runtime(
                services::ServiceRuntimeError::Forbidden { .. }
            )
        ),
        "{HARNESS}: BPU ROOT-only action error"
    );

    let bpu_role_manage = service
        .set_role_entitlement(role_code, security::ROLE_MANAGE, true, &bpu_ctx)
        .await;
    assert!(
        bpu_role_manage.is_err(),
        "{HARNESS}: BPU cannot grant ROLE_MANAGE"
    );
    assert!(
        matches!(
            bpu_role_manage.unwrap_err(),
            web::service_support::CoreServiceError::Runtime(
                services::ServiceRuntimeError::Forbidden { .. }
            )
        ),
        "{HARNESS}: BPU ROLE_MANAGE error"
    );
}
