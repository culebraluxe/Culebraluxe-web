//! Moved from `apple_mail.rs` (move only): target_arg, connect, repo_root, option, bands_arg, positive_int, own_addresses, configured_accounts, internal_addresses, resolve_account_id.

#[allow(unused_imports)]
use super::*;

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

pub(super) fn repo_root() -> PathBuf {
    crate::apple_sync::repo_root()
}

pub(crate) fn option<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
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
pub(super) fn bands_arg(args: &[String]) -> Result<Vec<String>, Box<dyn Error>> {
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
/// Every address this machine syncs mail for: the configured account list plus the
/// single-mailbox fallbacks. One source of truth, because the same list answers two questions —
/// which accounts to read, and which addresses can never be the counterparty.
pub(super) fn own_addresses() -> Vec<String> {
    let mut addresses: Vec<String> = Vec::new();
    let mut push = |value: &str| {
        if let Some(normalized) = domain::applemail::normalize_mailbox(value) {
            if !addresses.contains(&normalized) {
                addresses.push(normalized);
            }
        }
    };
    for key in [
        "MAIL_APP_ACCOUNTS",
        "APPLE_MAILBOX_ADDRESS",
        "ICLOUD_MAIL_ADDRESS",
    ] {
        for entry in std::env::var(key).unwrap_or_default().split(',') {
            push(entry);
        }
    }
    addresses
}

pub(super) fn configured_accounts(only: Option<&str>) -> Result<Vec<String>, Box<dyn Error>> {
    if let Some(account) = only.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok(vec![account.to_lowercase()]);
    }
    let accounts = own_addresses();
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
pub(super) fn internal_addresses() -> Result<BTreeSet<String>, Box<dyn Error>> {
    let raw = std::env::var("EMAIL_INTERNAL_ADDRESSES").unwrap_or_default();
    let mut addresses: BTreeSet<String> = raw
        .split(',')
        .filter_map(domain::applemail::normalize_mailbox)
        .collect();
    if addresses.is_empty() {
        return Err(io::Error::other(
            "EMAIL_INTERNAL_ADDRESSES is required: it decides inbound vs outbound",
        )
        .into());
    }
    // An account's own address is internal by construction: it is never the counterparty of
    // the mail it sends or receives, whether or not EMAIL_INTERNAL_ADDRESSES lists it.
    addresses.extend(own_addresses());
    Ok(addresses)
}

/// The Mail.app account UUID for one configured address: an explicit override, else Mail's own
/// answer through its AppleEvent bridge (the extractor never crosses that bridge for messages).
pub(super) fn resolve_account_id(account: &str) -> Result<String, Box<dyn Error>> {
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
