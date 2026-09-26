//! Regression proofs for ordinary service mutations and production event durability.
use async_trait::async_trait;
use db::{Database, DbTarget, FirmDao};
use domain::{FieldPatch, UpsertFirmRequest};
use server::{firms::FirmService, service_events::TransactionalDomainEventPort};
use service::{
    CapturingAuditPort, DefaultAuthorizationPort, DomainEventPort, ServiceActor, ServiceActorKind,
    ServiceContext, ServiceDomainEvent, ServiceInfrastructure, ServicePortError, ServicePrincipal,
};
use std::sync::Arc;
use uuid::Uuid;

struct FailAfterAppend(TransactionalDomainEventPort);
#[async_trait]
impl DomainEventPort for FailAfterAppend {
    async fn emit(&self, event: ServiceDomainEvent) -> Result<(), ServicePortError> {
        self.0.emit(event).await?;
        Err(ServicePortError::new(
            "deliberate failure after transactional outbox append",
        ))
    }
}
fn context(tag: &str) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("atomicity-proof".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: tag.into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "atomicity-proof".into(),
            level: "BUSINESS_POWER_USER".into(),
            role_codes: vec!["business_power_user".into()],
            account_type: "internal".into(),
            entitlement_codes: vec!["firm.write".into()],
        }),
    }
}
fn infra(events: Arc<dyn DomainEventPort>) -> ServiceInfrastructure {
    ServiceInfrastructure::new(
        Arc::new(DefaultAuthorizationPort),
        Arc::new(CapturingAuditPort::default()),
        events,
    )
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn ordinary_service_mutation_and_event_commit_or_rollback_together() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("service-atomicity-{}", Uuid::new_v4());
    let request = UpsertFirmRequest {
        firm_id: None,
        name: tag.clone(),
        legal_name: FieldPatch::Unchanged,
        kind: FieldPatch::Unchanged,
        status: Some("active".into()),
    };
    let failing = FirmService::new(
        FirmDao::new(db.clone()),
        infra(Arc::new(FailAfterAppend(
            TransactionalDomainEventPort::new(db.clone()),
        ))),
    );
    assert!(failing.upsert(&request, &context(&tag)).await.is_err());
    let firms: i64 = sqlx::query_scalar("select count(*) from firm where name=$1")
        .bind(&tag)
        .fetch_one(db.pool())
        .await
        .unwrap();
    let events: i64 =
        sqlx::query_scalar("select count(*) from outbox_message where correlation_id=$1")
            .bind(&tag)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        (firms, events),
        (0, 0),
        "a failed event must roll back the preceding mutation and append"
    );
    let service = FirmService::new(
        FirmDao::new(db.clone()),
        infra(Arc::new(TransactionalDomainEventPort::new(db.clone()))),
    );
    let firm = service.upsert(&request, &context(&tag)).await.unwrap();
    let events: i64 = sqlx::query_scalar(
        "select count(*) from outbox_message where correlation_id=$1 and aggregate_id=$2",
    )
    .bind(&tag)
    .bind(&firm.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("delete from outbox_message where correlation_id=$1")
        .bind(&tag)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("delete from firm where id=$1::uuid")
        .bind(&firm.id)
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(events, 1);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn abandoned_nested_transaction_cannot_be_committed_by_outer_scope() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("service-abandon-{}", Uuid::new_v4());
    let result: Result<(), db::DbFailure> = db
        .mutation(async {
            let mut tx = db.begin("test.nested").await?;
            sqlx::query("insert into firm(name,status) values($1,'active')")
                .bind(&tag)
                .execute(tx.connection())
                .await
                .map_err(|e| db::DbFailure::from_sqlx("test.insert", &e))?;
            drop(tx); // deliberate uncommitted inner scope, even though outer reports success
            Ok(())
        })
        .await;
    let count: i64 = sqlx::query_scalar("select count(*) from firm where name=$1")
        .bind(&tag)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert!(result.is_err());
    assert_eq!(count, 0);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV"]
async fn nested_repository_commit_and_success_audit_roll_back_with_outer_failure() {
    let db = Database::connect_target(DbTarget::Dev).await.unwrap();
    let tag = format!("service-nested-{}", Uuid::new_v4());
    let result: Result<(), db::DbFailure> = db
        .mutation(async {
            let mut tx = db.begin("test.repository").await?;
            sqlx::query("insert into firm(name,status) values($1,'active')")
                .bind(&tag)
                .execute(tx.connection())
                .await
                .map_err(|e| db::DbFailure::from_sqlx("test.insert", &e))?;
            db::SecurityAuditDao::new(db.clone())
                .record_tx(
                    &mut tx,
                    None,
                    "test.atomicity",
                    "rust-command",
                    &serde_json::json!({"correlationId":tag,"outcome":"success"}),
                )
                .await?;
            tx.commit().await?;
            Err(db::DbFailure::configuration(
                "test.abort",
                "deliberate failure after nested commit and audit",
            ))
        })
        .await;
    let firms: i64 = sqlx::query_scalar("select count(*) from firm where name=$1")
        .bind(&tag)
        .fetch_one(db.pool())
        .await
        .unwrap();
    let audits: i64 = sqlx::query_scalar(
        "select count(*) from security_audit_event where metadata->>'correlationId'=$1",
    )
    .bind(&tag)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(result.is_err());
    assert_eq!((firms, audits), (0, 0));
}
