use chrono::Utc;
use db::{Database, DbTarget};
use serde_json::{json, Map, Value};
use server::ServiceHarness;
use service::{
    CapturingAuditPort, CapturingDomainEventPort, CommandOutcome, CommandRequest,
    DefaultAuthorizationPort, ServiceActor, ServiceActorKind, ServiceContext,
    ServiceInfrastructure, ServicePrincipal,
};
use std::sync::Arc;
use uuid::Uuid;

struct Fixture {
    db: Database,
    contract_id: String,
    command_id: String,
}

impl Fixture {
    async fn create() -> Self {
        let db = Database::connect_from_env()
            .await
            .expect("connect DEV database");
        assert_eq!(
            db.target(),
            DbTarget::Dev,
            "command runtime tests are DEV-only"
        );

        let property_id = sqlx::query_scalar::<_, String>(
            "select id::text from property order by created_at asc nulls last, id limit 1",
        )
        .fetch_one(db.pool())
        .await
        .expect("DEV needs at least one Property");

        let role_id = sqlx::query_scalar::<_, String>(
            "select id::text from role
             where scope='contract_property' and code='SUBJECT_PROPERTY' and active=true
             limit 1",
        )
        .fetch_one(db.pool())
        .await
        .expect("SUBJECT_PROPERTY role must exist");

        let contract_id = Uuid::new_v4().to_string();
        sqlx::query(
            "insert into contract (
                id, contract_type, form_template_id, facts, status
             ) values ($1::uuid, 'purchase_sale', 'PR-PNS', '{}'::jsonb, 'draft')",
        )
        .bind(&contract_id)
        .execute(db.pool())
        .await
        .expect("insert command proof Contract");

        sqlx::query(
            "insert into contract_property (
                contract_id, property_id, role_id, role_scope, ordinal
             ) values ($1::uuid,$2::uuid,$3::uuid,'contract_property',0)",
        )
        .bind(&contract_id)
        .bind(&property_id)
        .bind(&role_id)
        .execute(db.pool())
        .await
        .expect("insert SUBJECT_PROPERTY mapping");

        Self {
            db,
            contract_id,
            command_id: format!("rust-command-proof-{}", Uuid::new_v4()),
        }
    }

    async fn context(&self) -> ServiceContext {
        let app_user_id = sqlx::query_scalar::<_, String>(
            "select id::text
             from app_user
             where active=true and account_type='internal'
             order by created_at asc nulls last, id
             limit 1",
        )
        .fetch_one(self.db.pool())
        .await
        .expect("DEV needs one active internal app_user");

        ServiceContext {
            actor: ServiceActor {
                id: Some(app_user_id.clone()),
                kind: ServiceActorKind::User,
            },
            correlation_id: format!("corr-{}", self.command_id),
            causation_id: Some("external-command-proof".into()),
            principal: Some(ServicePrincipal {
                app_user_id,
                level: "BUSINESS_POWER_USER".into(),
                role_codes: vec!["business_power_user".into()],
                account_type: "internal".into(),
                entitlement_codes: vec!["contract.execute".into()],
            }),
        }
    }

    fn request(&self) -> CommandRequest {
        CommandRequest {
            command_id: self.command_id.clone(),
            command_type: "contract.execute".into(),
            aggregate_type: "contract".into(),
            aggregate_id: Some(self.contract_id.clone()),
            requested_at: Utc::now().to_rfc3339(),
            input: Map::from_iter([("contractId".into(), json!(self.contract_id))]),
        }
    }

    async fn cleanup(&self) {
        let _ = sqlx::query("delete from outbox_message where causation_id=$1")
            .bind(&self.command_id)
            .execute(self.db.pool())
            .await;
        let _ = sqlx::query("delete from workflow_command_receipt where command_id=$1")
            .bind(&self.command_id)
            .execute(self.db.pool())
            .await;
        let _ = sqlx::query("delete from contract_property where contract_id=$1::uuid")
            .bind(&self.contract_id)
            .execute(self.db.pool())
            .await;
        let _ = sqlx::query("delete from contract where id=$1::uuid")
            .bind(&self.contract_id)
            .execute(self.db.pool())
            .await;
    }
}

fn infrastructure() -> ServiceInfrastructure {
    ServiceInfrastructure::new(
        Arc::new(DefaultAuthorizationPort),
        Arc::new(CapturingAuditPort::default()),
        Arc::new(CapturingDomainEventPort::default()),
    )
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV; run under the Rust DEV DB gate"]
async fn command_dev_commit_replay_and_intent_conflict_are_durable() {
    let fixture = Fixture::create().await;
    let harness = ServiceHarness::new(fixture.db.clone(), infrastructure()).unwrap();
    let context = fixture.context().await;
    let request = fixture.request();

    let first = harness
        .execute_command(&request, &context)
        .await
        .expect("first command dispatch");
    assert_eq!(first.outcome, CommandOutcome::Success);
    assert!(!first.replayed);
    assert_eq!(
        first.receipt_id.as_deref(),
        Some(fixture.command_id.as_str())
    );
    assert_eq!(first.emitted_events.len(), 1);

    let status = sqlx::query_scalar::<_, String>("select status from contract where id=$1::uuid")
        .bind(&fixture.contract_id)
        .fetch_one(fixture.db.pool())
        .await
        .unwrap();
    assert_eq!(status, "executed");

    let receipt_count = sqlx::query_scalar::<_, i64>(
        "select count(*)::bigint from workflow_command_receipt where command_id=$1",
    )
    .bind(&fixture.command_id)
    .fetch_one(fixture.db.pool())
    .await
    .unwrap();
    assert_eq!(receipt_count, 1);

    let outbox_ids = sqlx::query_scalar::<_, String>(
        "select id::text from outbox_message where causation_id=$1 order by created_at, id",
    )
    .bind(&fixture.command_id)
    .fetch_all(fixture.db.pool())
    .await
    .unwrap();
    assert_eq!(outbox_ids.len(), 1);
    assert_eq!(outbox_ids[0], first.emitted_events[0].event_id);

    let replay = harness
        .execute_command(&request, &context)
        .await
        .expect("exact replay");
    assert_eq!(replay.outcome, CommandOutcome::Success);
    assert!(replay.replayed);

    let post_replay_outbox = sqlx::query_scalar::<_, i64>(
        "select count(*)::bigint from outbox_message where causation_id=$1",
    )
    .bind(&fixture.command_id)
    .fetch_one(fixture.db.pool())
    .await
    .unwrap();
    assert_eq!(post_replay_outbox, 1);

    let mut changed = request.clone();
    changed
        .input
        .insert("differentIntent".into(), Value::Bool(true));
    let conflict = harness
        .execute_command(&changed, &context)
        .await
        .expect("intent conflict is a durable business result");
    assert_eq!(conflict.outcome, CommandOutcome::Conflict);
    assert_eq!(
        conflict.error.as_ref().map(|error| error.code.as_str()),
        Some("COMMAND_ID_INTENT_CONFLICT")
    );

    let post_conflict_outbox = sqlx::query_scalar::<_, i64>(
        "select count(*)::bigint from outbox_message where causation_id=$1",
    )
    .bind(&fixture.command_id)
    .fetch_one(fixture.db.pool())
    .await
    .unwrap();
    assert_eq!(post_conflict_outbox, 1);

    harness.kernel().shutdown().await.unwrap();
    fixture.cleanup().await;
}
