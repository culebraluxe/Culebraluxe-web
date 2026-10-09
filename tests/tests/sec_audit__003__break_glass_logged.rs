//! SEC.AUDIT — break-glass logged (TST-SEC-AUDIT-003).
//!
//! CONTRACT. Break-glass is the ROOT user authenticating outside the normal identity path (the `break-glass`
//! provider resolves the ROOT identity, `web/src/api/ui_auth.rs:158-160`), and its decisions run at the highest
//! privilege in the system. When that identity exercises a root-only command through the production
//! `SecurityService::decide` (`web/src/security/mod.rs`), the decision must be allowed AND must commit a durable
//! row through the service kernel (`ServiceRuntime::audit`) into the production `DurableSecurityAuditPort`
//! (`web/src/security/audit.rs`) — attributing the grant to the break-glass actor. A break-glass grant with no
//! row is power exercised with no witness: the trail must show WHO used the glass, on WHAT, and that it was
//! allowed. (The table's charter names this row explicitly: "break-glass root login success",
//! `db/migrations/017_security_audit_event.sql`.)
//!
//! THE BOUNDARY UNDER TEST IS PRODUCTION'S OWN. `DurableSecurityAuditPort` is the only production writer to
//! `security_audit_event`; `decide` is the production entry point; the authorization is the production
//! `CasbinAuthorizationPort`. The write is read back on a connection the DAO does not own. The test states only
//! what the row must contain.
//!
//! NEGATIVE CASES. A test that only recorded the break-glass grant could pass on a boundary that logs every call
//! as a break-glass success. So the same root-only command is also decided for a signed-in NON-root user, which
//! must be refused AND logged as a failure — the success row belongs to the glass-holder alone. And the row must
//! attribute the exact break-glass actor id: a grant logged against nobody (or anybody) fails the contract.
//!
//! ISOLATION. The database is the harness's disposable DEV target: `AuditPersistenceHarness::connect_declared` resolves
//! the declared environment and refuses `DbTarget::Prod` before any socket is opened
//! (`tests/src/database.rs:68-89`). Rows are addressed by a unique `correlationId` marker the production port
//! already writes, the fixture break-glass user exists only for this test, and cleanup deletes exactly those
//! rows — a zero leftover count is asserted, so DEV is left as it was found.
//!
//! Level: L2 Persistence — the production audit write against an isolated, disposable DEV/Neon target, harness
//! `AuditPersistenceHarness`.
//!
//! Run with:
//!   set -a; . ./.env.local; set +a
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_audit__003__break_glass_logged -- --ignored --nocapture
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

/// The break-glass identity: the ROOT user, whose roles the production policy reads for the grant.
fn break_glass_context(actor_id: &str, marker: &str) -> ServiceContext {
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
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-AUDIT-003); the file and the assay use it.
async fn sec_audit_003__break_glass_logged() {
    let harness = connect_dev().await;
    let api = "a break-glass grant must commit exactly one audit row attributing the grant to the glass-holder";
    let marker = format!("sec-audit-003-{}", harness.namespace());

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

    // (1) THE FIXTURE BREAK-GLASS USER. The row attributes the grant by `app_user_id`, which references
    //     `app_user(id)` — so the glass-holder needs a real user row, created here and deleted at the end.
    let fixture_email = format!(
        "sec-audit-003-break-glass-{}@test.invalid",
        harness.namespace()
    );
    let glass_id: String = sqlx::query_scalar(
        "insert into app_user (display_name, email) values ($1, $2) returning id::text",
    )
    .bind("sec-audit-003 break-glass root")
    .bind(&fixture_email)
    .fetch_one(harness.pool())
    .await
    .expect("the fixture break-glass user must insert");

    // (2) THE GLASS IS USED: the ROOT identity exercises the root-only command. `decide` allows it and must log
    //     the grant with the granting rule.
    let decision = service
        .decide(
            model::security::ROLE_MANAGE,
            OperationKind::Command,
            &break_glass_context(&glass_id, &marker),
        )
        .await
        .expect("a break-glass decision answers");
    assert!(
        decision.allowed,
        "{api}: the break-glass ROOT identity must be allowed"
    );
    assert_eq!(
        decision.policy_id, "rule:security.manage.root",
        "{api}: the row must name the granting rule, got {}",
        decision.policy_id,
    );

    // (3) THE COMMITTED TRUTH, read back on a connection the DAO does not own. The row must say success,
    //     allowed=true, the granting rule — and, crucially, the glass-holder's id, not nobody's.
    let rows = harness
        .rows_for(&marker)
        .await
        .expect("committed audit rows must be readable");
    assert_eq!(
        rows.len(),
        1,
        "{api}: one break-glass grant must commit one row, got {}",
        rows.len()
    );
    let row = &rows[0];
    assert_eq!(
        row.event_type,
        model::security::ROLE_MANAGE,
        "{api}: the row names the action"
    );
    assert_eq!(
        row.authentication_method.as_deref(),
        Some("rust-service"),
        "{api}: the row names the production writer",
    );
    assert_eq!(
        row.metadata["outcome"], "success",
        "{api}: the grant is a success: {}",
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
        Some(glass_id.as_str()),
        "{api}: the row must attribute the glass-holder, not nobody",
    );
    assert_eq!(
        row.metadata["actor"]["id"], glass_id,
        "{api}: the metadata must carry the same actor: {}",
        row.metadata,
    );
    assert_eq!(
        row.metadata["correlationId"], marker,
        "{api}: the row carries the caller's correlation: {}",
        row.metadata,
    );

    // (4) NEGATIVE: THE SUCCESS ROW BELONGS TO THE GLASS-HOLDER ALONE. The same root-only command decided for
    //     a signed-in NON-root user must be refused AND logged as a failure — a boundary that logged one canned
    //     break-glass success for every call would fail here, and so would a grant logged against nobody.
    let refused = service
        .decide(
            model::security::ROLE_MANAGE,
            OperationKind::Command,
            &staff_context(&glass_id, &marker),
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
        2,
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
    let successes = rows
        .iter()
        .filter(|row| {
            row.metadata["outcome"] == "success"
                && row.app_user_id.as_deref() == Some(glass_id.as_str())
        })
        .count();
    assert_eq!(
        successes, 1,
        "{api}: exactly the glass-holder's row must say success: {rows:?}",
    );

    // (5) LEAVE DEV AS IT WAS FOUND. Exactly this test's rows go, then the fixture user, and the leftover
    //     count proves nothing survived.
    let removed = harness
        .cleanup(&marker)
        .await
        .expect("cleanup deletes the marker rows");
    assert_eq!(
        removed, 2,
        "{api}: cleanup must remove exactly this test's rows"
    );
    sqlx::query("delete from app_user where id = $1::uuid")
        .bind(&glass_id)
        .execute(harness.pool())
        .await
        .expect("the fixture break-glass user must delete");
    assert_eq!(
        harness
            .leftover_count(&marker)
            .await
            .expect("leftover must be readable"),
        0,
        "{api}: no audit row may survive the test",
    );
}
