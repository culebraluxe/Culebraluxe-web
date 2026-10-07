//! Native signing boundary proofs against DEV (DocuSign clone).
//!
//! Run explicitly with:
//!   DATABASE_URL_DEV=... cargo test -p test-harness --test docsign_native_dev -- --ignored
//!
//! WHY THESE EXIST. The three signing services carry only pure-logic unit
//! tests (validation, codec, renderer). The properties that make this a
//! signing system rather than a form Mailer — turn order, consent gating,
//! field ownership, consent immutability, queue dedupe — only exist against
//! a real database, and a green unit suite says nothing about them.
//!
//! ISOLATION. Every fixture is tagged and removed afterwards: the envelope
//! (transaction_document, cascade-deleting the request, recipients, fields,
//! states, access and consent rows), the evidence rows (recipient FK is
//! `on delete restrict`, so they go first), and the email/outbox rows by
//! correlation id. Reads of ambient DEV rows are limited to borrowing one
//! existing deal id as the envelope anchor.

use db::EmailDao;
use db::{Database, DbTarget, SignerDao};
use model::{
    AcceptSignerConsentRequest, CompleteSignatureFieldRequest, CompleteSignerRequest,
    EmailMessageKind, QueueEmailRequest,
};
use services::{
    CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceActor,
    ServiceActorKind, ServiceContext, ServiceInfrastructure, ServicePrincipal,
};
use std::sync::Arc;
use uuid::Uuid;
use web::email::EmailService;
use web::service_support::CoreServiceError;
use web::signer::{SignerAccessTokenCodec, SignerService, DOCSIGN_EDGE_ACTOR};

fn context(tag: &str) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some(DOCSIGN_EDGE_ACTOR.into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: tag.into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "docsign-proof".into(),
            level: "USER".into(),
            role_codes: vec![],
            account_type: "internal".into(),
            entitlement_codes: vec![],
        }),
    }
}

fn infra() -> ServiceInfrastructure {
    ServiceInfrastructure::new(
        Arc::new(DefaultAuthorizationPort),
        Arc::new(CapturingAuditPort::default()),
        Arc::new(CapturingDomainEventPort::default()),
    )
}

struct Envelope {
    tag: String,
    request_id: String,
    a: String,
    b: String,
    c: String,
    field_a: String,
}

async fn recipient(db: &Database, request_id: &str, order: i32, step: i32, email: &str) -> String {
    sqlx::query_scalar::<_, String>(
        "insert into signature_envelope_recipient \
         (signature_request_id, recipient_name, recipient_email, signer_order, signing_step) \
         values ($1::uuid, $2, $3, $4, $5) returning id::text",
    )
    .bind(request_id)
    .bind(format!("Proof {email}"))
    .bind(format!("{email}@example.test"))
    .bind(order)
    .bind(step)
    .fetch_one(db.pool())
    .await
    .expect("recipient fixture")
}

/// Draft envelope on DEV: deal-anchored document, requested neutral status,
/// sequential mode, A+B in step 1, C in step 2, one required field for A.
/// Tags are unique per run because parallel lanes share DEV: a fixed tag
/// would let another runner's fixtures collide mid-test (seen live as a
/// link column changing between two sequential calls). Each test sweeps
/// its own tag on the way in and out; residue from crashed runs is swept
/// by hand, not by other runners.
async fn envelope(db: &Database, tag: &str) -> Envelope {
    sweep_tag(db, tag).await;
    let deal: String = sqlx::query_scalar("select id::text from deal limit 1")
        .fetch_one(db.pool())
        .await
        .expect("DEV must hold at least one deal to anchor the envelope");
    let txdoc: String = sqlx::query_scalar(
        "insert into transaction_document (deal_id, document_type, title, state, source) \
         values ($1::uuid, 'agreement', $2, 'draft', 'generated') returning id::text",
    )
    .bind(&deal)
    .bind(format!("docsign proof {tag}"))
    .fetch_one(db.pool())
    .await
    .expect("transaction_document fixture");
    let request_id: String = sqlx::query_scalar(
        "insert into signature_request (transaction_document_id, status) \
         values ($1::uuid, 'requested') returning id::text",
    )
    .bind(&txdoc)
    .fetch_one(db.pool())
    .await
    .expect("signature_request fixture");
    sqlx::query("insert into document_sign_request (signature_request_id) values ($1::uuid)")
        .bind(&request_id)
        .execute(db.pool())
        .await
        .expect("document_sign_request fixture");
    let a = recipient(db, &request_id, 1, 1, &format!("{tag}-a")).await;
    let b = recipient(db, &request_id, 2, 1, &format!("{tag}-b")).await;
    let c = recipient(db, &request_id, 3, 2, &format!("{tag}-c")).await;
    let field_a: String = sqlx::query_scalar(
        "insert into signature_field \
         (signature_request_id, recipient_id, field_key, field_type, page_number, \
          position_x, position_y, width, height, required) \
         values ($1::uuid, $2::uuid, 'sig-a', 'signature', 1, 10, 10, 30, 10, true) \
         returning id::text",
    )
    .bind(&request_id)
    .bind(&a)
    .fetch_one(db.pool())
    .await
    .expect("field fixture");
    Envelope {
        tag: tag.to_owned(),
        request_id,
        a,
        b,
        c,
        field_a,
    }
}

async fn cleanup(db: &Database, env: &Envelope) {
    sweep_tag(db, &env.tag).await;
}

/// Best-effort removal of one tag's fixture rows, in FK-safe order: evidence
/// first (its recipient link is `on delete restrict`), then the envelope
/// document (cascading the request, recipients, fields, states, access and
/// consent), then the email and outbox rows keyed by correlation id.
async fn sweep_tag(db: &Database, tag: &str) {
    let title = format!("docsign proof {tag}");
    // Unlink first: both media links are `on delete restrict`. The
    // returning clause hands back every linked id (audit, sealed, and
    // the fixture original) so no proof bytes survive the sweep.
    let media_ids: Vec<(Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "select signed_audit_media_id::text, signed_media_id::text, media_id::text \
         from transaction_document where title = $1",
    )
    .bind(&title)
    .fetch_all(db.pool())
    .await
    .expect("media list cleanup");
    sqlx::query(
        "update transaction_document set signed_audit_media_id = null, signed_media_id = null, media_id = null, signed_at = null \
         where title = $1",
    )
    .bind(&title)
    .execute(db.pool())
    .await
    .expect("media unlink cleanup");
    for (audit, sealed, original) in media_ids {
        for media_id in [audit, sealed, original].into_iter().flatten() {
            sqlx::query("delete from media where id = $1::uuid")
                .bind(&media_id)
                .execute(db.pool())
                .await
                .expect("media cleanup");
        }
    }
    sqlx::query(
        "delete from signature_evidence_event where signature_request_id in ( \
           select sr.id from signature_request sr \
           join transaction_document td on td.id = sr.transaction_document_id \
           where td.title = $1)",
    )
    .bind(&title)
    .execute(db.pool())
    .await
    .expect("evidence cleanup");
    sqlx::query("delete from email_message where correlation_id = $1 or recipient_email like $2")
        .bind(tag)
        .bind(format!("%{tag}%"))
        .execute(db.pool())
        .await
        .expect("email cleanup");
    sqlx::query("delete from outbox_message where correlation_id = $1")
        .bind(tag)
        .execute(db.pool())
        .await
        .expect("outbox cleanup");
    sqlx::query("delete from transaction_document where title = $1")
        .bind(title)
        .execute(db.pool())
        .await
        .expect("envelope cascade cleanup");
}

fn signer(db: &Database) -> SignerService<SignerDao> {
    SignerService::new(
        SignerDao::new(db.clone()),
        SignerAccessTokenCodec::for_test("docsign-proof-secret", "https://example.test"),
        infra(),
    )
}

async fn grant(
    service: &SignerService<SignerDao>,
    db: &Database,
    recipient: &str,
    ctx: &ServiceContext,
) -> String {
    let mut tx = db.begin("docsign-proof-grant").await.unwrap();
    let grant = service
        .issue_access_transactional(&mut tx, recipient, None, ctx)
        .await
        .expect("access grant");
    tx.commit().await.unwrap();
    grant.token
}

async fn complete_field(
    service: &SignerService<SignerDao>,
    db: &Database,
    recipient: &str,
    token: &str,
    field: &str,
    ctx: &ServiceContext,
) {
    let mut tx = db.begin("docsign-proof-field").await.unwrap();
    service
        .complete_field_transactional(
            &mut tx,
            &model::CompleteSignatureFieldRequest {
                ip_address: None,
                user_agent: None,
                recipient_id: recipient.into(),
                access_token: token.into(),
                field_id: field.into(),
                value: serde_json::json!({"signature": "proof-strokes"}),
            },
            ctx,
        )
        .await
        .expect("field completes");
    tx.commit().await.unwrap();
}

async fn complete(
    service: &SignerService<SignerDao>,
    db: &Database,
    recipient: &str,
    token: &str,
    ctx: &ServiceContext,
) -> model::SignerActionResult {
    let mut tx = db.begin("docsign-proof-complete").await.unwrap();
    let action = service
        .complete_transactional(
            &mut tx,
            &CompleteSignerRequest {
                ip_address: None,
                user_agent: None,
                recipient_id: recipient.into(),
                access_token: token.into(),
            },
            ctx,
        )
        .await
        .expect("recipient completes");
    tx.commit().await.unwrap();
    action
}

