use crate::mq_runtime::{MqSubscriber, MqSubscriberError};
use crate::service_support::{audit_result, authorize, CoreServiceError};
use apis::mail::{MailAttachment, MailConfig, MailError, OutgoingMail, SmtpMailer};
use async_trait::async_trait;
use db::{DbResult, DbTransaction, EmailDao, OutboxDelivery};
use model::{
    EmailMessage, EmailMessageKind, EmailMessageStatus, EmailQueueResult, QueueEmailRequest,
};
use services::{
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

/// Where a completion email's files come from. The production implementation reads them through the Vault's own door
/// (`VaultService::completion_artifacts`); this port keeps the email service from knowing about the Vault.
#[async_trait]
pub trait EmailAttachmentSource: Send + Sync {
    async fn completion_artifacts(
        &self,
        signature_request_id: &str,
        context: &ServiceContext,
    ) -> Result<Vec<MailAttachment>, CoreServiceError>;
}

pub struct EmailService<R> {
    repository: R,
    attachments: Option<Arc<dyn EmailAttachmentSource>>,
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
            attachments: None,
            transport,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    /// Give the service the means to attach a completed envelope's files (see [`EmailAttachmentSource`]).
    pub fn with_attachment_source(mut self, source: Arc<dyn EmailAttachmentSource>) -> Self {
        self.attachments = Some(source);
        self
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
            self.repository
                .queue_tx(tx, request)
                .await
                .map_err(Into::into)
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
            let mut mail = render_email(&message)?;
            let wants_files = message
                .template_payload
                .get("attachCompletion")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            if wants_files {
                let request_id = message
                    .template_payload
                    .get("signatureRequestId")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                let source = self.attachments.as_ref().ok_or_else(|| {
                    CoreServiceError::business(
                        "EMAIL_ATTACHMENTS_UNAVAILABLE",
                        "This message carries the signed document, but no attachment source is configured.",
                    )
                })?;
                // A failure here leaves the message queued for the MQ retry: a completion email that silently lost
                // its document would be worse than a late one.
                mail.attachments = source.completion_artifacts(request_id, context).await?;
            }
            if let Err(error) = transport.send(mail).await {
                self.repository
                    .mark_failed(&message.id, &error.to_string(), false)
                    .await?;
                return Err(CoreServiceError::infrastructure(
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
                serde_json::to_value(self.get(id, context).await.map_err(service_error)?)
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
        EmailMessageKind::SignatureInvitation => {
            "Document ready for your signature — CulebraLuxe".into()
        }
        EmailMessageKind::SignatureReminder => {
            "Reminder: document waiting for your signature — CulebraLuxe".into()
        }
        EmailMessageKind::SignatureCompleted => match string("documentTitle") {
            Some(title) => format!("Signed: {title} — CulebraLuxe"),
            None => "Document signing completed — CulebraLuxe".into(),
        },
        EmailMessageKind::SignatureDeclined => match string("documentTitle") {
            Some(title) => format!("Declined: {title} — CulebraLuxe"),
            None => "Document signing declined — CulebraLuxe".into(),
        },
    });

    let (text, html) = match message.message_kind {
        EmailMessageKind::SignatureInvitation | EmailMessageKind::SignatureReminder => {
            let name = string("recipientName").unwrap_or_else(|| "there".into());
            let url = string("signingUrl").ok_or_else(|| {
                CoreServiceError::business(
                    "EMAIL_TEMPLATE_INVALID",
                    "Signature invitation requires signingUrl.",
                )
            })?;
            let note = string("message");
            let reminder = message.message_kind == EmailMessageKind::SignatureReminder;
            let lead = if reminder {
                "A CulebraLuxe document is still waiting for your signature."
            } else {
                "A CulebraLuxe document is ready for you to review and sign."
            };
            let note_text = note
                .as_deref()
                .map(|value| format!("\n\nMessage from CulebraLuxe:\n{value}"))
                .unwrap_or_default();
            let text = format!(
                "Hello {name},\n\n{lead}\n\nOpen and review it here:\n{url}{note_text}\n\nThis link is unique to you. Do not forward it."
            );
            let html = signature_request_html(&name, lead, &url, note.as_deref());
            (text, Some(html))
        }
        EmailMessageKind::SignatureCompleted => {
            let name = string("recipientName");
            let title = string("documentTitle");
            let signers: Vec<String> = message
                .template_payload
                .get("signers")
                .and_then(serde_json::Value::as_array)
                .map(|names| {
                    names
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            let attached = message
                .template_payload
                .get("attachCompletion")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let lead = match &title {
                Some(title) => format!("{title} has been signed by everyone and is complete."),
                None => "The document has been signed by everyone and is complete.".to_owned(),
            };
            let detail = if signers.is_empty() {
                String::new()
            } else {
                format!("Signed by: {}.", signers.join(", "))
            };
            let files = if attached {
                "The signed document and its certificate of completion are attached. Keep both for your records."
            } else {
                "Your CulebraLuxe contact has the signed document and its certificate of completion."
            };
            let greeting = greeting_for(name.as_deref());
            let text = format!("{greeting}\n\n{lead}\n{detail}\n\n{files}\n\nCulebraLuxe");
            let html = branded_html(&Branded {
                heading: &greeting,
                paragraphs: &[&lead, &detail, files],
                note: None,
                button: None,
                footer: "This is a record of a completed signing. Please keep the attached files.",
            });
            (text, Some(html))
        }
        EmailMessageKind::SignatureDeclined => {
            let name = string("recipientName");
            let title = string("documentTitle");
            let decliner = string("declinerName").unwrap_or_else(|| "A signer".into());
            let reason = string("reason");
            let lead = match &title {
                Some(title) => format!("{decliner} declined to sign {title}."),
                None => format!("{decliner} declined to sign the document."),
            };
            let outcome = "The signing has ended and the document will not be completed. No one else needs to sign it.";
            let greeting = greeting_for(name.as_deref());
            let reason_text = reason
                .as_deref()
                .map(|reason| format!("\n\nTheir reason:\n{reason}"))
                .unwrap_or_default();
            let text = format!("{greeting}\n\n{lead}{reason_text}\n\n{outcome}\n\nCulebraLuxe");
            let html = branded_html(&Branded {
                heading: &greeting,
                paragraphs: &[&lead, outcome],
                note: reason.as_deref(),
                button: None,
                footer: "If you think this was a mistake, contact your CulebraLuxe representative to start again.",
            });
            (text, Some(html))
        }
    };

    Ok(OutgoingMail {
        to: message.recipient_email.clone(),
        subject,
        text,
        html,
        reply_to: None,

        attachments: Vec::new(),
    })
}

fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

fn greeting_for(name: Option<&str>) -> String {
    match name.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => format!("Hello {name},"),
        None => "Hello,".to_owned(),
    }
}

/// What the branded layout shows. Every string is escaped on the way in; the button link only if it is http(s).
struct Branded<'a> {
    heading: &'a str,
    paragraphs: &'a [&'a str],
    /// A quoted block (the sender's message, a decliner's reason).
    note: Option<&'a str>,
    button: Option<(&'a str, &'a str)>,
    footer: &'a str,
}

/// The site's public origin, for the logo (`PUBLIC_SITE_URL`, else the production host). Mail clients cannot read a
/// relative path, so the logo is an absolute link to the site's own copy.
fn site_origin() -> String {
    std::env::var("PUBLIC_SITE_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_owned())
        .filter(|value| value.starts_with("https://") || value.starts_with("http://"))
        .unwrap_or_else(|| "https://www.culebraluxe.com".into())
}

// The house colours: navy ground, gold accents, ivory type. The logo image carries its own navy (about #021429), so
// the header band is that colour and the picture sits in it without a visible edge.
const NAVY: &str = "#021429";
/// The page behind the card: the site's tan. The card itself is one navy (the logo's own), so there is no seam.
const TAN: &str = "#f4f1ea";
const GOLD: &str = "#c6a15b";
const IVORY: &str = "#f3ecd9";
const TEXT: &str = "#d9d4c5";
const MUTED: &str = "#8f98ab";

/// The branded layout every signing email shares: navy and gold, the CulebraLuxe logo on top. Inline styles and a
/// table layout only: mail clients ignore the rest.
fn branded_html(mail: &Branded<'_>) -> String {
    let paragraphs: String = mail
        .paragraphs
        .iter()
        .filter(|text| !text.trim().is_empty())
        .map(|text| {
            format!(
                r#"<tr><td style="padding:0 40px 14px;font-family:Helvetica,Arial,sans-serif;font-size:15px;line-height:24px;color:{TEXT}">{}</td></tr>"#,
                escape_html(text)
            )
        })
        .collect();
    let note = mail
        .note
        .map(|value| {
            format!(
                r#"<tr><td style="padding:6px 40px 24px"><div style="border-left:3px solid {GOLD};padding:4px 0 4px 14px;color:{IVORY};font-family:Georgia,'Times New Roman',serif;font-size:15px;line-height:23px;white-space:pre-wrap">{}</div></td></tr>"#,
                escape_html(value)
            )
        })
        .unwrap_or_default();
    let button = mail
        .button
        .map(|(label, url)| {
            let safe_url = if url.starts_with("https://") || url.starts_with("http://") {
                escape_html(url)
            } else {
                "#".into()
            };
            format!(
                r#"<tr><td style="padding:10px 40px 28px"><a href="{safe_url}" style="display:inline-block;background:{GOLD};color:{NAVY};text-decoration:none;font-family:Helvetica,Arial,sans-serif;font-size:13px;font-weight:bold;letter-spacing:1.6px;text-transform:uppercase;padding:15px 34px;border-radius:4px">{}</a></td></tr>
<tr><td style="padding:0 40px 12px;font-family:Helvetica,Arial,sans-serif;font-size:12px;line-height:19px;color:{MUTED}">If the button does not work, paste this link into your browser:<br><a href="{safe_url}" style="color:{GOLD};word-break:break-all">{safe_url}</a></td></tr>"#,
                escape_html(label)
            )
        })
        .unwrap_or_default();
    let logo = escape_html(&format!(
        "{}/images/culebraluxe-email-logo.png",
        site_origin()
    ));
    format!(
        r##"<!doctype html>
<html><head><meta name="color-scheme" content="light"><meta name="supported-color-schemes" content="light"></head>
<body style="margin:0;padding:0;background:{TAN}">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background:{TAN}"><tr><td align="center" style="padding:32px 12px">
<table role="presentation" width="600" cellpadding="0" cellspacing="0" style="max-width:600px;width:100%;background:{NAVY};border-radius:6px;font-family:Georgia,'Times New Roman',serif;color:{IVORY}">
<tr><td align="center" style="background:{NAVY};padding:26px 24px 22px"><img src="{logo}" alt="CulebraLuxe" width="300" style="display:block;border:0;outline:none;width:300px;max-width:100%;height:auto"></td></tr>
<tr><td style="height:2px;line-height:2px;font-size:0;background:{GOLD}">&nbsp;</td></tr>
<tr><td style="padding:34px 40px 8px;font-family:Helvetica,Arial,sans-serif;font-size:11px;letter-spacing:2.6px;text-transform:uppercase;color:{GOLD}">Luxesign</td></tr>
<tr><td style="padding:0 40px 16px;font-size:27px;line-height:34px;font-weight:300;color:{IVORY}">{heading}</td></tr>
{paragraphs}{note}{button}
<tr><td style="padding:14px 40px 30px;border-top:1px solid #1d2c47;font-family:Helvetica,Arial,sans-serif;font-size:12px;line-height:19px;color:{MUTED}">{footer}<br>Sent with <span style="color:{GOLD};letter-spacing:1.4px">LUXESIGN</span> &middot; <span style="color:{GOLD};letter-spacing:1.4px">CULEBRALUXE</span> &middot; Culebra, Puerto Rico</td></tr>
</table></td></tr></table></body></html>"##,
        heading = escape_html(mail.heading),
        footer = escape_html(mail.footer),
    )
}

/// The branded invitation / reminder.
fn signature_request_html(name: &str, lead: &str, url: &str, note: Option<&str>) -> String {
    let heading = greeting_for(Some(name));
    branded_html(&Branded {
        heading: &heading,
        paragraphs: &[lead],
        note,
        button: Some(("Review and sign", url)),
        footer: "This link is unique to you. Please do not forward it.",
    })
}

/// The production attachment source: the Vault's own door, so the email service never reads a document itself.
pub struct VaultAttachmentSource<VR: crate::vault::VaultRepository> {
    vault: Arc<crate::vault::VaultService<VR>>,
}

impl<VR: crate::vault::VaultRepository> VaultAttachmentSource<VR> {
    pub fn new(vault: Arc<crate::vault::VaultService<VR>>) -> Self {
        Self { vault }
    }
}

#[async_trait]
impl<VR: crate::vault::VaultRepository + 'static> EmailAttachmentSource
    for VaultAttachmentSource<VR>
{
    async fn completion_artifacts(
        &self,
        signature_request_id: &str,
        context: &ServiceContext,
    ) -> Result<Vec<MailAttachment>, CoreServiceError> {
        Ok(self
            .vault
            .completion_artifacts(signature_request_id, context)
            .await?
            .into_iter()
            .map(|media| MailAttachment {
                filename: media.filename,
                content_type: media.mime_type,
                bytes: media.bytes,
            })
            .collect())
    }
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
            actor: services::ServiceActor {
                id: Some(EMAIL_DELIVERY_ACTOR.into()),
                kind: services::ServiceActorKind::System,
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
        CoreServiceError::Runtime(error) => {
            ServiceDispatchError::infrastructure("SERVICE_RUNTIME", error.to_string(), true)
        }
    }
}

fn serialization_error(error: serde_json::Error) -> ServiceDispatchError {
    ServiceDispatchError::infrastructure("SERVICE_SERIALIZATION_FAILED", error.to_string(), false)
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
        let html = mail.html.expect("the invitation carries an HTML part");
        assert!(html.contains(r#"href="https://example.test/sign/token""#));
        assert!(html.contains("Review and sign"));
    }

    #[test]
    fn the_html_part_escapes_what_a_person_typed_and_refuses_a_non_http_link() {
        let mail = render_email(&message(serde_json::json!({
            "recipientName": "<script>alert(1)</script>",
            "signingUrl": "https://example.test/sign/a\"onmouseover=\"x",
            "message": "Fish & <b>chips</b>"
        })))
        .unwrap();
        let html = mail.html.unwrap();
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("Fish &amp; &lt;b&gt;chips&lt;/b&gt;"));
        assert!(!html.contains(r#"a"onmouseover"#));

        let bad = render_email(&message(serde_json::json!({
            "recipientName": "Buyer",
            "signingUrl": "javascript:alert(1)"
        })))
        .unwrap();
        assert!(!bad.html.unwrap().contains("javascript:"));
    }

    #[test]
    fn a_reminder_says_so() {
        let mut reminder = message(serde_json::json!({
            "recipientName": "Buyer",
            "signingUrl": "https://example.test/sign/token"
        }));
        reminder.message_kind = EmailMessageKind::SignatureReminder;
        let mail = render_email(&reminder).unwrap();
        assert!(mail.text.contains("still waiting"));
    }

    fn outcome(kind: EmailMessageKind, payload: serde_json::Value) -> OutgoingMail {
        let mut message = message(payload);
        message.message_kind = kind;
        render_email(&message).unwrap()
    }

    #[test]
    fn the_completed_notice_names_the_document_and_the_people_and_promises_the_files() {
        let mail = outcome(
            EmailMessageKind::SignatureCompleted,
            serde_json::json!({
                "recipientName": "Maria",
                "documentTitle": "Listing Agreement",
                "signers": ["Maria Alvarez", "Pedro Diaz"],
                "attachCompletion": true,
                "signatureRequestId": "req-1"
            }),
        );
        assert_eq!(mail.subject, "Signed: Listing Agreement — CulebraLuxe");
        assert!(mail
            .text
            .contains("Listing Agreement has been signed by everyone"));
        assert!(mail.text.contains("Maria Alvarez, Pedro Diaz"));
        assert!(mail.text.contains("attached"));
        let html = mail.html.expect("an HTML part");
        assert!(html.contains("Hello Maria,"));
        assert!(
            !html.contains("Review and sign"),
            "a completed notice has no signing button"
        );
    }

    #[test]
    fn a_copy_recipient_without_a_name_is_greeted_plainly() {
        let mail = outcome(
            EmailMessageKind::SignatureCompleted,
            serde_json::json!({ "recipientName": "", "documentTitle": "Deed" }),
        );
        assert!(mail.text.starts_with("Hello,\n"));
        assert!(mail
            .text
            .contains("Your CulebraLuxe contact has the signed document"));
    }

    #[test]
    fn the_declined_notice_says_who_and_why_and_escapes_what_a_person_typed() {
        let mail = outcome(
            EmailMessageKind::SignatureDeclined,
            serde_json::json!({
                "recipientName": "Pedro",
                "documentTitle": "Listing Agreement",
                "declinerName": "Maria <b>Alvarez</b>",
                "reason": "Price & terms <script>x</script>"
            }),
        );
        assert_eq!(mail.subject, "Declined: Listing Agreement — CulebraLuxe");
        assert!(mail.text.contains("declined to sign Listing Agreement"));
        assert!(mail.text.contains("Price & terms"));
        let html = mail.html.unwrap();
        assert!(!html.contains("<script>") && !html.contains("<b>Alvarez"));
        assert!(html.contains("Price &amp; terms &lt;script&gt;"));
    }
}
