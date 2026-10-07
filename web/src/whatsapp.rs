use crate::service_support::CoreServiceError;
use apis::whatsapp::{
    parse_webhook, verify_handshake, verify_signature, MetaWhatsAppConfig, WhatsAppDirection,
};
use async_trait::async_trait;
use db::{
    DbResult, WhatsAppCanonicalInput, WhatsAppDao, WhatsAppLandingInput, WhatsAppProcessOutcome,
};
use serde::Serialize;
use serde_json::Value;
use services::{ServiceErrorRecord, ServiceFailureSeverity, ServiceInfrastructure, ServiceRuntime};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WhatsAppWebhookOutcome {
    pub outcome: String,
    pub resolved_person_id: Option<String>,
    pub interaction_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WhatsAppWebhookResult {
    pub accepted: usize,
    pub relationship_projected: usize,
    pub retryable_failure: bool,
    pub outcomes: Vec<WhatsAppWebhookOutcome>,
}

#[async_trait]
pub trait WhatsAppRepository: Send {
    async fn land(&self, input: &WhatsAppLandingInput) -> DbResult<bool>;
    async fn process_event(
        &self,
        input: &WhatsAppCanonicalInput,
    ) -> DbResult<WhatsAppProcessOutcome>;
    async fn refresh_client_read_models(&self) -> DbResult<()>;
}

#[async_trait]
impl WhatsAppRepository for WhatsAppDao {
    async fn land(&self, input: &WhatsAppLandingInput) -> DbResult<bool> {
        WhatsAppDao::land(self, input).await
    }

    async fn process_event(
        &self,
        input: &WhatsAppCanonicalInput,
    ) -> DbResult<WhatsAppProcessOutcome> {
        WhatsAppDao::process_event(self, input).await
    }

    async fn refresh_client_read_models(&self) -> DbResult<()> {
        WhatsAppDao::refresh_client_read_models(self).await
    }
}

pub struct WhatsAppService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: WhatsAppRepository> WhatsAppService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub fn verify_handshake(
        &self,
        mode: Option<&str>,
        token: Option<&str>,
        challenge: Option<&str>,
    ) -> Result<Option<String>, CoreServiceError> {
        let config = MetaWhatsAppConfig::from_env()
            .map_err(|error| CoreServiceError::business("WHATSAPP_NOT_CONFIGURED", error))?;
        Ok(verify_handshake(
            mode,
            token,
            challenge,
            &config.verify_token,
        ))
    }

    pub async fn handle_webhook(
        &self,
        raw_body: &str,
        signature: Option<&str>,
    ) -> Result<WhatsAppWebhookResult, CoreServiceError> {
        let config = MetaWhatsAppConfig::from_env()
            .map_err(|error| CoreServiceError::business("WHATSAPP_NOT_CONFIGURED", error))?;
        if !verify_signature(raw_body, signature, &config.app_secret) {
            return Err(CoreServiceError::business(
                "WHATSAPP_SIGNATURE_INVALID",
                "Invalid WhatsApp signature.",
            ));
        }

        let raw: Value = serde_json::from_str(raw_body).map_err(|_| {
            CoreServiceError::business("WHATSAPP_PAYLOAD_INVALID", "Invalid WhatsApp payload.")
        })?;
        let events = parse_webhook(raw_body, &config, chrono::Utc::now()).map_err(|_| {
            CoreServiceError::business("WHATSAPP_PAYLOAD_INVALID", "Invalid WhatsApp payload.")
        })?;

        let mut outcomes = Vec::with_capacity(events.len());
        let mut relationship_projected = 0usize;
        let mut retryable_failure = false;

        for event in events {
            let direction = match event.direction {
                WhatsAppDirection::Inbound => "inbound",
                WhatsAppDirection::Outbound => "outbound",
            };
            let message_type = event
                .event_type
                .rsplit('.')
                .next()
                .unwrap_or("unknown")
                .to_owned();
            let (from_address, to_address) = match event.direction {
                WhatsAppDirection::Inbound => (
                    Some(event.external_phone_e164.clone()),
                    Some(event.owned_phone_e164.clone()),
                ),
                WhatsAppDirection::Outbound => (
                    Some(event.owned_phone_e164.clone()),
                    Some(event.external_phone_e164.clone()),
                ),
            };

            // Landing stays best-effort — a landing-table failure must not
            // prevent canonical CRM processing of an authenticated provider
            // event — but it is now RECORDED through the service error sink
            // instead of vanishing in `let _`.
            if let Err(error) = self
                .repository
                .land(&WhatsAppLandingInput {
                    source_account: Some(event.source_account.clone()),
                    source_message_id: event.external_event_id.clone(),
                    conversation_id: event.thread_id.clone(),
                    context_id: event.thread_id.clone(),
                    from_address,
                    to_address,
                    direction: direction.to_owned(),
                    message_type: Some(message_type.clone()),
                    text: event.summary.clone(),
                    media_id: event
                        .attachments
                        .first()
                        .map(|attachment| attachment.reference_id.clone()),
                    sent_at: Some(event.occurred_at.clone()),
                    raw: raw.clone(),
                })
                .await
            {
                let _ = self
                    .runtime
                    .error_sink()
                    .record(ServiceErrorRecord {
                        domain: "whatsapp".into(),
                        operation: "whatsapp.land".into(),
                        code: "WHATSAPP_LAND_FAILED".into(),
                        message: error.to_string(),
                        retryable: error.retryable,
                        stack: None,
                        correlation_id: format!("whatsapp:{}", event.external_event_id),
                        causation_id: None,
                        actor_id: None,
                        severity: ServiceFailureSeverity::Warning,
                    })
                    .await;
            }

            let outcome = self
                .repository
                .process_event(&WhatsAppCanonicalInput {
                    source_account: event.source_account.clone(),
                    external_event_id: event.external_event_id.clone(),
                    event_type: event.event_type.clone(),
                    occurred_at: event.occurred_at.clone(),
                    observed_at: event.observed_at.clone(),
                    direction: direction.to_owned(),
                    external_phone_e164: event.external_phone_e164.clone(),
                    external_display_name: event.external_display_name.clone(),
                    thread_id: event.thread_id.clone(),
                    summary: event.summary.clone(),
                    message_type,
                    attachment_metadata: serde_json::to_value(&event.attachments)
                        .unwrap_or_else(|_| serde_json::json!([])),
                })
                .await?;

            let mapped = match outcome {
                WhatsAppProcessOutcome::Completed {
                    person_id,
                    interaction_id,
                    ..
                } => {
                    relationship_projected += 1;
                    WhatsAppWebhookOutcome {
                        outcome: "completed".into(),
                        resolved_person_id: Some(person_id),
                        interaction_id: Some(interaction_id),
                    }
                }
                WhatsAppProcessOutcome::Duplicate { interaction_id } => WhatsAppWebhookOutcome {
                    outcome: "duplicate".into(),
                    resolved_person_id: None,
                    interaction_id,
                },
                WhatsAppProcessOutcome::ResolutionRequired => WhatsAppWebhookOutcome {
                    outcome: "resolution_required".into(),
                    resolved_person_id: None,
                    interaction_id: None,
                },
                WhatsAppProcessOutcome::Rejected => WhatsAppWebhookOutcome {
                    outcome: "rejected".into(),
                    resolved_person_id: None,
                    interaction_id: None,
                },
                WhatsAppProcessOutcome::InFlight => WhatsAppWebhookOutcome {
                    outcome: "in_flight".into(),
                    resolved_person_id: None,
                    interaction_id: None,
                },
                WhatsAppProcessOutcome::FailedRetryable { .. } => {
                    retryable_failure = true;
                    WhatsAppWebhookOutcome {
                        outcome: "failed_retryable".into(),
                        resolved_person_id: None,
                        interaction_id: None,
                    }
                }
                WhatsAppProcessOutcome::Poisoned { .. } => WhatsAppWebhookOutcome {
                    outcome: "poisoned".into(),
                    resolved_person_id: None,
                    interaction_id: None,
                },
            };
            outcomes.push(mapped);
        }

        if relationship_projected > 0 {
            // The canonical writes already committed. A refresh failure must
            // not cause Meta to replay a successfully processed webhook — but
            // it is RECORDED through the service error sink.
            if let Err(error) = self.repository.refresh_client_read_models().await {
                let _ = self
                    .runtime
                    .error_sink()
                    .record(ServiceErrorRecord {
                        domain: "whatsapp".into(),
                        operation: "whatsapp.refresh".into(),
                        code: "WHATSAPP_REFRESH_FAILED".into(),
                        message: error.to_string(),
                        retryable: error.retryable,
                        stack: None,
                        correlation_id: "whatsapp:refresh".into(),
                        causation_id: None,
                        actor_id: None,
                        severity: ServiceFailureSeverity::Warning,
                    })
                    .await;
            }
        }

        Ok(WhatsAppWebhookResult {
            accepted: outcomes.len(),
            relationship_projected,
            retryable_failure,
            outcomes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use db::DbFailure;
    use services::{
        CapturingAuditPort, CapturingDomainEventPort, CapturingServiceErrorSink,
        DefaultAuthorizationPort,
    };
    use std::sync::Arc;

    struct FakeRepo {
        land_error: bool,
        process_error: bool,
    }

    #[async_trait]
    impl WhatsAppRepository for FakeRepo {
        async fn land(&self, _input: &WhatsAppLandingInput) -> DbResult<bool> {
            if self.land_error {
                Err(DbFailure::configuration("whatsapp.land", "landing down"))
            } else {
                Ok(true)
            }
        }

        async fn process_event(
            &self,
            _input: &WhatsAppCanonicalInput,
        ) -> DbResult<WhatsAppProcessOutcome> {
            if self.process_error {
                Err(DbFailure::configuration("whatsapp.process", "process down"))
            } else {
                Ok(WhatsAppProcessOutcome::Duplicate {
                    interaction_id: None,
                })
            }
        }

        async fn refresh_client_read_models(&self) -> DbResult<()> {
            Ok(())
        }
    }

    fn test_env() {
        std::env::set_var("WHATSAPP_APP_SECRET", "fixture-app-secret");
        std::env::set_var("WHATSAPP_PHONE_NUMBER_ID", "12345");
        std::env::set_var("WHATSAPP_OWNED_PHONE_E164", "+17875550000");
        std::env::set_var("WHATSAPP_VERIFY_TOKEN", "fixture-verify-token");
    }

    fn infrastructure(sink: Arc<CapturingServiceErrorSink>) -> ServiceInfrastructure {
        ServiceInfrastructure::new(
            Arc::new(DefaultAuthorizationPort),
            Arc::new(CapturingAuditPort::default()),
            Arc::new(CapturingDomainEventPort::default()),
        )
        .with_error_sink(sink)
    }

    fn body() -> &'static str {
        r#"{
          "object":"whatsapp_business_account",
          "entry":[{
            "changes":[{
              "value":{
                "messaging_product":"whatsapp",
                "metadata":{"phone_number_id":"12345"},
                "contacts":[{"wa_id":"17875551212","profile":{"name":"Ami"}}],
                "messages":[{
                  "from":"17875551212",
                  "id":"wamid.fixture.1",
                  "timestamp":"1780000000",
                  "type":"text",
                  "text":{"body":"Hello"}
                }]
              }
            }]
          }]
        }"#
    }

    fn signature(raw: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let mut mac = Hmac::<Sha256>::new_from_slice(b"fixture-app-secret").expect("hmac key fits");
        mac.update(raw.as_bytes());
        let digest = mac.finalize().into_bytes();
        let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        format!("sha256={hex}")
    }

    #[tokio::test]
    async fn process_failure_keeps_its_database_identity() {
        test_env();
        let sink = Arc::new(CapturingServiceErrorSink::default());
        let service = WhatsAppService::new(
            FakeRepo {
                land_error: false,
                process_error: true,
            },
            infrastructure(sink),
        );
        let raw = body();
        let error = service
            .handle_webhook(raw, Some(&signature(raw)))
            .await
            .expect_err("process failure must fail the webhook");
        match error {
            CoreServiceError::Database(failure) => {
                assert_eq!(failure.operation, "whatsapp.process");
            }
            other => panic!("expected the database failure, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn landing_failure_is_recorded_but_processing_continues() {
        test_env();
        let sink = Arc::new(CapturingServiceErrorSink::default());
        let service = WhatsAppService::new(
            FakeRepo {
                land_error: true,
                process_error: false,
            },
            infrastructure(sink.clone()),
        );
        let raw = body();
        let result = service
            .handle_webhook(raw, Some(&signature(raw)))
            .await
            .expect("landing failure must not fail the webhook");
        assert_eq!(result.accepted, 1);
        let records = sink.records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].code, "WHATSAPP_LAND_FAILED");
    }
}
