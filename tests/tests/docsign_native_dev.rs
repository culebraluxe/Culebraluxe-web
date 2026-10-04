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
use web::email::EmailService;
use web::service_support::CoreServiceError;
use web::signer::{SignerAccessTokenCodec, SignerService, DOCSIGN_EDGE_ACTOR};
use db::EmailDao;

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
/// Tags are fixed per test (not random) with a leading sweep, so a previous
/// failed run's leftovers are removed rather than accumulated: cleanup on the
/// happy path is not enough, because a panic skips it.
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
    // Unlink first: the audit-media link is `on delete restrict`.
    let media_ids: Vec<Option<String>> = sqlx::query_scalar(
        "update transaction_document set signed_audit_media_id = null \
         where title = $1 returning signed_audit_media_id::text",
    )
    .bind(&title)
    .fetch_all(db.pool())
    .await
    .expect("audit unlink cleanup");
    for media_id in media_ids.into_iter().flatten() {
        sqlx::query("delete from media where id = $1::uuid")
            .bind(&media_id)
            .execute(db.pool())
            .await
            .expect("audit media cleanup");
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
    sqlx::query("delete from email_message where correlation_id = $1")
        .bind(tag)
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
    let tag = "docsign-turn";
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
    let tag = "docsign-gates";
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
    let tag = "docsign-replay";
    let env = envelope(&db, &tag).await;
    let service = signer(&db);
    let ctx = context(&tag);
    let token_a = grant(&service, &db, &env.a, &ctx).await;
    consent(&service, &db, &env.a, &token_a, &ctx).await;
    // A second acceptance with different text returns the existing evidence.
    let other_sha =
        "d4735e3a265e16eee03f59718b9b5d03019c07d8b6c51f90da3a666eec13ab35";
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
    let tag = "docsign-dedupe";
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
    sweep_tag(&db, tag).await;
}

async fn document_sign(db: &Database) -> web::document_sign::DocumentSignService<
    db::DocumentSignDao,
    db::SignatureDao,
    SignerDao,
    EmailDao,
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
        infra(),
    )
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn finalize_closes_a_signed_envelope_with_its_audit_trail() {
    use model::SignatureRequestStatus;
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = "docsign-finalize";
    let env = envelope(&db, tag).await;
    let signing = signer(&db);
    let ctx = context(tag);
    for (recipient, field) in
        [(&env.a, Some(env.field_a.as_str())), (&env.b, None), (&env.c, None)]
    {
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
    let status: String = sqlx::query_scalar(
        "select status from signature_request where id = $1::uuid",
    )
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
    let magic: Vec<u8> = sqlx::query_scalar(
        "select substring(file_data from 1 for 8) from media where id = $1::uuid",
    )
    .bind(&audit_media_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(&magic, b"%PDF-1.4");

    // Replay answers with the existing artifact instead of storing another.
    let media_before: i64 = sqlx::query_scalar(
        "select count(*) from media where id = $1::uuid",
    )
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
    assert_eq!(replay.audit_media_id.as_deref(), Some(audit_media_id.as_str()));
    let media_after: i64 = sqlx::query_scalar(
        "select count(*) from media where id = $1::uuid",
    )
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
    let tag = "docsign-sweep";
    let env = envelope(&db, tag).await;
    let signing = signer(&db);
    let ctx = context(tag);
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
        .transition_transactional(&mut tx, &env.request_id, model::SignatureRequestStatus::Sent, &ctx)
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
    let status: String = sqlx::query_scalar(
        "select status from signature_request where id = $1::uuid",
    )
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
    let tag = "docsign-import";
    let env = envelope(&db, tag).await;
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
        rect: TemplateAnchorRect { x: 72.0, y: 650.0, width: 180.0, height: 20.0 },
    };
    let service = document_sign(&db).await;
    let ctx = context(tag);
    let pre: Vec<(String, String)> = sqlx::query_as(
        "select f.field_key, f.recipient_id::text from signature_field f          join transaction_document td on td.title = 'docsign proof docsign-import'          join signature_request sr on sr.transaction_document_id = td.id          where f.signature_request_id = sr.id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    eprintln!("PRE-IMPORT fields: {pre:?}");
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
