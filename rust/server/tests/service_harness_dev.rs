use db::{Database, DbTarget};
use serde_json::json;
use server::ServiceHarness;
use service::{
    CapturingAuditPort, CapturingDomainEventPort, DefaultAuthorizationPort, ServiceActor,
    ServiceActorKind, ServiceContext, ServiceControlCommand, ServiceDispatchError, ServiceEnvelope,
    ServiceInfrastructure, ServicePrincipal, ServiceStatus,
};
use std::sync::Arc;

fn infrastructure() -> ServiceInfrastructure {
    ServiceInfrastructure::new(
        Arc::new(DefaultAuthorizationPort),
        Arc::new(CapturingAuditPort::default()),
        Arc::new(CapturingDomainEventPort::default()),
    )
}

fn context() -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some("external-harness-proof".into()),
            kind: ServiceActorKind::User,
        },
        correlation_id: "external-harness-proof".into(),
        causation_id: None,
        principal: Some(ServicePrincipal {
            app_user_id: "external-harness-proof".into(),
            level: "BUSINESS_POWER_USER".into(),
            role_codes: vec!["business_power_user".into()],
            account_type: "internal".into(),
            entitlement_codes: vec![],
        }),
    }
}

#[tokio::test]
#[ignore = "requires DATABASE_URL_DEV; run under the Rust DEV DB gate"]
async fn external_harness_boot_dispatch_drain_and_shutdown_are_real() {
    let db = Database::connect_from_env()
        .await
        .expect("connect DEV database");
    assert_eq!(db.target(), DbTarget::Dev, "external harness proof is DEV-only");

    let harness = ServiceHarness::isolated(db, infrastructure()).unwrap();
    harness.start().await.unwrap();

    let health = harness.runtime_health();
    assert_eq!(health.kernel.status, ServiceStatus::Running);
    assert_eq!(health.mq.status, ServiceStatus::Running);
    assert!(health.kernel.accepting);
    assert!(health.mq.accepting);

    let descriptors = harness.descriptors();
    assert!(descriptors.iter().any(|descriptor| descriptor.domain == "person"));
    assert!(descriptors.iter().any(|descriptor| descriptor.domain == "contract"));

    let value = harness
        .dispatch(
            &ServiceEnvelope {
                domain: "person".into(),
                operation: "person.search".into(),
                payload: json!({ "query": "__external_harness_no_match__", "limit": 1 }),
            },
            &context(),
        )
        .await
        .expect("query through canonical ServiceHarness");
    assert!(value.is_array());

    let drained = harness
        .control("person", ServiceControlCommand::Drain)
        .await
        .unwrap();
    assert_eq!(drained.status, ServiceStatus::Draining);
    assert!(!drained.health.accepting);

    let refused = harness
        .dispatch(
            &ServiceEnvelope {
                domain: "person".into(),
                operation: "person.search".into(),
                payload: json!({ "query": "anything", "limit": 1 }),
            },
            &context(),
        )
        .await;
    assert!(matches!(refused, Err(ServiceDispatchError::ServiceDraining(_))));

    harness.begin_shutdown();
    harness.wait_stopped().await.unwrap();

    let stopped = harness.runtime_health();
    assert_eq!(stopped.kernel.status, ServiceStatus::Stopped);
    assert_eq!(stopped.mq.status, ServiceStatus::Stopped);
    assert!(!stopped.kernel.accepting);
    assert!(!stopped.mq.accepting);
}
