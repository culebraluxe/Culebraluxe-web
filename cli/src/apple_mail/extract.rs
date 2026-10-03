//! Moved from `apple_mail.rs` (move only): ExtractPage, ExtractMailboxes, ExtractCursor, LocalMailRecord, extract_page, checkpoint_shard, cursor_token, parse_cursor.

#[allow(unused_imports)]
use super::*;

/// One page as the extractor reports it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExtractPage {
    #[serde(default)]
    pub(super) ok: bool,
    pub(super) account: Option<String>,
    pub(super) account_id: Option<String>,
    pub(super) mail_version: Option<String>,
    pub(super) since: Option<String>,
    pub(super) before: Option<String>,
    pub(super) mailboxes: Option<ExtractMailboxes>,
    #[serde(default)]
    pub(super) records: Vec<LocalMailRecord>,
    pub(super) next_cursor: Option<ExtractCursor>,
    #[serde(default)]
    pub(super) complete: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct ExtractMailboxes {
    #[serde(default)]
    pub(super) inbox: Vec<String>,
    #[serde(default)]
    pub(super) sent: Vec<String>,
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub(super) struct ExtractCursor {
    pub(super) date: f64,
    pub(super) rowid: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LocalMailRecord {
    pub(super) mailbox: String,
    pub(super) mailbox_name: Option<String>,
    pub(super) local_id: i64,
    pub(super) message_id: Option<String>,
    pub(super) occurred_at: Option<String>,
    pub(super) sender: Option<String>,
    #[serde(default)]
    pub(super) to: Vec<MailAddress>,
    #[serde(default)]
    pub(super) cc: Vec<MailAddress>,
    #[serde(default)]
    pub(super) bcc: Vec<MailAddress>,
    pub(super) subject: Option<String>,
}

/// One bounded page from the local Envelope Index. Read-only: the bridge snapshots the store and
/// opens the snapshot with `query_only`.
pub(super) fn extract_page(
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
pub(super) fn checkpoint_shard(band: &str) -> String {
    format!("band:{band}")
}

/// The cursor as one text token: the extractor's own keyset pair.
pub(super) fn cursor_token(cursor: ExtractCursor) -> String {
    format!("{},{}", cursor.date, cursor.rowid)
}

pub(super) fn parse_cursor(token: Option<&str>) -> Option<ExtractCursor> {
    let (date, rowid) = token?.split_once(',')?;
    Some(ExtractCursor {
        date: date.trim().parse().ok()?,
        rowid: rowid.trim().parse().ok()?,
    })
}
