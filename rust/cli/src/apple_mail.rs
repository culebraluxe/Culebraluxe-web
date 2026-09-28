// ---------------------------------------------------------------------------
// Apple Mail intake and promotion — the Rust replacement for
// `scripts/apple-mail-envelope-intake.ts` and `scripts/promote-applemail.ts`, both of which
// died with the TypeScript engine.
//
//   Mail.app Envelope Index  ->  scripts/macbridge/apple-mail-envelope-sqlite.py   (read-only)
//     -> l_applemail                        (landing: source evidence, no judgment)
//     -> evidence + reconciliation           (integration_relationship_evidence)
//     -> interaction                         (the comms event the CRM pane reads)
//     -> client read models
//
// The extractor stays a Python bridge on purpose: it must open Mail's local SQLite store under
// macOS TCC, and that is the one part of this chain a Mac-only script already does correctly.
// Nothing authoritative lives in it — it pages and reports JSON, the rules live in Rust.
//
// PRIVACY: envelope metadata only. No body, snippet, attachment or raw MIME is requested,
// transported or stored.
//
// Fail closed: the target is an explicit argument or the declared environment, and PROD is
// refused when the PROD and DEV connection strings are the same.
// ---------------------------------------------------------------------------
use db::{
    AppleMailLanding, Database, DbTarget, IntakeCheckpoint, IntakeCheckpointUpdate,
    InteractionDraft, LandingDao, RelationshipEvidenceDao,
};
use domain::applemail::{
    ICLOUD_MAIL_SOURCE, MailAddress, MailNormalization, apple_mail_replay_id, build_mail_evidence,
    mail_observation_interaction, normalize_landed_mail,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::process::Command;

/// Rows per landing statement. The single-row form costs a round trip per message; a real
/// mailbox window is tens of thousands of them.
const LANDING_BATCH: usize = 500;
/// The acquisition source name recorded in `integration_intake_checkpoint`.
const INTAKE_SOURCE: &str = "applemail";
/// The discrete windows the extractor offers, newest first.
const BANDS: [&str; 4] = ["0-1", "1-3", "3-6", "6-12"];

pub fn band_label(band: &str) -> &'static str {
    match band {
        "0-1" => "last 1 month",
        "1-3" => "months 1-3 ago",
        "3-6" => "months 3-6 ago",
        "6-12" => "months 6-12 ago",
        _ => "unknown band",
    }
}

/// One page as the extractor reports it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtractPage {
    #[serde(default)]
    ok: bool,
    account: Option<String>,
    account_id: Option<String>,
    mail_version: Option<String>,
    since: Option<String>,
    before: Option<String>,
    mailboxes: Option<ExtractMailboxes>,
    #[serde(default)]
    records: Vec<LocalMailRecord>,
    next_cursor: Option<ExtractCursor>,
    #[serde(default)]
    complete: bool,
}

#[derive(Debug, Deserialize)]
struct ExtractMailboxes {
    #[serde(default)]
    inbox: Vec<String>,
    #[serde(default)]
    sent: Vec<String>,
}

