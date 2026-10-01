use crate::mq_runtime::{MqSubscriber, MqSubscriberError};
use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, DbTransaction, EmailDao, OutboxDelivery};
use domain::{
    EmailMessage, EmailMessageKind, EmailMessageStatus, EmailQueueResult, QueueEmailRequest,
};
use integrations::mail::{MailConfig, MailError, OutgoingMail, SmtpMailer};
use service::{
    AbstractService, OperationKind, ServiceCapability, ServiceContext, ServiceDescriptor,
    ServiceDispatchError, ServiceEnvelope, ServiceExecutionPolicy, ServiceInfrastructure,
    ServiceRuntime,
};
use std::sync::Arc;

pub const EMAIL_DELIVERY_ROUTING_KEY: &str = "email.delivery.requested";
pub const EMAIL_DELIVERY_SUBSCRIPTION_ID: &str = "email-delivery";
pub const EMAIL_DELIVERY_ACTOR: &str = "email-delivery-worker";

#[async_trait]
pub trait EmailRepository: Send + Sync {
    async fn get(&self, id: &str) -> DbResult<Option<EmailMessage>>;
    async fn queue_tx(
        &self,
        tx: &mut DbTransaction,
        request: &QueueEmailRequest,
    ) -> DbResult<EmailQueueResult>;
    async fn claim_for_delivery(&self, id: &str) -> DbResult<Option<EmailMessage>>;
    async fn mark_sent(&self, id: &str, provider_message_id: Option<&str>) -> DbResult<()>;
    async fn mark_failed(&self, id: &str, error_message: &str, dead: bool) -> DbResult<()>;
}

#[async_trait]
impl EmailRepository for EmailDao {
    async fn get(&self, id: &str) -> DbResult<Option<EmailMessage>> {
        EmailDao::get(self, id).await
    }

    async fn queue_tx(
        &self,
        tx: &mut DbTransaction,
        request: &QueueEmailRequest,
    ) -> DbResult<EmailQueueResult> {
        EmailDao::queue_tx(self, tx, request).await
    }

    async fn claim_for_delivery(&self, id: &str) -> DbResult<Option<EmailMessage>> {
        EmailDao::claim_for_delivery(self, id).await
    }

    async fn mark_sent(&self, id: &str, provider_message_id: Option<&str>) -> DbResult<()> {
        EmailDao::mark_sent(self, id, provider_message_id).await
    }

    async fn mark_failed(&self, id: &str, error_message: &str, dead: bool) -> DbResult<()> {
        EmailDao::mark_failed(self, id, error_message, dead).await
    }
}

#[async_trait]
pub trait EmailTransport: Send + Sync {
    async fn send(&self, mail: OutgoingMail) -> Result<(), MailError>;
}

#[async_trait]
impl EmailTransport for SmtpMailer {
    async fn send(&self, mail: OutgoingMail) -> Result<(), MailError> {
        SmtpMailer::send(self, &mail).await
    }
}

pub struct EmailService<R> {
    repository: R,
    transport: Option<Arc<dyn EmailTransport>>,
    runtime: ServiceRuntime,
}

