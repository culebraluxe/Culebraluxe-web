//! Website lead emails: a notice to the team the moment a lead arrives, and a confirmation to the visitor.
//!
//! THE REQUEST CARRIES ONLY AN ID. The public website asks "notify about submission X"; what the emails say is read
//! from the database here, so the endpoint cannot be used to send anything but a real, recent lead's notice.
//!
//! ONCE PER LEAD. The lead is claimed (stamped `notified_at`) before sending, and released if the team notice fails,
//! so a retry can send it and a duplicate request cannot. If only the visitor's confirmation fails, the claim stands:
//! the team already has the lead, and a second team notice would be worse than a missing receipt.
//!
//! AUTHORIZED AS `website.lead.notify`, a command only the `public-website` system actor may run
//! (security/entitlements.rs).

use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, WebsiteLeadDao};
use domain::{WebsiteLead, WebsiteLeadNotice};
use integrations::mail::{MailConfig, MailError, OutgoingMail, SmtpMailer};
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};
use std::sync::Arc;

#[async_trait]
pub trait WebsiteLeadRepository: Send {
    async fn claim_for_notice(&mut self, submission_id: &str) -> DbResult<Option<WebsiteLead>>;
    async fn release_notice(&mut self, submission_id: &str) -> DbResult<()>;
}

#[async_trait]
impl WebsiteLeadRepository for WebsiteLeadDao {
    // Not retried: the claim writes, and a repeated write after a lost reply would read as "already handled".
    async fn claim_for_notice(&mut self, submission_id: &str) -> DbResult<Option<WebsiteLead>> {
        WebsiteLeadDao::claim_for_notice(self, submission_id).await
    }

    async fn release_notice(&mut self, submission_id: &str) -> DbResult<()> {
        WebsiteLeadDao::release_notice(self, submission_id).await
    }
}

/// Sends one email. The SMTP mailer in production; a recorder in tests.
#[async_trait]
pub trait MailPort: Send + Sync {
    async fn send(&self, mail: OutgoingMail) -> Result<(), MailError>;
}

#[async_trait]
impl MailPort for SmtpMailer {
    async fn send(&self, mail: OutgoingMail) -> Result<(), MailError> {
        SmtpMailer::send(self, &mail).await
    }
}

/// Where the team's notice goes and how links are written.
#[derive(Debug, Clone)]
pub struct LeadMailSettings {
    /// The team inbox (LEAD_NOTIFY_ADDRESS, else the business address the mail is sent from).
    pub team_address: String,
    /// The public site's origin, for property links (PUBLIC_SITE_URL, default https://culebraluxe.com).
    pub site_url: String,
}