#[derive(Debug, Deserialize, Clone, Copy)]
struct ExtractCursor {
    date: f64,
    rowid: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalMailRecord {
    mailbox: String,
    mailbox_name: Option<String>,
    local_id: i64,
    message_id: Option<String>,
    occurred_at: Option<String>,
    sender: Option<String>,
    #[serde(default)]
    to: Vec<MailAddress>,
    #[serde(default)]
    cc: Vec<MailAddress>,
    #[serde(default)]
    bcc: Vec<MailAddress>,
    subject: Option<String>,
}

/// Resolve the database target: an explicit `dev`/`prod` argument wins, otherwise the declared
/// environment decides — and an undeclared environment is a refusal, never a guess.
pub(crate) fn target_arg(args: &[String]) -> Result<DbTarget, Box<dyn Error>> {
    match args
        .iter()
        .find(|arg| arg.as_str() == "dev" || arg.as_str() == "prod")
    {
        Some(value) if value == "prod" => Ok(DbTarget::Prod),
        Some(_) => Ok(DbTarget::Dev),
        None => Ok(db::resolve_declared_target(
            std::env::var("VERCEL_ENV").ok().as_deref(),
            std::env::var("APP_ENV").ok().as_deref(),
        )?),
    }
}

/// A production target whose connection string is the development one is a misconfiguration,
/// and running against it silently is how a sync writes to the wrong database.
pub(crate) async fn connect(target: DbTarget) -> Result<Database, Box<dyn Error>> {
    if target == DbTarget::Prod {
        let prod = std::env::var("DATABASE_URL_PROD").ok();
        let dev = std::env::var("DATABASE_URL_DEV").ok();
        if let (Some(prod), Some(dev)) = (prod.as_deref(), dev.as_deref()) {
            if prod == dev {
                return Err(io::Error::other(
                    "PROD selected but DATABASE_URL_PROD equals DATABASE_URL_DEV; refusing to run",
                )
                .into());
            }
        }
    }
    Database::connect_target(target)
        .await
        .map_err(|error| io::Error::other(error.to_string()).into())
}

fn repo_root() -> PathBuf {
    crate::apple_sync::repo_root()
}

fn option<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let prefixed = format!("{name}=");
    if let Some(direct) = args
        .iter()
        .find(|arg| arg.starts_with(&prefixed))
        .map(String::as_str)
    {
        return Some(&direct[prefixed.len()..]);
    }
    let index = args.iter().position(|arg| arg == name)?;
    args.get(index + 1)
        .filter(|value| !value.starts_with("--"))
        .map(String::as_str)
}

/// Bands to read. `--band=all` walks every band in order and stops at the first failure.
fn bands_arg(args: &[String]) -> Result<Vec<String>, Box<dyn Error>> {
    let raw = option(args, "--band")
        .map(str::to_owned)
        .or_else(|| std::env::var("MAIL_SYNC_BAND").ok())
        .unwrap_or_else(|| "0-1".to_owned())
        .trim()
        .to_lowercase();
    if raw == "all" {
        return Ok(BANDS.iter().map(|band| (*band).to_owned()).collect());
    }
    if BANDS.contains(&raw.as_str()) {
        return Ok(vec![raw]);
    }
    Err(io::Error::other(format!(
        "--band must be one of {}, or all (got {raw})",
        BANDS.join(", ")
    ))
    .into())
}

pub(crate) fn positive_int(args: &[String], name: &str, fallback: i64) -> Result<i64, Box<dyn Error>> {
    match option(args, name) {
        None => Ok(fallback),
        Some(raw) => raw.parse::<i64>().ok().filter(|value| *value > 0).ok_or_else(|| {
            io::Error::other(format!("{name} must be a positive integer")).into()
        }),
    }
}

/// The accounts one run reads, lowercased and de-duplicated. The list is configuration, not
/// discovery: a run must never guess which mailbox it is about.
fn configured_accounts(only: Option<&str>) -> Result<Vec<String>, Box<dyn Error>> {
    if let Some(account) = only.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok(vec![account.to_lowercase()]);
    }
    let mut accounts: Vec<String> = Vec::new();
    let push = |value: &str, accounts: &mut Vec<String>| {
        let normalized = value.trim().to_lowercase();
        if !normalized.is_empty() && !accounts.contains(&normalized) {
            accounts.push(normalized);
        }
    };
    match std::env::var("MAIL_APP_ACCOUNTS")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        Some(configured) => {
            for entry in configured.split(',') {
                push(entry, &mut accounts);
            }
        }
        None => {
            let fallback = std::env::var("APPLE_MAILBOX_ADDRESS")
                .ok()
                .or_else(|| std::env::var("ICLOUD_MAIL_ADDRESS").ok())
                .unwrap_or_default();
            push(&fallback, &mut accounts);
        }
    }
    if accounts.is_empty() {
        return Err(io::Error::other(
            "no Mail.app account configured (MAIL_APP_ACCOUNTS / APPLE_MAILBOX_ADDRESS / ICLOUD_MAIL_ADDRESS)",
        )
        .into());
    }
    Ok(accounts)
}

