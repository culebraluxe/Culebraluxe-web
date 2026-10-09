//! SEC.AUDIT — no secrets in audit detail (TST-SEC-AUDIT-004).
//!
//! CONTRACT. The audit trail is read by operators, shipped to vendors, and kept for years — so the production
//! `DurableSecurityAuditPort` (`web/src/security/audit.rs`) must never write secret-shaped material into
//! `security_audit_event.metadata`: no passwords, tokens, hashes, keys, or bearer credentials in any key or any
//! string value. The port builds `metadata` from a FIXED shape (domain, operation, actor id/kind, correlation and
//! causation ids, outcome, error code, the authorization decision, source) and the only free text it carries is
//! the policy reason (`rule:security.manage.root`, `principal:missing`) — both rule names, never credentials. So
//! every row the auth-decision boundary commits must scan clean, on both the denial and the success path.
//!
//! THE BOUNDARY UNDER TEST IS PRODUCTION'S OWN. The rows are committed by driving the production
//! `SecurityService::decide` (denied and allowed) through the production port into the production table, and read
//! back on a connection the DAO does not own. The scanner below judges the committed JSON, not a model of it.
//!
//! NEGATIVE CASES. A scanner that never fires proves nothing, so it is turned on synthetic hostile metadata
//! first: secret keys (`token`, `password`, nested) and secret-shaped values (`Bearer …`, a private-key block)
//! must all be flagged, while the production shape's own `authorization` decision object must NOT be — precision
//! matters as much as recall, because a scanner that flags the decision object would fail the real rows. And a
//! signed-in non-root denial plus a ROOT grant are both committed and scanned, so the clean verdict covers the
//! failure path (which carries an error code) as well as the success path.
//!
//! SCOPE, STATED HONESTLY. This proves the auth-decision metadata the port builds today carries no secrets. The
//! port performs no redaction — it does not need to, because nothing it is handed is secret — so a producer that
//! stuffed a credential into a reason string would land verbatim; that producer does not exist on this boundary
//! (reasons are policy-rule names), and this test pins the shape that keeps it so.
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
//!   cargo test --manifest-path Cargo.toml -p test-harness --test sec_audit__004__no_secrets_in_audit_detail -- --ignored --nocapture
//! The plain command (no `--ignored`) passes with the test skipped, because the L2 contract needs a disposable DEV
//! database and the harness will never open a PRODUCTION one.

use serde_json::{json, Value};
use services::{
    CapturingDomainEventPort, OperationKind, ServiceActor, ServiceActorKind, ServiceContext,
    ServiceInfrastructure, ServicePrincipal,
};
use std::sync::Arc;
use test_harness::AuditPersistenceHarness;
use web::security::{CasbinAuthorizationPort, DurableSecurityAuditPort, SecurityService};

/// The harness name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "AuditPersistenceHarness/L2 Persistence";

/// Object keys that name secret material. Deliberately NOT in this list: `authorization` (the production
/// decision object — allowed/reason/policyId/mode) and `authentication_method` (the writer name). A scanner
/// that flagged those would fail the real rows, which is why the precision case below exists.
const SECRET_KEYS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "secrets",
    "token",
    "tokens",
    "api_key",
    "apikey",
    "access_token",
    "refresh_token",
    "private_key",
    "client_secret",
    "bearer",
    "credentials",
    "credential",
    "cookie",
    "set_cookie",
    "session_token",
];

/// Secret-shaped string values: a bearer credential, a key block, or an embedded assignment. Rule names such as
/// `rule:security.manage.root` carry none of these shapes.
const SECRET_VALUE_MARKERS: &[&str] = &["bearer ", "private key", "begin ", "password=", "secret="];

/// True when `metadata` carries secret-shaped material in any key or any string value, at any depth.
fn contains_secret(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, nested)| {
            let lowered = key.to_lowercase();
            SECRET_KEYS.iter().any(|secret| lowered == *secret) || contains_secret(nested)
        }),
        Value::Array(items) => items.iter().any(contains_secret),
        Value::String(text) => {
            let lowered = text.to_lowercase();
            SECRET_VALUE_MARKERS
                .iter()
                .any(|marker| lowered.contains(marker))
        }
        _ => false,
    }
}

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

