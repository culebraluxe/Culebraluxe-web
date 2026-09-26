use crate::{CommandDispatcher, ServiceRegistry};
use async_trait::async_trait;
use db::{AgreementExecutionDao, Database, IssuedAgreementDocumentRow, OutboxDelivery, WorkflowOpsDao};
use serde_json::{json, Map, Value};
use service::{
    CommandOutcome, CommandRequest, ServiceActor, ServiceActorKind, ServiceContext,
};
use std::sync::Arc;

use crate::mq_runtime::{MqSubscriber, MqSubscriberError};

pub const CRM26_SUBSCRIPTION_ID: &str = "crm26-agreement-execution";
pub const AGREEMENT_FULLY_EXECUTED_ROUTING_KEY: &str = "AGREEMENT_FULLY_EXECUTED";
const AGREEMENT_EXECUTION_ACTOR: &str = "agreement-execution-worker";

#[derive(Debug, Clone, PartialEq, Eq)]
struct AgreementExecutionLocator {
    transaction_document_id: String,
    issued_version: i32,
    template_id: String,
    contract_id: String,
}

fn required_string(
    payload: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<String, MqSubscriberError> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            MqSubscriberError::new(format!(
                "AGREEMENT_FULLY_EXECUTED payload requires non-empty {key}."
            ))
        })
}

fn parse_locator(payload: &Value) -> Result<AgreementExecutionLocator, MqSubscriberError> {
    let object = payload.as_object().ok_or_else(|| {
        MqSubscriberError::new("AGREEMENT_FULLY_EXECUTED payload must be an object.")
    })?;
    let issued_version = object
        .get("issuedVersion")
        .and_then(Value::as_i64)
        .filter(|value| *value >= 1 && *value <= i32::MAX as i64)
        .map(|value| value as i32)
        .ok_or_else(|| {
            MqSubscriberError::new(
                "AGREEMENT_FULLY_EXECUTED payload requires a positive integer issuedVersion.",
            )
        })?;

    Ok(AgreementExecutionLocator {
        transaction_document_id: required_string(object, "transactionDocumentId")?,
        issued_version,
        template_id: required_string(object, "templateId")?,
        contract_id: required_string(object, "contractId")?,
    })
}

fn execution_eligible_template(template_id: &str) -> bool {
    matches!(template_id, "PR-PNS" | "LISTING-01")
}

#[async_trait]
trait Crm26Port: Send + Sync {
    async fn load_issued_document(
        &self,
        document_id: &str,
    ) -> Result<Option<IssuedAgreementDocumentRow>, MqSubscriberError>;

    async fn execution_marker_matches(
        &self,
        document_id: &str,
        issued_version: i32,
        event_id: &str,
    ) -> Result<bool, MqSubscriberError>;

    async fn execute_contract(
        &self,
        contract_id: &str,
        evidence_document_id: &str,
        delivery: &OutboxDelivery,
    ) -> Result<(), MqSubscriberError>;

    async fn ensure_workflow(&self, contract_id: &str) -> Result<String, MqSubscriberError>;

    async fn find_actionable_task(
        &self,
        instance_id: &str,
        node_id: &str,
    ) -> Result<Option<String>, MqSubscriberError>;

    async fn complete_engine_task(
        &self,
        task_id: &str,
        transition: &str,
    ) -> Result<(), MqSubscriberError>;
}

#[derive(Clone)]
struct RustCrm26Port {
    agreement: AgreementExecutionDao,
    workflow: WorkflowOpsDao,
    commands: CommandDispatcher,
    registry: Arc<ServiceRegistry>,
}

impl RustCrm26Port {
    fn new(db: Database, commands: CommandDispatcher, registry: Arc<ServiceRegistry>) -> Self {
        Self {
            agreement: AgreementExecutionDao::new(db.clone()),
            workflow: WorkflowOpsDao::new(db),
            commands,
            registry,
        }
    }
}

fn worker_context(delivery: &OutboxDelivery) -> ServiceContext {
    ServiceContext {
        actor: ServiceActor {
            id: Some(AGREEMENT_EXECUTION_ACTOR.into()),
            kind: ServiceActorKind::System,
        },
        correlation_id: delivery
            .correlation_id
            .clone()
            .unwrap_or_else(|| delivery.event_id.clone()),
        causation_id: Some(delivery.event_id.clone()),
        principal: None,
    }
}

fn service_error(error: impl std::fmt::Display) -> MqSubscriberError {
    MqSubscriberError::new(error.to_string())
}