/// Internal addresses decide which side of a message is the counterparty. Required: without it
/// every message would be classified against nothing, silently.
fn internal_addresses() -> Result<BTreeSet<String>, Box<dyn Error>> {
    let raw = std::env::var("EMAIL_INTERNAL_ADDRESSES").unwrap_or_default();
    let addresses: BTreeSet<String> = raw
        .split(',')
        .filter_map(domain::applemail::normalize_mailbox)
        .collect();
    if addresses.is_empty() {
        return Err(io::Error::other(
            "EMAIL_INTERNAL_ADDRESSES is required: it decides inbound vs outbound",
        )
        .into());
    }
    Ok(addresses)
}

/// The Mail.app account UUID for one configured address: an explicit override, else Mail's own
/// answer through its AppleEvent bridge (the extractor never crosses that bridge for messages).
fn resolve_account_id(account: &str) -> Result<String, Box<dyn Error>> {
    if let Some(override_id) = std::env::var("APPLE_MAIL_ACCOUNT_ID")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        return Ok(override_id);
    }
    let script = repo_root().join("scripts/macbridge/apple-mail-account-id.jxa");
    let output = Command::new("/usr/bin/osascript")
        .arg("-l")
        .arg("JavaScript")
        .arg(&script)
        .arg(account)
        .output()
        .map_err(|error| {
            io::Error::other(format!("could not run osascript for {account}: {error}"))
        })?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "Mail.app account lookup failed for {account}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
        .into());
    }
    let id = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if id.is_empty() {
        return Err(io::Error::other(format!(
            "Mail.app returned an empty account id for {account}"
        ))
        .into());
    }
    Ok(id)
}

/// One bounded page from the local Envelope Index. Read-only: the bridge snapshots the store and
/// opens the snapshot with `query_only`.
fn extract_page(
    account: &str,
    account_id: &str,
    band: &str,
    page_size: i64,
    cursor: Option<ExtractCursor>,
    verify: bool,
) -> Result<ExtractPage, Box<dyn Error>> {
    let extractor = std::env::var("CULEBRALUXE_MAIL_EXTRACTOR")
        .ok()
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| repo_root().join("scripts/macbridge/apple-mail-envelope-sqlite.py"));
    if !extractor.is_file() {
        return Err(io::Error::other(format!(
            "Apple Mail extractor missing at {}",
            extractor.display()
        ))
        .into());
    }
    let limit = if verify {
        page_size.min(20)
    } else {
        page_size.min(1000)
    };
    let mut command = Command::new("/usr/bin/env");
    command
        .arg("python3")
        .arg(&extractor)
        .arg("--account")
        .arg(account)
        .arg("--account-id")
        .arg(account_id)
        .arg("--band")
        .arg(band)
        .arg("--limit")
        .arg(limit.to_string());
    if let Some(cursor) = cursor {
        command
            .arg("--cursor-date")
            .arg(cursor.date.to_string())
            .arg("--cursor-rowid")
            .arg(cursor.rowid.to_string());
    }
    if verify {
        command.arg("--verify");
    }

    let output = command.output().map_err(|error| {
        io::Error::other(format!("could not run the Apple Mail extractor: {error}"))
    })?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        eprint!("{stderr}");
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        // The bridge reports its failures as JSON on stderr, so both streams are quoted: an
        // extractor that cannot read Mail's store (macOS Full Disk Access) says so there.
        let detail = if stdout.trim().is_empty() {
            stderr.trim()
        } else {
            stdout.trim()
        };
        return Err(io::Error::other(format!(
            "Apple Mail extractor failed (exit {:?}): {}",
            output.status.code(),
            detail.chars().take(500).collect::<String>()
        ))
        .into());
    }
    let page: ExtractPage = serde_json::from_str(stdout.trim()).map_err(|error| {
        io::Error::other(format!(
            "Apple Mail extractor returned invalid JSON ({error}): {}",
            stdout.trim().chars().take(500).collect::<String>()
        ))
    })?;
    if !page.ok {
        return Err(io::Error::other("Apple Mail extractor reported ok=false").into());
    }
    Ok(page)
}

