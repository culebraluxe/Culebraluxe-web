//! SIG.DOMAIN — terminal state cannot reopen (TST-SIG-DOMAIN-003).
//!
//! Contract: once a signature request reaches a terminal state (completed, declined, voided, expired, error),
//! it cannot transition to any other state. The application enforces this via SignatureService.transition_transactional.
//!
//! This test verifies that the production SignatureService rejects attempts to transition from a terminal state.
//!
//! Level: L2 Persistence — the production `SignatureService` against an isolated, disposable DEV/Neon target.
//!
//! The negative case is the one that matters: if a terminal state could be reopened, the audit trail
//! would be broken and a completed signing could be undone.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test sig_domain__003__terminal_state_cannot_reopen -- --ignored

use db::{Database, SignatureDao};
use model::{
    PrepareSignatureRequest, PreparedSignatureRecipient, SignatureRecipientRole,
    SignatureRequestStatus,
};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceActor,
    ServiceActorKind, ServiceContext, ServiceInfrastructure,
};
use std::sync::Arc;
use test_harness::providers::FakeSignatureProvider;
use web::signature::SignatureService;

use test_harness::database::{HarnessDbError, TestDatabase};

const HARNESS: &str = "SignatureService/L2 Persistence";

async fn connect_dev() -> Result<(TestDatabase, SignatureService<SignatureDao>), HarnessDbError> {
    let database = TestDatabase::connect_declared(Some("dev"), Some("dev")).await?;
    let dao = SignatureDao::new(database.database().clone());
    let infrastructure = ServiceInfrastructure::new(
        Arc::new(DefaultAuthorizationPort),
        Arc::new(CapturingAuditPort::default()),
        Arc::new(CapturingDomainEventPort::default()),
    );
    let provider = Arc::new(FakeSignatureProvider::accepting());
    let service = SignatureService::new(dao, provider, infrastructure);
    Ok((database, service))
}

fn test_context() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("test".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: "sig-domain-003".into(),
        causation_id: None,
        principal: Some(services::ServicePrincipal {
            app_user_id: "test-system".into(),
            level: "system".into(),
            role_codes: vec!["root".into()],
            account_type: "internal".into(),
            entitlement_codes: vec!["signature.write".into(), "luxesign.write".into()],
        }),
    }
}

async fn create_transaction_document(database: &TestDatabase, ns: &str) -> String {
    sqlx::query_scalar(
        "insert into transaction_document (id, deal_id, document_type, title, state, source, source_external_id, created_at, updated_at)
         values (gen_random_uuid(), (select id from deal limit 1), 'agreement', 'Test Document', 'draft', 'generated', $1, now(), now())
         returning id::text",
    )
    .bind(format!("{ns}-doc"))
    .fetch_one(database.database().pool())
    .await
    .expect("transaction document must be created")
}

async fn create_completed_request(
    database: &TestDatabase,
    service: &SignatureService<SignatureDao>,
    ns: &str,
) -> String {
    let tx_doc_id = create_transaction_document(database, ns).await;
    let internal = test_context();
    let mut tx = database
        .database()
        .begin("sig-domain-003.create")
        .await
        .expect("tx");
    let result = service
        .prepare_transactional(
            &mut tx,
            &PrepareSignatureRequest {
                transaction_document_id: tx_doc_id.clone(),
                recipients: vec![PreparedSignatureRecipient {
                    role: SignatureRecipientRole::Signer,
                    name: "Test Signer".into(),
                    email: "signer@test.example".into(),
                    order: 1,
                    signing_step: 1,
                    execution_role: None,
                    execution_slot_id: None,
                }],
                message: None,
                created_by_user_id: None,
            },
            &internal,
        )
        .await
        .expect("prepare must succeed");
    let req_id = result.luxesign_request.id.clone();
    // Transition through the full lifecycle to completed
    service
        .transition_transactional(&mut tx, &req_id, SignatureRequestStatus::Sent, &internal)
        .await
        .expect("sent");
    service
        .transition_transactional(&mut tx, &req_id, SignatureRequestStatus::Viewed, &internal)
        .await
        .expect("viewed");
    service
        .transition_transactional(&mut tx, &req_id, SignatureRequestStatus::Signed, &internal)
        .await
        .expect("signed");
    service
        .transition_transactional(
            &mut tx,
            &req_id,
            SignatureRequestStatus::Completed,
            &internal,
        )
        .await
        .expect("completed");
    tx.commit().await.expect("commit");
    req_id
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SIG-DOMAIN-003)
async fn sig_domain_003__terminal_state_cannot_reopen() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let (database, service) = connect_dev().await.expect("dev database must connect");
    let ns = database.namespace().to_string();
    let internal = test_context();

    // 1. Create a completed signature request.
    let req_id = create_completed_request(&database, &service, &ns).await;

    // 2. Attempt to transition from completed to any other state - all must fail.
    let terminal_states = ["completed", "declined", "voided", "expired", "error"];
    for target in [
        "requested",
        "sent",
        "viewed",
        "signed",
        "declined",
        "voided",
        "expired",
        "error",
    ] {
        let mut tx = database
            .database()
            .begin("sig-domain-003.transition")
            .await
            .expect("tx");
        let result = service
            .transition_transactional(
                &mut tx,
                &req_id,
                SignatureRequestStatus::try_from(target).unwrap(),
                &internal,
            )
            .await;
        assert!(
            result.is_err(),
            "{HARNESS}: completed -> {target} must be rejected"
        );
        let error = result.unwrap_err();
        assert!(
            error.to_string().contains("not allowed") || error.to_string().contains("terminal") || error.to_string().contains("cannot"),
            "{HARNESS}: rejection must mention terminal/cannot/not allowed for completed -> {target}, got {error}"
        );
        tx.rollback().await.expect("rollback");
    }

    // 3. Verify the request is still completed.
    let status: String =
        sqlx::query_scalar("select status from luxesign_request where id = $1::uuid")
            .bind(&req_id)
            .fetch_one(database.database().pool())
            .await
            .expect("status must read");
    assert_eq!(
        status, "completed",
        "{HARNESS}: request must remain completed"
    );

    // 4. Test other terminal states (declined, voided, expired, error) also cannot reopen.
    for terminal in vec!["declined", "voided", "expired", "error"] {
        let tx_doc_id = create_transaction_document(&database, &ns).await;
        let req_id: String = sqlx::query_scalar(
            "insert into luxesign_request (id, transaction_document_id, status, created_at, updated_at)
             values (gen_random_uuid(), $1::uuid, $2, now(), now())
             returning id::text",
        )
        .bind(&tx_doc_id)
        .bind(terminal.to_string())
        .fetch_one(database.database().pool())
        .await
        .expect("signature request must be created");

        for target in ["requested", "sent", "viewed", "signed", "completed"] {
            let mut tx = database
                .database()
                .begin("sig-domain-003.transition")
                .await
                .expect("tx");
            let result = service
                .transition_transactional(
                    &mut tx,
                    &req_id,
                    SignatureRequestStatus::try_from(target).unwrap(),
                    &internal,
                )
                .await;
            assert!(
                result.is_err(),
                "{HARNESS}: {terminal} -> {target} must be rejected"
            );
            tx.rollback().await.expect("rollback");
        }
    }

    // 5. Cleanup.
    sqlx::query("delete from transaction_document where source_external_id like $1")
        .bind(format!("{}%", ns))
        .execute(database.database().pool())
        .await
        .expect("cleanup must work");
}
