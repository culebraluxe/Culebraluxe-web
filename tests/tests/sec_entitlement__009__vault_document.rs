//! SEC.ENTITLEMENT — Vault document actions (TST-SEC-ENTITLEMENT-009).
//!
//! Contract: `vault.read`, `vault.write`, `vault.issue` are controlled by entitlements;
//! external accounts cannot hold vault.read even with grant.
//!
//! Level: L2 Persistence — exercises the service against a real (DEV) database via the
//! harness's `TestDatabase`, then rolls back.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_entitlement__009__vault_document

use test_harness::database::{TestDatabase};
use db::SecurityDao;
use model::security;
use services::{
    CapturingAuditPort, CapturingDomainEventPort, AuthorizationPort,
    ServiceActor, ServiceActorKind, ServiceContext, ServiceInfrastructure,
    ServicePrincipal, OperationKind,
};
use web::security::{SecurityService, CasbinAuthorizationPort};
use std::sync::Arc;

const HARNESS: &str = "TestDatabase/L2 Persistence";

async fn test_infrastructure(audit: Arc<CapturingAuditPort>) -> ServiceInfrastructure {
    let auth_port = Arc::new(CasbinAuthorizationPort::new().await.expect("{HARNESS}: CasbinAuthorizationPort builds"));
    ServiceInfrastructure::new(auth_port, audit, Arc::new(CapturingDomainEventPort::default()))
}

fn root_context(correlation_id: &str) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor { id: Some("root-test-user".into()), kind: ServiceActorKind::User },
        correlation_id: correlation_id.into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "root-test-user".into(),
            level: "ROOT".into(),
            role_codes: vec!["root".into()],
            account_type: "internal".into(),
            entitlement_codes: vec!["security.principal.read".into(), security::ENTITLEMENT_MANAGE.into(), "vault.read".into(), "vault.write".into(), "vault.issue".into()],
        }),
    }
}

fn bpu_context(correlation_id: &str) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor { id: Some("bpu-test-user".into()), kind: ServiceActorKind::User },
        correlation_id: correlation_id.into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "bpu-test-user".into(),
            level: "BUSINESS_POWER_USER".into(),
            role_codes: vec!["business_power_user".into()],
            account_type: "internal".into(),
            entitlement_codes: vec!["security.principal.read".into(), "vault.read".into(), "vault.write".into(), "vault.issue".into()],
        }),
    }
}

fn external_context(correlation_id: &str) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor { id: Some("external-user".into()), kind: ServiceActorKind::User },
        correlation_id: correlation_id.into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "external-user".into(),
            level: "GUEST".into(),
            role_codes: vec!["guest".into()],
            account_type: "guest".into(),
            entitlement_codes: vec!["vault.read".into()],
        }),
    }
}

