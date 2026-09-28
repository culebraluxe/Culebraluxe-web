// ---------------------------------------------------------------------------
// ODS acquisition DAOs: landing, replay-safe interaction writes, resumable checkpoints.
//
// The landing layer is the ONLY intake target (migration 158). A feed lands in its own
// `l_*` table with no judgment, promotion reads it once, and nothing client-facing ever reads
// an `l_` table. These are the writes and reads that make that contract mechanical:
//
//   * `land_*_batch`      — set-based, one statement per page, `on conflict do nothing` on the
//                           source's own replay key. Batching is not an optimisation detail: a
//                           row-at-a-time load is two round trips per message, which is the
//                           difference between a job and an evening.
//   * `create_interaction`— the canonical comms event, replay-safe on
//                           `(source_system, source_external_id)`.
//   * checkpoint read/write — the resume token from migration 162. A tiny token, not a
//                           workflow: the intake owns its own loop.
//
// The `impl LandingDao` block lives here rather than in `landing.rs` because these are the ODS
// methods; the type is the same one, and a second file beats growing one past reading size.
// ---------------------------------------------------------------------------
use crate::{DbFailure, DbResult, LandingDao};
use domain::applemail::{LandedAppleMail, MailAddress};
use serde_json::Value;

/// One Apple Mail message on its way into `l_applemail`. Bounded envelope metadata only: `raw`
/// is the exporter's own record for the row, and no body, snippet or MIME ever reaches here.
#[derive(Debug, Clone)]
pub struct AppleMailLanding {
    pub source_account: String,
    pub source_message_id: String,
    pub mailbox_kind: Option<String>,
    pub mailbox_name: Option<String>,
    pub local_id: Option<i64>,
    pub message_id: Option<String>,
    pub occurred_at: Option<String>,
    pub sender: Option<String>,
    pub to_recipients: Value,
    pub cc_recipients: Value,
    pub bcc_recipients: Value,
    pub subject: Option<String>,
    pub raw: Value,
}

/// One Google mail message on its way into `l_email`. Metadata only: no body preview, no snippet.
#[derive(Debug, Clone)]
pub struct EmailLanding {
    pub source_account: String,
    pub source_message_id: String,
    pub thread_id: Option<String>,
    pub from_address: Option<String>,
    pub to_address: Option<String>,
    pub subject: Option<String>,
    pub sent_at: Option<String>,
    pub raw: Value,
}

/// One row of `l_applemail` as the repository reads it. Normalization happens on the way out:
/// `occurred_at` is an ISO-8601 UTC string and the recipient lists are typed addresses, so no
/// promotion code needs to know what the driver returned.
#[derive(Debug, sqlx::FromRow)]
struct LandedAppleMailDbRow {
    source_account: String,
    source_message_id: String,
    mailbox_kind: Option<String>,
    mailbox_name: Option<String>,
    local_id: Option<i64>,
    message_id: Option<String>,
    occurred_at: Option<String>,
    sender: Option<String>,
    to_recipients: Option<Value>,
    cc_recipients: Option<Value>,
    bcc_recipients: Option<Value>,
    subject: Option<String>,
}

/// One canonical comms event written from promoted evidence.
#[derive(Debug, Clone)]
pub struct InteractionDraft {
    pub person_id: String,
    pub channel: String,
    pub event_type: String,
    pub direction: Option<String>,
    pub occurred_at: String,
    pub title: Option<String>,
    pub source_system: String,
    pub source_external_id: String,
    pub source_metadata: Value,
}

/// A resumable acquisition cursor (`integration_intake_checkpoint`, migration 162).
#[derive(Debug, Clone, Default)]
pub struct IntakeCheckpoint {
    pub cursor: Option<String>,
    pub pages_completed: i64,
    pub records_seen: i64,
    pub records_landed: i64,
    pub records_replayed: i64,
    pub status: String,
}

