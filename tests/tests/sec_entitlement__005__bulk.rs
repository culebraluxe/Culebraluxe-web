//! SEC.ENTITLEMENT — Bulk role entitlement operations (TST-SEC-ENTITLEMENT-005).
//!
//! Contract: Multiple `set_role_entitlement` calls in sequence work correctly.
//!
//! Level: L2 Persistence — exercises the service against a real (DEV) database via the
//! harness's `TestDatabase`, then rolls back.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_entitlement__005__bulk

use test_harness::database::{TestDatabase};
use db::SecurityDao;
use services::{
    CapturingAuditPort, CapturingDomainEventPort,
    ServiceActor, ServiceActorKind, ServiceContext, ServiceInfrastructure,
    ServicePrincipal,
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
            entitlement_codes: vec!["security.principal.read".into(), model::security::ENTITLEMENT_MANAGE.into()],
        }),
    }
}

#[tokio::test]
#[allow(non_snake_case)]
async fn sec_entitlement_005__bulk() {
    let db = TestDatabase::connect_from_env().await.expect("{HARNESS}: connect to DEV database");
    let dao = SecurityDao::new(db.database().clone());
    let audit = Arc::new(CapturingAuditPort::default());
    let infrastructure = test_infrastructure(audit.clone()).await;
    let service = SecurityService::new(dao, infrastructure);

    let context = root_context("sec-ent-005-bulk");
    let role_code = "test_role_bulk";
    let actions = vec!["property.read", "deal.read", "contract.read", "person.read", "project.read"];

    // Seed role
    sqlx::query("insert into security_role (code, name, active, account_type) values ($1, $2, true, 'internal') on conflict (code) do nothing")
        .bind(role_code).bind("Test Role Bulk").execute(db.database().pool()).await.expect("{HARNESS}: insert role");

    // 1. Bulk grant: grant multiple entitlements in sequence
    for action in &actions {
        let result = service.set_role_entitlement(role_code, action, true, &context).await;
        assert!(result.is_ok(), "{HARNESS}: bulk grant {} succeeds: {:?}", action, result.err());
    }

    // 2. Verify all grants exist
    let list = service.list_role_entitlements(&context).await.expect("{HARNESS}: list after bulk grant");
    let role_ent = list.iter().find(|re| re.role_code == role_code).expect("{HARNESS}: role in list");
    for action in &actions {
        assert!(role_ent.entitlement_codes.contains(&action.to_string()), "{HARNESS}: {} in entitlements", action);
    }

    // 3. Bulk revoke: revoke half of them
    let to_revoke = &actions[0..2];
    for action in to_revoke {
        let result = service.set_role_entitlement(role_code, action, false, &context).await;
        assert!(result.is_ok(), "{HARNESS}: bulk revoke {} succeeds: {:?}", action, result.err());
    }

    // 4. Verify correct grants remain
    let list2 = service.list_role_entitlements(&context).await.expect("{HARNESS}: list after bulk revoke");
    let role_ent2 = list2.iter().find(|re| re.role_code == role_code).expect("{HARNESS}: role in list");
    for action in to_revoke {
        assert!(!role_ent2.entitlement_codes.contains(&action.to_string()), "{HARNESS}: {} revoked", action);
    }
    for action in &actions[2..] {
        assert!(role_ent2.entitlement_codes.contains(&action.to_string()), "{HARNESS}: {} still granted", action);
    }

    // 5. NEGATIVE: non-root cannot bulk manage
    let user_context = ServiceContext {
        actor: ServiceActor { id: Some("user".into()), kind: ServiceActorKind::User },
        correlation_id: "sec-ent-005-user".into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "user".into(), level: "USER".into(), role_codes: vec!["user".into()],
            account_type: "internal".into(), entitlement_codes: vec!["security.principal.read".into(), model::security::ENTITLEMENT_MANAGE.into()],
        }),
    };
    let user_grant = service.set_role_entitlement(role_code, "property.write", true, &user_context).await;
    assert!(user_grant.is_err(), "{HARNESS}: non-root bulk grant refused");
}