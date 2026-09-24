//! Outbound email as the business: SMTP to iCloud Mail, sending as `ICLOUD_MAIL_ADDRESS` (lisa@culebraluxe.com).
//!
//! WHY SMTP AND NOT THE MAIL APP. The existing intake READS mail by driving Mail.app on a signed-in Mac, where the
//! keychain answers for the account. The website sends from a server with no keychain, so it must log in to Apple's
//! mail server itself — and Apple accepts only an APP-SPECIFIC password there (account.apple.com → Sign-In and
//! Security → App-Specific Passwords). An authentication failure is reported as exactly that, with the fix, rather than
//! as a generic send error.
//!
//! CONFIGURATION, all from the environment:
//!   ICLOUD_MAIL_ADDRESS        the From address (required)
//!   ICLOUD_SMTP_APP_PASSWORD   the app-specific password (required)
//!   ICLOUD_MAIL_USERNAME       the login; Apple wants the full address, so a bare name falls back to ICLOUD_MAIL_ADDRESS
//!   ICLOUD_SMTP_HOST / _PORT   default smtp.mail.me.com:587 (STARTTLS)
//!   MAIL_FROM_NAME             default "CulebraLuxe"

use lettre::message::{header::ContentType, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use std::fmt;

/// Where and as whom mail is sent. Built from the environment by `MailConfig::from_env`.
#[derive(Clone)]
pub struct MailConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    password: String,
    pub from_address: String,
    pub from_name: String,
}

impl fmt::Debug for MailConfig {
    // The password never reaches a log line.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MailConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field("from_address", &self.from_address)
            .field("from_name", &self.from_name)
            .finish()
    }
}

/// One message to send. Plain text always; HTML when the caller has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingMail {
    pub to: String,
    pub subject: String,
    pub text: String,
    pub html: Option<String>,
    pub reply_to: Option<String>,
}

/// Why a send did not happen, in terms a person can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailError {
    /// A required setting is missing or malformed.
    Config(String),
    /// Apple refused the login: almost always a regular password where an app-specific one is required.
    Authentication(String),
    /// The server could not be reached (network, firewall, TLS).
    Connection(String),
    /// The server took the login but refused this message (a recipient, a limit).
    Rejected(String),
}

impl fmt::Display for MailError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MailError::Config(detail) => write!(f, "mail is not configured: {detail}"),
            MailError::Authentication(detail) => write!(
                f,
                "Apple refused the login ({detail}). Apple's mail server accepts only an APP-SPECIFIC password: create \
                 one at account.apple.com -> Sign-In and Security -> App-Specific Passwords, put it in \
                 ICLOUD_SMTP_APP_PASSWORD, and make ICLOUD_MAIL_USERNAME the full Apple Account email."
            ),
            MailError::Connection(detail) => write!(f, "could not reach the mail server: {detail}"),
            MailError::Rejected(detail) => write!(f, "the mail server refused the message: {detail}"),
        }
    }
}

impl std::error::Error for MailError {}

impl MailConfig {
    /// Read the configuration from the process environment.
    pub fn from_env() -> Result<Self, MailError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// The same, from any key lookup, so the rules are testable without touching the process environment.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, MailError> {
        let value = |key: &str| {
            get(key)
                .map(|value| unquote(&value))
                .filter(|value| !value.is_empty())
        };
        let from_address = value("ICLOUD_MAIL_ADDRESS")
            .ok_or_else(|| MailError::Config("ICLOUD_MAIL_ADDRESS is not set".into()))?;
        if !from_address.contains('@') {
            return Err(MailError::Config(format!(
                "ICLOUD_MAIL_ADDRESS is not an email address: {from_address}"
            )));
        }
        let password = value("ICLOUD_SMTP_APP_PASSWORD")
            .ok_or_else(|| MailError::Config("ICLOUD_SMTP_APP_PASSWORD is not set".into()))?;
        // Apple's login is the full account address; a bare local part ("lisa") cannot log in.
        let username = value("ICLOUD_MAIL_USERNAME")
            .filter(|username| username.contains('@'))
            .unwrap_or_else(|| from_address.clone());
        let port = match value("ICLOUD_SMTP_PORT") {
            Some(port) => port.parse().map_err(|_| {
                MailError::Config(format!("ICLOUD_SMTP_PORT is not a port number: {port}"))
            })?,
            None => 587,
        };
        Ok(Self {
            host: value("ICLOUD_SMTP_HOST").unwrap_or_else(|| "smtp.mail.me.com".into()),
            port,
            username,
            password,
            from_address,
            from_name: value("MAIL_FROM_NAME").unwrap_or_else(|| "CulebraLuxe".into()),
        })
    }

    /// Whether the password has the shape Apple gives app-specific passwords (four groups of four lowercase letters).
    /// Advisory only — used to warn before a login that is likely to fail, never to refuse one.
    pub fn looks_app_specific(&self) -> bool {
        let groups: Vec<&str> = self.password.split('-').collect();
        groups.len() == 4
            && groups
                .iter()
                .all(|group| group.len() == 4 && group.chars().all(|c| c.is_ascii_lowercase()))
    }
}

