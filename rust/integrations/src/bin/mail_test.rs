//! `pnpm mail:test you@example.com` — send ONE test email as the business, and say plainly what happened.
//!
//! Run it on a machine that can reach Apple's mail server (your Mac, not the cloud sandbox). It checks the settings,
//! logs in, and sends a short test message; if Apple refuses the login it says why and what to change.

use integrations::mail::{MailConfig, MailError, OutgoingMail, SmtpMailer};

#[tokio::main]
async fn main() {
    let Some(to) = std::env::args().nth(1).filter(|to| to.contains('@')) else {
        eprintln!("usage: pnpm mail:test <recipient@example.com>");
        std::process::exit(2);
    };
    match run(&to).await {
        Ok(()) => println!(
            "SENT: a test email is on its way to {to}. Check the inbox (and spam) in a minute."
        ),
        Err(error) => {
            eprintln!("NOT SENT: {error}");
            std::process::exit(1);
        }
    }
}

async fn run(to: &str) -> Result<(), MailError> {
    let config = MailConfig::from_env()?;
    println!("from:     {} <{}>", config.from_name, config.from_address);
    println!(
        "login:    {} @ {}:{}",
        config.username, config.host, config.port
    );
    if !config.looks_app_specific() {
        println!(
            "note:     the password is not in Apple's app-specific format (xxxx-xxxx-xxxx-xxxx); trying anyway."
        );
    }
    let mailer = SmtpMailer::new(config)?;
    mailer.verify().await?;
    println!("login:    accepted by the server");
    mailer
        .send(&OutgoingMail {
            to: to.to_string(),
            subject: "CulebraLuxe website: test email".into(),
            text: "This is a test from the CulebraLuxe website's mail sender. If you are reading it, the website can \
                   send email as this address."
                .into(),
            html: None,
            reply_to: None,
        })
        .await
}