async fn consent(
    service: &SignerService<SignerDao>,
    db: &Database,
    recipient: &str,
    token: &str,
    ctx: &ServiceContext,
) {
    // 64 lowercase hex chars: the service requires the exact-evidence shape.
    let sha = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
    let mut tx = db.begin("docsign-proof-consent").await.unwrap();
    service
        .accept_consent_transactional(
            &mut tx,
            &AcceptSignerConsentRequest {
                recipient_id: recipient.into(),
                access_token: token.into(),
                consent_version: "v1".into(),
                consent_text: "I agree.".into(),
                consent_text_sha256: sha.into(),
                ip_address: None,
                user_agent: None,
            },
            ctx,
        )
        .await
        .expect("consent");
    tx.commit().await.unwrap();
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn parallel_group_acts_together_and_later_step_waits() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("turn-{}", Uuid::new_v4());
    let env = envelope(&db, &tag).await;
    let dao = SignerDao::new(db.clone());
    // A and B share step 1: both may act. C in step 2 waits.
    assert!(dao.is_turn(&env.a).await.unwrap());
    assert!(dao.is_turn(&env.b).await.unwrap());
    assert!(!dao.is_turn(&env.c).await.unwrap());

    // Completing A alone does not open step 2.
    let service = signer(&db);
    let ctx = context(&tag);
    let token_a = grant(&service, &db, &env.a, &ctx).await;
    consent(&service, &db, &env.a, &token_a, &ctx).await;
    complete_field(&service, &db, &env.a, &token_a, &env.field_a, &ctx).await;
    complete(&service, &db, &env.a, &token_a, &ctx).await;
    assert!(!dao.is_turn(&env.c).await.unwrap());

    // Completing B opens step 2 but the envelope is not ready: C waits.
    let token_b = grant(&service, &db, &env.b, &ctx).await;
    consent(&service, &db, &env.b, &token_b, &ctx).await;
    let action = complete(&service, &db, &env.b, &token_b, &ctx).await;
    assert!(!action.envelope_ready_to_finalize);
    assert!(dao.is_turn(&env.c).await.unwrap());

    // Completing C finishes the envelope.
    let token_c = grant(&service, &db, &env.c, &ctx).await;
    consent(&service, &db, &env.c, &token_c, &ctx).await;
    let action = complete(&service, &db, &env.c, &token_c, &ctx).await;
    assert!(action.envelope_ready_to_finalize);
    cleanup(&db, &env).await;
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn consent_gates_completion_and_field_ownership_holds() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("gates-{}", Uuid::new_v4());
    let env = envelope(&db, &tag).await;
    let service = signer(&db);
    let ctx = context(&tag);
    let token_a = grant(&service, &db, &env.a, &ctx).await;

    // No consent yet: completion refuses.
    let mut tx = db.begin("docsign-proof-gate").await.unwrap();
    let refused = service
        .complete_transactional(
            &mut tx,
            &CompleteSignerRequest {
                ip_address: None,
                user_agent: None,
                recipient_id: env.a.clone(),
                access_token: token_a.clone(),
            },
            &ctx,
        )
        .await
        .expect_err("consent is required");
    tx.rollback().await.unwrap();
    assert_eq!(refused.code(), "SIGNER_CONSENT_REQUIRED");

    consent(&service, &db, &env.a, &token_a, &ctx).await;

    // A cannot touch a field it does not own (use C's... C has no field, so
    // mint the refusal with a foreign id: the SQL owner-scope rejects it).
    let mut tx = db.begin("docsign-proof-gate").await.unwrap();
    let foreign = service
        .complete_field_transactional(
            &mut tx,
            &model::CompleteSignatureFieldRequest {
                ip_address: None,
                user_agent: None,
                recipient_id: env.b.clone(),
                access_token: token_a.clone(),
                field_id: env.field_a.clone(),
                value: serde_json::json!({"signature": "x"}),
            },
            &ctx,
        )
        .await
        .expect_err("B cannot complete A's field with A's token");
    tx.rollback().await.unwrap();
    // B's token names B while the field belongs to A: recipient binding fails
    // before ownership is even reached.
    assert_eq!(foreign.code(), "SIGNER_ACCESS_INVALID");

    // Same-recipient wrong-field: B's own token against A's field id.
    let token_b = grant(&service, &db, &env.b, &ctx).await;
    consent(&service, &db, &env.b, &token_b, &ctx).await;
    let mut tx = db.begin("docsign-proof-gate").await.unwrap();
    let refused = service
        .complete_field_transactional(
            &mut tx,
            &model::CompleteSignatureFieldRequest {
                ip_address: None,
                user_agent: None,
                recipient_id: env.b.clone(),
                access_token: token_b,
                field_id: env.field_a.clone(),
                value: serde_json::json!({"signature": "x"}),
            },
            &ctx,
        )
        .await
        .expect_err("B cannot complete A's field");
    tx.rollback().await.unwrap();
    assert_eq!(refused.code(), "SIGNER_FIELD_NOT_OWNED");
    cleanup(&db, &env).await;
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn consent_is_immutable_and_completion_replays_safely() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("replay-{}", Uuid::new_v4());
    let env = envelope(&db, &tag).await;
    let service = signer(&db);
    let ctx = context(&tag);
    let token_a = grant(&service, &db, &env.a, &ctx).await;
    consent(&service, &db, &env.a, &token_a, &ctx).await;
    // A second acceptance with different text returns the existing evidence.
    let other_sha = "d4735e3a265e16eee03f59718b9b5d03019c07d8b6c51f90da3a666eec13ab35";
    let mut tx = db.begin("docsign-proof-consent").await.unwrap();
    service
        .accept_consent_transactional(
            &mut tx,
            &AcceptSignerConsentRequest {
                recipient_id: env.a.clone(),
                access_token: token_a.clone(),
                consent_version: "v2".into(),
                consent_text: "Different words.".into(),
                consent_text_sha256: other_sha.into(),
                ip_address: None,
                user_agent: None,
            },
            &ctx,
        )
        .await
        .expect("second acceptance returns existing evidence");
    tx.commit().await.unwrap();
    let kept: String = sqlx::query_scalar(
        "select consent_text from signature_recipient_consent where recipient_id = $1::uuid",
    )
    .bind(&env.a)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(kept, "I agree.");

    // Complete twice: the second is the same Completed answer, not an error.
    complete_field(&service, &db, &env.a, &token_a, &env.field_a, &ctx).await;
    for _ in 0..2 {
        let action = complete(&service, &db, &env.a, &token_a, &ctx).await;
        assert_eq!(action.state, model::SignerState::Completed);
    }
    cleanup(&db, &env).await;
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn email_dedupe_keeps_one_invitation_per_key() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("dedupe-{}", Uuid::new_v4());
    let service = EmailService::new(EmailDao::new(db.clone()), None, infra());
    let ctx = context(&tag);
    let request = QueueEmailRequest {
        message_kind: EmailMessageKind::SignatureInvitation,
        recipient_email: format!("{tag}@example.test"),
        template_key: "document-sign.invitation".into(),
        template_payload: serde_json::json!({
            "recipientName": "Proof",
            "signingUrl": "https://example.test/sign/x",
        }),
        dedupe_key: format!("signature-invite:{tag}"),
        correlation_id: Some(tag.to_owned()),
        causation_id: None,
    };
    let mut tx = db.begin("docsign-proof-email").await.unwrap();
    let first = service
        .queue_transactional(&mut tx, &request, &ctx)
        .await
        .expect("first queue");
    tx.commit().await.unwrap();
    assert!(!first.existing);
    let mut tx = db.begin("docsign-proof-email").await.unwrap();
    let second = service
        .queue_transactional(&mut tx, &request, &ctx)
        .await
        .expect("replay returns existing");
    tx.commit().await.unwrap();
    assert!(second.existing);
    assert_eq!(first.message_id, second.message_id);
    sweep_tag(&db, &tag).await;
}

async fn document_sign(
    db: &Database,
) -> web::document_sign::DocumentSignService<
    db::DocumentSignDao,
    db::SignatureDao,
    SignerDao,
    EmailDao,
    db::VaultDao,
