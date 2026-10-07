//! SEC.AUDIT — denied action logged (TST-SEC-AUDIT-001).
//!
//! CONTRACT. An authorization denial must leave a durable row: when the production `SecurityService::decide`
//! (`web/src/security/mod.rs`) refuses an action, it audits the refusal through the service kernel
//! (`ServiceRuntime::audit`, `middle/services/src/runtime.rs`) into the production `DurableSecurityAuditPort`
//! (`web/src/security/audit.rs`), which writes `security_audit_event` through the production `SecurityAuditDao`.
//! A denial that is returned but never recorded is unobservable — the audit trail would say nothing exactly where
//! an operator most needs it to speak. So: every denied `decide` commits exactly one row carrying the failure,
//! the decision, and the actor, and an allowed decision on the same action commits a row that says success — the
//! failure rows are the decisions', not wallpaper.
//!
//! THE BOUNDARY UNDER TEST IS PRODUCTION'S OWN. `decide` is the production entry point (the API authorize route
//! calls it, `web/src/api/routes/security_service.rs:305`); the authorization is the production
//! `CasbinAuthorizationPort`; the write is the production port into the production table, read back on a
//! connection the DAO does not own. The test states only what the rows must contain.
//!
//! NEGATIVE CASES. A test that only recorded denials could pass on a boundary that logs every call as a failure.
//! So the same root-only action is also decided for a ROOT principal, which must be allowed AND logged as a
//! success — and a second denial (the `tech.operate` domain floor for a signed-in non-root user) must carry its
//! own reason. A boundary that logged one canned failure for everything would fail here.
//!
//! ISOLATION. The database is the harness's disposable DEV target: `SecurityHarness::connect_declared` resolves
//! the declared environment and refuses `DbTarget::Prod` before any socket is opened
//! (`tests/src/database.rs:68-89`). Rows are addressed by a unique `correlationId` marker the production port
//! already writes, the fixture user exists only for this test, and cleanup deletes exactly those rows — a zero
//! leftover count is asserted, so DEV is left as it was found.
//!
//! Level: L2 Persistence — the production audit write against an isolated, disposable DEV/Neon target, harness
//! `SecurityHarness`.
//!
//! Run with:
//!   set -a; . ./.env.local; set +a
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_audit__001__denied_action_logged -- --ignored --nocapture
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use services::{
    CapturingDomainEventPort, OperationKind, ServiceActor, ServiceActorKind, ServiceContext,
    ServiceInfrastructure, ServicePrincipal,
};
use std::sync::Arc;
use test_harness::SecurityHarness;
use web::security::{CasbinAuthorizationPort, DurableSecurityAuditPort, SecurityService};

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "SecurityHarness/L2 Persistence";

/// Connect to the disposable DEV target, tolerating a cold-pool timeout under concurrent test load.
///
/// Infrastructure, not the contract: `SecurityHarness` still refuses PRODUCTION before any socket is opened.
async fn connect_dev() -> SecurityHarness {
    let mut last: Option<String> = None;
    for attempt in 1..=4 {
        match SecurityHarness::connect_declared(Some("dev"), Some("dev")).await {
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

fn system_context(marker: &str) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: None,
            kind: ServiceActorKind::System,
        },
        correlation_id: marker.to_owned(),
        causation_id: None,
        principal: None,
    }
}

