//! Moved from `apple_mail.rs` (move only): BandTally, intake_band, mail_landing_batch.

#[allow(unused_imports)]
use super::*;

/// What one band read did. Counters only — a run reports what it landed, never a message.
#[derive(Debug, Default)]
pub(super) struct BandTally {
    pub(super) pages: i64,
    pub(super) records: i64,
    pub(super) landed: i64,
    pub(super) replayed: i64,
    pub(super) complete: bool,
}

/// Read one band for one account, page by page, landing each page before the checkpoint moves.
///
/// A band marked complete is deliberately NOT skipped: every band slides with "now", so the
/// newest band must be re-read to pick up what has arrived since. Landing is replay-safe, so the
/// only cost is the read. The checkpoint earns its keep by resuming an interrupted run.
pub(super) async fn intake_band(
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
pub(super) fn mail_landing_batch(
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
