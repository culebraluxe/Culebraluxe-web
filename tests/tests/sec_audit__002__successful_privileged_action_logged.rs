//! SEC.AUDIT — successful privileged action logged (TST-SEC-AUDIT-002).
//!
//! CONTRACT. A privileged success must leave the same durable trace as a denial: when the production
//! `SecurityService::decide` (`web/src/security/mod.rs`) ALLOWS a root-only command for a ROOT principal, the
//! service kernel (`ServiceRuntime::audit`, `middle/services/src/runtime.rs`) must record the success through the
//! production `DurableSecurityAuditPort` (`web/src/security/audit.rs`) into `security_audit_event`. An allowed
//! privilege escalation with no row is the mirror image of an unlogged denial — the trail would show the refusals
//! and go silent exactly where power was exercised. So: every allowed privileged `decide` commits exactly one row
//! carrying success, allowed=true, the granting rule, and the actor.
//!
//! THE BOUNDARY UNDER TEST IS PRODUCTION'S OWN. `decide` is the production entry point (the API authorize route
//! calls it, `web/src/api/routes/security_service.rs:305`); the authorization is the production
//! `CasbinAuthorizationPort`; the write is the production port into the production table, read back on a
//! connection the DAO does not own. The test states only what the rows must contain.
//!
//! NEGATIVE CASES. A test that only recorded successes could pass on a boundary that logs every call as a success.
//! So the same root-only action is also decided for a signed-in NON-root user, which must be refused AND logged
//! as a failure — and a second privileged command (`security.entitlement.manage`) must carry its own success row.
//! A boundary that logged one canned success for everything would fail here.
//!
//! ISOLATION. The database is the harness's disposable DEV target: `AuditPersistenceHarness::connect_declared` resolves
//! the declared environment and refuses `DbTarget::Prod` before any socket is opened
//! (`tests/src/database.rs:68-89`). Rows are addressed by a unique `correlationId` marker the production port
//! already writes, the fixture user exists only for this test, and cleanup deletes exactly those rows — a zero
//! leftover count is asserted, so DEV is left as it was found.
//!
//! Level: L2 Persistence — the production audit write against an isolated, disposable DEV/Neon target, harness
//! `AuditPersistenceHarness`.
//!
//! Run with:
//!   set -a; . ./.env.local; set +a
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_audit__002__successful_privileged_action_logged -- --ignored --nocapture
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use services::{
    CapturingDomainEventPort, OperationKind, ServiceActor, ServiceActorKind, ServiceContext,
    ServiceInfrastructure, ServicePrincipal,
};
use std::sync::Arc;
use test_harness::AuditPersistenceHarness;
use web::security::{CasbinAuthorizationPort, DurableSecurityAuditPort, SecurityService};

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "AuditPersistenceHarness/L2 Persistence";

/// Connect to the disposable DEV target, tolerating a cold-pool timeout under concurrent test load.
///
/// Infrastructure, not the contract: `AuditPersistenceHarness` still refuses PRODUCTION before any socket is opened.
async fn connect_dev() -> AuditPersistenceHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match AuditPersistenceHarness::connect_declared(Some("dev"), Some("dev")).await {
            Ok(harness) => return harness,
            Err(error) => {
                eprintln!("{HARNESS}: DEV connect attempt {attempt} failed: {error}");
                last = Some(error.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
            }
        }
    }
    panic!(
        "{HARNESS}: disposable DEV target unreachable: {}",
        last.unwrap_or_default()
    );
}

fn root_context(actor_id: &str, marker: &str) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some(actor_id.to_owned()),
            kind: ServiceActorKind::User,
        },
        correlation_id: marker.to_owned(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: actor_id.to_owned(),
            level: "ROOT".to_owned(),
            role_codes: vec!["root".to_owned()],
            account_type: model::security::INTERNAL_ACCOUNT.to_owned(),
            entitlement_codes: Vec::new(),
        }),
    }
}

