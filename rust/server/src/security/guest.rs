//! External guests of the public website: sign-in by emailed code, and provisioning on first sign-in.
//!
//! A GUEST IS AN APP_USER with account_type 'external' and the 'guest' role, reached through auth_identity like any
//! other sign-in, and resolved by `SecurityService::resolve_identity`. The policy refuses every grant to an external
//! account (`account:external`), so a guest cannot reach the portal.
//!
//! THE EMAIL CODE: six digits from the OS random source, emailed from the business address, kept only as a hash
//! salted with its row id. Ten minutes, one use, five tries; a new code expires the older ones. Sending is limited to
//! five an hour per address (thirty seconds apart) and twenty an hour per IP.
//!
//! AUTHORIZATION (security/entitlements.rs, all reserved): the public website may request and verify codes; the
//! Auth.js edge may provision a guest for an identity it has proved (Google).

use crate::service_support::{audit_result, authorize, CoreServiceError};
use crate::website_leads::{LeadMailSettings, MailPort};
use async_trait::async_trait;
use db::{DbResult, GuestDao};
use domain::security::{GuestClaim, GuestCodeAttempt, GuestCodeHistory, GUEST_PROVIDERS};
use integrations::mail::OutgoingMail;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub const GUEST_EMAIL_CODE_PROVIDER: &str = "email-code";
const CODE_TTL_MINUTES: i32 = 10;
const MAX_CODES_PER_EMAIL_PER_HOUR: i64 = 5;
const MIN_SECONDS_BETWEEN_CODES: i64 = 30;
const MAX_CODES_PER_IP_PER_HOUR: i64 = 20;

#[async_trait]
pub trait GuestRepository: Send {
    async fn code_history(
        &mut self,
        email: &str,
        requester_ip: Option<&str>,
    ) -> DbResult<GuestCodeHistory>;
    async fn issue_code(
        &mut self,
        id: &str,
        email: &str,
        code_hash: &str,
        requester_ip: Option<&str>,
    ) -> DbResult<()>;
    async fn attempt_code(&mut self, email: &str) -> DbResult<Option<GuestCodeAttempt>>;
    async fn consume_code(&mut self, id: &str) -> DbResult<bool>;
    async fn provision(&mut self, claim: &GuestClaim, display_name: &str) -> DbResult<String>;
}

#[async_trait]
impl GuestRepository for GuestDao {
    async fn code_history(
        &mut self,
        email: &str,
        requester_ip: Option<&str>,
    ) -> DbResult<GuestCodeHistory> {
        GuestDao::code_history(self, email, requester_ip).await
    }
    async fn issue_code(
        &mut self,
        id: &str,
        email: &str,
        code_hash: &str,
        requester_ip: Option<&str>,
    ) -> DbResult<()> {
        GuestDao::issue_code(self, id, email, code_hash, requester_ip, CODE_TTL_MINUTES).await
    }
    async fn attempt_code(&mut self, email: &str) -> DbResult<Option<GuestCodeAttempt>> {
        GuestDao::attempt_code(self, email).await
    }
    async fn consume_code(&mut self, id: &str) -> DbResult<bool> {
        GuestDao::consume_code(self, id).await
    }
    async fn provision(&mut self, claim: &GuestClaim, display_name: &str) -> DbResult<String> {
        GuestDao::provision(self, claim, display_name).await
    }
}

pub struct GuestSignInService<R> {
    repository: R,
    mail: Option<(Arc<dyn MailPort>, LeadMailSettings)>,
    runtime: ServiceRuntime,
}

impl<R: GuestRepository> GuestSignInService<R> {
    /// `mail` is `None` when the mail settings are absent: sending a code is then refused rather than faked.
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

