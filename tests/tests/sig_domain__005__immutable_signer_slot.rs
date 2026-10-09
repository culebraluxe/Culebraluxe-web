//! SIG.DOMAIN — immutable signer slot (TST-SIG-DOMAIN-005).
//!
//! Contract: within a single signature request, each signer must have a unique execution_slot_id.
//! The database enforces this via a unique partial index on
//! `signature_envelope_recipient(signature_request_id, execution_slot_id) WHERE execution_slot_id IS NOT NULL`.
//!
//! This test verifies that the database rejects attempts to assign the same execution_slot_id
//! to multiple signers within the same signature request.
//!
//! Level: L2 Persistence — the production database constraints against an isolated, disposable DEV/Neon target.
//!
//! The negative case is the one that matters: if duplicate execution_slot_ids were allowed,
//! two signers could be assigned to the same slot, breaking the signing workflow.
//!
//! Run with:
//!   DATABASE_URL_DEV=... cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test sig_domain__005__immutable_signer_slot -- --ignored

use db::{Database, SignatureDao};
use model::{PrepareSignatureRequest, PreparedSignatureRecipient, SignatureRecipientRole};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceActor,
    ServiceActorKind, ServiceContext, ServiceInfrastructure,
};
use std::sync::Arc;
use test_harness::providers::FakeSignatureProvider;

use test_harness::database::{HarnessDbError, TestDatabase};

const HARNESS: &str = "SignatureDao/L2 Persistence";

async fn connect_dev() -> Result<(TestDatabase, SignatureDao), HarnessDbError> {
    let database = TestDatabase::connect_declared(Some("dev"), Some("dev")).await?;
    let dao = SignatureDao::new(database.database().clone());
    Ok((database, dao))
}

fn test_context() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("test".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: "sig-domain-005".into(),
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
    let snapshot = serde_json::json!({
        "issuedParticipants": [
            {
                "slotId": "buyer:slot-1",
                "role": "buyer",
                "email": "signer@test.example"
            }
        ]
    });
    let checksum = "test-checksum-sha256";
    sqlx::query_scalar(
        "insert into transaction_document (id, deal_id, document_type, title, state, source, source_external_id, source_snapshot, issued_checksum_sha256, template_id, template_version, issued_version, created_at, updated_at)
         values (gen_random_uuid(), (select id from deal limit 1), 'agreement', 'Test Document', 'draft', 'generated', $1, $2, $3, 'test-template', 1, 1, now(), now())
         returning id::text",
    )
    .bind(format!("{ns}-doc"))
    .bind(snapshot)
    .bind(checksum)
    .fetch_one(database.database().pool())
    .await
    .expect("transaction document must be created")
}

#[tokio::test]
#[ignore = "needs DATABASE_URL_DEV (a disposable DEV branch); TestDatabase refuses PROD before any socket"]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-SIG-DOMAIN-005)
async fn sig_domain_005__immutable_signer_slot() {
    // 0. L2 boundary: an isolated disposable DEV/Neon target, never PRODUCTION.
    let (database, signature_dao) = connect_dev().await.expect("dev database must connect");
    let ns = database.namespace().to_string();
    let internal = test_context();

    // 1. Create a transaction document.
    let tx_doc_id = create_transaction_document(&database, &ns).await;

    // 2. Create a signature request with one signer with execution_slot_id.
    let mut tx = database
        .database()
        .begin("sig-domain-005.prepare")
        .await
        .expect("tx");
    let result = signature_dao
        .prepare_tx(
            &mut tx,
            &PrepareSignatureRequest {
                transaction_document_id: tx_doc_id.clone(),
                recipients: vec![PreparedSignatureRecipient {
                    role: SignatureRecipientRole::Signer,
                    name: "Test Signer".into(),
                    email: "signer@test.example".into(),
                    order: 1,
                    signing_step: 1,
                    execution_role: Some("buyer".into()),
                    execution_slot_id: Some("buyer:slot-1".into()),
                }],
                message: None,
                created_by_user_id: None,
            },
        )
        .await
        .expect("prepare must succeed");
    let req_id = result.signature_request.id.clone();
    tx.commit().await.expect("commit");

    // 3. Verify the signer slot was created correctly.
    let slot_id: Option<String> = sqlx::query_scalar(
        "select execution_slot_id from signature_envelope_recipient where signature_request_id = $1::uuid",
    )
    .bind(&req_id)
    .fetch_one(database.database().pool())
    .await
    .expect("slot must read");
    assert_eq!(
        slot_id,
        Some("buyer:slot-1".into()),
        "{HARNESS}: signer must have correct execution_slot_id"
    );

    // 4. Attempt to add a second signer with the same execution_slot_id - must be rejected by unique constraint.
    //    We use replace_recipients_tx to replace the recipient list with two signers having the same slot.
    let mut tx2 = database
        .database()
        .begin("sig-domain-005.duplicate_slot")
        .await
        .expect("tx2");
    let result = signature_dao
        .replace_recipients_tx(
            &mut tx2,
            &req_id,
            &[
                PreparedSignatureRecipient {
                    role: SignatureRecipientRole::Signer,
                    name: "Signer 1".into(),
                    email: "signer1@test.example".into(),
                    order: 1,
                    signing_step: 1,
                    execution_role: Some("buyer".into()),
                    execution_slot_id: Some("buyer:slot-1".into()),
                },
                PreparedSignatureRecipient {
                    role: SignatureRecipientRole::Signer,
                    name: "Signer 2".into(),
                    email: "signer2@test.example".into(),
                    order: 2,
                    signing_step: 1,
                    execution_role: Some("seller".into()),
                    execution_slot_id: Some("buyer:slot-1".into()), // Same slot!
                },
            ],
        )
        .await;

    assert!(
        result.is_err(),
        "{HARNESS}: duplicate execution_slot_id must be rejected"
    );
    let error = result.unwrap_err();
    let error_str = error.to_string().to_lowercase();
    assert!(
        error_str.contains("duplicate")
            || error_str.contains("unique")
            || error_str.contains("already"),
        "{HARNESS}: rejection must mention duplicate/unique/already, got {error}"
    );
    tx2.rollback().await.expect("rollback");

    // 5. Verify the original slot is unchanged.
    let slot_id: Option<String> = sqlx::query_scalar(
        "select execution_slot_id from signature_envelope_recipient where signature_request_id = $1::uuid",
    )
    .bind(&req_id)
    .fetch_one(database.database().pool())
    .await
    .expect("slot must read");
    assert_eq!(
        slot_id,
        Some("buyer:slot-1".into()),
        "{HARNESS}: signer slot must remain unchanged"
    );

    // 6. Cleanup.
    sqlx::query("delete from transaction_document where source_external_id like $1")
        .bind(format!("{}%", ns))
        .execute(database.database().pool())
        .await
        .expect("cleanup must work");
}
