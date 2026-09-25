use async_trait::async_trait;
use db::{
    DbResult, WhatsAppCanonicalInput, WhatsAppDao, WhatsAppLandingInput,
    WhatsAppProcessOutcome,
};
use integrations::whatsapp::{
    parse_webhook, verify_handshake, verify_signature, MetaWhatsAppConfig,
    WhatsAppDirection,
};
use serde::Serialize;
use serde_json::Value;

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
    async fn land(&mut self, input: &WhatsAppLandingInput) -> DbResult<bool>;
    async fn process_event(
        &mut self,
        input: &WhatsAppCanonicalInput,
    ) -> DbResult<WhatsAppProcessOutcome>;
    async fn refresh_client_read_models(&mut self) -> DbResult<()>;
}

#[async_trait]
impl WhatsAppRepository for WhatsAppDao {
    async fn land(&mut self, input: &WhatsAppLandingInput) -> DbResult<bool> {
        WhatsAppDao::land(self, input).await
    }

    async fn process_event(
        &mut self,
        input: &WhatsAppCanonicalInput,
    ) -> DbResult<WhatsAppProcessOutcome> {
        WhatsAppDao::process_event(self, input).await
    }

    async fn refresh_client_read_models(&mut self) -> DbResult<()> {
        WhatsAppDao::refresh_client_read_models(self).await
    }
}

pub struct WhatsAppService<R> {
    repository: R,
}

impl<R: WhatsAppRepository> WhatsAppService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn verify_handshake(
        &self,
        mode: Option<&str>,
        token: Option<&str>,
        challenge: Option<&str>,
    ) -> Result<Option<String>, String> {
        let config = MetaWhatsAppConfig::from_env()?;
        Ok(verify_handshake(mode, token, challenge, &config.verify_token))
    }

    pub async fn handle_webhook(
        &mut self,
        raw_body: &str,
        signature: Option<&str>,
    ) -> Result<WhatsAppWebhookResult, String> {
        let config = MetaWhatsAppConfig::from_env()?;
        if !verify_signature(raw_body, signature, &config.app_secret) {
            return Err("WHATSAPP_SIGNATURE_INVALID".into());
        }

        let raw: Value =
            serde_json::from_str(raw_body).map_err(|_| "WHATSAPP_PAYLOAD_INVALID".to_owned())?;
        let events = parse_webhook(raw_body, &config, chrono::Utc::now())
            .map_err(|_| "WHATSAPP_PAYLOAD_INVALID".to_owned())?;

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

            // Landing is intentionally best-effort, preserving the production
            // invariant that a landing-table failure must not prevent canonical
            // CRM processing of an authenticated provider event.
            let _ = self
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
                .await;

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
                .await
                .map_err(|error| format!("WHATSAPP_DB:{}", error.operation))?;

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
            // The canonical writes already committed. A refresh failure must not
            // cause Meta to replay a successfully processed webhook.
            let _ = self.repository.refresh_client_read_models().await;
        }

        Ok(WhatsAppWebhookResult {
            accepted: outcomes.len(),
            relationship_projected,
            retryable_failure,
            outcomes,
        })
    }
}