impl<R: EmailRepository> EmailService<R> {
    pub fn new(
        repository: R,
        transport: Option<Arc<dyn EmailTransport>>,
        infrastructure: ServiceInfrastructure,
    ) -> Self {
        Self {
            repository,
            transport,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn get(
        &self,
        id: &str,
        context: &ServiceContext,
    ) -> Result<Option<EmailMessage>, CoreServiceError> {
        const OP: &str = "email.get";
        let decision = authorize(
            &self.runtime,
            "email",
            "email.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self.repository.get(id).await.map_err(Into::into);
        audit_result(&self.runtime, "email", OP, context, decision, &result).await?;
        result
    }

    pub async fn queue_transactional(
        &self,
        tx: &mut DbTransaction,
        request: &QueueEmailRequest,
        context: &ServiceContext,
    ) -> Result<EmailQueueResult, CoreServiceError> {
        const OP: &str = "email.queue";
        let decision = authorize(
            &self.runtime,
            "email",
            "email.queue",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = if request.recipient_email.trim().is_empty()
            || request.template_key.trim().is_empty()
            || request.dedupe_key.trim().is_empty()
            || !request.template_payload.is_object()
        {
            Err(CoreServiceError::business(
                "EMAIL_TEMPLATE_INVALID",
                "Email recipient, templateKey, dedupeKey and object templatePayload are required.",
            ))
        } else {
            self.repository.queue_tx(tx, request).await.map_err(Into::into)
        };
        audit_result(&self.runtime, "email", OP, context, decision, &result).await?;
        result
    }

    pub async fn deliver(
        &self,
        message_id: &str,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        const OP: &str = "email.deliver";
        let decision = authorize(
            &self.runtime,
            "email",
            "email.deliver",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;

        let result = async {
            let Some(message) = self.repository.claim_for_delivery(message_id).await? else {
                return Err(CoreServiceError::business(
                    "EMAIL_NOT_FOUND",
                    "Queued email message was not found.",
                ));
            };
            if matches!(
                message.status,
                EmailMessageStatus::Sent | EmailMessageStatus::Dead | EmailMessageStatus::Cancelled
            ) {
                return Ok(());
            }

            let transport = self.transport.as_ref().ok_or_else(|| {
                CoreServiceError::business(
                    "EMAIL_NOT_CONFIGURED",
                    "Transactional email transport is not configured.",
                )
            })?;
            let mail = render_email(&message)?;
            if let Err(error) = transport.send(mail).await {
                self.repository
                    .mark_failed(&message.id, &error.to_string(), false)
                    .await?;
                return Err(CoreServiceError::business(
                    "EMAIL_DELIVERY_FAILED",
                    error.to_string(),
                ));
            }
            self.repository.mark_sent(&message.id, None).await?;
            Ok(())
        }
        .await;

        audit_result(&self.runtime, "email", OP, context, decision, &result).await?;
        result
    }
}

fn capability(
    name: &str,
    kind: OperationKind,
    description: &str,
    authorization: &str,
    idempotent: bool,
    execution: ServiceExecutionPolicy,
) -> ServiceCapability {
    ServiceCapability {
        name: name.into(),
        kind,
        description: description.into(),
        authorization: authorization.into(),
        idempotent,
        execution,
    }
}

#[async_trait]
impl<R: EmailRepository + 'static> AbstractService for EmailService<R> {
    fn descriptor(&self) -> ServiceDescriptor {
        ServiceDescriptor {
            domain: "email".into(),
            version: "1".into(),
            description: "Durable transactional email service".into(),
            capabilities: vec![
                capability(
                    "email.get",
                    OperationKind::Query,
                    "Read durable transactional email state.",
                    "email.read",
                    true,
                    ServiceExecutionPolicy::inline(),
                ),
                capability(
                    "email.queue",
                    OperationKind::Command,
                    "Queue transactional email through the durable command runtime.",
                    "email.queue",
                    true,
                    ServiceExecutionPolicy::ordered("dedupeKey"),
                ),
                capability(
                    "email.deliver",
                    OperationKind::Command,
                    "Deliver one queued message from the MQ worker.",
                    "email.deliver",
                    true,
                    ServiceExecutionPolicy::ordered("messageId"),
                ),
            ],
            dependencies: vec![],
            invariants: vec![
                "Business services queue durable messages; SMTP is never called inside their transaction.".into(),
                "Delivery retries use the existing MQ lease/retry boundary.".into(),
            ],
        }
    }

    async fn dispatch(
        &self,
        envelope: &ServiceEnvelope,
        context: &ServiceContext,
    ) -> Result<serde_json::Value, ServiceDispatchError> {
        match envelope.operation.as_str() {
            "email.get" => {
                let id = envelope
                    .payload
                    .get("messageId")
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| ServiceDispatchError::InvalidPayload {
                        domain: envelope.domain.clone(),
                        operation: envelope.operation.clone(),
                        message: "messageId is required.".into(),
                    })?;
                serde_json::to_value(
                    self.get(id, context)
                        .await
                        .map_err(service_error)?,
                )
                .map_err(serialization_error)
            }
            "email.queue" | "email.deliver" => Err(ServiceDispatchError::business(
                "DURABLE_COMMAND_REQUIRED",
                format!(
                    "{} must enter through the durable command/MQ dispatcher.",
                    envelope.operation
                ),
                false,
            )),
            operation => Err(ServiceDispatchError::UnknownOperation {
                domain: "email".into(),
                operation: operation.into(),
            }),
        }
    }
}

fn render_email(message: &EmailMessage) -> Result<OutgoingMail, CoreServiceError> {
    let string = |key: &str| {
        message
            .template_payload
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };

    let subject = string("subject").unwrap_or_else(|| match message.message_kind {
        EmailMessageKind::SignatureInvitation => "Document ready for your signature — CulebraLuxe".into(),
        EmailMessageKind::SignatureReminder => "Reminder: document waiting for your signature — CulebraLuxe".into(),
        EmailMessageKind::SignatureCompleted => "Document signing completed — CulebraLuxe".into(),
        EmailMessageKind::SignatureDeclined => "Document signing declined — CulebraLuxe".into(),
    });

    let text = match message.message_kind {
        EmailMessageKind::SignatureInvitation | EmailMessageKind::SignatureReminder => {
            let name = string("recipientName").unwrap_or_else(|| "there".into());
            let url = string("signingUrl").ok_or_else(|| {
                CoreServiceError::business(
                    "EMAIL_TEMPLATE_INVALID",
                    "Signature invitation requires signingUrl.",
                )
            })?;
            let note = string("message")
                .map(|value| format!("\n\nMessage from CulebraLuxe:\n{value}"))
                .unwrap_or_default();
            format!(
                "Hello {name},\n\nA CulebraLuxe document is ready for you.\n\nOpen and review it here:\n{url}{note}\n\nThis link is unique to you. Do not forward it."
            )
        }
        EmailMessageKind::SignatureCompleted => {
            "The CulebraLuxe document signing process has completed.".into()
        }
        EmailMessageKind::SignatureDeclined => {
            "A recipient declined the CulebraLuxe document signing request.".into()
        }
    };

    Ok(OutgoingMail {
        to: message.recipient_email.clone(),
        subject,
        text,
        html: None,
        reply_to: None,
    })
}

pub fn transport_from_env() -> Option<Arc<dyn EmailTransport>> {
    let config = MailConfig::from_env().ok()?;
    let mailer = SmtpMailer::new(config).ok()?;
    Some(Arc::new(mailer))
}

pub struct EmailDeliverySubscriber<R> {
    service: Arc<EmailService<R>>,
}

impl<R> EmailDeliverySubscriber<R> {
    pub fn new(service: Arc<EmailService<R>>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl<R: EmailRepository + 'static> MqSubscriber for EmailDeliverySubscriber<R> {
    fn id(&self) -> &str {
        EMAIL_DELIVERY_SUBSCRIPTION_ID
    }

    fn routing_key(&self) -> &str {
        EMAIL_DELIVERY_ROUTING_KEY
    }

    fn max_attempts(&self) -> i32 {
        5
    }

    fn retry_backoff_seconds(&self) -> i32 {
        30
    }

    async fn handle(&self, delivery: &OutboxDelivery) -> Result<(), MqSubscriberError> {
        let message_id = delivery
            .payload
            .get("messageId")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                MqSubscriberError::new("email.delivery.requested requires messageId.")
            })?;
        let context = ServiceContext {
            actor: service::ServiceActor {
                id: Some(EMAIL_DELIVERY_ACTOR.into()),
                kind: service::ServiceActorKind::System,
            },
            correlation_id: delivery
                .correlation_id
                .clone()
                .unwrap_or_else(|| delivery.event_id.clone()),
            causation_id: Some(delivery.event_id.clone()),
            principal: None,
        };
        self.service
            .deliver(message_id, &context)
            .await
            .map_err(|error| MqSubscriberError::new(error.to_string()))
    }
}

fn service_error(error: CoreServiceError) -> ServiceDispatchError {
    match error {
        CoreServiceError::Business { code, message } => {
            ServiceDispatchError::business(code, message, false)
        }
        CoreServiceError::Database(error) => {
            ServiceDispatchError::infrastructure("DATABASE", error.to_string(), error.retryable)
        }
        CoreServiceError::Runtime(error) => ServiceDispatchError::infrastructure(
            "SERVICE_RUNTIME",
            error.to_string(),
            true,
        ),
    }
}

fn serialization_error(error: serde_json::Error) -> ServiceDispatchError {
    ServiceDispatchError::infrastructure(
        "SERVICE_SERIALIZATION_FAILED",
        error.to_string(),
        false,
    )
}


#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn message(payload: serde_json::Value) -> EmailMessage {
        let now = Utc::now().to_rfc3339();
        EmailMessage {
            id: "11111111-1111-4111-8111-111111111111".into(),
            message_kind: EmailMessageKind::SignatureInvitation,
            recipient_email: "buyer@example.test".into(),
            template_key: "document-sign.invitation".into(),
            template_payload: payload,
            dedupe_key: "invite-1".into(),
            status: EmailMessageStatus::Queued,
            provider_message_id: None,
            attempt_count: 0,
            last_error: None,
            correlation_id: Some("corr-1".into()),
            causation_id: None,
            queued_at: now.clone(),
            sent_at: None,
            updated_at: now,
        }
    }

    #[test]
    fn invitation_renderer_requires_the_recipient_signing_url() {
        let error = render_email(&message(serde_json::json!({
            "recipientName": "Buyer"
        })))
        .unwrap_err();
        assert_eq!(error.code(), "EMAIL_TEMPLATE_INVALID");

        let mail = render_email(&message(serde_json::json!({
            "recipientName": "Buyer",
            "signingUrl": "https://example.test/sign/token"
        })))
        .unwrap();
        assert_eq!(mail.to, "buyer@example.test");
        assert!(mail.text.contains("https://example.test/sign/token"));
        assert!(mail.text.contains("Do not forward"));
    }
}