    /// Email a sign-in code. `requester_ip` is the visitor's address as the website saw it.
    pub async fn request_code(
        &mut self,
        email: &str,
        requester_ip: Option<&str>,
        context: &ServiceContext,
    ) -> Result<(), CoreServiceError> {
        const OP: &str = "security.requestGuestCode";
        let decision = authorize(
            &self.runtime,
            "security",
            "security.guestCode.request",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = self.request_code_authorized(email, requester_ip).await;
        audit_result(&self.runtime, "security", OP, context, decision, &result).await?;
        result
    }

    async fn request_code_authorized(
        &mut self,
        email: &str,
        requester_ip: Option<&str>,
    ) -> Result<(), CoreServiceError> {
        let email = normalize_email(email)?;
        let requester_ip = requester_ip.map(str::trim).filter(|ip| !ip.is_empty());
        let Some((mailer, settings)) = self.mail.clone() else {
            return Err(CoreServiceError::business(
                "MAIL_NOT_CONFIGURED",
                "Sign-in codes need ICLOUD_MAIL_ADDRESS and ICLOUD_SMTP_APP_PASSWORD.",
            ));
        };
        let history = self.repository.code_history(&email, requester_ip).await?;
        if history.email_last_hour >= MAX_CODES_PER_EMAIL_PER_HOUR
            || history.ip_last_hour >= MAX_CODES_PER_IP_PER_HOUR
            || history
                .seconds_since_last_for_email
                .is_some_and(|seconds| seconds < MIN_SECONDS_BETWEEN_CODES)
        {
            return Err(CoreServiceError::business(
                "GUEST_CODE_RATE_LIMITED",
                "Too many codes requested. Please wait a little and try again.",
            ));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let code = new_code();
        self.repository
            .issue_code(&id, &email, &code_hash(&id, &email, &code), requester_ip)
            .await?;
        mailer
            .send(code_email(&email, &code, &settings))
            .await
            .map_err(|error| CoreServiceError::business("GUEST_CODE_NOT_SENT", error.to_string()))
    }

    /// Check a code. On success the guest for that email exists (provisioned on first sign-in) and the verified,
    /// lower-cased email is returned: it is the `email-code` identity's subject.
    pub async fn verify_code(
        &mut self,
        email: &str,
        code: &str,
        context: &ServiceContext,
    ) -> Result<String, CoreServiceError> {
        const OP: &str = "security.verifyGuestCode";
        let decision = authorize(
            &self.runtime,
            "security",
            "security.guestCode.verify",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = self.verify_code_authorized(email, code).await;
        audit_result(&self.runtime, "security", OP, context, decision, &result).await?;
        result
    }

    async fn verify_code_authorized(
        &mut self,
        email: &str,
        code: &str,
    ) -> Result<String, CoreServiceError> {
        let invalid = || {
            CoreServiceError::business(
                "GUEST_CODE_INVALID",
                "That code is not right, or it has expired. Check the latest email or request a new code.",
            )
        };
        let email = normalize_email(email)?;
        let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
            return Err(invalid());
        }
        let Some(live) = self.repository.attempt_code(&email).await? else {
            return Err(invalid());
        };
        // The stored value is a hash, so comparing it in variable time reveals nothing about the code.
        if live.code_hash != code_hash(&live.id, &email, &code)
            || !self.repository.consume_code(&live.id).await?
        {
            return Err(invalid());
        }
        let claim = GuestClaim {
            provider: GUEST_EMAIL_CODE_PROVIDER.into(),
            subject: email.clone(),
            email: Some(email.clone()),
            email_verified: true,
            display_name: None,
        };
        self.repository
            .provision(&claim, &display_name_for(&claim))
            .await?;
        Ok(email)
    }

    /// Provision the guest for an identity the Auth.js edge has proved (first sign-in creates it; later ones stamp
    /// the last login). Returns the app_user id.
    pub async fn provision(
        &mut self,
        claim: GuestClaim,
        context: &ServiceContext,
    ) -> Result<String, CoreServiceError> {
        const OP: &str = "security.provisionGuest";
        let decision = authorize(
            &self.runtime,
            "security",
            "security.guest.provision",
            OP,
            OperationKind::Command,
            context,
        )
        .await?;
        let result = match normalize_claim(claim) {
            Ok(claim) => self
                .repository
                .provision(&claim, &display_name_for(&claim))
                .await
                .map_err(Into::into),
            Err(error) => Err(error),
        };
        audit_result(&self.runtime, "security", OP, context, decision, &result).await?;
        result
    }
}

/// Build the service's mail from the environment (shared with the lead emails).
pub fn mail_from_env() -> Option<(Arc<dyn MailPort>, LeadMailSettings)> {
    crate::website_leads::mail_from_env()
}

fn normalize_claim(claim: GuestClaim) -> Result<GuestClaim, CoreServiceError> {
    if !GUEST_PROVIDERS.contains(&claim.provider.as_str()) {
        return Err(CoreServiceError::business(
            "GUEST_PROVIDER_INVALID",
            "Guests sign in with Google or an emailed code.",
        ));
    }
    let subject = claim.subject.trim().to_string();
    if subject.is_empty() || subject.len() > 255 {
        return Err(CoreServiceError::business(
            "GUEST_SUBJECT_INVALID",
            "A sign-in needs its account id.",
        ));
    }
    let display_name = claim
        .display_name
        .map(|name| name.trim().chars().take(120).collect::<String>())
        .filter(|name| !name.is_empty());
    if claim.provider == GUEST_EMAIL_CODE_PROVIDER {
        // The subject IS the email the code was answered at.
        let email = normalize_email(&subject)?;
        return Ok(GuestClaim {
            provider: claim.provider,
            subject: email.clone(),
            email: Some(email),
            email_verified: true,
            display_name,
        });
    }
    let email = claim
        .email
        .as_deref()
        .and_then(|email| normalize_email(email).ok());
    Ok(GuestClaim {
        provider: claim.provider,
        subject,
        email_verified: claim.email_verified && email.is_some(),
        email,
        display_name,
    })
}

/// A new guest's display name: the provider's name, else the email's local part, else "Guest".
fn display_name_for(claim: &GuestClaim) -> String {
    claim
        .display_name
        .clone()
        .or_else(|| {
            claim
                .email
                .as_deref()
                .and_then(|email| email.split('@').next())
                .filter(|local| !local.is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "Guest".into())
}

/// Lower-case and check an address a visitor typed: one `@`, a dotted domain, nothing that could break a mail header.
pub fn normalize_email(raw: &str) -> Result<String, CoreServiceError> {
    let email = raw.trim().to_lowercase();
    let valid = (3..=254).contains(&email.len())
        && email
            .chars()
            .all(|c| c.is_ascii_graphic() && !"<>()[]\\,;:\"".contains(c))
        && match email.split_once('@') {
            Some((local, domain)) => {
                !local.is_empty()
                    && !domain.contains('@')
                    && domain.contains('.')
                    && !domain.starts_with('.')
                    && !domain.ends_with('.')
            }
            None => false,
        };
    if valid {
        Ok(email)
    } else {
        Err(CoreServiceError::business(
            "GUEST_EMAIL_INVALID",
            "Please enter a valid email address.",
        ))
    }
}

/// Six digits from the OS random source (a v4 uuid's 56 low bits are all random), zero-padded.
fn new_code() -> String {
    let random = uuid::Uuid::new_v4().as_u128() & ((1u128 << 56) - 1);
    format!("{:06}", random % 1_000_000)
}

fn code_hash(id: &str, email: &str, code: &str) -> String {
    let digest = Sha256::digest(format!("{id}:{email}:{code}").as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The sign-in email: the code first, where a phone's preview shows it.
pub fn code_email(email: &str, code: &str, settings: &LeadMailSettings) -> OutgoingMail {
    OutgoingMail {
        to: email.to_string(),
        subject: format!("{code} is your CulebraLuxe sign-in code"),
        text: format!(
            "Your CulebraLuxe sign-in code is:\n\n    {code}\n\nIt works once, for the next {CODE_TTL_MINUTES} minutes. \
             If you did not ask to sign in, you can ignore this email.\n\nThe CulebraLuxe team\n{}\n",
            settings.site_url
        ),
        html: None,
        reply_to: Some(settings.team_address.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::CasbinAuthorizationPort;
    use integrations::mail::MailError;
    use service::{CapturingAuditPort, CapturingDomainEventPort, ServiceActor, ServiceActorKind};
    use std::sync::Mutex;

    type Code = (String, String, String, i32, bool); // id, email, hash, attempts, consumed

    #[derive(Clone, Default)]
    struct Memory {
        codes: Arc<Mutex<Vec<Code>>>,
        history: Arc<Mutex<GuestCodeHistory>>,
        provisioned: Arc<Mutex<Vec<(GuestClaim, String)>>>,
    }

    #[async_trait]
    impl GuestRepository for Memory {
        async fn code_history(&mut self, _: &str, _: Option<&str>) -> DbResult<GuestCodeHistory> {
            Ok(*self.history.lock().unwrap())
        }
        async fn issue_code(
            &mut self,
            id: &str,
            email: &str,
            hash: &str,
            _: Option<&str>,
        ) -> DbResult<()> {
            let mut codes = self.codes.lock().unwrap();
            for code in codes.iter_mut().filter(|code| code.1 == email) {
                code.4 = true;
            }
            codes.push((id.into(), email.into(), hash.into(), 0, false));
            Ok(())
        }
        async fn attempt_code(&mut self, email: &str) -> DbResult<Option<GuestCodeAttempt>> {
            let mut codes = self.codes.lock().unwrap();
            Ok(codes
                .iter_mut()
                .rev()
                .find(|code| code.1 == email && !code.4 && code.3 < db::GUEST_CODE_MAX_ATTEMPTS)
                .map(|code| {
                    code.3 += 1;
                    GuestCodeAttempt {
                        id: code.0.clone(),
                        code_hash: code.2.clone(),
                    }
                }))
        }
        async fn consume_code(&mut self, id: &str) -> DbResult<bool> {
            let mut codes = self.codes.lock().unwrap();
            let code = codes.iter_mut().find(|code| code.0 == id).unwrap();
            let fresh = !code.4;
            code.4 = true;
            Ok(fresh)
        }
        async fn provision(&mut self, claim: &GuestClaim, display_name: &str) -> DbResult<String> {
            self.provisioned
                .lock()
                .unwrap()
                .push((claim.clone(), display_name.into()));
            Ok("guest-user-1".into())
        }
    }

    #[derive(Default)]
    struct Outbox {
        sent: Mutex<Vec<OutgoingMail>>,
    }

    #[async_trait]
    impl MailPort for Outbox {
        async fn send(&self, mail: OutgoingMail) -> Result<(), MailError> {
            self.sent.lock().unwrap().push(mail);
            Ok(())
        }
    }

    fn system(actor: &str) -> ServiceContext {
        ServiceContext {
            actor: ServiceActor {
                id: Some(actor.into()),
                kind: ServiceActorKind::System,
            },
            correlation_id: "guest-test".into(),
            causation_id: None,
            principal: None,
        }
    }

    async fn service(memory: Memory, outbox: Arc<Outbox>) -> GuestSignInService<Memory> {
        let infrastructure = ServiceInfrastructure::new(
            Arc::new(CasbinAuthorizationPort::new().await.unwrap()),
            Arc::new(CapturingAuditPort::default()),
            Arc::new(CapturingDomainEventPort::default()),
        );
        let settings = LeadMailSettings {
            team_address: "lisa@culebraluxe.com".into(),
            site_url: "https://culebraluxe.com".into(),
        };
        GuestSignInService::new(memory, Some((outbox, settings)), infrastructure)
    }

    fn last_code(outbox: &Outbox) -> String {
        outbox.sent.lock().unwrap().last().unwrap().subject[..6].to_string()
    }

    #[tokio::test]
    async fn an_emailed_code_signs_in_once_and_provisions_the_guest() {
        let (memory, outbox) = (Memory::default(), Arc::new(Outbox::default()));
        let mut service = service(memory.clone(), outbox.clone()).await;
        let site = system("public-website");
        service
            .request_code(" Ada@Example.com ", Some("203.0.113.9"), &site)
            .await
            .unwrap();
        let code = last_code(&outbox);
        assert_eq!(outbox.sent.lock().unwrap()[0].to, "ada@example.com");
        assert!(
            !memory.codes.lock().unwrap()[0].2.contains(&code),
            "only a hash is kept"
        );

        assert_eq!(
            service
                .verify_code("ada@example.com", &code, &site)
                .await
                .unwrap(),
            "ada@example.com"
        );
        let provisioned = memory.provisioned.lock().unwrap().clone();
        assert_eq!(provisioned[0].0.provider, "email-code");
        assert_eq!(provisioned[0].0.subject, "ada@example.com");
        assert_eq!(provisioned[0].1, "ada");
        assert!(
            service
                .verify_code("ada@example.com", &code, &site)
                .await
                .is_err(),
            "once"
        );
    }

    #[tokio::test]
    async fn five_wrong_tries_end_a_code_and_only_the_newest_works() {
        let outbox = Arc::new(Outbox::default());
        let mut service = service(Memory::default(), outbox.clone()).await;
        let site = system("public-website");
        service
            .request_code("ada@example.com", None, &site)
            .await
            .unwrap();
        let first = last_code(&outbox);
        service
            .request_code("ada@example.com", None, &site)
            .await
            .unwrap();
        let second = last_code(&outbox);
        if first != second {
            assert!(service
                .verify_code("ada@example.com", &first, &site)
                .await
                .is_err());
        }
        let wrong = if second == "000000" {
            "111111"
        } else {
            "000000"
        };
        for _ in 0..5 {
            let _ = service.verify_code("ada@example.com", wrong, &site).await;
        }
        assert!(service
            .verify_code("ada@example.com", &second, &site)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn sending_is_rate_limited_and_bad_addresses_are_refused() {
        let (memory, outbox) = (Memory::default(), Arc::new(Outbox::default()));
        let mut service = service(memory.clone(), outbox.clone()).await;
        let site = system("public-website");
        for bad in [
            "",
            "ada",
            "ada@example",
            "a b@example.com",
            "ada@example.com\r\nBcc: x@y.z",
        ] {
            assert!(
                service.request_code(bad, None, &site).await.is_err(),
                "{bad:?}"
            );
        }
        for history in [
            GuestCodeHistory {
                email_last_hour: 5,
                ..Default::default()
            },
            GuestCodeHistory {
                ip_last_hour: 20,
                ..Default::default()
            },
            GuestCodeHistory {
                seconds_since_last_for_email: Some(5),
                ..Default::default()
            },
        ] {
            *memory.history.lock().unwrap() = history;
            let error = service
                .request_code("ada@example.com", Some("203.0.113.9"), &site)
                .await
                .unwrap_err();
            assert_eq!(error.code(), "GUEST_CODE_RATE_LIMITED");
        }
        assert!(outbox.sent.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_edge_provisions_google_guests_and_links_only_verified_email() {
        let memory = Memory::default();
        let mut service = service(memory.clone(), Arc::new(Outbox::default())).await;
        let edge = system(service::AUTHJS_EDGE_ACTOR);
        let claim = |verified| GuestClaim {
            provider: "google".into(),
            subject: "1234567890".into(),
            email: Some("Ada@Example.com".into()),
            email_verified: verified,
            display_name: Some("Ada Lovelace".into()),
        };
        service.provision(claim(true), &edge).await.unwrap();
        service.provision(claim(false), &edge).await.unwrap();
        let provisioned = memory.provisioned.lock().unwrap().clone();
        assert_eq!(provisioned[0].0.email.as_deref(), Some("ada@example.com"));
        assert!(provisioned[0].0.email_verified);
        assert_eq!(provisioned[0].1, "Ada Lovelace");
        assert!(!provisioned[1].0.email_verified);

        let mut staff = claim(true);
        staff.provider = "break-glass".into();
        assert!(
            service.provision(staff, &edge).await.is_err(),
            "only guest providers"
        );
    }

    #[tokio::test]
    async fn each_door_is_its_own_actors_alone() {
        let outbox = Arc::new(Outbox::default());
        let memory = Memory::default();
        let mut service = service(memory.clone(), outbox.clone()).await;
        let edge = system(service::AUTHJS_EDGE_ACTOR);
        let site = system("public-website");
        assert!(service
            .request_code("ada@example.com", None, &edge)
            .await
            .is_err());
        assert!(service
            .verify_code("ada@example.com", "123456", &edge)
            .await
            .is_err());
        let claim = GuestClaim {
            provider: "google".into(),
            subject: "1".into(),
            email: None,
            email_verified: false,
            display_name: None,
        };
        assert!(service.provision(claim, &site).await.is_err());
        assert!(outbox.sent.lock().unwrap().is_empty());
        assert!(memory.provisioned.lock().unwrap().is_empty());
    }
}
