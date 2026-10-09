//! SEC.ENTITLEMENT — Delete/revoke role entitlement grant (TST-SEC-ENTITLEMENT-006).
//!
//! Contract: `SecurityService::set_role_entitlement` with `granted=false` removes a grant.
//!
//! Level: L2 Persistence — exercises the service against a real (DEV) database via the
//! harness's `TestDatabase`, then rolls back.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_entitlement__006__delete

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
async fn sec_entitlement_006__delete() {
    let db = TestDatabase::connect_from_env().await.expect("{HARNESS}: connect to DEV database");
    let dao = SecurityDao::new(db.database().clone());
    let audit = Arc::new(CapturingAuditPort::default());
    let infrastructure = test_infrastructure(audit.clone()).await;
    let service = SecurityService::new(dao, infrastructure);

    let context = root_context("sec-ent-006-delete");
    let role_code = "test_role_delete";
    let action = "property.read";

    // Seed role and grant
    sqlx::query("insert into security_role (code, name, active, account_type) values ($1, $2, true, 'internal') on conflict (code) do nothing")
        .bind(role_code).bind("Test Role Delete").execute(db.database().pool()).await.expect("{HARNESS}: insert role");
    sqlx::query("insert into role_entitlement (role_id, entitlement_id) select r.id, e.id from security_role r, entitlement e where r.code = $1 and e.code = $2 on conflict do nothing")
        .bind(role_code).bind(action).execute(db.database().pool()).await.expect("{HARNESS}: insert grant");

    // 1. Verify grant exists
    let list = service.list_role_entitlements(&context).await.expect("{HARNESS}: list before delete");
    let role_ent = list.iter().find(|re| re.role_code == role_code).expect("{HARNESS}: role in list");
    assert!(role_ent.entitlement_codes.contains(&action.to_string()), "{HARNESS}: grant exists initially");

    // 2. Delete (revoke) the grant
    let result = service.set_role_entitlement(role_code, action, false, &context).await;
    assert!(result.is_ok(), "{HARNESS}: revoke grant succeeds: {:?}", result.err());

    // 3. Verify grant is removed
    let list2 = service.list_role_entitlements(&context).await.expect("{HARNESS}: list after delete");
    let role_ent2 = list2.iter().find(|re| re.role_code == role_code).expect("{HARNESS}: role in list");
    assert!(!role_ent2.entitlement_codes.contains(&action.to_string()), "{HARNESS}: grant removed");

    // 4. NEGATIVE: deleting from inactive role fails
    let inactive_role = "inactive_delete_role";
    sqlx::query("insert into security_role (code, name, active, account_type) values ($1, $2, false, 'internal') on conflict (code) do nothing")
        .bind(inactive_role).bind("Inactive Delete Role").execute(db.database().pool()).await.expect("{HARNESS}: insert inactive role");
    sqlx::query("insert into role_entitlement (role_id, entitlement_id) select r.id, e.id from security_role r, entitlement e where r.code = $1 and e.code = $2 on conflict do nothing")
        .bind(inactive_role).bind(action).execute(db.database().pool()).await.expect("{HARNESS}: insert inactive grant");

    let inactive_delete = service.set_role_entitlement(inactive_role, action, false, &context).await;
    assert!(inactive_delete.is_err(), "{HARNESS}: delete from inactive role fails");
    assert!(matches!(inactive_delete.unwrap_err(), web::service_support::CoreServiceError::Business { code, .. } if code == "ENTITLEMENT_TARGET_UNKNOWN"), "{HARNESS}: inactive role error");

    // 5. NEGATIVE: non-root cannot delete
    let user_context = ServiceContext {
        actor: ServiceActor { id: Some("user".into()), kind: ServiceActorKind::User },
        correlation_id: "sec-ent-006-user".into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "user".into(), level: "USER".into(), role_codes: vec!["user".into()],
            account_type: "internal".into(), entitlement_codes: vec!["security.principal.read".into(), model::security::ENTITLEMENT_MANAGE.into()],
        }),
    };
    let user_delete = service.set_role_entitlement(role_code, action, false, &user_context).await;
    assert!(user_delete.is_err(), "{HARNESS}: non-root cannot delete entitlements");
}