#[async_trait]
impl Crm26Port for RustCrm26Port {
    async fn load_issued_document(
        &self,
        document_id: &str,
    ) -> Result<Option<IssuedAgreementDocumentRow>, MqSubscriberError> {
        self.agreement
            .load_issued_document(document_id)
            .await
            .map_err(service_error)
    }

    async fn execution_marker_matches(
        &self,
        document_id: &str,
        issued_version: i32,
        event_id: &str,
    ) -> Result<bool, MqSubscriberError> {
        self.agreement
            .marker_matches(document_id, issued_version, event_id)
            .await
            .map_err(service_error)
    }

    async fn execute_contract(
        &self,
        contract_id: &str,
        evidence_document_id: &str,
        delivery: &OutboxDelivery,
    ) -> Result<(), MqSubscriberError> {
        let mut input = Map::new();
        input.insert("contractId".into(), json!(contract_id));
        input.insert("evidenceDocumentId".into(), json!(evidence_document_id));
        let request = CommandRequest {
            command_id: format!("crm26:{}:contract.execute", delivery.event_id),
            command_type: "contract.execute".into(),
            aggregate_type: "contract".into(),
            aggregate_id: Some(contract_id.to_owned()),
            requested_at: delivery.occurred_at.to_rfc3339(),
            input,
        };
        let context = worker_context(delivery);
        let (domain, operation, payload) = self
            .commands
            .scheduling_route(&request)
            .ok_or_else(|| MqSubscriberError::new("contract.execute has no scheduler route."))?;
        let dispatcher = self.commands.clone();
        let scheduled_request = request.clone();
        let scheduled_context = context.clone();
        let result = self
            .registry
            .run_task(domain, operation, &payload, async move {
                dispatcher
                    .execute(&scheduled_request, &scheduled_context)
                    .await
            })
            .await
            .map_err(service_error)?
            .map_err(service_error)?;

        if result.outcome == CommandOutcome::Success {
            return Ok(());
        }

        let code = result
            .error
            .as_ref()
            .map(|error| error.code.as_str())
            .unwrap_or(result.outcome.as_str());
        let message = result
            .error
            .as_ref()
            .map(|error| error.message.as_str())
            .or(result.message.as_deref())
            .unwrap_or("contract.execute failed");
        Err(MqSubscriberError::new(format!("{code}: {message}")))
    }

    async fn ensure_workflow(&self, contract_id: &str) -> Result<String, MqSubscriberError> {
        let contract_id = contract_id.to_owned();
        tokio::task::spawn_blocking(move || {
            forge::engine::re_runtime::start_residential_transaction("contract", &contract_id)
                .map(|result| result.instance_id)
                .map_err(|error| error.to_string())
        })
        .await
        .map_err(service_error)?
        .map_err(MqSubscriberError::new)
    }

    async fn find_actionable_task(
        &self,
        instance_id: &str,
        node_id: &str,
    ) -> Result<Option<String>, MqSubscriberError> {
        self.workflow
            .find_actionable_task(instance_id, node_id)
            .await
            .map_err(service_error)
    }

    async fn complete_engine_task(
        &self,
        task_id: &str,
        transition: &str,
    ) -> Result<(), MqSubscriberError> {
        let task_id = task_id.to_owned();
        let transition = transition.to_owned();
        tokio::task::spawn_blocking(move || {
            match forge::engine::re_runtime::complete_engine_task(
                &task_id,
                "system",
                Some(&transition),
            ) {
                Ok(()) => Ok(()),
                Err(error) if error.code() == "TASK_ALREADY_COMPLETED" => Ok(()),
                Err(error) => Err(error.to_string()),
            }
        })
        .await
        .map_err(service_error)?
        .map_err(MqSubscriberError::new)
    }
}

pub struct Crm26AgreementExecutionSubscriber {
    port: Arc<dyn Crm26Port>,
}

impl Crm26AgreementExecutionSubscriber {
    pub fn production(
        db: Database,
        commands: CommandDispatcher,
        registry: Arc<ServiceRegistry>,
    ) -> Self {
        Self {
            port: Arc::new(RustCrm26Port::new(db, commands, registry)),
        }
    }

    #[cfg(test)]
    fn with_port(port: Arc<dyn Crm26Port>) -> Self {
        Self { port }
    }