> {
    use web::document_sign::DocumentSignService;
    use web::security::CasbinAuthorizationPort;
    use web::signature::SignatureService;
    // The finalize path transitions the canonical request under the
    // service-to-service actor, which the Default test port denies and the
    // production Casbin port explicitly admits: use the real port here so
    // the test proves the production rule, not the test double's.
    let signature_infra = ServiceInfrastructure::new(
        Arc::new(
            CasbinAuthorizationPort::new()
                .await
                .expect("casbin port builds"),
        ),
        Arc::new(services::CapturingAuditPort::default()),
        Arc::new(services::CapturingDomainEventPort::default()),
    );
    DocumentSignService::new(
        db::DocumentSignDao::new(db.clone()),
        Arc::new(SignatureService::new_optional(
            db::SignatureDao::new(db.clone()),
            None,
            signature_infra,
        )),
        Arc::new(signer(db)),
        Arc::new(EmailService::new(EmailDao::new(db.clone()), None, infra())),
        Arc::new(web::vault::VaultService::new(
            db::VaultDao::new(db.clone()),
            web::vault::artifact::shared(),
            infra(),
        )),
        infra(),
    )
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn finalize_closes_a_signed_envelope_with_its_audit_trail() {
    use model::SignatureRequestStatus;
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("finalize-{}", Uuid::new_v4());
    let env = envelope(&db, &tag).await;
    let signing = signer(&db);
    let ctx = context(&tag);
    for (recipient, field) in [
        (&env.a, Some(env.field_a.as_str())),
        (&env.b, None),
        (&env.c, None),
    ] {
        let token = grant(&signing, &db, recipient, &ctx).await;
        consent(&signing, &db, recipient, &token, &ctx).await;
        if let Some(field) = field {
            complete_field(&signing, &db, recipient, &token, field, &ctx).await;
        }
        complete(&signing, &db, recipient, &token, &ctx).await;
    }

    let service = document_sign(&db).await;
    // Finalize before Signed refuses: the envelope is still requested.
    let mut tx = db.begin("docsign-proof-finalize").await.unwrap();
    let refused = service
        .finalize_transactional(&mut tx, &env.request_id, &ctx)
        .await
        .expect_err("requested envelopes cannot finalize");
    tx.rollback().await.unwrap();
    assert_eq!(refused.code(), "DOCUMENT_SIGN_NOT_MUTABLE");

    // The envelope's original: a one-page PDF stored the Vault way, so
    // finalize has bytes to seal.
    let original: Vec<u8> = {
        use model::forms_font::encode;
        use web::vault::pdf::{Content, Pdf, Rgb};
        let mut pdf = Pdf::new();
        let font = pdf.font("Helvetica");
        let tree = pdf.reserve();
        let resources = pdf.dictionary(&web::vault::pdf::resources(&[("F1", font)], &[]));
        let mut content = Content::new();
        content.text(
            "F1",
            12.0,
            54.0,
            700.0,
            Rgb::from_bytes(3, 15, 35),
            &encode("Original").unwrap(),
        );
        let page = pdf
            .page(612.0, 792.0, tree, resources, &content.into_bytes())
            .unwrap();
        let info = pdf.info("T", "A", "S", "C", "P", "D:20260101000000");
        pdf.finish(tree, &[page], Some(info)).unwrap()
    };
    let original_id: String = sqlx::query_scalar(
        "insert into media (file_data, filename, mime_type, file_size, media_type) \
         values ($1, 'original.pdf', 'application/pdf', $2, 'document') returning id::text",
    )
    .bind(&original)
    .bind(original.len() as i64)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("update transaction_document set media_id = $2::uuid where title = $1")
        .bind(format!("docsign proof {tag}"))
        .bind(&original_id)
        .execute(db.pool())
        .await
        .unwrap();

    // Drive the canonical machine to Signed, then finalize.
    let signature = web::signature::SignatureService::new_optional(
        db::SignatureDao::new(db.clone()),
        None,
        infra(),
    );
    let mut tx = db.begin("docsign-proof-finalize").await.unwrap();
    for target in [SignatureRequestStatus::Sent, SignatureRequestStatus::Signed] {
        signature
            .transition_transactional(&mut tx, &env.request_id, target, &ctx)
            .await
            .expect("step the canonical machine");
    }
    tx.commit().await.unwrap();
    let mut tx = db.begin("docsign-proof-finalize").await.unwrap();
    let done = service
        .finalize_transactional(&mut tx, &env.request_id, &ctx)
        .await
        .expect("finalize");
    tx.commit().await.unwrap();
    assert!(!done.already_completed);
    let audit_media_id = done.audit_media_id.expect("audit artifact stored");

    // The canonical request is Completed and the audit trail is linked.
    let status: String =
        sqlx::query_scalar("select status from signature_request where id = $1::uuid")
            .bind(&env.request_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(status, "completed");
    let linked: Option<String> = sqlx::query_scalar(
        "select td.signed_audit_media_id::text from transaction_document td \
         join signature_request sr on sr.transaction_document_id = td.id \
         where sr.id = $1::uuid",
    )
    .bind(&env.request_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(linked.as_deref(), Some(audit_media_id.as_str()));
    let mime: String = sqlx::query_scalar("select mime_type from media where id = $1::uuid")
        .bind(&audit_media_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(mime, "application/pdf");
    let sealed_len: i64 = sqlx::query_scalar("select file_size from media where id = $1::uuid")
        .bind(&audit_media_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    let _ = sealed_len;
    let signed_id: Option<String> = sqlx::query_scalar(
        "select td.signed_media_id::text from transaction_document td \
         join signature_request sr on sr.transaction_document_id = td.id \
         where sr.id = $1::uuid",
    )
    .bind(&env.request_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let signed_id = signed_id.expect("sealed PDF linked");
    let signed_magic: Vec<u8> = sqlx::query_scalar(
        "select substring(file_data from 1 for 8) from media where id = $1::uuid",
    )
    .bind(&signed_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(&signed_magic, b"%PDF-1.4");
    let signed_len: i64 = sqlx::query_scalar("select file_size from media where id = $1::uuid")
        .bind(&signed_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert!(
        signed_len > original.len() as i64,
        "the sealed document carries the overlay"
    );
    let magic: Vec<u8> = sqlx::query_scalar(
        "select substring(file_data from 1 for 8) from media where id = $1::uuid",
    )
    .bind(&audit_media_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(&magic, b"%PDF-1.4");

    // Replay answers with the existing artifact instead of storing another.
    let media_before: i64 = sqlx::query_scalar("select count(*) from media where id = $1::uuid")
        .bind(&audit_media_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    let mut tx = db.begin("docsign-proof-finalize").await.unwrap();
    let replay = service
        .finalize_transactional(&mut tx, &env.request_id, &ctx)
        .await
        .expect("replay answers");
    tx.commit().await.unwrap();
    assert!(replay.already_completed);
    assert_eq!(
        replay.audit_media_id.as_deref(),
        Some(audit_media_id.as_str())
    );
    let media_after: i64 = sqlx::query_scalar("select count(*) from media where id = $1::uuid")
        .bind(&audit_media_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(media_before, media_after);
    cleanup(&db, &env).await;
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn sweep_expires_overdue_grants_and_envelopes() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("sweep-{}", Uuid::new_v4());
    let env = envelope(&db, &tag).await;
    let signing = signer(&db);
    let ctx = context(&tag);
    let service = document_sign(&db).await;

    // Grant already lapsed for A, envelope clock in the past. The grant
    // is issued normally then aged back (keeping expires_at > created_at,
    // which the table constrains); only time travel is faked, not the rows.
    let mut tx = db.begin("docsign-proof-sweep").await.unwrap();
    signing
        .issue_access_transactional(&mut tx, &env.a, None, &ctx)
        .await
        .expect("grant");
    tx.commit().await.unwrap();
    sqlx::query(
        "update signature_recipient_access          set created_at = now() - interval '10 days', expires_at = now() - interval '2 days'          where recipient_id = $1::uuid",
    )
    .bind(&env.a)
    .execute(db.pool())
    .await
    .unwrap();
    let past = chrono::Utc::now() - chrono::Duration::days(2);
    sqlx::query(
        "update document_sign_request          set created_at = now() - interval '10 days', expires_at = $2          where signature_request_id = $1::uuid",
    )
    .bind(&env.request_id)
    .bind(past)
    .execute(db.pool())
    .await
    .unwrap();
    // Canonical must be past requested for the envelope to be sweepable.
    let signature = web::signature::SignatureService::new_optional(
        db::SignatureDao::new(db.clone()),
        None,
        infra(),
    );
    let mut tx = db.begin("docsign-proof-sweep").await.unwrap();
    signature
        .transition_transactional(
            &mut tx,
            &env.request_id,
            model::SignatureRequestStatus::Sent,
            &ctx,
        )
        .await
        .expect("sent");
    tx.commit().await.unwrap();

    let mut tx = db.begin("docsign-proof-sweep").await.unwrap();
    let swept = service
        .sweep_due_transactional(&mut tx, &ctx)
        .await
        .expect("sweep");
    tx.commit().await.unwrap();
    assert!(swept.expired_recipients.contains(&env.a));
    assert!(swept.expired_envelopes.contains(&env.request_id));

    let state: String = sqlx::query_scalar(
        "select state from signature_recipient_state where recipient_id = $1::uuid",
    )
    .bind(&env.a)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(state, "expired");
    let status: String =
        sqlx::query_scalar("select status from signature_request where id = $1::uuid")
            .bind(&env.request_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(status, "expired");

    // Second sweep is a no-op: terminal states never reopen.
    let mut tx = db.begin("docsign-proof-sweep").await.unwrap();
    let swept = service
        .sweep_due_transactional(&mut tx, &ctx)
        .await
        .expect("resweep");
    tx.commit().await.unwrap();
    assert!(swept.expired_recipients.is_empty());
    assert!(swept.expired_envelopes.is_empty());
    cleanup(&db, &env).await;
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn import_builds_owned_fields_from_template_anchors() {
    use model::{ImportAnchorFieldsRequest, TemplateAnchor, TemplateAnchorRect};
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("import-{}", Uuid::new_v4());
    let env = envelope(&db, &tag).await;
    // B claims the seller slot; A and C stay slotless.
    sqlx::query(
        "update signature_envelope_recipient          set execution_role = 'seller', execution_slot_id = 's1' where id = $1::uuid",
    )
    .bind(&env.b)
    .execute(db.pool())
    .await
    .unwrap();
    let anchor = |kind: &str| TemplateAnchor {
        role: "seller".into(),
        slot_id: Some("s1".into()),
        kind: kind.into(),
        page_index: 0,
        page_width: 612.0,
        page_height: 792.0,
        rect: TemplateAnchorRect {
            x: 72.0,
            y: 650.0,
            width: 180.0,
            height: 20.0,
        },
    };
    let service = document_sign(&db).await;
    let ctx = context(&tag);
    let mut tx = db.begin("docsign-proof-import").await.unwrap();
    let imported = service
        .import_fields_transactional(
            &mut tx,
            &ImportAnchorFieldsRequest {
                signature_request_id: env.request_id.clone(),
                anchors: vec![anchor("signature"), anchor("date")],
            },
            &ctx,
        )
        .await
        .expect("import");
    tx.commit().await.unwrap();
    assert_eq!(imported.created_field_ids.len(), 2);

    // The imported fields belong to B; the fixture field stays with A.
    let mut owners: Vec<String> = sqlx::query_scalar(
        "select distinct recipient_id::text from signature_field where signature_request_id = $1::uuid",
    )
    .bind(&env.request_id)
    .fetch_all(db.pool())
    .await
    .unwrap();
    owners.sort();
    let mut expected = vec![env.a.clone(), env.b.clone()];
    expected.sort();
    assert_eq!(owners, expected);

    // Re-import upserts by key: no duplicates.
    let mut tx = db.begin("docsign-proof-import").await.unwrap();
    let replay = service
        .import_fields_transactional(
            &mut tx,
            &ImportAnchorFieldsRequest {
                signature_request_id: env.request_id.clone(),
                anchors: vec![anchor("signature")],
            },
            &ctx,
        )
        .await
        .expect("re-import");
    tx.commit().await.unwrap();
    assert_eq!(replay.created_field_ids.len(), 1);
    let count: i64 = sqlx::query_scalar(
        "select count(*) from signature_field where signature_request_id = $1::uuid",
    )
    .bind(&env.request_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    // The fixture field plus the two imports; the replay added nothing.
    assert_eq!(count, 3);

    // The desk path: no anchors supplied, so the template's own blocks
    // are read from the issued document's snapshot.
    sqlx::query(
        "update transaction_document set source_snapshot = $2, issued_checksum_sha256 = 'proof', template_id = 'proof', template_version = 1, issued_version = 1 where title = $1",
    )
    .bind(format!("docsign proof {tag}"))
    .bind(serde_json::json!({
        "signatureAnchors": [{
            "role": "seller", "slotId": "s1", "kind": "initials",
            "pageIndex": 0, "pageWidth": 612.0, "pageHeight": 792.0,
            "rect": { "x": 300.0, "y": 650.0, "width": 80.0, "height": 20.0 },
        }],
    }))
    .execute(db.pool())
    .await
    .unwrap();
    let service = document_sign(&db).await;
    let mut tx = db.begin("docsign-proof-import").await.unwrap();
    let from_template = service
        .import_fields_transactional(
            &mut tx,
            &model::ImportAnchorFieldsRequest {
                signature_request_id: env.request_id.clone(),
                anchors: vec![],
            },
            &ctx,
        )
        .await
        .expect("template import");
    tx.commit().await.unwrap();
    assert_eq!(from_template.created_field_ids.len(), 1);
    let owner: String =
        sqlx::query_scalar("select recipient_id::text from signature_field where id = $1::uuid")
            .bind(&from_template.created_field_ids[0])
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(owner, env.b);

    // An anchor no recipient claims fails loud instead of half-mapping.
    let mut tx = db.begin("docsign-proof-import").await.unwrap();
    let refused = service
        .import_fields_transactional(
            &mut tx,
            &ImportAnchorFieldsRequest {
                signature_request_id: env.request_id.clone(),
                anchors: vec![TemplateAnchor {
                    role: "ghost".into(),
                    ..anchor("signature")
                }],
            },
            &ctx,
        )
        .await
        .expect_err("unclaimed anchor refuses");
    tx.rollback().await.unwrap();
    assert_eq!(refused.code(), "DOCUMENT_SIGN_FIELD_INVALID");
    cleanup(&db, &env).await;
}

// ── THE HTTP EDGE ────────────────────────────────────────────────────────────────────────────────────────────────
//
// Everything above drives the services directly. A signer is not a service: they hold an emailed link and a browser.
// This drives the SAME production composition root the server runs (`web::api::build_router`, over DEV, without the
// TCP listener) through the six public `/v1/signer/*` routes, with a real access token minted by the same codec the
// router builds from the environment — so what is proven is the path a person's click takes: token → session → open →
// consent → field → complete, the durable command dispatcher in between, and the refusals at each door.

const HTTP_KEY: &str = "docsign-http-proof-internal-key";
const CONSENT_SHA: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

async fn post(
    router: &axum::Router,
    path: &str,
    body: serde_json::Value,
) -> (u16, serde_json::Value) {
    use test_harness::http::{call, TestRequest};
    let response = call(router, TestRequest::post(path).json(&body)).await;
    let status = response.status().as_u16();
    let json = serde_json::from_str(&response.text()).unwrap_or(serde_json::Value::Null);
    (status, json)
}

/// A command answered over the edge is HTTP 200 with the verdict INSIDE the envelope (`value.outcome`): the durable
/// command runtime's receipt, replayable by command id. `success` is the only good answer.
fn outcome(body: &serde_json::Value) -> String {
    body["value"]["outcome"]
        .as_str()
        .unwrap_or("(no outcome)")
        .to_string()
}

async fn recipient_state(db: &Database, recipient: &str) -> (String, bool) {
    sqlx::query_as::<_, (String, bool)>(
        "select state, completed_at is not null from signature_recipient_state where recipient_id = $1::uuid",
    )
    .bind(recipient)
    .fetch_one(db.pool())
    .await
    .expect("the recipient has a state row")
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn signers_complete_an_envelope_over_the_public_http_edge() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("http-{}", Uuid::new_v4());
    let env = envelope(&db, &tag).await;
    let ctx = context(&tag);

    // The production composition root over DEV, and an issuer built with the router's own codec (env-derived secret).
    let infrastructure = web::service_bootstrap::production_service_infrastructure(&db)
        .await
        .expect("the production service infrastructure composes");
    let router = web::api::build_router(
        db.clone(),
        infrastructure,
        web::api::ApiConfig {
            internal_api_key: Arc::from(HTTP_KEY),
        },
    );
    let codec = SignerAccessTokenCodec::from_env()
        .expect("DOCSIGN_ACCESS_SECRET, AUTH_SECRET or CULEBRA_INTERNAL_API_KEY must be set (as the router needs)");
    let issuing = SignerService::new(SignerDao::new(db.clone()), codec, infra());

    // ── The doors refuse what they must ──────────────────────────────────────────────────────────────────────────
    let (status, body) = post(&router, "/v1/signer/session", serde_json::json!({})).await;
    assert_eq!(status, 401, "no link, no session: {body}");
    assert_eq!(body["ok"], false);
    let (status, _) = post(
        &router,
        "/v1/signer/session",
        serde_json::json!({ "accessToken": "not-a-signing-link" }),
    )
    .await;
    assert_eq!(status, 401, "a forged link is refused");

    // ── A and B (step 1, together) ───────────────────────────────────────────────────────────────────────────────
    let token_a = grant(&issuing, &db, &env.a, &ctx).await;
    let token_b = grant(&issuing, &db, &env.b, &ctx).await;

    let (status, body) = post(
        &router,
        "/v1/signer/session",
        serde_json::json!({ "accessToken": token_a }),
    )
    .await;
    assert_eq!(status, 200, "a valid link opens a session: {body}");
    assert_eq!(body["ok"], true);
    assert_eq!(
        body["value"]["recipient"]["id"], env.a,
        "the session names the recipient the TOKEN belongs to"
    );

    // Completing before consent is refused by the command's own verdict, and changes nothing.
    let (status, body) = post(
        &router,
        "/v1/signer/complete",
        serde_json::json!({ "accessToken": token_b, "recipientId": env.b }),
    )
    .await;
    assert!(
        status >= 400 || outcome(&body) != "success",
        "complete before consent must be refused, got {status}: {body}"
    );
    assert_ne!(
        recipient_state(&db, &env.b).await.0,
        "completed",
        "the refusal left B unfinished"
    );

    // A forged recipientId cannot ride a valid token onto someone else's lane: B's token naming A is refused.
    let (status, body) = post(
        &router,
        "/v1/signer/consent",
        serde_json::json!({
            "accessToken": token_b, "recipientId": env.a, "consentVersion": "v1",
            "consentText": "I agree.", "consentTextSha256": CONSENT_SHA,
        }),
    )
    .await;
    assert!(
        status >= 400 || outcome(&body) != "success",
        "a token cannot act for another recipient, got {status}: {body}"
    );

    for (recipient, token, field) in [
        (&env.a, &token_a, Some(env.field_a.as_str())),
        (&env.b, &token_b, None),
    ] {
        let (status, body) = post(
            &router,
            "/v1/signer/open",
            serde_json::json!({ "accessToken": token, "recipientId": recipient }),
        )
        .await;
        assert!(
            status == 200 && outcome(&body) == "success",
            "open: {status} {body}"
        );
        let (status, body) = post(
            &router,
            "/v1/signer/consent",
            serde_json::json!({
                "accessToken": token, "recipientId": recipient, "consentVersion": "v1",
                "consentText": "I agree.", "consentTextSha256": CONSENT_SHA,
            }),
        )
        .await;
        assert!(
            status == 200 && outcome(&body) == "success",
            "consent: {status} {body}"
        );
        if let Some(field) = field {
            let (status, body) = post(
                &router,
                "/v1/signer/field",
                serde_json::json!({
                    "accessToken": token, "recipientId": recipient, "fieldId": field,
                    "value": { "signature": "proof-strokes" },
                }),
            )
            .await;
            assert!(
                status == 200 && outcome(&body) == "success",
                "field: {status} {body}"
            );
        }
        let (status, body) = post(
            &router,
            "/v1/signer/complete",
            serde_json::json!({ "accessToken": token, "recipientId": recipient }),
        )
        .await;
        assert!(
            status == 200 && outcome(&body) == "success",
            "complete: {status} {body}"
        );
    }
    for recipient in [&env.a, &env.b] {
        let (state, has_completed_at) = recipient_state(&db, recipient).await;
        assert_eq!(
            state, "completed",
            "step 1 signers are completed in the database"
        );
        assert!(has_completed_at, "and carry a completion time");
    }

    // ── C (step 2) acts only once step 1 is done — and now it is ────────────────────────────────────────────────
    let token_c = grant(&issuing, &db, &env.c, &ctx).await;
    for (path, body) in [
        (
            "/v1/signer/open",
            serde_json::json!({ "accessToken": token_c, "recipientId": env.c }),
        ),
        (
            "/v1/signer/consent",
            serde_json::json!({
                "accessToken": token_c, "recipientId": env.c, "consentVersion": "v1",
                "consentText": "I agree.", "consentTextSha256": CONSENT_SHA,
            }),
        ),
        (
            "/v1/signer/complete",
            serde_json::json!({ "accessToken": token_c, "recipientId": env.c }),
        ),
    ] {
        let (status, response) = post(&router, path, body).await;
        assert!(
            status == 200 && outcome(&response) == "success",
            "{path}: {status} {response}"
        );
    }
    assert_eq!(recipient_state(&db, &env.c).await.0, "completed");

    // ── What the database holds is the audit trail ──────────────────────────────────────────────────────────────
    let responses: i64 = sqlx::query_scalar(
        "select count(*) from signature_field_response where field_id = $1::uuid",
    )
    .bind(&env.field_a)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(responses, 1, "A's field response was recorded once");
    let events: Vec<String> = sqlx::query_scalar(
        "select event_type from signature_evidence_event where signature_request_id = $1::uuid order by id",
    )
    .bind(&env.request_id)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert!(
        events.len() >= 9,
        "three signers each leave open, consent and completion evidence: {events:?}"
    );
    cleanup(&db, &env).await;
}

// ── THE SIGNER IS SHOWN THE DOCUMENT ─────────────────────────────────────────────────────────────────────────────
//
// A person who cannot read a document cannot meaningfully consent to sign it, and until the signer's Vault door existed
// the public page showed only a list of fields. This proves the door end to end: the envelope's own PDF, byte for byte,
// to the signer holding its link — and nothing to a draft, a voided envelope, or no link at all.

/// A one-page PDF built the Vault's own way, with a recognisable line of text.
fn proof_pdf(text: &str) -> Vec<u8> {
    use model::forms_font::encode;
    use web::vault::pdf::{Content, Pdf, Rgb};
    let mut pdf = Pdf::new();
    let font = pdf.font("Helvetica");
    let tree = pdf.reserve();
    let resources = pdf.dictionary(&web::vault::pdf::resources(&[("F1", font)], &[]));
    let mut content = Content::new();
    content.text(
        "F1",
        12.0,
        54.0,
        700.0,
        Rgb::from_bytes(3, 15, 35),
        &encode(text).unwrap(),
    );
    let page = pdf
        .page(612.0, 792.0, tree, resources, &content.into_bytes())
        .unwrap();
    let info = pdf.info("T", "A", "S", "C", "P", "D:20260101000000");
    pdf.finish(tree, &[page], Some(info)).unwrap()
}

/// Store `original` as the envelope's document the way the Vault does, and link it to the transaction document.
async fn attach_original(db: &Database, tag: &str, original: &[u8]) {
    let media_id: String = sqlx::query_scalar(
        "insert into media (file_data, filename, mime_type, file_size, media_type) \
         values ($1, 'agreement.pdf', 'application/pdf', $2, 'document') returning id::text",
    )
    .bind(original)
    .bind(original.len() as i64)
    .fetch_one(db.pool())
    .await
    .expect("original media");
    sqlx::query("update transaction_document set media_id = $2::uuid where title = $1")
        .bind(format!("docsign proof {tag}"))
        .bind(&media_id)
        .execute(db.pool())
        .await
        .expect("link the original");
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn a_signer_is_shown_the_document_they_are_asked_to_sign() {
    use test_harness::http::{call, TestRequest};
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("document-{}", Uuid::new_v4());
    let env = envelope(&db, &tag).await;
    let ctx = context(&tag);
    let original = proof_pdf("This agreement is the document the signer must be able to read.");
    attach_original(&db, &tag, &original).await;

    let infrastructure = web::service_bootstrap::production_service_infrastructure(&db)
        .await
        .expect("the production service infrastructure composes");
    let router = web::api::build_router(
        db.clone(),
        infrastructure,
        web::api::ApiConfig {
            internal_api_key: Arc::from(HTTP_KEY),
        },
    );
    let codec = SignerAccessTokenCodec::from_env().expect("a signer access secret is configured");
    let issuing = SignerService::new(SignerDao::new(db.clone()), codec, infra());
    let token_a = grant(&issuing, &db, &env.a, &ctx).await;
    let token_b = grant(&issuing, &db, &env.b, &ctx).await;
    let get = |token: String| {
        let router = router.clone();
        async move {
            call(
                &router,
                TestRequest::get(format!("/v1/signer/document/{token}")),
            )
            .await
        }
    };

    // A forged link is refused (a link is the only credential this edge has).
    let response = get("not-a-signing-link".into()).await;
    assert_eq!(response.status().as_u16(), 401, "a forged link is refused");

    // A DRAFT is the operator's: it has links (the fixture minted them) but was never issued, so it shows nothing.
    let response = get(token_a.clone()).await;
    assert_eq!(
        response.status().as_u16(),
        404,
        "an envelope that was never issued shows no document: {}",
        response.text()
    );

    // ISSUED: both recipients of the envelope see the envelope's own PDF, byte for byte, as a PDF the browser can open.
    sqlx::query(
        "update document_sign_request set issued_at = now() where signature_request_id = $1::uuid",
    )
    .bind(&env.request_id)
    .execute(db.pool())
    .await
    .unwrap();
    for token in [&token_a, &token_b] {
        let response = get(token.clone()).await;
        assert_eq!(
            response.status().as_u16(),
            200,
            "an issued envelope's document is shown: {}",
            response.text()
        );
        assert_eq!(response.header("content-type"), Some("application/pdf"));
        assert!(
            response
                .header("content-disposition")
                .unwrap_or("")
                .starts_with("inline"),
            "the document opens in the browser's own viewer"
        );
        assert_eq!(
            response.header("cache-control"),
            Some("private, no-store"),
            "a signed-for document is never cached"
        );
        assert_eq!(response.header("x-content-type-options"), Some("nosniff"));
        assert!(
            response.bytes().starts_with(b"%PDF-"),
            "what arrives is a PDF"
        );
        assert_eq!(
            response.bytes(),
            original.as_slice(),
            "the signer is shown exactly the stored document"
        );
    }

    // A download is an attachment, same bytes.
    let response = call(
        &router,
        TestRequest::get(format!("/v1/signer/document/{token_a}?download=1")),
    )
    .await;
    assert!(response
        .header("content-disposition")
        .unwrap_or("")
        .starts_with("attachment"));

    // A VOIDED envelope shows nothing, even to a recipient whose link has not expired.
    sqlx::query("update signature_request set status = 'voided' where id = $1::uuid")
        .bind(&env.request_id)
        .execute(db.pool())
        .await
        .unwrap();
    let response = get(token_a.clone()).await;
    assert!(
        matches!(response.status().as_u16(), 400 | 401 | 404),
        "a voided envelope must not hand out its document, got {}",
        response.status()
    );
    cleanup(&db, &env).await;
}

/// The service as production composes it: the real policy, so the system-actor steps inside "send" are decided by
/// the same rules that decide them in production.
async fn document_sign_production(
    db: &Database,
) -> std::sync::Arc<web::document_sign::ProductionDocumentSignService> {
    let infrastructure = web::service_bootstrap::production_service_infrastructure(db)
        .await
        .expect("the production service infrastructure composes");
    web::composition::ServiceCatalog::new(db.clone(), infrastructure).document_sign()
}

/// A PDF with `pages` pages, each carrying its number.
fn multi_page_pdf(pages: usize) -> Vec<u8> {
    use model::forms_font::encode;
    use web::vault::pdf::{Content, Pdf, Rgb};
    let mut pdf = Pdf::new();
    let font = pdf.font("Helvetica");
    let tree = pdf.reserve();
    let resources = pdf.dictionary(&web::vault::pdf::resources(&[("F1", font)], &[]));
    let mut ids = Vec::new();
    for number in 1..=pages {
        let mut content = Content::new();
        content.text(
            "F1",
            12.0,
            54.0,
            700.0,
            Rgb::from_bytes(3, 15, 35),
            &encode(&format!("Page {number}")).unwrap(),
        );
        ids.push(
            pdf.page(612.0, 792.0, tree, resources, &content.into_bytes())
                .unwrap(),
        );
    }
    let info = pdf.info("T", "A", "S", "C", "P", "D:20260101000000");
    pdf.finish(tree, &ids, Some(info)).unwrap()
}

/// A transaction document with a stored PDF and nothing else: the starting point of "send".
async fn bare_document(db: &Database, tag: &str, pages: usize) -> String {
    let deal: String = sqlx::query_scalar("select id::text from deal limit 1")
        .fetch_one(db.pool())
        .await
        .expect("DEV must hold at least one deal");
    let id: String = sqlx::query_scalar(
        "insert into transaction_document (deal_id, document_type, title, state, source) \
         values ($1::uuid, 'agreement', $2, 'draft', 'generated') returning id::text",
    )
    .bind(&deal)
    .bind(format!("docsign proof {tag}"))
    .fetch_one(db.pool())
    .await
    .unwrap();
    attach_original(db, tag, &multi_page_pdf(pages)).await;
    id
}

fn person(name: &str, email: &str, order: i32) -> model::DocumentSignRecipientInput {
    model::DocumentSignRecipientInput {
        role: model::SignatureRecipientRole::Signer,
        name: name.into(),
        email: email.into(),
        signer_order: order,
        signing_step: 1,
        execution_role: None,
        execution_slot_id: None,
    }
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn send_prepares_places_and_issues_in_one_step_and_knows_the_page_count() {
    use model::{DocumentSigningMode, SendDocumentSignRequest, SendFieldPlacement};
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("send-{}", Uuid::new_v4());
    sweep_tag(&db, &tag).await;
    let document = bare_document(&db, &tag, 3).await;
    let service = document_sign_production(&db).await;
    // The author is a real app user: the draft records who prepared it.
    let author: String = sqlx::query_scalar("select id::text from app_user limit 1")
        .fetch_one(db.pool())
        .await
        .expect("DEV must hold an app user");
    let mut ctx = context(&tag);
    {
        // The production policy decides here, as it does for the desk operator.
        let principal = ctx.principal.as_mut().unwrap();
        principal.app_user_id = author;
        principal.level = "BUSINESS_POWER_USER".into();
        principal.role_codes = vec!["owner".into()];
    }
    let request = |placement| SendDocumentSignRequest {
        transaction_document_id: document.clone(),
        recipients: vec![
            person("Ada", &format!("{tag}-a@example.test"), 1),
            person("Bo", &format!("{tag}-b@example.test"), 2),
        ],
        subject: Some("Purchase agreement".into()),
        message: None,
        signing_mode: DocumentSigningMode::Parallel,
        expires_at: None,
        placement,
        copy_to: vec![],
        reminder_every_days: None,
    };

    // A page the document does not have is refused BEFORE anything is written.
    let mut tx = db.begin("docsign-proof-send").await.unwrap();
    let refused = service
        .send_transactional(
            &mut tx,
            &request(SendFieldPlacement::Page { page_number: 5 }),
            &ctx,
        )
        .await
        .expect_err("page 5 of 3");
    let _ = tx.rollback().await;
    assert!(
        format!("{refused:?}").contains("3 page"),
        "the refusal names the real page count: {refused:?}"
    );
    let drafts: i64 = sqlx::query_scalar(
        "select count(*) from signature_request where transaction_document_id = $1::uuid",
    )
    .bind(&document)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(drafts, 0, "a refused send leaves no draft behind");

    // The last page: one signature per signer, issued, one invitation each.
    let mut tx = db.begin("docsign-proof-send").await.unwrap();
    let sent = service
        .send_transactional(&mut tx, &request(SendFieldPlacement::LastPage), &ctx)
        .await
        .expect("send");
    tx.commit().await.unwrap();
    assert_eq!(sent.issued.invitation_message_ids.len(), 2);
    assert_eq!(sent.snapshot.fields.len(), 2);
    assert!(
        sent.snapshot
            .fields
            .iter()
            .all(|field| field.page_number == 3),
        "the signature goes on the last page"
    );
    let owners: std::collections::BTreeSet<_> = sent
        .snapshot
        .fields
        .iter()
        .map(|field| field.recipient_id.clone())
        .collect();
    assert_eq!(owners.len(), 2, "each person has their own box");
    let issued: bool = sqlx::query_scalar(
        "select issued_at is not null from document_sign_request where signature_request_id = $1::uuid",
    )
    .bind(&sent.snapshot.signature_request.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(issued);

    sweep_tag(&db, &tag).await;
}

// ── Sealing, outcome emails, and the signer's own copy ───────────────────────────────────────────────────────────

/// An email transport that keeps what it is asked to send. Nothing leaves the test.
struct CapturingTransport(std::sync::Mutex<Vec<apis::mail::OutgoingMail>>);

#[async_trait::async_trait]
impl web::email::EmailTransport for CapturingTransport {
    async fn send(&self, mail: apis::mail::OutgoingMail) -> Result<(), apis::mail::MailError> {
        self.0.lock().unwrap().push(mail);
        Ok(())
    }
}

/// The token in a signing link (`<site>/sign/<token>`) that was queued for `email`.
async fn token_for(db: &Database, email: &str) -> String {
    let url: String = sqlx::query_scalar(
        "select template_payload->>'signingUrl' from email_message \
          where recipient_email = $1 and message_kind = 'signature_invitation' order by queued_at desc limit 1",
    )
    .bind(email)
    .fetch_one(db.pool())
    .await
    .expect("an invitation was queued with a signing link");
    url.rsplit_once("/sign/")
        .map(|(_, token)| token.to_owned())
        .expect("the link has the /sign/ shape")
}

async fn http(
    router: &axum::Router,
    path: &str,
    body: serde_json::Value,
) -> (u16, serde_json::Value) {
    use test_harness::http::{call, TestRequest};
    let response = call(
        router,
        TestRequest::post(path)
            .header("x-forwarded-for", "203.0.113.7, 10.0.0.1")
            .header("user-agent", "DocsignProof/1.0")
            .json(&body),
    )
    .await;
    let status = response.status().as_u16();
    (
        status,
        serde_json::from_str(&response.text()).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn a_signed_envelope_is_sealed_everyone_is_told_and_a_decline_says_who_and_why() {
    use model::{DocumentSigningMode, SendDocumentSignRequest, SendFieldPlacement};
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("seal-{}", Uuid::new_v4());
    sweep_tag(&db, &tag).await;

    // The sender: a real app user (the draft records who prepared it), with an address that is the test's own.
    let operator_email = format!("{tag}-operator@example.test");
    let operator: String = sqlx::query_scalar(
        "insert into app_user (display_name, email) values ($1, $2) returning id::text",
    )
    .bind("Docsign Proof Operator")
    .bind(&operator_email)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let mut ctx = context(&tag);
    {
        let principal = ctx.principal.as_mut().unwrap();
        principal.app_user_id = operator.clone();
        principal.level = "BUSINESS_POWER_USER".into();
        principal.role_codes = vec!["owner".into()];
    }

    let infrastructure = web::service_bootstrap::production_service_infrastructure(&db)
        .await
        .expect("production infrastructure");
    let catalog = web::composition::ServiceCatalog::new(db.clone(), infrastructure.clone());
    let service = catalog.document_sign();
    let router = web::api::build_router(
        db.clone(),
        infrastructure.clone(),
        web::api::ApiConfig {
            internal_api_key: Arc::from(HTTP_KEY),
        },
    );

    // ── Send: two signers, the broker copied, signatures on the last page ────────────────────────────────────────
    let document = bare_document(&db, &tag, 2).await;
    let ada = format!("{tag}-ada@example.test");
    let bo = format!("{tag}-bo@example.test");
    let broker = format!("{tag}-broker@example.test");
    let mut tx = db.begin("docsign-proof-seal").await.unwrap();
    let sent = service
        .send_transactional(
            &mut tx,
            &SendDocumentSignRequest {
                transaction_document_id: document.clone(),
                recipients: vec![person("Ada Alvarez", &ada, 1), person("Bo Díaz", &bo, 2)],
                subject: Some("Listing Agreement".into()),
                message: Some("Please sign today.".into()),
                signing_mode: DocumentSigningMode::Parallel,
                expires_at: None,
                placement: SendFieldPlacement::LastPage,
                copy_to: vec![broker.clone(), " ".into(), broker.to_uppercase()],
                reminder_every_days: Some(3),
            },
            &ctx,
        )
        .await
        .expect("send");
    tx.commit().await.unwrap();
    let request_id = sent.snapshot.signature_request.id.clone();

    // ── Both sign, over the public edge, as people do ────────────────────────────────────────────────────────────
    for (name, email) in [("Ada Alvarez", &ada), ("Bo Díaz", &bo)] {
        let token = token_for(&db, email).await;
        let (_, session) = http(
            &router,
            "/v1/signer/session",
            serde_json::json!({ "accessToken": token }),
        )
        .await;
        let recipient = session["value"]["recipient"]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(
            session["value"]["parties"].as_array().unwrap().len(),
            2,
            "a signer sees who else is on it"
        );
        assert_eq!(
            session["value"]["documentTitle"],
            format!("docsign proof {tag}")
        );
        assert_eq!(session["value"]["message"], "Please sign today.");
        assert_eq!(
            session["value"]["envelopeStatus"].as_str().unwrap_or(""),
            "sent"
        );
        let field = session["value"]["fields"][0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        for (path, body) in [
            (
                "/v1/signer/open",
                serde_json::json!({ "accessToken": token, "recipientId": recipient }),
            ),
            (
                "/v1/signer/consent",
                serde_json::json!({
                    "accessToken": token, "recipientId": recipient, "consentVersion": "v1",
                    "consentText": "I agree.", "consentTextSha256": CONSENT_SHA,
                }),
            ),
            (
                "/v1/signer/field",
                serde_json::json!({
                    "accessToken": token, "recipientId": recipient, "fieldId": field,
                    "value": { "style": 1, "name": name },
                }),
            ),
            (
                "/v1/signer/complete",
                serde_json::json!({ "accessToken": token, "recipientId": recipient }),
            ),
        ] {
            let (status, body) = http(&router, path, body).await;
            assert!(
                status == 200 && outcome(&body) == "success",
                "{path}: {status} {body}"
            );
        }
    }
    let seen: i64 = sqlx::query_scalar(
        "select count(*) from signature_evidence_event where signature_request_id = $1::uuid \
            and evidence->>'ipAddress' = '203.0.113.7' and evidence->>'userAgent' = 'DocsignProof/1.0'",
    )
    .bind(&request_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(
        seen >= 8,
        "every signer action records where it came from (the proxy's client address): {seen}"
    );

    // ── Seal ─────────────────────────────────────────────────────────────────────────────────────────────────────
    let mut tx = db.begin("docsign-proof-seal").await.unwrap();
    let done = service
        .finalize_transactional(&mut tx, &request_id, &ctx)
        .await
        .expect("finalize");
    tx.commit().await.unwrap();
    assert!(done.signed_media_id.is_some() && done.audit_media_id.is_some());
    assert_eq!(
        done.notification_message_ids.len(),
        4,
        "Ada, Bo, the broker (once, however it was typed) and the sender are each told once"
    );
    let queued: Vec<(String, bool)> = sqlx::query_as(
        "select recipient_email, (template_payload->>'attachCompletion')::bool from email_message \
          where message_kind = 'signature_completed' and recipient_email like $1 order by recipient_email",
    )
    .bind(format!("%{tag}%"))
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(queued.len(), 4);
    assert!(queued.iter().all(|(_, attach)| *attach));
    assert!(
        queued.iter().any(|(email, _)| *email == operator_email),
        "the sender is copied"
    );

    // The delivery worker attaches the sealed document and the certificate, read through the Vault's own door.
    let transport = Arc::new(CapturingTransport(std::sync::Mutex::new(Vec::new())));
    let mailer = web::email::EmailService::new(
        EmailDao::new(db.clone()),
        Some(transport.clone()),
        infrastructure.clone(),
    )
    .with_attachment_source(Arc::new(web::email::VaultAttachmentSource::new(
        catalog.vault(),
    )));
    let worker = ServiceContext {
        actor: ServiceActor {
            id: Some(web::email::EMAIL_DELIVERY_ACTOR.into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: tag.clone(),
        causation_id: None,
        principal: None,
    };
    let completion_id: String = sqlx::query_scalar(
        "select id::text from email_message where message_kind = 'signature_completed' and recipient_email = $1",
    )
    .bind(&ada)
    .fetch_one(db.pool())
    .await
    .unwrap();
    mailer
        .deliver(&completion_id, &worker)
        .await
        .expect("the completion email is delivered");
    let sent_mail = transport.0.lock().unwrap().clone();
    assert_eq!(sent_mail.len(), 1);
    let mail = &sent_mail[0];
    assert_eq!(
        mail.subject,
        format!("Signed: docsign proof {tag} — CulebraLuxe")
    );
    assert!(mail
        .html
        .as_deref()
        .unwrap_or("")
        .contains("signed by everyone"));
    assert_eq!(
        mail.attachments.len(),
        2,
        "the signed document and its certificate"
    );
    assert!(mail.attachments[0]
        .filename
        .starts_with("signature-signed-"));
    assert!(mail.attachments[1]
        .filename
        .starts_with("signature-completion-"));
    assert!(mail
        .attachments
        .iter()
        .all(|file| file.bytes.starts_with(b"%PDF-")));

    // …and only that worker may use the door: a signer's own identity is refused the completion artifacts.
    let intruder = ServiceContext {
        actor: ServiceActor {
            id: Some("signature-recipient:someone".into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: tag.clone(),
        causation_id: None,
        principal: None,
    };
    assert!(catalog
        .vault()
        .completion_artifacts(&request_id, &intruder)
        .await
        .is_err());

    // The signer's own copy: the same link, now that the envelope is complete.
    let token = token_for(&db, &ada).await;
    {
        use test_harness::http::{call, TestRequest};
        let response = call(
            &router,
            TestRequest::get(format!("/v1/signer/signed/{token}")),
        )
        .await;
        assert_eq!(response.status().as_u16(), 200, "{}", response.text());
        assert_eq!(response.header("content-type"), Some("application/pdf"));
        assert!(response
            .header("content-disposition")
            .unwrap_or("")
            .starts_with("attachment"));
        assert!(response.bytes().starts_with(b"%PDF-"));
    }

    sweep_tag(&db, &tag).await;

    // ── A decline: everyone but the decliner is told who, and why ────────────────────────────────────────────────
    let tag = format!("decl-{}", Uuid::new_v4());
    let document = bare_document(&db, &tag, 1).await;
    let (cat, dan) = (
        format!("{tag}-cat@example.test"),
        format!("{tag}-dan@example.test"),
    );
    let mut ctx = context(&tag);
    ctx.principal = Some(ServicePrincipal {
        app_user_id: operator.clone(),
        level: "BUSINESS_POWER_USER".into(),
        role_codes: vec!["owner".into()],
        account_type: "internal".into(),
        entitlement_codes: vec![],
    });
    let mut tx = db.begin("docsign-proof-decline").await.unwrap();
    let sent = service
        .send_transactional(
            &mut tx,
            &SendDocumentSignRequest {
                transaction_document_id: document,
                recipients: vec![person("Cat", &cat, 1), person("Dan", &dan, 2)],
                subject: None,
                message: None,
                signing_mode: DocumentSigningMode::Parallel,
                expires_at: None,
                placement: SendFieldPlacement::LastPage,
                copy_to: vec![],
                reminder_every_days: None,
            },
            &ctx,
        )
        .await
        .expect("send");
    tx.commit().await.unwrap();
    let token = token_for(&db, &cat).await;
    let (_, session) = http(
        &router,
        "/v1/signer/session",
        serde_json::json!({ "accessToken": token }),
    )
    .await;
    let recipient = session["value"]["recipient"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, body) = http(
        &router,
        "/v1/signer/decline",
        serde_json::json!({ "accessToken": token, "recipientId": recipient, "reason": "The price is wrong." }),
    )
    .await;
    assert!(
        status == 200 && outcome(&body) == "success",
        "decline: {status} {body}"
    );
    let told: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "select recipient_email, template_payload->>'declinerName', template_payload->>'reason' from email_message \
          where message_kind = 'signature_declined' and template_payload->>'signatureRequestId' = $1 order by recipient_email",
    )
    .bind(&sent.snapshot.signature_request.id)
    .fetch_all(db.pool())
    .await
    .unwrap();
    let who: std::collections::BTreeSet<&str> =
        told.iter().map(|(email, _, _)| email.as_str()).collect();
    assert_eq!(
        who,
        [dan.as_str(), operator_email.as_str()]
            .into_iter()
            .collect(),
        "Dan and the sender hear of it; Cat, who declined, does not"
    );
    assert!(told
        .iter()
        .all(|(_, decliner, reason)| decliner.as_deref() == Some("Cat")
            && reason.as_deref() == Some("The price is wrong.")));
    sweep_tag(&db, &tag).await;
    sqlx::query("delete from app_user where id = $1::uuid")
        .bind(&operator)
        .execute(db.pool())
        .await
        .unwrap();
}

/// The broker's standing pre-signature, in the shape issuance resolves it. The picture is a stand-in (the brand mark).
fn lisa_presignature() -> model::forms_applied_signature::FormAppliedSignature {
    use model::forms_applied_signature::{
        AppliedSignatureImageMimeType, FormAppliedSignature, BROKER_SIGNATURE_CONSENT_BASIS,
        BROKER_SIGNATURE_DATE_SEMANTIC,
    };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../public/brand/CLLOGO.png");
    FormAppliedSignature {
        role: "SELLER_BROKER".into(),
        slot_id: None,
        signer_name: "Lisa Penfield".into(),
        credential_line: "Real Estate Broker License #: C-9931".into(),
        signer_app_user_id: "user-1".into(),
        image_bytes: std::fs::read(path).expect("the brand mark is in the repository"),
        image_mime_type: AppliedSignatureImageMimeType::Png,
        asset_media_id: "media-1".into(),
        asset_checksum_sha256: "a".repeat(64),
        applied_at: "2026-10-07T13:30:00Z".into(),
        consent_basis: BROKER_SIGNATURE_CONSENT_BASIS.into(),
        date_semantic: BROKER_SIGNATURE_DATE_SEMANTIC.into(),
    }
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn a_presigned_listing_agreement_is_sent_by_its_own_anchors_signed_and_sealed_with_pictures()
{
    use base64::Engine as _;
    use model::{
        forms::FormSignerPerson, DocumentSigningMode, SendDocumentSignRequest, SendFieldPlacement,
        VaultRenderRequest,
    };
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("anchors-{}", Uuid::new_v4());
    sweep_tag(&db, &tag).await;

    // A real Listing Agreement, rendered by the Forms pipeline with Lisa's pre-signature drawn in.
    let participants = vec![
        FormSignerPerson {
            person_id: None,
            name: "Ada Alvarez".into(),
            email: None,
            role: "SELLER".into(),
            slot_id: None,
        },
        FormSignerPerson {
            person_id: None,
            name: "Lisa Penfield".into(),
            email: None,
            role: "SELLER_BROKER".into(),
            slot_id: None,
        },
    ];
    let artifact = web::vault::artifact::shared()
        .render_issued_document(VaultRenderRequest {
            form_instance_id: "form-1".into(),
            contract_id: None,
            template_id: "LISTING-01".into(),
            template_version: 4,
            field_values: [
                ("sellerName", "Ada Alvarez"),
                ("brokerName", "Lisa Penfield"),
                ("property", "Casa Luar"),
                ("propertyLocation", "Culebra, Puerto Rico"),
                ("listPrice", "1250000"),
                ("startDate", "2026-10-15"),
                ("endDate", "2027-10-15"),
                ("listingType", "Exclusive Right to Sell"),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
            sections: Default::default(),
            issued_version: 1,
            participants,
            actor_app_user_id: None,
            issued_at: Some("2026-10-07T13:30:00Z".into()),
            applied_signatures: vec![lisa_presignature()],
        })
        .await
        .expect("the agreement renders");
    if let Ok(dir) = std::env::var("DUMP_DIR") {
        std::fs::write(format!("{dir}/anchors_original.pdf"), &artifact.bytes).unwrap();
    }
    let metadata = artifact.render_metadata.clone();
    assert_eq!(
        metadata["appliedSignatures"].as_array().unwrap().len(),
        1,
        "Lisa's signature is recorded as applied"
    );

    let deal: String = sqlx::query_scalar("select id::text from deal limit 1")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let document: String = sqlx::query_scalar(
        "insert into transaction_document (deal_id, document_type, title, state, source, source_snapshot, \
             issued_checksum_sha256, template_id, template_version, issued_version, form_instance_id) \
         values ($1::uuid, 'agreement', $2, 'draft', 'generated', $3::jsonb, repeat('a', 64), 'LISTING-01', 4, \
                 (floor(random() * 1000000) + 1000)::int, (select id from document_form_instance limit 1)) \
         returning id::text",
    )
    .bind(&deal)
    .bind(format!("docsign proof {tag}"))
    .bind(serde_json::json!({ "render": metadata, "templateId": "LISTING-01" }))
    .fetch_one(db.pool())
    .await
    .unwrap();
    attach_original(&db, &tag, &artifact.bytes).await;

    let operator: String = sqlx::query_scalar("insert into app_user (display_name, email) values ('Docsign Proof Operator', $1) returning id::text")
        .bind(format!("{tag}-operator@example.test")).fetch_one(db.pool()).await.unwrap();
    let mut ctx = context(&tag);
    {
        let principal = ctx.principal.as_mut().unwrap();
        principal.app_user_id = operator.clone();
        principal.level = "BUSINESS_POWER_USER".into();
        principal.role_codes = vec!["owner".into()];
    }
    let infrastructure = web::service_bootstrap::production_service_infrastructure(&db)
        .await
        .unwrap();
    let catalog = web::composition::ServiceCatalog::new(db.clone(), infrastructure.clone());
    let service = catalog.document_sign();
    let router = web::api::build_router(
        db.clone(),
        infrastructure,
        web::api::ApiConfig {
            internal_api_key: Arc::from(HTTP_KEY),
        },
    );

    // Send by the form's own anchors: ONE seller recipient claims the SELLER block; Lisa's block is already signed.
    let ada = format!("{tag}-ada@example.test");
    let mut tx = db.begin("anchors").await.unwrap();
    let sent = service
        .send_transactional(
            &mut tx,
            &SendDocumentSignRequest {
                transaction_document_id: document.clone(),
                recipients: vec![person("Ada Alvarez", &ada, 1)],
                subject: Some("Listing Agreement".into()),
                message: None,
                signing_mode: DocumentSigningMode::Parallel,
                expires_at: None,
                placement: SendFieldPlacement::Template,
                copy_to: vec![],
                reminder_every_days: Some(0),
            },
            &ctx,
        )
        .await
        .expect("send by anchors");
    tx.commit().await.unwrap();
    let kinds: std::collections::BTreeSet<_> = sent
        .snapshot
        .fields
        .iter()
        .map(|f| f.field_type.as_str())
        .collect();
    assert_eq!(
        kinds,
        ["date", "initials", "signature"].into_iter().collect(),
        "the seller's three blocks, and none for Lisa"
    );
    assert!(
        sent.snapshot.fields.iter().all(|f| f.page_number == 5),
        "on the page the template put them"
    );

    // Ada signs once; her picture is the one she saw.
    let picture = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../public/images/culebraluxe-email-logo.png"),
    )
    .unwrap();
    let data_url = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&picture)
    );
    let token = token_for(&db, &ada).await;
    let (_, session) = http(
        &router,
        "/v1/signer/session",
        serde_json::json!({ "accessToken": token }),
    )
    .await;
    let recipient = session["value"]["recipient"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    http(
        &router,
        "/v1/signer/open",
        serde_json::json!({ "accessToken": token, "recipientId": recipient }),
    )
    .await;
    let (status, body) = http(&router, "/v1/signer/consent", serde_json::json!({
        "accessToken": token, "recipientId": recipient, "consentVersion": "v1", "consentText": "I agree.", "consentTextSha256": CONSENT_SHA })).await;
    assert!(
        status == 200 && outcome(&body) == "success",
        "consent {body}"
    );
    // A picture that is not a PNG is refused at the door.
    let bad = sent
        .snapshot
        .fields
        .iter()
        .find(|f| f.field_type == model::SignatureFieldType::Signature)
        .unwrap();
    let (_, refused) = http(
        &router,
        "/v1/signer/field",
        serde_json::json!({
        "accessToken": token, "recipientId": recipient, "fieldId": bad.id,
        "value": { "style": 0, "name": "Ada", "image": "data:image/png;base64,bm90IGEgcG5n" } }),
    )
    .await;
    assert_ne!(
        outcome(&refused),
        "success",
        "a non-PNG signature picture is refused: {refused}"
    );
    for field in &sent.snapshot.fields {
        let value = match field.field_type {
            model::SignatureFieldType::Signature => {
                serde_json::json!({ "style": 0, "name": "Ada Alvarez", "image": data_url })
            }
            model::SignatureFieldType::Initials => {
                serde_json::json!({ "style": 0, "name": "Ada Alvarez", "initialsImage": data_url })
            }
            _ => serde_json::json!({ "auto": true }),
        };
        let (status, body) = http(
            &router,
            "/v1/signer/field",
            serde_json::json!({
            "accessToken": token, "recipientId": recipient, "fieldId": field.id, "value": value }),
        )
        .await;
        assert!(
            status == 200 && outcome(&body) == "success",
            "field {:?}: {body}",
            field.field_type
        );
    }
    let (status, body) = http(
        &router,
        "/v1/signer/complete",
        serde_json::json!({ "accessToken": token, "recipientId": recipient }),
    )
    .await;
    assert!(
        status == 200 && outcome(&body) == "success",
        "complete {body}"
    );

    let mut tx = db.begin("anchors").await.unwrap();
    let done = service
        .finalize_transactional(&mut tx, &sent.snapshot.signature_request.id, &ctx)
        .await
        .expect("finalize");
    tx.commit().await.unwrap();
    let signed = done.signed_media_id.expect("sealed");
    let sealed: Vec<u8> = sqlx::query_scalar("select file_data from media where id = $1::uuid")
        .bind(&signed)
        .fetch_one(db.pool())
        .await
        .unwrap();
    if let Ok(dir) = std::env::var("DUMP_DIR") {
        std::fs::write(format!("{dir}/anchors_sealed.pdf"), &sealed).unwrap();
        let cert: Vec<u8> = sqlx::query_scalar("select file_data from media where id = $1::uuid")
            .bind(done.audit_media_id.as_ref().unwrap())
            .fetch_one(db.pool())
            .await
            .unwrap();
        std::fs::write(format!("{dir}/anchors_cert.pdf"), &cert).unwrap();
    }
    assert!(
        sealed.len() > artifact.bytes.len(),
        "the seal added the signer's pictures"
    );

    sweep_tag(&db, &tag).await;
    sqlx::query("delete from app_user where id = $1::uuid")
        .bind(&operator)
        .execute(db.pool())
        .await
        .unwrap();
}
