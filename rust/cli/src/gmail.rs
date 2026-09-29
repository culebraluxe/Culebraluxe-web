// ---------------------------------------------------------------------------
// Gmail latest-context sync — the Rust replacement for `scripts/gmail-metadata-sync.ts`, which
// died with the TypeScript engine.
//
// For every exact-linked Gmail identity, fetch a bounded window of the newest Gmail METADATA and
// materialize the newest unambiguous direct email carrying a subject:
//
//   Gmail API (metadata only)  ->  l_email                       (landing, the golden rule)
//     -> gmail_metadata_to_context  ->  interaction              (what the CRM pane reads)
//     -> client read models
//
// PRIVACY: headers only. No body, snippet, attachment or raw MIME is requested or stored, and
// `format=metadata` is the only format this command ever asks for.
//
// FAIL CLOSED. The credentials come from GOOGLE_CLIENT_ID / GOOGLE_CLIENT_SECRET /
// GOOGLE_REFRESH_TOKEN and a missing one is a refusal naming the key — never a silent no-op that
// reports success while syncing nothing.
// ---------------------------------------------------------------------------
use crate::apple_mail;
use db::{EmailLanding, InteractionDraft, LandingDao, RelationshipEvidenceDao};
use domain::gmail::{
    gmail_metadata_to_context, GmailContextResult, GmailMetadataMessage, GMAIL_CONTEXT_SOURCE,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::error::Error;
use std::io;
use std::time::Duration;

/// How many messages one identity's window covers. Bounded on purpose: this is "latest context",
/// not a mailbox mirror.
const DEFAULT_WINDOW: usize = 25;
/// The headers the metadata read asks for, and only these.
const HEADER_NAMES: [&str; 5] = ["From", "To", "Cc", "Bcc", "Subject"];

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GmailProfile {
    #[serde(default)]
    email_address: String,
}

#[derive(Debug, Deserialize)]
struct GmailList {
    #[serde(default)]
    messages: Vec<GmailListedMessage>,
}

#[derive(Debug, Deserialize)]
struct GmailListedMessage {
    #[serde(default)]
    id: String,
}

/// A Google API failure is surfaced with its status and a bounded excerpt, never swallowed.
fn api_error(operation: &str, status: u16, body: &str) -> Box<dyn Error> {
    io::Error::other(format!(
        "{operation} failed ({status}): {}",
        body.trim().chars().take(300).collect::<String>()
    ))
    .into()
}

fn required_env(key: &str) -> Result<String, Box<dyn Error>> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            io::Error::other(format!(
                "missing required environment variable: {key} (the Gmail sync cannot run without it)"
            ))
            .into()
        })
}

/// Exchange the refresh token for an access token. `gmail.readonly` is the scope the token must
/// carry; a token without it fails here rather than producing an empty, "successful" sync.
async fn access_token(client: &reqwest::Client) -> Result<String, Box<dyn Error>> {
    let response = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", required_env("GOOGLE_CLIENT_ID")?),
            ("client_secret", required_env("GOOGLE_CLIENT_SECRET")?),
            ("refresh_token", required_env("GOOGLE_REFRESH_TOKEN")?),
            ("grant_type", "refresh_token".to_owned()),
        ])
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|error| io::Error::other(format!("Google token refresh failed: {error}")))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let parsed: TokenResponse = serde_json::from_str(&body).unwrap_or(TokenResponse {
        access_token: None,
        error: None,
    });
    match parsed.access_token.filter(|value| !value.trim().is_empty()) {
        Some(token) => Ok(token),
        None => Err(io::Error::other(format!(
            "Google token refresh failed ({}){}. The refresh token must include the gmail.readonly scope.",
            status.as_u16(),
            parsed
                .error
                .map(|error| format!(" ({error})"))
                .unwrap_or_default()
        ))
        .into()),
    }
}

