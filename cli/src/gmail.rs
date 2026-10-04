// ---------------------------------------------------------------------------
// Gmail, both halves of one source.
//
// `gmail-census` is the identity half and `gmail-sync` is the context half. The first turns the
// approved bounded census artifact into relationship evidence; the second reads the
// exact-linked identities the first produced and materializes their newest metadata:
//
//   approved census CSV (relationship-intel load)  ->  evidence + reconciliation   (gmail-census)
//   Gmail API (metadata only)                      ->  l_email -> interaction      (gmail-sync)
//
// Neither half decides who anyone is on its own: both write the decision the crate's one
// adjudicator returned, against the crate's one evidence source (`GMAIL_CONTEXT_SOURCE`).
//
// PRIVACY: headers and aggregate counts only. No body, snippet, attachment or raw MIME is ever
// requested or stored, and `format=metadata` is the only format `gmail-sync` asks for.
//
// FAIL CLOSED. `gmail-sync` refuses to run without GOOGLE_CLIENT_ID / GOOGLE_CLIENT_SECRET /
// GOOGLE_REFRESH_TOKEN and names the missing key — never a silent no-op that reports success while
// syncing nothing. `gmail-census` refuses an artifact it cannot read, a batch that does not balance,
// and an artifact with no usable rows.
// ---------------------------------------------------------------------------
use crate::apple_mail;
use db::{EmailLanding, EvidenceUpsert, InteractionDraft, LandingDao, RelationshipEvidenceDao};
use model::decide_apple_handle;
use model::gmail::{
    gmail_metadata_to_context, parse_gmail_census, GmailContextResult, GmailMetadataMessage,
    GMAIL_CONTEXT_SOURCE,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

/// How many messages one identity's window covers. Bounded on purpose: this is "latest context",
/// not a mailbox mirror.
const DEFAULT_WINDOW: usize = 25;
/// The headers the metadata read asks for, and only these.
const HEADER_NAMES: [&str; 5] = ["From", "To", "Cc", "Bcc", "Subject"];

/// The approved bounded census artifact, at the path the deleted `scripts/rel-intel-load-gmail.ts`
/// read it from: this is a load of the batch the Captain approved, not a location this command
/// invents. The artifact carries aggregate correspondent evidence from a private mailbox, so it
/// stays in the checkout and is never staged under `public/`, which is the deploy artifact tree.
const DEFAULT_CENSUS_FILE: &str = "docs/marlowe-gmail-relationship-census-private-2026-08-24.csv";

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
        model::applemail::normalize_mailbox(&profile.email_address).ok_or_else(|| {
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

/// `gmail-census [dev|prod] [--file=PATH] [--verify] [--refresh]`
///
/// The Rust replacement for `scripts/rel-intel-load-gmail.ts` and the
/// `lib/relationship-intel/gmail-census.ts` parser behind it, both deleted with the TypeScript engine
/// (commit 4cf98110). This is the half of the Gmail chain that went missing with them: it is what
/// makes a Gmail correspondent an identity at all, and `gmail-sync` reads the identities it
/// exact-links — with no census load, the metadata sync has nothing to read.
///
///   approved census CSV -> neutral evidence -> the one adjudicator -> reconciliation
///
/// The artifact is bounded, operator-supplied and approval-gated, so this is a load and not a pull:
/// it never touches the Gmail API, and it never invents a row the file does not carry. What the run
/// reports about coverage is read back from the rows themselves, not from a tally it kept in memory.
///
/// Replay-safe: evidence upserts on `(source, source_account, source_identity_key)` and a decision
/// preserves an established canonical link, so re-loading the same artifact is harmless.
pub async fn gmail_census(args: &[String]) -> Result<(), Box<dyn Error>> {
    crate::apple_sync::load_env();
    let target = apple_mail::target_arg(args)?;
    let verify_only = args.iter().any(|arg| arg == "--verify");
    let refresh = args.iter().any(|arg| arg == "--refresh");
    let file = census_file(args);

    let raw = fs::read_to_string(&file).map_err(|error| {
        io::Error::other(format!(
            "census artifact unreadable at {}: {error} (this is a bounded, operator-supplied \
             export; point --file at the approved copy)",
            file.display()
        ))
    })?;
    let batch = parse_gmail_census(&raw);

    // The accounting is a refusal, not a warning: a batch that does not balance has lost a row, and a
    // lost row reported as a successful load is what this accounting exists to catch.
    if !batch.balances() {
        return Err(io::Error::other(format!(
            "{} does not balance: declared={} accepted={} rejected={} quarantined={} \
             deduplicated={} — refusing to load",
            file.display(),
            batch.declared,
            batch.accepted,
            batch.rejected,
            batch.quarantined,
            batch.deduplicated
        ))
        .into());
    }
    if batch.accepted == 0 {
        return Err(io::Error::other(format!(
            "{} carries no readable correspondent rows — refusing to report an empty load as success",
            file.display()
        ))
        .into());
    }
    for refusal in &batch.rejections {
        eprintln!("refused line {}: {}", refusal.line, refusal.reason);
    }
    for held in &batch.quarantine {
        eprintln!("quarantined line {}: {}", held.line, held.reason);
    }

    let file_sha256 = file_sha256(&raw);
    println!(
        "[census] {} -> target={} declared={} accepted={} rejected={} quarantined={} \
         deduplicated={} sha256={}",
        file.display(),
        target.as_str(),
        batch.declared,
        batch.accepted,
        batch.rejected,
        batch.quarantined,
        batch.deduplicated,
        file_sha256
    );

    if verify_only {
        println!(
            "{}",
            json!({
                "source": GMAIL_CONTEXT_SOURCE,
                "target": target.as_str(),
                "file": file.display().to_string(),
                "fileSha256": file_sha256,
                "applied": false,
                "declared": batch.declared,
                "accepted": batch.accepted,
                "rejected": batch.rejected,
                "quarantined": batch.quarantined,
                "deduplicated": batch.deduplicated,
            })
        );
        println!("verify only — the artifact parses and balances; nothing written.");
        return Ok(());
    }

    let database = apple_mail::connect(target).await?;
    let evidence_dao = RelationshipEvidenceDao::new(database.clone());
    let landing = LandingDao::new(database);
    // The two reads adjudication needs, once each — the same two the Messages and Apple Mail intakes
    // do, against the same shared rules, so this feed cannot drift from those.
    let owners = crate::apple_messages::OwnerIndex::load(&evidence_dao).await?;
    let links: HashMap<(String, String), String> = evidence_dao
        .source_links(GMAIL_CONTEXT_SOURCE)
        .await?
        .into_iter()
        .map(|link| {
            (
                (link.source_account, link.source_identity_key),
                link.canonical_person_id,
            )
        })
        .collect();

    // A database failure fails the run. Nothing here is counted and swallowed: a load that reports
    // success while writing nothing is the failure mode this command exists to end.
    let mut reconcile_tally: BTreeMap<String, i64> = BTreeMap::new();
    let mut exact_linked = 0i64;
    for row in &batch.rows {
        let lookup = crate::apple_messages::lookup_for(&row.evidence, &owners, &links);
        let decision = decide_apple_handle(&row.evidence, &lookup);
        *reconcile_tally
            .entry(decision.review_state.clone())
            .or_insert(0) += 1;
        if decision.review_state == "exact_linked" && decision.canonical_person_id.is_some() {
            exact_linked += 1;
        }
        let id = evidence_dao
            .upsert_evidence(&EvidenceUpsert::from(&row.evidence))
            .await?;
        evidence_dao.record_decision(&id, &decision).await?;
    }

    // Evidence alone materializes no interaction, so the client read models are rebuilt when asked
    // for rather than assumed — the rule `messages-intake --evidence-only` already follows.
    if refresh {
        landing.refresh_client_read_models().await?;
    }

    let coverage = evidence_dao.coverage(GMAIL_CONTEXT_SOURCE).await?;
    println!(
        "{}",
        json!({
            "source": GMAIL_CONTEXT_SOURCE,
            "target": target.as_str(),
            "file": file.display().to_string(),
            "fileSha256": file_sha256,
            "applied": true,
            "declared": batch.declared,
            "accepted": batch.accepted,
            "rejected": batch.rejected,
            "quarantined": batch.quarantined,
            "deduplicated": batch.deduplicated,
            "evidenceRowsWritten": batch.accepted,
            "exactLinked": exact_linked,
            "reconcileTally": reconcile_tally,
            "persistedCoverage": {
                "rows": coverage.row_count,
                "firstObservedAt": coverage.first_observed_at,
                "lastObservedAt": coverage.last_observed_at,
            },
            "refreshed": refresh,
        })
    );
    println!(
        "complete: {} evidence rows decided ({exact_linked} exact-linked), {} rows now held for \
         {GMAIL_CONTEXT_SOURCE}",
        batch.accepted, coverage.row_count
    );
    Ok(())
}

/// The artifact to read: `--file`, or the approved batch in this checkout.
fn census_file(args: &[String]) -> PathBuf {
    match apple_mail::option(args, "--file") {
        Some(path) => PathBuf::from(path),
        None => crate::apple_sync::repo_root().join(DEFAULT_CENSUS_FILE),
    }
}

/// sha256 of the artifact's bytes, lowercase hex — the batch's own identity, reported so a load can
/// be tied to the exact file it read.
fn file_sha256(raw: &str) -> String {
    Sha256::digest(raw.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