/// The checkpoint shard key for one band. The band is part of the shard identity: a band that
/// has finished is never re-read, and a wider band can never be skipped because a narrower one
/// already completed.
fn checkpoint_shard(band: &str) -> String {
    format!("band:{band}")
}

/// The cursor as one text token: the extractor's own keyset pair.
fn cursor_token(cursor: ExtractCursor) -> String {
    format!("{},{}", cursor.date, cursor.rowid)
}

fn parse_cursor(token: Option<&str>) -> Option<ExtractCursor> {
    let (date, rowid) = token?.split_once(',')?;
    Some(ExtractCursor {
        date: date.trim().parse().ok()?,
        rowid: rowid.trim().parse().ok()?,
    })
}

/// What one band read did. Counters only — a run reports what it landed, never a message.
#[derive(Debug, Default)]
struct BandTally {
    pages: i64,
    records: i64,
    landed: i64,
    replayed: i64,
    complete: bool,
}

/// Read one band for one account, page by page, landing each page before the checkpoint moves.
///
/// A band marked complete is deliberately NOT skipped: every band slides with "now", so the
/// newest band must be re-read to pick up what has arrived since. Landing is replay-safe, so the
/// only cost is the read. The checkpoint earns its keep by resuming an interrupted run.
async fn intake_band(
    landing: &LandingDao,
    account: &str,
    account_id: &str,
    band: &str,
    page_size: i64,
    max_pages: Option<i64>,
    reset: bool,
) -> Result<BandTally, Box<dyn Error>> {
    let shard = checkpoint_shard(band);
    if reset {
        landing
            .save_intake_checkpoint(&IntakeCheckpointUpdate {
                source: INTAKE_SOURCE.to_owned(),
                source_account: account.to_owned(),
                shard_key: shard.clone(),
                shard_start: None,
                shard_end: None,
                cursor: None,
                pages_completed: 0,
                records_seen: 0,
                records_landed: 0,
                records_replayed: 0,
                status: "in_progress".to_owned(),
            })
            .await?;
    }
    let mut checkpoint = landing
        .intake_checkpoint(INTAKE_SOURCE, account, &shard)
        .await?
        .unwrap_or_else(|| IntakeCheckpoint {
            status: "in_progress".to_owned(),
            ..Default::default()
        });

    let mut tally = BandTally::default();
    let mut pages_this_run = 0i64;
    loop {
        let page = extract_page(
            account,
            account_id,
            band,
            page_size,
            parse_cursor(checkpoint.cursor.as_deref()),
            false,
        )?;
        let records = page.records;
        let mut landed = 0i64;

        for chunk in records.chunks(LANDING_BATCH) {
            let batch = mail_landing_batch(account, chunk)?;
            landed += landing.land_applemail_batch(&batch).await? as i64;
        }

        // The checkpoint moves only after every row in this page has landed or replayed.
        let replayed = records.len() as i64 - landed;
        let next = IntakeCheckpointUpdate {
            source: INTAKE_SOURCE.to_owned(),
            source_account: account.to_owned(),
            shard_key: shard.clone(),
            shard_start: page.since.clone(),
            shard_end: page.before.clone(),
            cursor: page.next_cursor.map(cursor_token),
            pages_completed: checkpoint.pages_completed + 1,
            records_seen: checkpoint.records_seen + records.len() as i64,
            records_landed: checkpoint.records_landed + landed,
            records_replayed: checkpoint.records_replayed + replayed,
            status: if page.complete {
                "complete".to_owned()
            } else {
                "in_progress".to_owned()
            },
        };
        landing.save_intake_checkpoint(&next).await?;

        tally.pages += 1;
        tally.records += records.len() as i64;
        tally.landed += landed;
        tally.replayed += replayed;
        pages_this_run += 1;

        println!(
            "applemail {account} band={band} page={} received={} landed={} replayed={} complete={}",
            next.pages_completed,
            records.len(),
            landed,
            replayed,
            if page.complete { "yes" } else { "no" }
        );

        checkpoint = IntakeCheckpoint {
            cursor: next.cursor.clone(),
            pages_completed: next.pages_completed,
            records_seen: next.records_seen,
            records_landed: next.records_landed,
            records_replayed: next.records_replayed,
            status: next.status.clone(),
        };

        if page.complete {
            tally.complete = true;
            println!(
                "Apple Mail band {band} ({}) complete: pages={} records={} landed={} replayed={}",
                band_label(band),
                checkpoint.pages_completed,
                checkpoint.records_seen,
                checkpoint.records_landed,
                checkpoint.records_replayed
            );
            if checkpoint.records_seen == 0 {
                eprintln!(
                    "applemail WARNING {account} band={band} landed nothing. The extractor reads this \
                     account's Inbox and Sent mailboxes only, so a zero means Mail.app delivered no mail \
                     into them for this window - a Gmail account whose traffic sits in [Gmail]/All Mail \
                     reads as empty. Measured on this Mac, 2026-09-28: both Gmail accounts do. Nothing \
                     landed is not the same as nothing to land."
                );
            }
            return Ok(tally);
        }
        if page.next_cursor.is_none() {
            return Err(io::Error::other(
                "extractor returned an incomplete page without a continuation cursor",
            )
            .into());
        }
        if max_pages.map(|limit| pages_this_run >= limit).unwrap_or(false) {
            println!(
                "stopped cleanly after --max-pages={}; rerun the same command to resume",
                max_pages.unwrap_or_default()
            );
            return Ok(tally);
        }
    }
}