/// One metadata-only GET against the Gmail API.
async fn gmail_get<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    token: &str,
    path_and_query: &str,
) -> Result<T, Box<dyn Error>> {
    let response = client
        .get(format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/{path_and_query}"
        ))
        .header("authorization", format!("Bearer {token}"))
        .header("accept", "application/json")
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|error| io::Error::other(format!("Gmail API request failed: {error}")))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(api_error("Gmail API", status.as_u16(), &body));
    }
    serde_json::from_str(&body).map_err(|error| {
        io::Error::other(format!("Gmail API returned undecodable JSON: {error}")).into()
    })
}

/// Percent-encode a query value for the `q=` parameter — the same encoding `encodeURIComponent`
/// produced in the deleted TypeScript.
fn encode_query(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// The newest unambiguous direct email for one identity, landing each candidate into `l_email` as
/// it is read.
///
/// Landing happens for every candidate, before any judgment: the L table is source evidence, and
/// the interaction is the interpretation built from it.
async fn newest_context(
    client: &reqwest::Client,
    token: &str,
    landing: &LandingDao,
    target_email: &str,
    internal_email: &str,
    person_id: &str,
    window: usize,
) -> Result<Option<InteractionDraft>, Box<dyn Error>> {
    let query = format!(
        "{{from:{email} to:{email} cc:{email} bcc:{email}}} -in:trash -in:spam -label:drafts",
        email = target_email
    );
    let list: GmailList = gmail_get(
        client,
        token,
        &format!("messages?maxResults={window}&q={}", encode_query(&query)),
    )
    .await?;

    for candidate in list.messages {
        if candidate.id.trim().is_empty() {
            continue;
        }
        let metadata_headers = HEADER_NAMES
            .iter()
            .map(|name| format!("metadataHeaders={name}"))
            .collect::<Vec<_>>()
            .join("&");
        let message: GmailMetadataMessage = gmail_get(
            client,
            token,
            &format!(
                "messages/{}?format=metadata&{metadata_headers}",
                encode_query(&candidate.id)
            ),
        )
        .await?;

        // GOLDEN RULE: all input lands in its own L table first, before anything is decided.
        let message_id = if message.id.trim().is_empty() {
            candidate.id.trim().to_owned()
        } else {
            message.id.trim().to_owned()
        };
        let header = |name: &str| -> Option<String> {
            message.payload.as_ref().and_then(|payload| {
                payload
                    .headers
                    .iter()
                    .find(|header| {
                        header
                            .name
                            .as_deref()
                            .map(|value| value.trim().to_lowercase())
                            == Some(name.to_lowercase())
                    })
                    .and_then(|header| header.value.clone())
            })
        };
        let sent_at = message
            .internal_date
            .as_deref()
            .map(str::trim)
            .and_then(|value| value.parse::<i64>().ok())
            .and_then(chrono::DateTime::from_timestamp_millis)
            .map(|value| value.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string());
        landing
            .land_email_batch(&[EmailLanding {
                source_account: internal_email.to_owned(),
                source_message_id: message_id,
                thread_id: message.thread_id.clone(),
                from_address: header("from"),
                to_address: header("to"),
                subject: header("subject"),
                sent_at,
                raw: serde_json::to_value(&message).unwrap_or(Value::Null),
            }])
            .await?;

        if let GmailContextResult::Ok(interaction) =
            gmail_metadata_to_context(&message, target_email, internal_email, person_id)
        {
            return Ok(Some(InteractionDraft {
                person_id: interaction.person_id,
                channel: interaction.channel.to_owned(),
                event_type: interaction.event_type.to_owned(),
                direction: Some(interaction.direction.to_owned()),
                occurred_at: interaction.occurred_at,
                title: interaction.title,
                source_system: interaction.source_system.to_owned(),
                source_external_id: interaction.source_external_id,
                source_metadata: interaction.source_metadata,
            }));
        }
    }
    Ok(None)
}

/// `gmail-sync [dev|prod] [--window=N] [--verify]`
///
/// For every exact-linked Gmail identity, land the metadata window into `l_email` and write the
/// newest unambiguous direct email as the canonical interaction. Idempotent: evidence is read, the
/// landing is replay-safe, and the interaction keys on the Gmail message id.
pub async fn gmail_sync(args: &[String]) -> Result<(), Box<dyn Error>> {
    crate::apple_sync::load_env();
    let target = apple_mail::target_arg(args)?;
    let verify_only = args.iter().any(|arg| arg == "--verify");
    let window =
        apple_mail::positive_int(args, "--window", DEFAULT_WINDOW as i64)?.clamp(1, 100) as usize;

    let database = apple_mail::connect(target).await?;
    let client = reqwest::Client::builder()
        .build()
        .map_err(|error| io::Error::other(format!("could not build the HTTP client: {error}")))?;

    // Credentials first: an unresolvable token is a refusal that names the missing key, not an
    // empty run reported as success.
    let token = access_token(&client).await?;
    let profile: GmailProfile = gmail_get(&client, &token, "profile").await?;
    let internal_email =
        domain::applemail::normalize_mailbox(&profile.email_address).ok_or_else(|| {
            io::Error::other(format!(
                "Gmail profile returned no usable address ({:?})",
                profile.email_address
            ))
        })?;
    println!("source account: {internal_email}");

    let evidence_dao = RelationshipEvidenceDao::new(database.clone());
    let landing = LandingDao::new(database);
    let identities = evidence_dao
        .candidates(Some(GMAIL_CONTEXT_SOURCE), Some("exact_linked"), &[], 10000)
        .await?;
    println!("exact-linked Gmail identities: {}", identities.len());

    if verify_only {
        println!("verify only — credentials, profile and identity count resolved; no writes.");
        return Ok(());
    }

    let mut inserted = 0i64;
    let mut replayed = 0i64;
    let mut no_context = 0i64;
    let mut errors = 0i64;
    for (index, row) in identities.iter().enumerate() {
        let Some(person_id) = row.canonical_person_id.as_deref() else {
            continue;
        };
        match newest_context(
            &client,
            &token,
            &landing,
            &row.source_identity_key,
            &internal_email,
            person_id,
            window,
        )
        .await
        {
            Ok(Some(draft)) => {
                if landing.create_interaction(&draft).await? {
                    inserted += 1;
                } else {
                    replayed += 1;
                }
            }
            // No unambiguous direct email in the window is a decision, not a failure.
            Ok(None) => no_context += 1,
            Err(error) => {
                // One identity that Google will not answer for must not stop the others, and it
                // must still fail the run: a sync that hides errors reports success while syncing
                // nothing.
                errors += 1;
                eprintln!("identity failed: {}: {error}", row.source_identity_key);
            }
        }
        let processed = index + 1;
        if processed % 10 == 0 || processed == identities.len() {
            println!(
                "progress: {processed}/{} identities | inserted={inserted} replayed={replayed} no_context={no_context} errors={errors}",
                identities.len()
            );
        }
    }

    let refreshed = inserted > 0;
    if refreshed {
        println!("refreshing Client relationship read models");
        landing.refresh_client_read_models().await?;
    }
    println!(
        "{}",
        json!({
            "source": GMAIL_CONTEXT_SOURCE,
            "target": target.as_str(),
            "sourceAccount": internal_email,
            "window": window,
            "identities": identities.len(),
            "interactionsInserted": inserted,
            "interactionsReplayed": replayed,
            "noContext": no_context,
            "errors": errors,
            "refreshed": refreshed,
        })
    );
    println!(
        "complete: inserted={inserted} replayed={replayed} no_context={no_context} errors={errors}"
    );

    if errors > 0 {
        return Err(io::Error::other(format!(
            "Gmail metadata sync completed with {errors} identity errors"
        ))
        .into());
    }
    Ok(())
}