fn staff_context(actor_id: &str, marker: &str) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some(actor_id.to_owned()),
            kind: ServiceActorKind::User,
        },
        correlation_id: marker.to_owned(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: actor_id.to_owned(),
            level: "USER".to_owned(),
            role_codes: vec!["staff".to_owned()],
            account_type: model::security::INTERNAL_ACCOUNT.to_owned(),
            entitlement_codes: Vec::new(),
        }),
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV target); the harness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-AUDIT-002); the file and the assay use it.
async fn sec_audit_002__successful_privileged_action_logged() {
    let harness = connect_dev().await;
    let api = "an allowed privileged decide must commit exactly one audit row carrying the success and the actor";
    let marker = format!("sec-audit-002-{}", harness.namespace());

    let database = harness.database().database().clone();
    let infrastructure = ServiceInfrastructure::new(
        Arc::new(
            CasbinAuthorizationPort::new()
                .await
                .expect("the production authorization port builds"),
        ),
        Arc::new(DurableSecurityAuditPort::new(harness.dao().clone())),
        Arc::new(CapturingDomainEventPort::default()),
    );
    let service = SecurityService::new(db::SecurityDao::new(database), infrastructure);

    // (1) THE FIXTURE USER. Audit rows attribute a signed-in actor by `app_user_id`, which references
    //     `app_user(id)` — so a ROOT success needs a real user row, created here and deleted at the end.
    let fixture_email = format!("sec-audit-002-{}@test.invalid", harness.namespace());
    let fixture_id: String = sqlx::query_scalar(
        "insert into app_user (display_name, email) values ($1, $2) returning id::text",
    )
    .bind("sec-audit-002 privileged-user")
    .bind(&fixture_email)
    .fetch_one(harness.pool())
    .await
    .expect("the fixture user must insert");

    // (2) SUCCESS ONE: ROOT may manage roles. `decide` allows it and must log the success with the granting rule.
    let decision = service
        .decide(
            model::security::ROLE_MANAGE,
            OperationKind::Command,
            &root_context(&fixture_id, &marker),
        )
        .await
        .expect("a ROOT decision answers");
    assert!(
        decision.allowed,
        "{api}: ROOT must be allowed the root-only command"
    );
    assert_eq!(
        decision.policy_id, "rule:security.manage.root",
        "{api}: the row must name the granting rule, got {}",
        decision.policy_id,
    );

    // (3) SUCCESS TWO: ROOT may manage entitlements — a second privileged command with its own success row, so
    //     one canned row cannot stand in for both.
    let decision = service
        .decide(
            model::security::ENTITLEMENT_MANAGE,
            OperationKind::Command,
            &root_context(&fixture_id, &marker),
        )
        .await
        .expect("a ROOT decision answers");
    assert!(
        decision.allowed,
        "{api}: ROOT must be allowed the entitlement command"
    );

    // (4) THE COMMITTED TRUTH, read back on a connection the DAO does not own. Two privileged successes, two
    //     rows — each carrying outcome success, allowed=true, the granting rule, and the ROOT actor.
    let rows = harness
        .rows_for(&marker)
        .await
        .expect("committed audit rows must be readable");
    assert_eq!(
        rows.len(),
        2,
        "{api}: two privileged successes must commit two rows, got {}",
        rows.len()
    );
    for row in &rows {
        assert_eq!(
            row.authentication_method.as_deref(),
            Some("rust-service"),
            "{api}: the row names the production writer",
        );
        assert_eq!(
            row.metadata["outcome"], "success",
            "{api}: an allowed action is a success: {}",
            row.metadata
        );
        assert_eq!(
            row.metadata["authorization"]["allowed"], true,
            "{api}: the grant must travel into the row: {}",
            row.metadata,
        );
        assert_eq!(
            row.metadata["authorization"]["policyId"], "rule:security.manage.root",
            "{api}: the row must name the granting rule: {}",
            row.metadata,
        );
        assert_eq!(
            row.app_user_id.as_deref(),
            Some(fixture_id.as_str()),
            "{api}: the success must attribute the ROOT actor",
        );
        assert_eq!(
            row.metadata["correlationId"], marker,
            "{api}: the row carries the caller's correlation: {}",
            row.metadata,
        );
    }
    let actions: Vec<&str> = rows.iter().map(|row| row.event_type.as_str()).collect();
    assert!(
        actions.contains(&model::security::ROLE_MANAGE)
            && actions.contains(&model::security::ENTITLEMENT_MANAGE),
        "{api}: each privileged command must name its own row, got {actions:?}",
    );

    // (5) NEGATIVE: THE SUCCESS ROWS ARE THE GRANTS', NOT WALLPAPER. The same root-only action decided for a
    //     signed-in NON-root user must be refused AND logged as a failure — a boundary that logged one canned
    //     success for every call would fail here.
    let refused = service
        .decide(
            model::security::ROLE_MANAGE,
            OperationKind::Command,
            &staff_context(&fixture_id, &marker),
        )
        .await
        .expect("a denial is an answer, not an error");
    assert!(
        !refused.allowed,
        "{api}: staff must be refused the root-only command"
    );
    let rows = harness
        .rows_for(&marker)
        .await
        .expect("rows must be readable");
    assert_eq!(
        rows.len(),
        3,
        "{api}: the refusal must also be logged, got {}",
        rows.len()
    );
    let failures = rows
        .iter()
        .filter(|row| {
            row.metadata["outcome"] == "failure"
                && row.metadata["authorization"]["allowed"] == false
        })
        .count();
    assert_eq!(
        failures, 1,
        "{api}: exactly one row must say failure: {rows:?}"
    );

    // (6) LEAVE DEV AS IT WAS FOUND. Exactly this test's rows go, then the fixture user, and the leftover
    //     count proves nothing survived.
    let removed = harness
        .cleanup(&marker)
        .await
        .expect("cleanup deletes the marker rows");
    assert_eq!(
        removed, 3,
        "{api}: cleanup must remove exactly this test's rows"
    );
    sqlx::query("delete from app_user where id = $1::uuid")
        .bind(&fixture_id)
        .execute(harness.pool())
        .await
        .expect("the fixture user must delete");
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("leftover must be readable"),
        0,
        "{api}: no audit row may survive the test",
    );
}