/// One page's rows as landing inputs. The replay identity is computed here and a row without one
/// is a refusal: landing under a made-up key would corrupt the replay guarantee.
fn mail_landing_batch(
    account: &str,
    records: &[LocalMailRecord],
) -> Result<Vec<AppleMailLanding>, Box<dyn Error>> {
    let mut batch: Vec<AppleMailLanding> = Vec::with_capacity(records.len());
    for record in records {
        let Some(source_message_id) = apple_mail_replay_id(
            record.message_id.as_deref(),
            &record.mailbox,
            record.local_id,
        ) else {
            return Err(io::Error::other(format!(
                "Apple Mail row {} has no stable replay identity (mailbox {}, no Message-ID)",
                record.local_id, record.mailbox
            ))
            .into());
        };
        batch.push(AppleMailLanding {
            source_account: account.to_owned(),
            source_message_id,
            mailbox_kind: Some(record.mailbox.clone()),
            mailbox_name: record.mailbox_name.clone(),
            local_id: Some(record.local_id),
            message_id: record.message_id.clone(),
            occurred_at: record.occurred_at.clone(),
            sender: record.sender.clone(),
            to_recipients: json!(record.to),
            cc_recipients: json!(record.cc),
            bcc_recipients: json!(record.bcc),
            subject: record.subject.clone(),
            // The exporter's own record for this row: provenance, envelope metadata only.
            raw: json!({
                "mailbox": record.mailbox,
                "mailboxName": record.mailbox_name,
                "localId": record.local_id,
                "messageId": record.message_id,
                "occurredAt": record.occurred_at,
                "sender": record.sender,
                "to": record.to,
                "cc": record.cc,
                "bcc": record.bcc,
                "subject": record.subject,
            }),
        });
    }
    Ok(batch)
}

