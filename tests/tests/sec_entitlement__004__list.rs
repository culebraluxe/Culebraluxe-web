//! SEC.ENTITLEMENT — List role entitlements (TST-SEC-ENTITLEMENT-004).
//!
//! Contract: `SecurityService::list_role_entitlements` returns all active role-entitlement pairs
//! with proper filtering for active roles and entitlements.
//!
//! Level: L2 Persistence — exercises the service against a real (DEV) database via the
//! harness's `TestDatabase`, then rolls back.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_entitlement__004__list

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
#[allow(non_snake_case)]
async fn sec_entitlement_004__list() {
    let db = TestDatabase::connect_from_env()
        .await
        .expect("{HARNESS}: connect to DEV database");
    let dao = SecurityDao::new(db.database().clone());
    let audit = Arc::new(CapturingAuditPort::default());
    let infrastructure = test_infrastructure(audit.clone()).await;
    let service = SecurityService::new(dao, infrastructure);

    let context = root_context("sec-ent-004-list");

    // 1. List returns all active role-entitlement pairs (including seeded data)
    let list = service
        .list_role_entitlements(&context)
        .await
        .expect("{HARNESS}: list succeeds");
    assert!(!list.is_empty(), "{HARNESS}: list is not empty");

    // 2. Each entry has required fields
    for role_ent in &list {
        assert!(
            !role_ent.role_code.is_empty(),
            "{HARNESS}: role_code present"
        );
        assert!(
            !role_ent.account_type.is_empty(),
            "{HARNESS}: account_type present"
        );
        // entitlement_codes may be empty for roles with no grants
    }

    // 3. Seed a new role with multiple entitlements
    let role_code = "test_role_list";
    let actions = vec!["property.read", "deal.read", "contract.read"];

    sqlx::query("insert into security_role (code, name, active, account_type) values ($1, $2, true, 'internal') on conflict (code) do nothing")
        .bind(role_code).bind("Test Role List").execute(db.database().pool()).await.expect("{HARNESS}: insert role");

    for action in &actions {
        sqlx::query("insert into role_entitlement (role_id, entitlement_id) select r.id, e.id from security_role r, entitlement e where r.code = $1 and e.code = $2 on conflict do nothing")
            .bind(role_code).bind(action).execute(db.database().pool()).await.expect("{HARNESS}: insert grant");
    }

    // 4. Verify the new role appears with all entitlements
    let list2 = service
        .list_role_entitlements(&context)
        .await
        .expect("{HARNESS}: list after seed");
    let role_ent = list2
        .iter()
        .find(|re| re.role_code == role_code)
        .expect("{HARNESS}: test role in list");
    for action in &actions {
        assert!(
            role_ent.entitlement_codes.contains(&action.to_string()),
            "{HARNESS}: action {} in list",
            action
        );
    }

    // 5. NEGATIVE: inactive roles excluded
    let inactive_role = "inactive_list_role";
    sqlx::query("insert into security_role (code, name, active, account_type) values ($1, $2, false, 'internal') on conflict (code) do nothing")
        .bind(inactive_role).bind("Inactive List Role").execute(db.database().pool()).await.expect("{HARNESS}: insert inactive role");
    sqlx::query("insert into role_entitlement (role_id, entitlement_id) select r.id, e.id from security_role r, entitlement e where r.code = $1 and e.code = $2 on conflict do nothing")
        .bind(inactive_role).bind("property.read").execute(db.database().pool()).await.expect("{HARNESS}: insert inactive grant");

    let list3 = service
        .list_role_entitlements(&context)
        .await
        .expect("{HARNESS}: list with inactive role");
    let inactive_ent = list3.iter().find(|re| re.role_code == inactive_role);
    assert!(
        inactive_ent.is_none(),
        "{HARNESS}: inactive role excluded from list"
    );

    // 6. NEGATIVE: external account cannot list
    let external_context = ServiceContext {
        actor: ServiceActor {
            id: Some("external".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "sec-ent-004-ext".into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "external".into(),
            level: "GUEST".into(),
            role_codes: vec!["guest".into()],
            account_type: "guest".into(),
            entitlement_codes: vec!["security.principal.read".into()],
        }),
    };
    let ext_result = service.list_role_entitlements(&external_context).await;
    assert!(ext_result.is_err(), "{HARNESS}: external account refused");
}