/// `.env` values are often quoted; the quotes are not part of the value.
fn unquote(value: &str) -> String {
    let trimmed = value.trim();
    for quote in ['"', '\''] {
        if trimmed.len() >= 2 && trimmed.starts_with(quote) && trimmed.ends_with(quote) {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
    }
    trimmed.to_string()
}

/// Build the RFC 5322 message: From the business, To the recipient, text and optional HTML alternatives.
pub fn build_message(config: &MailConfig, mail: &OutgoingMail) -> Result<Message, MailError> {
    let from: Mailbox = format!("{} <{}>", config.from_name, config.from_address)
        .parse()
        .map_err(|error| MailError::Config(format!("the From address is invalid: {error}")))?;
    let to: Mailbox = mail.to.parse().map_err(|error| {
        MailError::Rejected(format!("the recipient address is invalid: {error}"))
    })?;
    let mut builder = Message::builder()
        .from(from)
        .to(to)
        .subject(mail.subject.clone());
    if let Some(reply_to) = mail.reply_to.as_deref() {
        let reply_to: Mailbox = reply_to.parse().map_err(|error| {
            MailError::Rejected(format!("the reply-to address is invalid: {error}"))
        })?;
        builder = builder.reply_to(reply_to);
    }
    let built = match mail.html.as_deref() {
        Some(html) => builder.multipart(
            MultiPart::alternative()
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_PLAIN)
                        .body(mail.text.clone()),
                )
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_HTML)
                        .body(html.to_string()),
                ),
        ),
        None => builder
            .header(ContentType::TEXT_PLAIN)
            .body(mail.text.clone()),
    };
    built.map_err(|error| MailError::Rejected(format!("the message could not be built: {error}")))
}

/// Sends mail through the configured SMTP server.
pub struct SmtpMailer {
    config: MailConfig,
    transport: AsyncSmtpTransport<Tokio1Executor>,
}

impl SmtpMailer {
    pub fn new(config: MailConfig) -> Result<Self, MailError> {
        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
            .map_err(|error| MailError::Connection(error.to_string()))?
            .port(config.port)
            .credentials(Credentials::new(
                config.username.clone(),
                config.password.clone(),
            ))
            // A blocked network fails in seconds with a clear message rather than hanging.
            .timeout(Some(std::time::Duration::from_secs(20)))
            .build();
        Ok(Self { config, transport })
    }

    pub fn config(&self) -> &MailConfig {
        &self.config
    }

    /// Log in and check the server accepts us, without sending anything.
    pub async fn verify(&self) -> Result<(), MailError> {
        match self.transport.test_connection().await {
            Ok(true) => Ok(()),
            Ok(false) => Err(MailError::Connection(
                "the server did not accept the connection".into(),
            )),
            Err(error) => Err(classify(&error)),
        }
    }

    pub async fn send(&self, mail: &OutgoingMail) -> Result<(), MailError> {
        let message = build_message(&self.config, mail)?;
        self.transport
            .send(message)
            .await
            .map(|_| ())
            .map_err(|error| classify(&error))
    }
}

/// Sort an SMTP failure into what a person does about it. Apple answers a bad login with 535.
fn classify(error: &lettre::transport::smtp::Error) -> MailError {
    let detail = error.to_string();
    let lower = detail.to_lowercase();
    if lower.contains("535")
        || lower.contains("authentication")
        || (lower.contains("auth") && lower.contains("fail"))
    {
        MailError::Authentication(detail)
    } else if error.is_permanent() {
        MailError::Rejected(detail)
    } else {
        MailError::Connection(detail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config(pairs: &[(&str, &str)]) -> Result<MailConfig, MailError> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        MailConfig::from_lookup(|key| map.get(key).cloned())
    }

    #[test]
    fn a_bare_username_falls_back_to_the_full_address_and_quotes_are_stripped() {
        let config = config(&[
            ("ICLOUD_MAIL_ADDRESS", "\"lisa@culebraluxe.com\""),
            ("ICLOUD_MAIL_USERNAME", "lisa"),
            ("ICLOUD_SMTP_APP_PASSWORD", "abcd-efgh-ijkl-mnop"),
        ])
        .unwrap();
        assert_eq!(config.username, "lisa@culebraluxe.com");
        assert_eq!(config.from_address, "lisa@culebraluxe.com");
        assert_eq!(
            (config.host.as_str(), config.port),
            ("smtp.mail.me.com", 587)
        );
        assert!(config.looks_app_specific());
    }

    #[test]
    fn a_regular_password_is_flagged_but_not_refused() {
        let config = config(&[
            ("ICLOUD_MAIL_ADDRESS", "lisa@culebraluxe.com"),
            ("ICLOUD_SMTP_APP_PASSWORD", "Regular1!"),
        ])
        .unwrap();
        assert!(!config.looks_app_specific());
    }

    #[test]
    fn missing_settings_say_which_one() {
        let error = config(&[("ICLOUD_MAIL_ADDRESS", "lisa@culebraluxe.com")]).unwrap_err();
        assert_eq!(
            error,
            MailError::Config("ICLOUD_SMTP_APP_PASSWORD is not set".into())
        );
    }

    #[test]
    fn the_password_never_prints() {
        let config = config(&[
            ("ICLOUD_MAIL_ADDRESS", "lisa@culebraluxe.com"),
            ("ICLOUD_SMTP_APP_PASSWORD", "secret-value"),
        ])
        .unwrap();
        assert!(!format!("{config:?}").contains("secret-value"));
    }

    #[test]
    fn a_message_is_from_the_business_with_both_parts() {
        let config = config(&[
            ("ICLOUD_MAIL_ADDRESS", "lisa@culebraluxe.com"),
            ("ICLOUD_SMTP_APP_PASSWORD", "x"),
        ])
        .unwrap();
        let mail = OutgoingMail {
            to: "buyer@example.com".into(),
            subject: "Your shortlist".into(),
            text: "Hello".into(),
            html: Some("<p>Hello</p>".into()),
            reply_to: Some("lisa@culebraluxe.com".into()),
        };
        let raw = String::from_utf8(build_message(&config, &mail).unwrap().formatted()).unwrap();
        assert!(raw.contains("From: CulebraLuxe <lisa@culebraluxe.com>"));
        assert!(raw.contains("To: buyer@example.com"));
        assert!(raw.contains("text/html"));
    }
}