/// `apple-sync mail-intake` — land every configured account's mail for the requested band(s).
///
/// One account Mail.app will not answer for must not stop the others: a daily sync that dies on
/// the first account is useless. Its failure is reported and the exit code is non-zero.
pub async fn mail_intake(args: &[String]) -> Result<(), Box<dyn Error>> {
    crate::apple_sync::load_env();
    let target = target_arg(args)?;
    let accounts = configured_accounts(option(args, "--account"))?;
    let bands = bands_arg(args)?;
    let page_size = positive_int(args, "--page-size", LANDING_BATCH as i64)?.min(1000);
    let max_pages = option(args, "--max-pages")
        .map(|_| positive_int(args, "--max-pages", 1))
        .transpose()?;
    let verify = args.iter().any(|arg| arg == "--verify");
    let reset = args.iter().any(|arg| arg == "--reset-checkpoint");

    let database = connect(target).await?;
    let landing = LandingDao::new(database);

    let mut failures: Vec<String> = Vec::new();
    let mut summary = Vec::new();
    for account in &accounts {
        let account_id = match resolve_account_id(account) {
            Ok(account_id) => account_id,
            Err(error) => {
                failures.push(format!("{account}: {error}"));
                continue;
            }
        };

        if verify {
            match extract_page(account, &account_id, &bands[0], page_size, None, true) {
                Ok(page) => {
                    let mailboxes = page.mailboxes.unwrap_or(ExtractMailboxes {
                        inbox: Vec::new(),
                        sent: Vec::new(),
                    });
                    println!("Apple Mail Envelope Index verification OK");
                    println!("account={}", page.account.unwrap_or_else(|| account.clone()));
                    println!(
                        "account_id={}",
                        page.account_id.unwrap_or_else(|| account_id.clone())
                    );
                    println!("mail_version={}", page.mail_version.unwrap_or_default());
                    println!("band={} ({})", bands[0], band_label(&bands[0]));
                    println!(
                        "window={} .. {}",
                        page.since.unwrap_or_default(),
                        page.before.unwrap_or_else(|| "now".to_owned())
                    );
                    println!("inbox={}", mailboxes.inbox.join(", "));
                    println!("sent={}", mailboxes.sent.join(", "));
                    println!("sample_records={}", page.records.len());
                    println!("database_writes=0");
                }
                Err(error) => failures.push(format!("{account} verify: {error}")),
            }
            continue;
        }

        for band in &bands {
            match intake_band(
                &landing,
                account,
                &account_id,
                band,
                page_size,
                max_pages,
                reset,
            )
            .await
            {
                Ok(tally) => summary.push(json!({
                    "account": account,
                    "band": band,
                    "pages": tally.pages,
                    "records": tally.records,
                    "landed": tally.landed,
                    "replayed": tally.replayed,
                    "complete": tally.complete,
                })),
                Err(error) => {
                    failures.push(format!("{account} band {band}: {error}"));
                    break;
                }
            }
        }
    }

    println!(
        "{}",
        json!({
            "source": INTAKE_SOURCE,
            "target": target.as_str(),
            "bands": bands,
            "accounts": accounts,
            "summary": summary,
            "failures": failures,
        })
    );

    if failures.is_empty() {
        Ok(())
    } else {
        for failure in &failures {
            eprintln!("applemail FAILED {failure}");
        }
        Err(io::Error::other(format!(
            "{} account/band failure(s); the rest of the run completed",
            failures.len()
        ))
        .into())
    }
}

