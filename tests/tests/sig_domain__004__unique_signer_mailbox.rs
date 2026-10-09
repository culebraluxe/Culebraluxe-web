//! SIG.DOMAIN — unique signer mailbox (TST-SIG-DOMAIN-004).
//!
//! Contract: each signer in a signature request must have a unique email address.
//! The database enforces this via application logic in SignatureService.prepare_transactional.
//!
//! This test verifies that the production SignatureService rejects attempts to add a signer
//! with an email that already exists in the same signature request.
//!
//! Level: L2 Persistence — the production `SignatureService` against an isolated, disposable DEV/Neon target.
//!
//! The negative case is the one that matters: if duplicate emails were allowed, a signer could
//! be added multiple times, breaking the signing workflow.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test sig_domain__004__unique_signer_mailbox -- --ignored

use db::{Database, SignatureDao};
use model::{PrepareSignatureRequest, PreparedSignatureRecipient, SignatureRecipientRole};
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
        correlation_id: "sig-domain-004".into(),
        causation_id: None,
        principal: Some(services::ServicePrincipal {
            app_user_id: "test-system".into(),
            level: "system".into(),
            role_codes: vec!["root".into()],
            account_type: "internal".into(),
            entitlement_codes: vec!["signature.write".into()],
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

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SIG-DOMAIN-004)
async fn sig_domain_004__unique_signer_mailbox() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let (database, service) = connect_dev().await.expect("dev database must connect");
    let ns = database.namespace().to_string();
    let internal = test_context();

    // 1. Create a transaction document.
    let tx_doc_id = create_transaction_document(&database, &ns).await;

    // 2. Create a signature request with one signer.
    let mut tx = database
        .database()
        .begin("sig-domain-004.prepare")
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
    let req_id = result.signature_request.id.clone();
    tx.commit().await.expect("commit");

    // 3. Attempt to add a second signer with the same email - must be rejected.
    let mut tx2 = database
        .database()
        .begin("sig-domain-004.add_signer")
        .await
        .expect("tx2");
    let result = service
        .prepare_transactional(
            &mut tx2,
            &PrepareSignatureRequest {
                transaction_document_id: tx_doc_id.clone(),
                recipients: vec![
                    PreparedSignatureRecipient {
                        role: SignatureRecipientRole::Signer,
                        name: "Test Signer".into(),
                        email: "signer@test.example".into(),
                        order: 1,
                        signing_step: 1,
                        execution_role: None,
                        execution_slot_id: None,
                    },
                    PreparedSignatureRecipient {
                        role: SignatureRecipientRole::Signer,
                        name: "Duplicate Signer".into(),
                        email: "signer@test.example".into(), // Same email!
                        order: 2,
                        signing_step: 1,
                        execution_role: None,
                        execution_slot_id: None,
                    },
                ],
                message: None,
                created_by_user_id: None,
            },
            &internal,
        )
        .await;

    assert!(
        result.is_err(),
        "{HARNESS}: duplicate signer email must be rejected"
    );
    let error = result.unwrap_err();
    assert!(
        error.to_string().contains("duplicate")
            || error.to_string().contains("unique")
            || error.to_string().contains("already"),
        "{HARNESS}: rejection must mention duplicate/unique/already, got {error}"
    );
    tx2.rollback().await.expect("rollback");

    // 4. Verify the original request still has only one signer.
    let count: i64 = sqlx::query_scalar(
        "select count(*) from signature_envelope_recipient where signature_request_id = $1::uuid",
    )
    .bind(&req_id)
    .fetch_one(database.database().pool())
    .await
    .expect("count must work");
    assert_eq!(
        count, 1,
        "{HARNESS}: original request must have only one signer"
    );

    // 5. Cleanup.
    sqlx::query("delete from transaction_document where source_external_id like $1")
        .bind(format!("{}%", ns))
        .execute(database.database().pool())
        .await
        .expect("cleanup must work");
}