    async fn complete_if_actionable(
        &self,
        instance_id: &str,
        node_id: &str,
        transition: &str,
    ) -> Result<(), MqSubscriberError> {
        if let Some(task_id) = self.port.find_actionable_task(instance_id, node_id).await? {
            self.port.complete_engine_task(&task_id, transition).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl MqSubscriber for Crm26AgreementExecutionSubscriber {
    fn id(&self) -> &str {
        CRM26_SUBSCRIPTION_ID
    }

    fn routing_key(&self) -> &str {
        AGREEMENT_FULLY_EXECUTED_ROUTING_KEY
    }

    fn max_attempts(&self) -> i32 {
        3
    }

    fn retry_backoff_seconds(&self) -> i32 {
        10
    }

    async fn handle(&self, delivery: &OutboxDelivery) -> Result<(), MqSubscriberError> {
        let locator = parse_locator(&delivery.payload)?;
        let document = self
            .port
            .load_issued_document(&locator.transaction_document_id)
            .await?
            .ok_or_else(|| {
                MqSubscriberError::new(format!(
                    "Issued transaction document {} not found.",
                    locator.transaction_document_id
                ))
            })?;

        if document.template_id.as_deref() != Some(locator.template_id.as_str()) {
            return Err(MqSubscriberError::new(format!(
                "Lineage mismatch: event template {} != document {:?}.",
                locator.template_id, document.template_id
            )));
        }
        if document.issued_version != Some(locator.issued_version) {
            return Err(MqSubscriberError::new(format!(
                "Lineage mismatch: event issuedVersion {} != document {:?}.",
                locator.issued_version, document.issued_version
            )));
        }
        if document.contract_id.as_deref() != Some(locator.contract_id.as_str()) {
            return Err(MqSubscriberError::new(format!(
                "Lineage mismatch: event Contract {} != document {:?}.",
                locator.contract_id, document.contract_id
            )));
        }
        if !execution_eligible_template(&locator.template_id) {
            return Err(MqSubscriberError::new(format!(
                "Template {} is not execution-eligible.",
                locator.template_id
            )));
        }
        if !self
            .port
            .execution_marker_matches(
                &locator.transaction_document_id,
                locator.issued_version,
                &delivery.event_id,
            )
            .await?
        {
            return Err(MqSubscriberError::new(format!(
                "No agreement_execution marker for {} v{} carrying event {}.",
                locator.transaction_document_id, locator.issued_version, delivery.event_id
            )));
        }

        self.port
            .execute_contract(
                &locator.contract_id,
                &locator.transaction_document_id,
                delivery,
            )
            .await?;
        let instance_id = self.port.ensure_workflow(&locator.contract_id).await?;
        self.complete_if_actionable(&instance_id, "pns_preparation", "prepared")
            .await?;
        self.complete_if_actionable(&instance_id, "pns_executed", "executed")
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::{
        collections::HashMap,
        sync::Mutex,
    };

    struct FakePort {
        document: Option<IssuedAgreementDocumentRow>,
        marker_matches: bool,
        calls: Mutex<Vec<String>>,
        tasks: Mutex<HashMap<String, String>>,
        execute_fails: bool,
    }

    impl FakePort {
        fn valid() -> Self {
            Self {
                document: Some(IssuedAgreementDocumentRow {
                    id: "00000000-0000-0000-0000-000000000001".into(),
                    contract_id: Some("00000000-0000-0000-0000-000000000002".into()),
                    template_id: Some("PR-PNS".into()),
                    issued_version: Some(1),
                }),
                marker_matches: true,
                calls: Mutex::new(Vec::new()),
                tasks: Mutex::new(HashMap::from([
                    ("pns_preparation".into(), "task-prep".into()),
                    ("pns_executed".into(), "task-exec".into()),
                ])),
                execute_fails: false,
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl Crm26Port for FakePort {
        async fn load_issued_document(
            &self,
            document_id: &str,
        ) -> Result<Option<IssuedAgreementDocumentRow>, MqSubscriberError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("load:{document_id}"));
            Ok(self.document.clone())
        }

        async fn execution_marker_matches(
            &self,
            _document_id: &str,
            _issued_version: i32,
            _event_id: &str,
        ) -> Result<bool, MqSubscriberError> {
            self.calls.lock().unwrap().push("marker".into());
            Ok(self.marker_matches)
        }

        async fn execute_contract(
            &self,
            contract_id: &str,
            evidence_document_id: &str,
            _delivery: &OutboxDelivery,
        ) -> Result<(), MqSubscriberError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("execute:{contract_id}:{evidence_document_id}"));
            if self.execute_fails {
                Err(MqSubscriberError::new("contract write failed"))
            } else {
                Ok(())
            }
        }

        async fn ensure_workflow(&self, contract_id: &str) -> Result<String, MqSubscriberError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("workflow:{contract_id}"));
            Ok("instance-1".into())
        }

        async fn find_actionable_task(
            &self,
            _instance_id: &str,
            node_id: &str,
        ) -> Result<Option<String>, MqSubscriberError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("find:{node_id}"));
            Ok(self.tasks.lock().unwrap().get(node_id).cloned())
        }