/// A checkpoint advance, written only after every record of a page has landed or replayed.
#[derive(Debug, Clone)]
pub struct IntakeCheckpointUpdate {
    pub source: String,
    pub source_account: String,
    pub shard_key: String,
    pub shard_start: Option<String>,
    pub shard_end: Option<String>,
    pub cursor: Option<String>,
    pub pages_completed: i64,
    pub records_seen: i64,
    pub records_landed: i64,
    pub records_replayed: i64,
    pub status: String,
}

impl LandingDao {
    /// Set-based landing for a bounded batch of Apple Mail rows. Returns how many rows were
    /// genuinely new; a re-read of the same window lands nothing twice.
    pub async fn land_applemail_batch(&self, inputs: &[AppleMailLanding]) -> DbResult<usize> {
        if inputs.is_empty() {
            return Ok(0);
        }
        let account: Vec<String> = inputs
            .iter()
            .map(|input| input.source_account.clone())
            .collect();
        let message_id: Vec<String> = inputs
            .iter()
            .map(|input| input.source_message_id.clone())
            .collect();
        let mailbox_kind: Vec<Option<String>> = inputs
            .iter()
            .map(|input| input.mailbox_kind.clone())
            .collect();
        let mailbox_name: Vec<Option<String>> = inputs
            .iter()
            .map(|input| input.mailbox_name.clone())
            .collect();
        let local_id: Vec<Option<i64>> = inputs.iter().map(|input| input.local_id).collect();
        let rfc_message_id: Vec<Option<String>> = inputs
            .iter()
            .map(|input| input.message_id.clone())
            .collect();
        let occurred_at: Vec<Option<String>> = inputs
            .iter()
            .map(|input| input.occurred_at.clone())
            .collect();
        let sender: Vec<Option<String>> =
            inputs.iter().map(|input| input.sender.clone()).collect();
        let to: Vec<String> = inputs
            .iter()
            .map(|input| serde_json::to_string(&input.to_recipients).unwrap_or_else(|_| "[]".into()))
            .collect();
        let cc: Vec<String> = inputs
            .iter()
            .map(|input| serde_json::to_string(&input.cc_recipients).unwrap_or_else(|_| "[]".into()))
            .collect();
        let bcc: Vec<String> = inputs
            .iter()
            .map(|input| {
                serde_json::to_string(&input.bcc_recipients).unwrap_or_else(|_| "[]".into())
            })
            .collect();
        let subject: Vec<Option<String>> =
            inputs.iter().map(|input| input.subject.clone()).collect();
        let raw: Vec<String> = inputs
            .iter()
            .map(|input| serde_json::to_string(&input.raw).unwrap_or_else(|_| "null".to_owned()))
            .collect();

        let rows = sqlx::query(
            r#"
            insert into l_applemail (
              source_account, source_message_id, mailbox_kind, mailbox_name, local_id,
              message_id, occurred_at, sender, to_recipients, cc_recipients, bcc_recipients,
              subject, raw
            )
            select
              t.source_account, t.source_message_id, t.mailbox_kind, t.mailbox_name,
              t.local_id, t.message_id, t.occurred_at::timestamptz, t.sender,
              (t.to_recipients)::jsonb, (t.cc_recipients)::jsonb, (t.bcc_recipients)::jsonb,
              t.subject, (t.raw)::jsonb
            from unnest(
              $1::text[], $2::text[], $3::text[], $4::text[], $5::bigint[],
              $6::text[], $7::text[], $8::text[], $9::text[], $10::text[], $11::text[],
              $12::text[], $13::text[]
            ) as t(
              source_account, source_message_id, mailbox_kind, mailbox_name, local_id,
              message_id, occurred_at, sender, to_recipients, cc_recipients, bcc_recipients,
              subject, raw
            )
            on conflict (coalesce(source_account, ''), source_message_id) do nothing
            returning id
            "#,
        )
        .bind(account)
        .bind(message_id)
        .bind(mailbox_kind)
        .bind(mailbox_name)
        .bind(local_id)
        .bind(rfc_message_id)
        .bind(occurred_at)
        .bind(sender)
        .bind(to)
        .bind(cc)
        .bind(bcc)
        .bind(subject)
        .bind(raw)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("landing.applemail.batch", &error))?;
        Ok(rows.len())
    }

    /// Set-based landing for a bounded batch of Google mail rows (metadata only). Same contract as
    /// the Apple landing: replay-safe on `(source_account, source_message_id)`, nothing judged here.
    pub async fn land_email_batch(&self, inputs: &[EmailLanding]) -> DbResult<usize> {
        if inputs.is_empty() {
            return Ok(0);
        }
        let account: Vec<String> = inputs
            .iter()
            .map(|input| input.source_account.clone())
            .collect();
        let message_id: Vec<String> = inputs
            .iter()
            .map(|input| input.source_message_id.clone())
            .collect();
        let thread_id: Vec<Option<String>> =
            inputs.iter().map(|input| input.thread_id.clone()).collect();
        let from_address: Vec<Option<String>> = inputs
            .iter()
            .map(|input| input.from_address.clone())
            .collect();
        let to_address: Vec<Option<String>> = inputs
            .iter()
            .map(|input| input.to_address.clone())
            .collect();
        let subject: Vec<Option<String>> =
            inputs.iter().map(|input| input.subject.clone()).collect();
        let sent_at: Vec<Option<String>> = inputs.iter().map(|input| input.sent_at.clone()).collect();
        let raw: Vec<String> = inputs
            .iter()
            .map(|input| serde_json::to_string(&input.raw).unwrap_or_else(|_| "null".to_owned()))
            .collect();

        let rows = sqlx::query(
            r#"
            insert into l_email (
              source_account, source_message_id, thread_id, from_address, to_address,
              subject, sent_at, body_preview, labels, raw
            )
            select
              t.source_account, t.source_message_id, t.thread_id, t.from_address, t.to_address,
              t.subject, t.sent_at::timestamptz, null, null::text[], (t.raw)::jsonb
            from unnest(
              $1::text[], $2::text[], $3::text[], $4::text[], $5::text[],
              $6::text[], $7::text[], $8::text[]
            ) as t(
              source_account, source_message_id, thread_id, from_address, to_address,
              subject, sent_at, raw
            )
            on conflict (coalesce(source_account, ''), source_message_id) do nothing
            returning id
            "#,
        )
        .bind(account)
        .bind(message_id)
        .bind(thread_id)
        .bind(from_address)
        .bind(to_address)
        .bind(subject)
        .bind(sent_at)
        .bind(raw)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("landing.email.batch", &error))?;
        Ok(rows.len())
    }

    /// The landed mail one promotion pass considers, oldest first. `days` bounds the window; an
    /// account narrows it to one mailbox account.
    ///
    /// `occurred_at` is formatted to ISO-8601 UTC with milliseconds in SQL, so the value is the
    /// shape the deleted promotion compared and window ordering is unchanged.
    pub async fn landed_applemail(
        &self,
        days: i64,
        account: Option<&str>,
    ) -> DbResult<Vec<LandedAppleMail>> {
        let rows = sqlx::query_as::<_, LandedAppleMailDbRow>(
            r#"
            select source_account, source_message_id, mailbox_kind, mailbox_name, local_id,
                   message_id,
                   to_char(occurred_at at time zone 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"')
                     as occurred_at,
                   sender, to_recipients, cc_recipients, bcc_recipients, subject
              from l_applemail
             where occurred_at >= now() - make_interval(days => $1::int)
               and ($2::text is null or lower(source_account) = lower($2::text))
             order by occurred_at asc, source_message_id asc
            "#,
        )
        .bind(days.clamp(1, 3650) as i32)
        .bind(account)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("landing.applemail.landed", &error))?;

        Ok(rows
            .into_iter()
            .map(|row| LandedAppleMail {
                source_account: row.source_account,
                source_message_id: row.source_message_id,
                mailbox_kind: row.mailbox_kind,
                mailbox_name: row.mailbox_name,
                local_id: row.local_id,
                message_id: row.message_id,
                occurred_at: row.occurred_at,
                sender: row.sender,
                to_recipients: mail_addresses(row.to_recipients),
                cc_recipients: mail_addresses(row.cc_recipients),
                bcc_recipients: mail_addresses(row.bcc_recipients),
                subject: row.subject,
            })
            .collect())
    }

    /// Write one canonical comms event, replay-safe on `(source_system, source_external_id)`.
    /// `false` means the event was already there — a replay, not a failure.
    pub async fn create_interaction(&self, input: &InteractionDraft) -> DbResult<bool> {
        let row = sqlx::query_scalar::<_, String>(
            r#"
            insert into interaction (
              person_id, property_id, deal_id, channel, event_type, direction, occurred_at,
              title, summary, duration_seconds, source_system, source_external_id, source_metadata
            ) values (
              $1::uuid, null, null, $2, $3, $4, $5::timestamptz,
              $6, null, null, $7, $8, $9
            )
            on conflict (source_system, source_external_id)
              where source_system is not null and source_external_id is not null
            do nothing
            returning id::text
            "#,
        )
        .bind(&input.person_id)
        .bind(&input.channel)
        .bind(&input.event_type)
        .bind(&input.direction)
        .bind(&input.occurred_at)
        .bind(&input.title)
        .bind(&input.source_system)
        .bind(&input.source_external_id)
        .bind(&input.source_metadata)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("landing.interaction.create", &error))?;
        Ok(row.is_some())
    }

    /// The saved resume token for one `(source, source_account, shard)`, when there is one.
    pub async fn intake_checkpoint(
        &self,
        source: &str,
        source_account: &str,
        shard_key: &str,
    ) -> DbResult<Option<IntakeCheckpoint>> {
        let row = sqlx::query_as::<_, (Option<String>, i64, i64, i64, i64, String)>(
            r#"
            select cursor,
                   pages_completed::bigint as pages_completed,
                   records_seen,
                   records_landed,
                   records_replayed,
                   status
              from integration_intake_checkpoint
             where source = $1 and source_account = $2 and shard_key = $3
            "#,
        )
        .bind(source)
        .bind(source_account)
        .bind(shard_key)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("landing.checkpoint.read", &error))?;

        Ok(row.map(
            |(cursor, pages_completed, records_seen, records_landed, records_replayed, status)| {
                IntakeCheckpoint {
                    cursor,
                    pages_completed,
                    records_seen,
                    records_landed,
                    records_replayed,
                    status,
                }
            },
        ))
    }

    /// Advance a checkpoint. Called only after every record of the page has landed, so an
    /// interrupted page is simply redone and the landing replay key absorbs the duplicates.
    pub async fn save_intake_checkpoint(&self, update: &IntakeCheckpointUpdate) -> DbResult<()> {
        sqlx::query(
            r#"
            insert into integration_intake_checkpoint (
              source, source_account, shard_key, shard_start, shard_end, cursor,
              pages_completed, records_seen, records_landed, records_replayed, status
            ) values (
              $1, $2, $3, $4::timestamptz, $5::timestamptz, $6,
              $7, $8, $9, $10, $11
            )
            on conflict (source, source_account, shard_key) do update set
              shard_start = excluded.shard_start,
              shard_end = excluded.shard_end,
              cursor = excluded.cursor,
              pages_completed = excluded.pages_completed,
              records_seen = excluded.records_seen,
              records_landed = excluded.records_landed,
              records_replayed = excluded.records_replayed,
              status = excluded.status,
              updated_at = now()
            "#,
        )
        .bind(&update.source)
        .bind(&update.source_account)
        .bind(&update.shard_key)
        .bind(&update.shard_start)
        .bind(&update.shard_end)
        .bind(&update.cursor)
        .bind(update.pages_completed)
        .bind(update.records_seen)
        .bind(update.records_landed)
        .bind(update.records_replayed)
        .bind(&update.status)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("landing.checkpoint.write", &error))?;
        Ok(())
    }
}

/// Recipient jsonb -> typed addresses. A value the column cannot represent is reported as
/// absent rather than invented; landing never validates, promotion does.
fn mail_addresses(value: Option<Value>) -> Vec<MailAddress> {
    let Some(value) = value else {
        return Vec::new();
    };
    serde_json::from_value::<Vec<MailAddress>>(value).unwrap_or_default()
}