impl LeadMailSettings {
    pub fn from_env(mail: &MailConfig) -> Self {
        let value = |key: &str| {
            std::env::var(key)
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        Self {
            team_address: value("LEAD_NOTIFY_ADDRESS").unwrap_or_else(|| mail.from_address.clone()),
            site_url: value("PUBLIC_SITE_URL")
                .unwrap_or_else(|| "https://culebraluxe.com".into())
                .trim_end_matches('/')
                .to_string(),
        }
    }
}

pub struct WebsiteLeadService<R> {
    repository: R,
    mail: Option<(Arc<dyn MailPort>, LeadMailSettings)>,
    runtime: ServiceRuntime,
}

impl<R: WebsiteLeadRepository> WebsiteLeadService<R> {
    /// `mail` is `None` when the mail settings are absent: the service then refuses rather than pretending to send.
    pub fn new(
        repository: R,
        mail: Option<(Arc<dyn MailPort>, LeadMailSettings)>,
        infrastructure: ServiceInfrastructure,
    ) -> Self {
        Self {
            repository,
            mail,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn notify(
        &mut self,
        submission_id: &str,
        context: &ServiceContext,
    ) -> Result<WebsiteLeadNotice, CoreServiceError> {
        const OP: &str = "website.notifyLead";
        let decision = authorize(
            &self.runtime,
            "website",
            "website.lead.notify",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = self.notify_authorized(submission_id).await;
        audit_result(&self.runtime, "website", OP, context, decision, &result).await?;
        result
    }

    async fn notify_authorized(
        &mut self,
        submission_id: &str,
    ) -> Result<WebsiteLeadNotice, CoreServiceError> {
        if uuid::Uuid::parse_str(submission_id).is_err() {
            return Err(CoreServiceError::business(
                "WEBSITE_LEAD_NOT_FOUND",
                "No such website lead.",
            ));
        }
        let Some((mailer, settings)) = self.mail.clone() else {
            return Err(CoreServiceError::business(
                "MAIL_NOT_CONFIGURED",
                "Lead emails need ICLOUD_MAIL_ADDRESS and ICLOUD_SMTP_APP_PASSWORD.",
            ));
        };
        let Some(lead) = self.repository.claim_for_notice(submission_id).await? else {
            return Ok(WebsiteLeadNotice::AlreadyHandled);
        };
        if let Err(error) = mailer.send(team_notice(&lead, &settings)).await {
            self.repository.release_notice(submission_id).await?;
            return Err(CoreServiceError::business(
                "LEAD_NOTICE_NOT_SENT",
                error.to_string(),
            ));
        }
        mailer
            .send(visitor_confirmation(&lead, &settings))
            .await
            .map_err(|error| {
                CoreServiceError::business("LEAD_CONFIRMATION_NOT_SENT", error.to_string())
            })?;
        Ok(WebsiteLeadNotice::Sent)
    }
}

/// Build the service from the environment's mail settings, as the composition root does.
pub fn mail_from_env() -> Option<(Arc<dyn MailPort>, LeadMailSettings)> {
    let config = MailConfig::from_env().ok()?;
    let settings = LeadMailSettings::from_env(&config);
    let mailer = SmtpMailer::new(config).ok()?;
    Some((Arc::new(mailer), settings))
}

fn request_label(request_type: &str) -> &'static str {
    match request_type {
        "private_viewing" => "Private viewing request",
        "property_information" => "Property information request",
        _ => "General enquiry",
    }
}

/// Relative property links in a message ("/properties/casa-luar") written out in full, so they work in an inbox.
fn absolute_links(text: &str, site_url: &str) -> String {
    text.replace(" /properties/", &format!(" {site_url}/properties/"))
}

fn first_name(display_name: &str) -> &str {
    display_name
        .split_whitespace()
        .next()
        .unwrap_or(display_name)
}

/// The team's notice: everything needed to answer, and Reply goes straight to the visitor.
pub fn team_notice(lead: &WebsiteLead, settings: &LeadMailSettings) -> OutgoingMail {
    let label = request_label(&lead.request_type);
    let mut subject = format!("New website lead: {} \u{2014} {label}", lead.display_name);
    let mut text = format!(
        "New enquiry from the CulebraLuxe website.\n\nName:     {}\nEmail:    {}\nRequest:  {label}\n",
        lead.display_name, lead.email
    );
    if let (Some(name), Some(slug)) = (lead.property_name.as_deref(), lead.property_slug.as_deref())
    {
        subject.push_str(&format!(" \u{2014} {name}"));
        text.push_str(&format!(
            "Property: {name} \u{2014} {}/properties/{slug}\n",
            settings.site_url
        ));
    }
    if let Some(message) = lead
        .message
        .as_deref()
        .filter(|message| !message.trim().is_empty())
    {
        text.push_str(&format!(
            "\nMessage:\n{}\n",
            absolute_links(message.trim(), &settings.site_url)
        ));
    }
    text.push_str(&format!(
        "\nReply to this email to answer {} directly.\n",
        first_name(&lead.display_name)
    ));
    OutgoingMail {
        to: settings.team_address.clone(),
        subject,
        text,
        html: None,
        reply_to: Some(lead.email.clone()),
    }
}

/// The visitor's confirmation, from the business address, with what they sent.
pub fn visitor_confirmation(lead: &WebsiteLead, settings: &LeadMailSettings) -> OutgoingMail {
    let what = match (lead.request_type.as_str(), lead.property_name.as_deref()) {
        ("private_viewing", Some(name)) => format!("your request for a private viewing of {name}"),
        ("private_viewing", None) => "your request for a private viewing".to_string(),
        ("property_information", Some(name)) => format!("your request for information on {name}"),
        _ => "your note".to_string(),
    };
    let mut text = format!(
        "Dear {},\n\nThank you for contacting CulebraLuxe. We have received {what}, and a member of our team will \
         be in touch within one business day.\n",
        first_name(&lead.display_name)
    );
    if let Some(message) = lead
        .message
        .as_deref()
        .filter(|message| !message.trim().is_empty())
    {
        text.push_str(&format!(
            "\nFor your reference, you wrote:\n\n{}\n",
            absolute_links(message.trim(), &settings.site_url)
        ));
    }
    text.push_str(&format!(
        "\nWarm regards,\nThe CulebraLuxe team\n{}\n{}\n",
        settings.team_address, settings.site_url
    ));
    OutgoingMail {
        to: lead.email.clone(),
        subject: "We have received your request \u{2014} CulebraLuxe".into(),
        text,
        html: None,
        reply_to: Some(settings.team_address.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::CasbinAuthorizationPort;
    use service::{CapturingAuditPort, CapturingDomainEventPort, ServiceActor, ServiceActorKind};
    use std::sync::Mutex;

    const ID: &str = "11111111-1111-4111-8111-111111111111";

    #[derive(Clone, Default)]
    struct Leads {
        claimed: Arc<Mutex<bool>>,
        released: Arc<Mutex<u32>>,
    }

    #[async_trait]
    impl WebsiteLeadRepository for Leads {
        async fn claim_for_notice(&mut self, _id: &str) -> DbResult<Option<WebsiteLead>> {
            let mut claimed = self.claimed.lock().unwrap();
            if *claimed {
                return Ok(None);
            }
            *claimed = true;
            Ok(Some(WebsiteLead {
                id: ID.into(),
                request_type: "private_viewing".into(),
                display_name: "Ada Lovelace".into(),
                email: "ada@example.com".into(),
                message: Some(
                    "Please send me details:\n- Casa Luar ($2,500,000) /properties/casa-luar"
                        .into(),
                ),
                property_name: Some("Casa Luar".into()),
                property_slug: Some("casa-luar".into()),
            }))
        }
        async fn release_notice(&mut self, _id: &str) -> DbResult<()> {
            *self.claimed.lock().unwrap() = false;
            *self.released.lock().unwrap() += 1;
            Ok(())
        }
    }

    #[derive(Default)]
    struct Outbox {
        sent: Mutex<Vec<OutgoingMail>>,
        fail: Mutex<bool>,
    }

    #[async_trait]
    impl MailPort for Outbox {
        async fn send(&self, mail: OutgoingMail) -> Result<(), MailError> {
            if *self.fail.lock().unwrap() {
                return Err(MailError::Connection("offline".into()));
            }
            self.sent.lock().unwrap().push(mail);
            Ok(())
        }
    }

    fn settings() -> LeadMailSettings {
        LeadMailSettings {
            team_address: "lisa@culebraluxe.com".into(),
            site_url: "https://culebraluxe.com".into(),
        }
    }

    fn public_website() -> ServiceContext {
        ServiceContext {
            actor: ServiceActor {
                id: Some("public-website".into()),
                kind: ServiceActorKind::System,
            },
            correlation_id: "website-lead-test".into(),
            causation_id: None,
            principal: None,
        }
    }

    async fn service(leads: Leads, outbox: Arc<Outbox>) -> WebsiteLeadService<Leads> {
        let infrastructure = ServiceInfrastructure::new(
            Arc::new(CasbinAuthorizationPort::new().await.unwrap()),
            Arc::new(CapturingAuditPort::default()),
            Arc::new(CapturingDomainEventPort::default()),
        );
        WebsiteLeadService::new(leads, Some((outbox, settings())), infrastructure)
    }

    #[tokio::test]
    async fn a_lead_is_emailed_to_the_team_and_confirmed_to_the_visitor_once() {
        let outbox = Arc::new(Outbox::default());
        let mut service = service(Leads::default(), outbox.clone()).await;
        assert_eq!(
            service.notify(ID, &public_website()).await.unwrap(),
            WebsiteLeadNotice::Sent
        );
        assert_eq!(
            service.notify(ID, &public_website()).await.unwrap(),
            WebsiteLeadNotice::AlreadyHandled
        );
        let sent = outbox.sent.lock().unwrap();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0].to, "lisa@culebraluxe.com");
        assert_eq!(sent[0].reply_to.as_deref(), Some("ada@example.com"));
        assert!(sent[0].subject.contains("Ada Lovelace") && sent[0].subject.contains("Casa Luar"));
        assert!(sent[0]
            .text
            .contains("https://culebraluxe.com/properties/casa-luar"));
        assert_eq!(sent[1].to, "ada@example.com");
        assert!(sent[1].text.starts_with("Dear Ada,"));
        assert!(sent[1].text.contains("private viewing of Casa Luar"));
    }

    #[tokio::test]
    async fn a_failed_team_notice_is_released_so_a_retry_can_send_it() {
        let outbox = Arc::new(Outbox::default());
        *outbox.fail.lock().unwrap() = true;
        let leads = Leads::default();
        let mut service = service(leads.clone(), outbox.clone()).await;
        assert!(service.notify(ID, &public_website()).await.is_err());
        assert_eq!(*leads.released.lock().unwrap(), 1);
        *outbox.fail.lock().unwrap() = false;
        assert_eq!(
            service.notify(ID, &public_website()).await.unwrap(),
            WebsiteLeadNotice::Sent
        );
    }

    #[tokio::test]
    async fn only_the_public_website_may_send_and_a_bad_id_sends_nothing() {
        let outbox = Arc::new(Outbox::default());
        let mut service = service(Leads::default(), outbox.clone()).await;
        let stranger = ServiceContext {
            actor: ServiceActor {
                id: Some("someone-else".into()),
                kind: ServiceActorKind::System,
            },
            ..public_website()
        };
        assert!(service.notify(ID, &stranger).await.is_err());
        assert!(service
            .notify("not-a-uuid", &public_website())
            .await
            .is_err());
        assert!(outbox.sent.lock().unwrap().is_empty());
    }
}
