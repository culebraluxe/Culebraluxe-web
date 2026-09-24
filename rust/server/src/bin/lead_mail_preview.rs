//! `pnpm mail:preview you@example.com` — send the two website-lead emails, filled with a SAMPLE lead, so the team can
//! see exactly what arrives: the notice goes to the team inbox, and the visitor's confirmation to the address given.
//! Both subjects start with [Preview]. Nothing is read from or written to the database.

use domain::WebsiteLead;
use integrations::mail::{MailConfig, SmtpMailer};
use server::website_leads::{team_notice, visitor_confirmation, LeadMailSettings};

#[tokio::main]
async fn main() {
    let Some(visitor) = std::env::args().nth(1).filter(|to| to.contains('@')) else {
        eprintln!("usage: pnpm mail:preview <where-the-visitor-confirmation-goes@example.com>");
        std::process::exit(2);
    };
    let config = match MailConfig::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("NOT SENT: {error}");
            std::process::exit(1);
        }
    };
    let settings = LeadMailSettings::from_env(&config);
    let lead = WebsiteLead {
        id: "preview".into(),
        request_type: "private_viewing".into(),
        display_name: "Sample Visitor".into(),
        email: visitor.clone(),
        message: Some(
            "Please send me details on my saved properties:\n- Casa Luar ($2,500,000) /properties/casa-luar".into(),
        ),
        property_name: Some("Casa Luar".into()),
        property_slug: Some("casa-luar".into()),
    };
    let mailer = match SmtpMailer::new(config) {
        Ok(mailer) => mailer,
        Err(error) => {
            eprintln!("NOT SENT: {error}");
            std::process::exit(1);
        }
    };
    for (who, mut mail) in [
        ("team notice", team_notice(&lead, &settings)),
        (
            "visitor confirmation",
            visitor_confirmation(&lead, &settings),
        ),
    ] {
        mail.subject = format!("[Preview] {}", mail.subject);
        match mailer.send(&mail).await {
            Ok(()) => println!("SENT: {who} to {}", mail.to),
            Err(error) => {
                eprintln!("NOT SENT ({who}): {error}");
                std::process::exit(1);
            }
        }
    }
}
