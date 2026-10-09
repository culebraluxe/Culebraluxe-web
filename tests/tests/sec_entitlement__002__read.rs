//! SEC.ENTITLEMENT — Read role entitlements (TST-SEC-ENTITLEMENT-002).
//!
//! Contract: `SecurityService::list_role_entitlements` returns all active role-entitlement pairs.
//!
//! Level: L2 Persistence — exercises the service against a real (DEV) database via the
//! harness's `TestDatabase`, then rolls back.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_entitlement__002__read

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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-ENTITLEMENT-002); the file and the assay use it.
async fn sec_entitlement_002__read() {
    // 1. Setup: isolated DEV database and SecurityService with real DAO.
    let db = TestDatabase::connect_from_env()
        .await
        .expect("{HARNESS}: connect to DEV database");
    let dao = SecurityDao::new(db.database().clone());

    let audit = Arc::new(CapturingAuditPort::default());
    let infrastructure = test_infrastructure(audit.clone()).await;
    let service = SecurityService::new(dao, infrastructure);

    // 2. Seed some test data directly in the database.
    let role_code = "test_role_read";
    let action1 = "property.read";
    let action2 = "deal.read";

    sqlx::query(
        "insert into security_role (code, name, active, account_type) 
         values ($1, $2, true, 'internal')
         on conflict (code) do nothing",
    )
    .bind(role_code)
    .bind("Test Role Read")
    .execute(db.database().pool())
    .await
    .expect("{HARNESS}: insert test role");

    // Insert role_entitlement rows directly.
    sqlx::query(
        "insert into role_entitlement (role_id, entitlement_id)
         select r.id, e.id
         from security_role r, entitlement e
         where r.code = $1 and e.code = $2
         on conflict do nothing",
    )
    .bind(role_code)
    .bind(action1)
    .execute(db.database().pool())
    .await
    .expect("{HARNESS}: insert role entitlement 1");

    sqlx::query(
        "insert into role_entitlement (role_id, entitlement_id)
         select r.id, e.id
         from security_role r, entitlement e
         where r.code = $1 and e.code = $2
         on conflict do nothing",
    )
    .bind(role_code)
    .bind(action2)
    .execute(db.database().pool())
    .await
    .expect("{HARNESS}: insert role entitlement 2");

    // 3. Read the role entitlements.
    let context = root_context("sec-ent-002-read");
    let result = service.list_role_entitlements(&context).await;

    assert!(
        result.is_ok(),
        "{HARNESS}: list role entitlements succeeds: {:?}",
        result.err()
    );

    let entitlements = result.unwrap();

    // 4. Verify the seeded data appears in the results.
    let role_ent = entitlements.iter().find(|re| re.role_code == role_code);
    assert!(role_ent.is_some(), "{HARNESS}: test role appears in list");

    let actions: Vec<&str> = role_ent
        .unwrap()
        .entitlement_codes
        .iter()
        .map(|s| s.as_str())
        .collect();
    assert!(
        actions.contains(&action1),
        "{HARNESS}: action1 appears in role's entitlements: {:?}",
        actions
    );
    assert!(
        actions.contains(&action2),
        "{HARNESS}: action2 appears in role's entitlements: {:?}",
        actions
    );

    // 5. Verify inactive roles are NOT included.
    let inactive_role = "inactive_test_role_read";
    sqlx::query(
        "insert into security_role (code, name, active, account_type) 
         values ($1, $2, false, 'internal')
         on conflict (code) do nothing",
    )
    .bind(inactive_role)
    .bind("Inactive Test Role Read")
    .execute(db.database().pool())
    .await
    .expect("{HARNESS}: insert inactive test role");

    sqlx::query(
        "insert into role_entitlement (role_id, entitlement_id)
         select r.id, e.id
         from security_role r, entitlement e
         where r.code = $1 and e.code = $2
         on conflict do nothing",
    )
    .bind(inactive_role)
    .bind(action1)
    .execute(db.database().pool())
    .await
    .expect("{HARNESS}: insert inactive role entitlement");

    let result2 = service.list_role_entitlements(&context).await;
    assert!(
        result2.is_ok(),
        "{HARNESS}: list role entitlements succeeds after inactive insert"
    );

    let entitlements2 = result2.unwrap();
    let inactive_ent = entitlements2
        .iter()
        .find(|re| re.role_code == inactive_role);
    assert!(
        inactive_ent.is_none(),
        "{HARNESS}: inactive role does NOT appear in list"
    );

    // 6. Verify inactive entitlements are NOT included.
    let inactive_entitlement_action = "calendar.read";
    sqlx::query("update entitlement set active = false where code = $1")
        .bind(inactive_entitlement_action)
        .execute(db.database().pool())
        .await
        .expect("{HARNESS}: deactivate entitlement");

    let result3 = service.list_role_entitlements(&context).await;
    assert!(
        result3.is_ok(),
        "{HARNESS}: list role entitlements succeeds after deactivating entitlement"
    );

    let entitlements3 = result3.unwrap();
    let role_ent = entitlements3.iter().find(|re| re.role_code == role_code);
    let actions3: Vec<&str> = role_ent
        .unwrap()
        .entitlement_codes
        .iter()
        .map(|s| s.as_str())
        .collect();
    assert!(
        !actions3.contains(&inactive_entitlement_action),
        "{HARNESS}: inactive entitlement does NOT appear in role's entitlements: {:?}",
        actions3
    );

    // 7. NEGATIVE CASE: external account cannot read role entitlements.
    let external_context = ServiceContext {
        actor: ServiceActor {
            id: Some("external-user".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "sec-ent-002-external".into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "external-user".into(),
            level: "GUEST".into(),
            role_codes: vec!["guest".into()],
            account_type: "guest".into(),
            entitlement_codes: vec!["security.principal.read".into()],
        }),
    };

    let external_result = service.list_role_entitlements(&external_context).await;
    assert!(
        external_result.is_err(),
        "{HARNESS}: external account cannot list role entitlements"
    );
}