#[tokio::test]
#[allow(non_snake_case)]
async fn sec_entitlement_009__vault_document() {
    let db = TestDatabase::connect_from_env().await.expect("{HARNESS}: connect to DEV database");
    let dao = SecurityDao::new(db.database().clone());
    let audit = Arc::new(CapturingAuditPort::default());
    let infrastructure = test_infrastructure(audit.clone()).await;
    let service = SecurityService::new(dao, infrastructure);

    // Test via the authorization decision endpoint (CasbinAuthorizationPort)
    let auth_port = Arc::new(CasbinAuthorizationPort::new().await.expect("{HARNESS}: CasbinAuthorizationPort builds"));

    // 1. ROOT with grants can access vault actions
    let root_ctx = root_context("sec-ent-009-root");
    for action in ["vault.read", "vault.write", "vault.issue"] {
        let decision = auth_port.authorize(services::AuthorizationRequest {
            domain: "vault",
            action,
            operation: action,
            kind: if action == "vault.read" { OperationKind::Query } else { OperationKind::Command },
            actor: root_ctx.actor.clone(),
            principal: root_ctx.principal.clone(),
        }).await.expect("{HARNESS}: auth call succeeds");
        assert!(decision.allowed, "{HARNESS}: ROOT allowed {}", action);
    }

    // 2. BUSINESS_POWER_USER with grants can access vault actions
    let bpu_ctx = bpu_context("sec-ent-009-bpu");
    for action in ["vault.read", "vault.write", "vault.issue"] {
        let decision = auth_port.authorize(services::AuthorizationRequest {
            domain: "vault",
            action,
            operation: action,
            kind: if action == "vault.read" { OperationKind::Query } else { OperationKind::Command },
            actor: bpu_ctx.actor.clone(),
            principal: bpu_ctx.principal.clone(),
        }).await.expect("{HARNESS}: auth call succeeds");
        assert!(decision.allowed, "{HARNESS}: BPU allowed {}", action);
    }

    // 3. EXTERNAL account cannot access vault.read even with grant
    let ext_ctx = external_context("sec-ent-009-ext");
    let ext_decision = auth_port.authorize(services::AuthorizationRequest {
        domain: "vault",
        action: "vault.read",
        operation: "vault.read",
        kind: OperationKind::Query,
        actor: ext_ctx.actor.clone(),
        principal: ext_ctx.principal.clone(),
    }).await.expect("{HARNESS}: auth call succeeds");
    assert!(!ext_decision.allowed, "{HARNESS}: external denied vault.read despite grant");
    assert_eq!(ext_decision.policy_id, "account:external", "{HARNESS}: external denial policy");

    // 4. EXTERNAL account cannot access vault.write
    let ext_write = auth_port.authorize(services::AuthorizationRequest {
        domain: "vault",
        action: "vault.write",
        operation: "vault.write",
        kind: OperationKind::Command,
        actor: ext_ctx.actor.clone(),
        principal: ext_ctx.principal.clone(),
    }).await.expect("{HARNESS}: auth call succeeds");
    assert!(!ext_write.allowed, "{HARNESS}: external denied vault.write");

    // 5. Test SecurityService::set_role_entitlement for vault actions
    let role_code = "test_role_vault";
    sqlx::query("insert into security_role (code, name, active, account_type) values ($1, $2, true, 'internal') on conflict (code) do nothing")
        .bind(role_code).bind("Test Role Vault").execute(db.database().pool()).await.expect("{HARNESS}: insert role");

    // ROOT can grant vault actions to roles
    let vault_grant = service.set_role_entitlement(role_code, "vault.read", true, &root_ctx).await;
    assert!(vault_grant.is_ok(), "{HARNESS}: ROOT can grant vault.read: {:?}", vault_grant.err());

    let vault_write_grant = service.set_role_entitlement(role_code, "vault.write", true, &root_ctx).await;
    assert!(vault_write_grant.is_ok(), "{HARNESS}: ROOT can grant vault.write: {:?}", vault_write_grant.err());

    // Verify grants appear in list
    let list = service.list_role_entitlements(&root_ctx).await.expect("{HARNESS}: list vault grants");
    let role_ent = list.iter().find(|re| re.role_code == role_code).expect("{HARNESS}: role in list");
    assert!(role_ent.entitlement_codes.contains(&"vault.read".to_string()), "{HARNESS}: vault.read in grants");
    assert!(role_ent.entitlement_codes.contains(&"vault.write".to_string()), "{HARNESS}: vault.write in grants");

    // 6. NEGATIVE: non-root cannot grant vault actions
    let bpu_grant_vault = service.set_role_entitlement(role_code, "vault.issue", true, &bpu_ctx).await;
    assert!(bpu_grant_vault.is_err(), "{HARNESS}: BPU cannot grant vault.issue");
    assert!(matches!(bpu_grant_vault.unwrap_err(), web::service_support::CoreServiceError::Runtime(services::ServiceRuntimeError::Forbidden { .. })), "{HARNESS}: BPU vault grant error is FORBIDDEN");
}