fn user_context(actor_id: &str, marker: &str, roles: Vec<&str>) -> ServiceContext {
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
            role_codes: roles.into_iter().map(str::to_owned).collect(),
            account_type: model::security::INTERNAL_ACCOUNT.to_owned(),
            entitlement_codes: Vec::new(),
        }),
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV target); the harness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-AUDIT-001); the file and the assay use it.
async fn sec_audit_001__denied_action_logged() {
    let harness = connect_dev().await;
    let api = "a denied decide must commit exactly one audit row carrying the failure, the decision and the actor";
    let marker = format!("sec-audit-001-{}", harness.namespace());

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
    //     `app_user(id)` — so a USER denial needs a real user row, created here and deleted at the end.
    let fixture_email = format!("sec-audit-001-{}@test.invalid", harness.namespace());
    let fixture_id: String = sqlx::query_scalar(
        "insert into app_user (display_name, email) values ($1, $2) returning id::text",
    )
    .bind("sec-audit-001 denied-user")
    .bind(&fixture_email)
    .fetch_one(harness.pool())
    .await
    .expect("the fixture user must insert");

    // (2) DENIAL ONE: a caller with no principal reaches a root-only command. `decide` answers the denial
    //     (it does not raise it) and must log it as a failure.
    let decision = service
        .decide(
            model::security::ROLE_MANAGE,
            OperationKind::Command,
            &system_context(&marker),
        )
        .await
        .expect("a denial is an answer, not an error");
    assert!(
        !decision.allowed,
        "{api}: a principal-less caller must be refused"
    );
    assert_eq!(
        decision.policy_id, "principal:missing",
        "{api}: the row must name the rule that refused, got {}",
        decision.policy_id,
    );

    // (3) DENIAL TWO: a signed-in non-root user reaches the same root-only command — a different rule refuses,
    //     and the row must carry THAT reason and THAT actor.
    let decision = service
        .decide(
            model::security::ROLE_MANAGE,
            OperationKind::Command,
            &user_context(&fixture_id, &marker, vec!["staff"]),
        )
        .await
        .expect("a denial is an answer, not an error");
    assert!(
        !decision.allowed,
        "{api}: a non-root user must be refused the root-only command"
    );
    assert_eq!(
        decision.policy_id, "rule:security.manage.root",
        "{api}: the row must name the root-only rule, got {}",
        decision.policy_id,
    );

    // (4) THE COMMITTED TRUTH, read back on a connection the DAO does not own. Two denials, two rows — each
    //     carrying outcome failure, allowed=false, its own reason, and its own actor attribution.
    let rows = harness
        .rows_for(&marker)
        .await
        .expect("committed audit rows must be readable");
    assert_eq!(
        rows.len(),
        2,
        "{api}: two denials must commit two rows, got {}",
        rows.len()
    );
    for row in &rows {
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
            row.metadata["outcome"], "failure",
            "{api}: a denial is a failure: {}",
            row.metadata
        );
        assert_eq!(
            row.metadata["authorization"]["allowed"], false,
            "{api}: the decision must travel into the row: {}",
            row.metadata,
        );
        assert!(
            row.metadata["errorCode"]
                .as_str()
                .is_some_and(|code| !code.is_empty()),
            "{api}: a denial without its reason is unauditable: {}",
            row.metadata,
        );
        assert_eq!(
            row.metadata["correlationId"], marker,
            "{api}: the row carries the caller's correlation: {}",
            row.metadata,
        );
    }
    // The decision's discriminating field is `policyId` (`reason` is always "allowed" or
    // "no matching entitlement" by production construction, `web/src/security/entitlements.rs`): each denial
    // must carry its own rule, while both share the same human reason.
    let policies: Vec<&str> = rows
        .iter()
        .filter_map(|row| row.metadata["authorization"]["policyId"].as_str())
        .collect();
    assert!(
        policies.contains(&"principal:missing") && policies.contains(&"rule:security.manage.root"),
        "{api}: each denial must carry its own rule, got {policies:?}",
    );
    let reasons: Vec<&str> = rows
        .iter()
        .filter_map(|row| row.metadata["errorCode"].as_str())
        .collect();
    assert!(
        reasons
            .iter()
            .all(|reason| *reason == "no matching entitlement"),
        "{api}: denials share production's refusal reason, got {reasons:?}",
    );
    let attributed = rows
        .iter()
        .filter(|row| row.app_user_id.as_deref() == Some(fixture_id.as_str()))
        .count();
    assert_eq!(
        attributed, 1,
        "{api}: exactly the signed-in denial attributes the user; the principal-less one must not invent one",
    );

    // (5) NEGATIVE: THE FAILURE ROWS ARE THE DECISIONS', NOT WALLPAPER. The same action decided for a ROOT
    //     principal must be allowed AND logged as a success — a boundary that logged one canned failure for
    //     every call would fail here.
    let allowed = service
        .decide(
            model::security::ROLE_MANAGE,
            OperationKind::Command,
            &user_context(&fixture_id, &marker, vec!["root"]),
        )
        .await
        .expect("a ROOT decision answers");
    assert!(
        allowed.allowed,
        "{api}: ROOT must be allowed the root-only command"
    );
    let rows = harness
        .rows_for(&marker)
        .await
        .expect("rows must be readable");
    assert_eq!(
        rows.len(),
        3,
        "{api}: the allowed decision must also be logged, got {}",
        rows.len()
    );
    let successes = rows
        .iter()
        .filter(|row| {
            row.metadata["outcome"] == "success" && row.metadata["authorization"]["allowed"] == true
        })
        .count();
    assert_eq!(
        successes, 1,
        "{api}: exactly one row must say success: {rows:?}"
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