/// `apple-sync mail-promote` — `l_applemail` -> evidence -> interaction -> client read models.
///
/// The ONLY reader of a mail landing table. Nothing client-facing reads `l_applemail`, and this
/// pass never re-reads Mail: it works from what was landed, so the classification rules live in
/// one place (`domain::applemail`) instead of two.
pub async fn mail_promote(args: &[String]) -> Result<(), Box<dyn Error>> {
    crate::apple_sync::load_env();
    let target = target_arg(args)?;
    let days = positive_int(args, "--days", 90)?;
    let account = option(args, "--account").map(str::to_owned);
    let verify_only = args.iter().any(|arg| arg == "--verify");
    let internal = internal_addresses()?;

    let database = connect(target).await?;
    let landing = LandingDao::new(database.clone());
    let evidence_dao = RelationshipEvidenceDao::new(database);

    let rows = landing.landed_applemail(days, account.as_deref()).await?;
    let MailNormalization {
        observations,
        skipped,
    } = normalize_landed_mail(&rows, &internal);
    let accounts: BTreeSet<String> = observations
        .iter()
        .map(|row| row.source_account.clone())
        .collect();
    println!(
        "l_applemail: {} landed rows in the last {} day(s){} -> {} observations",
        rows.len(),
        days,
        account
            .as_deref()
            .map(|value| format!(" for {value}"))
            .unwrap_or_default(),
        observations.len()
    );
    println!("skipped: {}", json!(skipped));

    if verify_only {
        println!(
            "verify only — no writes. accounts={}",
            accounts.iter().cloned().collect::<Vec<_>>().join(", ")
        );
        return Ok(());
    }
    if observations.is_empty() {
        println!("nothing to promote.");
        return Ok(());
    }

    // Evidence, reconciled to a canonical Person — one row per counterparty email.
    let builds = build_mail_evidence(&observations);
    let owners = crate::apple_messages::OwnerIndex::load(&evidence_dao).await?;
    let links: HashMap<(String, String), String> = evidence_dao
        .source_links(ICLOUD_MAIL_SOURCE)
        .await?
        .into_iter()
        .map(|link| {
            (
                (link.source_account, link.source_identity_key),
                link.canonical_person_id,
            )
        })
        .collect();

    let mut tally: BTreeMap<String, i64> = BTreeMap::new();
    for row in &builds {
        let lookup = crate::apple_messages::lookup_for(row, &owners, &links);
        let decision = domain::decide_apple_handle(row, &lookup);
        *tally.entry(decision.review_state.clone()).or_insert(0) += 1;
        let id = evidence_dao
            .upsert_evidence(&db::EvidenceUpsert::from(row))
            .await?;
        evidence_dao.record_decision(&id, &decision).await?;
    }
    println!(
        "evidence: {} counterparties, reconcile tally {}",
        builds.len(),
        json!(tally)
    );

    // Only evidence already linked to a Person can become a comms event. The link is read back
    // rather than taken from the decision in memory: the row is the one writer of a link, and an
    // established link survives a later ambiguous pass (`record_decision` keeps it).
    let linked: HashMap<(String, String), String> = evidence_dao
        .candidates(Some(ICLOUD_MAIL_SOURCE), Some("exact_linked"), &[], 10000)
        .await?
        .into_iter()
        .filter(|row| accounts.contains(&row.source_account))
        .filter_map(|row| {
            row.canonical_person_id
                .map(|person_id| ((row.source_account, row.source_identity_key), person_id))
        })
        .collect();

    let mut inserted = 0i64;
    let mut replayed = 0i64;
    let mut unlinked = 0i64;
    for observation in &observations {
        let Some(person_id) = linked.get(&(
            observation.source_account.clone(),
            observation.external_email.clone(),
        )) else {
            // Evidence stays staged for the unlinked: this is a decision, not a failure.
            unlinked += 1;
            continue;
        };
        let interaction = mail_observation_interaction(observation, person_id);
        let draft = InteractionDraft {
            person_id: interaction.person_id,
            channel: interaction.channel.to_owned(),
            event_type: interaction.event_type.to_owned(),
            direction: Some(interaction.direction.to_owned()),
            occurred_at: interaction.occurred_at,
            title: interaction.title,
            source_system: interaction.source_system.to_owned(),
            source_external_id: interaction.source_external_id,
            source_metadata: interaction.source_metadata,
        };
        if landing.create_interaction(&draft).await? {
            inserted += 1;
        } else {
            replayed += 1;
        }
    }

    let refreshed = inserted > 0;
    if refreshed {
        landing.refresh_client_read_models().await?;
    }
    println!(
        "{}",
        json!({
            "source": ICLOUD_MAIL_SOURCE,
            "target": target.as_str(),
            "days": days,
            "landedRows": rows.len(),
            "observations": observations.len(),
            "skipped": skipped,
            "evidenceRows": builds.len(),
            "reconcileTally": tally,
            "interactionsInserted": inserted,
            "interactionsReplayed": replayed,
            "unlinked": unlinked,
            "refreshed": refreshed,
        })
    );
    println!(
        "promotion complete: interactions inserted={inserted} replayed={replayed} unlinked={unlinked} (evidence stays staged for the unlinked)"
    );
    Ok(())
}
