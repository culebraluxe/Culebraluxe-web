//! SEC.ENTITLEMENT — Create role entitlement grant (TST-SEC-ENTITLEMENT-001).
//!
//! Contract: `SecurityService::set_role_entitlement` with `granted=true` creates a new
//! role_entitlement row for an active internal role and a catalogued action.
//!
//! Level: L2 Persistence — exercises the service against a real (DEV) database via the
//! harness's `TestDatabase`, then rolls back.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_entitlement__001__create

use db::SecurityDao;
use services::{
    CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceActor,
    ServiceActorKind, ServiceContext, ServiceInfrastructure, ServicePrincipal,
};
use std::sync::Arc;
use test_harness::database::TestDatabase;
use web::security::SecurityService;

const HARNESS: &str = "TestDatabase/L2 Persistence";

fn test_infrastructure(audit: Arc<CapturingAuditPort>) -> ServiceInfrastructure {
    ServiceInfrastructure::new(
        Arc::new(DefaultAuthorizationPort),
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-ENTITLEMENT-001); the file and the assay use it.
async fn sec_entitlement_001__create() {
    // 1. Setup: isolated DEV database and SecurityService with real DAO.
    let db = TestDatabase::connect_from_env()
        .await
        .expect("{HARNESS}: connect to DEV database");
    let dao = SecurityDao::new(db.database().clone());

    let audit = Arc::new(CapturingAuditPort::default());
    let service = SecurityService::new(dao, test_infrastructure(audit.clone()));

    // 2. Create a new role entitlement grant.
    let context = root_context("sec-ent-001-create");
    let role_code = "test_role_create";
    let action = "property.read";

    // First ensure the role exists (insert it directly via SQL since DAO doesn't expose role creation)
    sqlx::query(
        "insert into security_role (code, name, active, account_type) 
         values ($1, $2, true, 'internal')
         on conflict (code) do nothing",
    )
    .bind(role_code)
    .bind("Test Role Create")
    .execute(db.database().pool())
    .await
    .expect("{HARNESS}: insert test role");

    // 3. Grant the entitlement (CREATE).
    let result = service
        .set_role_entitlement(role_code, action, true, &context)
        .await;

    assert!(
        result.is_ok(),
        "{HARNESS}: grant entitlement succeeds: {:?}",
        result.err()
    );

    // 4. Verify the grant exists by listing role entitlements.
    let list_result = service.list_role_entitlements(&context).await;
    assert!(
        list_result.is_ok(),
        "{HARNESS}: list role entitlements succeeds"
    );

    let entitlements = list_result.unwrap();
    let role_ent = entitlements.iter().find(|re| re.role_code == role_code);
    assert!(role_ent.is_some(), "{HARNESS}: test role appears in list");

    let actions: Vec<&str> = role_ent
        .unwrap()
        .entitlement_codes
        .iter()
        .map(|s| s.as_str())
        .collect();
    assert!(
        actions.contains(&action),
        "{HARNESS}: granted action appears in role's entitlements: {:?}",
        actions
    );

    // 5. NEGATIVE CASE: granting to an inactive role fails.
    let inactive_role = "inactive_test_role";
    sqlx::query(
        "insert into security_role (code, name, active, account_type) 
         values ($1, $2, false, 'internal')
         on conflict (code) do nothing",
    )
    .bind(inactive_role)
    .bind("Inactive Test Role")
    .execute(db.database().pool())
    .await
    .expect("{HARNESS}: insert inactive test role");

    let inactive_result = service
        .set_role_entitlement(inactive_role, action, true, &context)
        .await;
    assert!(
        inactive_result.is_err(),
        "{HARNESS}: grant to inactive role fails"
    );
    assert!(
        matches!(inactive_result.unwrap_err(), web::service_support::CoreServiceError::Business { code, .. } if code == "ENTITLEMENT_TARGET_UNKNOWN"),
        "{HARNESS}: inactive role error is ENTITLEMENT_TARGET_UNKNOWN"
    );

    // 6. NEGATIVE CASE: granting an unknown action fails (catalog validation happens at API layer,
    //    but the DAO should also refuse unknown action/role combinations).
    let unknown_action_result = service
        .set_role_entitlement(
            role_code,
            "unknown.action.that.does.not.exist",
            true,
            &context,
        )
        .await;
    assert!(
        unknown_action_result.is_err(),
        "{HARNESS}: grant of unknown action fails"
    );
}
