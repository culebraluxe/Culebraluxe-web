//! SEC.ENTITLEMENT — Update role entitlement grant (TST-SEC-ENTITLEMENT-003).
//!
//! Contract: `SecurityService::set_role_entitlement` with `granted=false` revokes an existing grant.
//!
//! Level: L2 Persistence — exercises the service against a real (DEV) database via the
//! harness's `TestDatabase`, then rolls back.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_entitlement__003__update

use db::SecurityDao;
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
                model::security::ENTITLEMENT_MANAGE.into(),
            ],
        }),
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (APP_ENV=dev): uses disposable Postgres; production is refused"]
#[allow(non_snake_case)]
async fn sec_entitlement_003__update() {
    let db = TestDatabase::connect_from_env()
        .await
        .expect("{HARNESS}: connect to DEV database");
    let dao = SecurityDao::new(db.database().clone());
    let audit = Arc::new(CapturingAuditPort::default());
    let infrastructure = test_infrastructure(audit.clone()).await;
    let service = SecurityService::new(dao, infrastructure);

    let role_code = "test_role_update";
    let action = "property.read";
    let context = root_context("sec-ent-003-update");

    // Seed role and initial grant
    sqlx::query("insert into security_role (code, name, active, account_type) values ($1, $2, true, 'internal') on conflict (code) do nothing")
        .bind(role_code).bind("Test Role Update").execute(db.database().pool()).await.expect("{HARNESS}: insert role");
    sqlx::query("insert into role_entitlement (role_id, entitlement_id) select r.id, e.id from security_role r, entitlement e where r.code = $1 and e.code = $2 on conflict do nothing")
        .bind(role_code).bind(action).execute(db.database().pool()).await.expect("{HARNESS}: insert initial grant");

    // 1. Verify grant exists
    let list = service
        .list_role_entitlements(&context)
        .await
        .expect("{HARNESS}: list before update");
    let role_ent = list
        .iter()
        .find(|re| re.role_code == role_code)
        .expect("{HARNESS}: role in list");
    assert!(
        role_ent.entitlement_codes.contains(&action.to_string()),
        "{HARNESS}: grant exists initially"
    );

    // 2. Update: revoke the grant (granted=false)
    let revoke_result = service
        .set_role_entitlement(role_code, action, false, &context)
        .await;
    assert!(
        revoke_result.is_ok(),
        "{HARNESS}: revoke grant succeeds: {:?}",
        revoke_result.err()
    );

    // 3. Verify grant is removed
    let list2 = service
        .list_role_entitlements(&context)
        .await
        .expect("{HARNESS}: list after update");
    let role_ent2 = list2
        .iter()
        .find(|re| re.role_code == role_code)
        .expect("{HARNESS}: role still in list");
    assert!(
        !role_ent2.entitlement_codes.contains(&action.to_string()),
        "{HARNESS}: grant removed after revoke"
    );

    // 4. NEGATIVE: revoking grant for non-existent role/action returns ENTITLEMENT_TARGET_UNKNOWN
    let revoke_missing_role = service
        .set_role_entitlement("non_existent_role", action, false, &context)
        .await;
    assert!(
        revoke_missing_role.is_err(),
        "{HARNESS}: revoke for non-existent role fails"
    );
    assert!(
        matches!(revoke_missing_role.unwrap_err(), web::service_support::CoreServiceError::Business { code, .. } if code == "ENTITLEMENT_TARGET_UNKNOWN"),
        "{HARNESS}: non-existent role error is ENTITLEMENT_TARGET_UNKNOWN"
    );

    let revoke_missing_action = service
        .set_role_entitlement(role_code, "non.existent.action", false, &context)
        .await;
    assert!(
        revoke_missing_action.is_err(),
        "{HARNESS}: revoke for non-existent action fails"
    );
    assert!(
        matches!(revoke_missing_action.unwrap_err(), web::service_support::CoreServiceError::Business { code, .. } if code == "ENTITLEMENT_TARGET_UNKNOWN"),
        "{HARNESS}: non-existent action error is ENTITLEMENT_TARGET_UNKNOWN"
    );

    // 5. NEGATIVE: non-root cannot revoke
    let user_context = ServiceContext {
        actor: ServiceActor {
            id: Some("user".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "sec-ent-003-user".into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "user".into(),
            level: "USER".into(),
            role_codes: vec!["user".into()],
            account_type: "internal".into(),
            entitlement_codes: vec![
                "security.principal.read".into(),
                model::security::ENTITLEMENT_MANAGE.into(),
            ],
        }),
    };
    let user_revoke = service
        .set_role_entitlement(role_code, action, true, &user_context)
        .await;
    assert!(
        user_revoke.is_err(),
        "{HARNESS}: non-root cannot manage entitlements"
    );
}