fn principal_context(
    actor_id: &str,
    marker: &str,
    level: &str,
    roles: Vec<&str>,
) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some(actor_id.to_owned()),
            kind: ServiceActorKind::User,
        },
        correlation_id: marker.to_owned(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: actor_id.to_owned(),
            level: level.to_owned(),
            role_codes: roles.into_iter().map(str::to_owned).collect(),
            account_type: model::security::INTERNAL_ACCOUNT.to_owned(),
            entitlement_codes: Vec::new(),
        }),
    }
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV target); the harness refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SEC-AUDIT-004); the file and the assay use it.
async fn sec_audit_004__no_secrets_in_audit_detail() {
    let api = "committed audit metadata must carry no secret-shaped key or value on any path";
    let scanner = "the secret scanner";

    // (1) NEGATIVE FIRST: THE SCANNER MUST BITE. Hostile metadata — secret keys at the top level and nested,
    //     and secret-shaped values — must all be flagged, or a clean verdict on the real rows would be vacuous.
    for hostile in [
        json!({"token": "abc123"}),
        json!({"password": "hunter2"}),
        json!({"nested": {"client_secret": "shh"}}),
        json!({"authorization": {"allowed": false}, "note": "Bearer abc123"}),
        json!({"detail": "-----BEGIN PRIVATE KEY-----"}),
    ] {
        assert!(
            contains_secret(&hostile),
            "{scanner}: must flag hostile metadata: {hostile}",
        );
    }

    // (2) AND IT MUST NOT BITE THE PRODUCTION SHAPE. The decision object, the writer name, rule reasons and
    //     correlation ids are the everyday contents of these rows — flagging them would fail clean rows.
    for clean in [
        json!({"authorization": {"allowed": false, "reason": "no matching entitlement", "policyId": "principal:missing", "mode": "enforced"}}),
        json!({"authentication_method": "rust-service", "domain": "security", "operation": "security.role.manage"}),
        json!({"correlationId": "sec-audit-004-abc", "errorCode": "no matching entitlement", "outcome": "failure"}),
    ] {
        assert!(
            !contains_secret(&clean),
            "{scanner}: must not flag the production shape: {clean}",
        );
    }

    // (3) THE COMMITTED TRUTH: a denial (which carries an error code) and a grant, both through the production
    //     boundary, both scanned as committed.
    let harness = connect_dev().await;
    let marker = format!("sec-audit-004-{}", harness.namespace());
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

    let fixture_email = format!("sec-audit-004-{}@test.invalid", harness.namespace());
    let fixture_id: String = sqlx::query_scalar(
        "insert into app_user (display_name, email) values ($1, $2) returning id::text",
    )
    .bind("sec-audit-004 scanner-user")
    .bind(&fixture_email)
    .fetch_one(harness.pool())
    .await
    .expect("the fixture user must insert");

    service
        .decide(
            model::security::ROLE_MANAGE,
            OperationKind::Command,
            &principal_context(&fixture_id, &marker, "USER", vec!["staff"]),
        )
        .await
        .expect("a denial is an answer, not an error");
    service
        .decide(
            model::security::ROLE_MANAGE,
            OperationKind::Command,
            &principal_context(&fixture_id, &marker, "ROOT", vec!["root"]),
        )
        .await
        .expect("a ROOT decision answers");

    let rows = harness
        .rows_for(&marker)
        .await
        .expect("committed audit rows must be readable");
    assert_eq!(
        rows.len(),
        2,
        "{api}: two decisions must commit two rows, got {}",
        rows.len()
    );
    let outcomes: Vec<&str> = rows
        .iter()
        .filter_map(|row| row.metadata["outcome"].as_str())
        .collect();
    assert!(
        outcomes.contains(&"failure") && outcomes.contains(&"success"),
        "{api}: the scan must cover both the error-code path and the success path, got {outcomes:?}",
    );
    for row in &rows {
        assert!(
            !contains_secret(&row.metadata),
            "{api}: committed metadata must scan clean: {}",
            row.metadata,
        );
    }

    // (4) LEAVE DEV AS IT WAS FOUND. Exactly this test's rows go, then the fixture user, and the leftover
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