        async fn complete_engine_task(
            &self,
            task_id: &str,
            transition: &str,
        ) -> Result<(), MqSubscriberError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("complete:{task_id}:{transition}"));
            self.tasks
                .lock()
                .unwrap()
                .retain(|_, value| value != task_id);
            Ok(())
        }
    }

    fn delivery(payload: Value) -> OutboxDelivery {
        OutboxDelivery {
            delivery_id: "00000000-0000-0000-0000-000000000010".into(),
            event_id: "00000000-0000-0000-0000-000000000011".into(),
            subscription_id: CRM26_SUBSCRIPTION_ID.into(),
            event_type: AGREEMENT_FULLY_EXECUTED_ROUTING_KEY.into(),
            actor_app_user_id: None,
            aggregate_type: Some("transaction_document".into()),
            aggregate_id: Some("00000000-0000-0000-0000-000000000001".into()),
            correlation_id: Some("corr-1".into()),
            causation_id: Some("cmd-1".into()),
            occurred_at: Utc::now(),
            payload,
            attempt_count: 1,
            max_attempts: 3,
            retry_backoff_seconds: 10,
            lease_until: None,
        }
    }

    fn valid_payload() -> Value {
        json!({
            "transactionDocumentId": "00000000-0000-0000-0000-000000000001",
            "issuedVersion": 1,
            "templateId": "PR-PNS",
            "contractId": "00000000-0000-0000-0000-000000000002"
        })
    }

    #[test]
    fn crm26_payload_requires_explicit_contract_lineage() {
        let error = parse_locator(&json!({
            "transactionDocumentId": "doc",
            "issuedVersion": 1,
            "templateId": "PR-PNS"
        }))
        .unwrap_err();
        assert!(error.to_string().contains("contractId"));
    }

    #[tokio::test]
    async fn crm26_execution_order_matches_contract_workflow_boundary() {
        let port = Arc::new(FakePort::valid());
        let subscriber = Crm26AgreementExecutionSubscriber::with_port(port.clone());
        subscriber.handle(&delivery(valid_payload())).await.unwrap();

        assert_eq!(
            port.calls(),
            vec![
                "load:00000000-0000-0000-0000-000000000001",
                "marker",
                "execute:00000000-0000-0000-0000-000000000002:00000000-0000-0000-0000-000000000001",
                "workflow:00000000-0000-0000-0000-000000000002",
                "find:pns_preparation",
                "complete:task-prep:prepared",
                "find:pns_executed",
                "complete:task-exec:executed",
            ]
        );
    }

    #[tokio::test]
    async fn crm26_contract_failure_stops_workflow_advancement() {
        let mut fake = FakePort::valid();
        fake.execute_fails = true;
        let port = Arc::new(fake);
        let subscriber = Crm26AgreementExecutionSubscriber::with_port(port.clone());
        assert!(subscriber.handle(&delivery(valid_payload())).await.is_err());
        assert!(!port.calls().iter().any(|call| call.starts_with("workflow:")));
    }

    #[tokio::test]
    async fn crm26_task_completion_is_replay_safe() {
        let port = Arc::new(FakePort::valid());
        let subscriber = Crm26AgreementExecutionSubscriber::with_port(port.clone());
        subscriber
            .complete_if_actionable("instance-1", "pns_executed", "executed")
            .await
            .unwrap();
        subscriber
            .complete_if_actionable("instance-1", "pns_executed", "executed")
            .await
            .unwrap();

        assert_eq!(
            port.calls()
                .iter()
                .filter(|call| call.as_str() == "complete:task-exec:executed")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn crm26_rejects_marker_or_lineage_mismatch_before_mutation() {
        let mut fake = FakePort::valid();
        fake.marker_matches = false;
        let port = Arc::new(fake);
        let subscriber = Crm26AgreementExecutionSubscriber::with_port(port.clone());
        assert!(subscriber.handle(&delivery(valid_payload())).await.is_err());
        assert!(!port.calls().iter().any(|call| call.starts_with("execute:")));

        let mut fake = FakePort::valid();
        fake.document.as_mut().unwrap().issued_version = Some(2);
        let port = Arc::new(fake);
        let subscriber = Crm26AgreementExecutionSubscriber::with_port(port.clone());
        assert!(subscriber.handle(&delivery(valid_payload())).await.is_err());
        assert!(!port.calls().iter().any(|call| call == "marker"));
    }
}
