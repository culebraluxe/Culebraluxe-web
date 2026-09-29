// ---------------------------------------------------------------------------
// Landing (ODS) writes and the client read-model refresh.
//
// The landing tables (`l_imessage`, `l_email`, `l_applemail`, `l_call`, `l_whatsapp`, ...) keep the
// raw observed record so a source can always be re-derived; `interaction` is the canonical
// relationship memory built from it. Both are replay-safe: the landing insert is
// `on conflict do nothing` on `(coalesce(source_account,''), source_message_id)` and the canonical
// write is keyed on `(source_system, source_external_id)`.
//
// Batching is not an optimisation detail here, it is the difference between minutes and hours: the
// TypeScript materializer's five hours of Apple intake were 93,000 messages x 2 round trips, not
// database work. One set-based statement per 500 rows is what makes this a job instead of an evening.
// ---------------------------------------------------------------------------
use crate::{Database, DbFailure, DbResult};
use serde_json::Value;
use sqlx::Row;

/// One raw Apple message on its way into `l_imessage`.
#[derive(Debug, Clone)]
pub struct ImessageLanding {
    pub source_account: Option<String>,
    pub source_message_id: String,
    pub conversation_id: Option<String>,
    pub handle: Option<String>,
    /// `outgoing` when the owner sent it.
    pub direction: Option<String>,
    pub service: Option<String>,
    pub sent_at: Option<String>,
    pub text: Option<String>,
    pub raw: Value,
}

/// One raw Apple call on its way into `l_call`. Video is FaceTime; direction is the source's own.
#[derive(Debug, Clone)]
pub struct CallLanding {
    pub source_account: Option<String>,
    pub source_message_id: String,
    pub handle: Option<String>,
    /// `outgoing` / `incoming`.
    pub direction: Option<String>,
    /// `audio` / `video`.
    pub call_type: Option<String>,
    pub answered: Option<bool>,
    /// Source precision, never rounded: the landing table is evidence, and migration 161 exists
    /// because rounding Apple's float here was interpretation.
    pub duration_seconds: Option<f64>,
    pub started_at: Option<String>,
    pub raw: Value,
}

/// Input for [`LandingDao::upsert_latest_interaction`].
#[derive(Debug, Clone)]
pub struct LatestInteraction {
    pub person_id: String,
    pub channel: String,
    pub event_type: String,
    pub direction: Option<String>,
    pub occurred_at: String,
    pub summary: Option<String>,
    /// Call duration in whole seconds. `None` for channels that have no such notion.
    pub duration_seconds: Option<i64>,
    pub source_system: String,
    pub source_external_id: String,
    pub source_metadata: Value,
}

/// What a latest-interaction upsert did. `Ignored` means the stored row is newer than the candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatestInteractionOutcome {
    Inserted,
    Updated,
    Ignored,
}

#[derive(Clone)]
pub struct LandingDao {
    /// Crate-visible so the ODS module (`apple_ods`) can add its own statements to the same DAO.
    pub(crate) db: Database,
}

impl LandingDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Set-based landing for a bounded batch. Returns how many rows were genuinely new.
    pub async fn land_imessage_batch(&self, inputs: &[ImessageLanding]) -> DbResult<usize> {
        if inputs.is_empty() {
            return Ok(0);
        }
        let account: Vec<Option<String>> = inputs
            .iter()
            .map(|input| input.source_account.clone())
            .collect();
        let message_id: Vec<String> = inputs
            .iter()
            .map(|input| input.source_message_id.clone())
            .collect();
        let conversation: Vec<Option<String>> = inputs
            .iter()
            .map(|input| input.conversation_id.clone())
            .collect();
        let handle: Vec<Option<String>> = inputs.iter().map(|input| input.handle.clone()).collect();
        let direction: Vec<Option<String>> =
            inputs.iter().map(|input| input.direction.clone()).collect();
        let service: Vec<Option<String>> =
            inputs.iter().map(|input| input.service.clone()).collect();
        let sent_at: Vec<Option<String>> =
            inputs.iter().map(|input| input.sent_at.clone()).collect();
        let text: Vec<Option<String>> = inputs.iter().map(|input| input.text.clone()).collect();
        let raw: Vec<String> = inputs
            .iter()
            .map(|input| serde_json::to_string(&input.raw).unwrap_or_else(|_| "null".to_owned()))
            .collect();

        let rows = sqlx::query(
            r#"
            insert into l_imessage (
              source_account, source_message_id, conversation_id, handle,
              direction, service, sent_at, text_content, raw
            )
            select
              t.source_account, t.source_message_id, t.conversation_id, t.handle,
              t.direction, t.service, t.sent_at, t.text_content, (t.raw)::jsonb
            from unnest(
              $1::text[], $2::text[], $3::text[], $4::text[], $5::text[], $6::text[],
              $7::timestamptz[], $8::text[], $9::text[]
            ) as t(
              source_account, source_message_id, conversation_id, handle,
              direction, service, sent_at, text_content, raw
            )
            on conflict (coalesce(source_account, ''), source_message_id) do nothing
            returning id
            "#,
        )
        .bind(account)
        .bind(message_id)
        .bind(conversation)
        .bind(handle)
        .bind(direction)
        .bind(service)
        .bind(sent_at)
        .bind(text)
        .bind(raw)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("landing.imessage.batch", &error))?;
        Ok(rows.len())
    }

    /// Set-based landing for a bounded batch of calls. Returns how many rows were genuinely new.
    ///
    /// Same contract as every landing write: `(coalesce(source_account,''), source_message_id)` is
    /// the replay key, `raw` carries the untouched source payload, and the duration keeps the
    /// source's own precision.
    pub async fn land_call_batch(&self, inputs: &[CallLanding]) -> DbResult<usize> {
        if inputs.is_empty() {
            return Ok(0);
        }
        let account: Vec<Option<String>> = inputs
            .iter()
            .map(|input| input.source_account.clone())
            .collect();
        let message_id: Vec<String> = inputs
            .iter()
            .map(|input| input.source_message_id.clone())
            .collect();
        let handle: Vec<Option<String>> = inputs.iter().map(|input| input.handle.clone()).collect();
        let direction: Vec<Option<String>> =
            inputs.iter().map(|input| input.direction.clone()).collect();
        let call_type: Vec<Option<String>> =
            inputs.iter().map(|input| input.call_type.clone()).collect();
        let answered: Vec<Option<bool>> = inputs.iter().map(|input| input.answered).collect();
        let duration: Vec<Option<f64>> = inputs.iter().map(|input| input.duration_seconds).collect();
        let started_at: Vec<Option<String>> =
            inputs.iter().map(|input| input.started_at.clone()).collect();
        let raw: Vec<String> = inputs
            .iter()
            .map(|input| serde_json::to_string(&input.raw).unwrap_or_else(|_| "null".to_owned()))
            .collect();

        let rows = sqlx::query(
            r#"
            insert into l_call (
              source_account, source_message_id, handle, direction, call_type,
              answered, duration_seconds, started_at, raw
            )
            select
              t.source_account, t.source_message_id, t.handle, t.direction, t.call_type,
              t.answered, t.duration_seconds, t.started_at, (t.raw)::jsonb
            from unnest(
              $1::text[], $2::text[], $3::text[], $4::text[], $5::text[],
              $6::bool[], $7::double precision[], $8::timestamptz[], $9::text[]
            ) as t(
              source_account, source_message_id, handle, direction, call_type,
              answered, duration_seconds, started_at, raw
            )
            on conflict (coalesce(source_account, ''), source_message_id) do nothing
            returning id
            "#,
        )
        .bind(account)
        .bind(message_id)
        .bind(handle)
        .bind(direction)
        .bind(call_type)
        .bind(answered)
        .bind(duration)
        .bind(started_at)
        .bind(raw)
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("landing.call.batch", &error))?;
        Ok(rows.len())
    }

    /// Upsert the newest interaction for one Person x channel.
    ///
    /// The identity is `latest:<person>:<channel>`, keyed on the source rather than the message, so
    /// each run UPDATES that one row instead of appending another — and the `where` clause means an
    /// older message can never overwrite a newer one.
    pub async fn upsert_latest_interaction(
        &self,
        input: &LatestInteraction,
    ) -> DbResult<LatestInteractionOutcome> {
        let row = sqlx::query(
            r#"
            insert into interaction (
              person_id, property_id, deal_id, channel, event_type, direction, occurred_at,
              title, summary, duration_seconds, source_system, source_external_id, source_metadata
            ) values (
              $1::uuid, null, null, $2, $3, $4, $5::timestamptz,
              null, $6, $10::int, $7, $8, $9
            )
            on conflict (source_system, source_external_id)
              where source_system is not null and source_external_id is not null
            do update set
              person_id = excluded.person_id,
              channel = excluded.channel,
              event_type = excluded.event_type,
              direction = excluded.direction,
              occurred_at = excluded.occurred_at,
              title = excluded.title,
              summary = excluded.summary,
              source_metadata = excluded.source_metadata
            where interaction.occurred_at < excluded.occurred_at
            returning id::text as id, (xmax = 0) as inserted
            "#,
        )
        .bind(&input.person_id)
        .bind(&input.channel)
        .bind(&input.event_type)
        .bind(&input.direction)
        .bind(&input.occurred_at)
        .bind(&input.summary)
        .bind(&input.source_system)
        .bind(&input.source_external_id)
        .bind(&input.source_metadata)
        .bind(input.duration_seconds)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("landing.interaction.latest", &error))?;

        let Some(row) = row else {
            return Ok(LatestInteractionOutcome::Ignored);
        };
        let inserted: bool = row
            .try_get("inserted")
            .map_err(|error| DbFailure::from_sqlx("landing.interaction.latest", &error))?;
        Ok(if inserted {
            LatestInteractionOutcome::Inserted
        } else {
            LatestInteractionOutcome::Updated
        })
    }

    /// Rebuild the three client materialized read models, dependency-safe order FIRST: the
    /// source-grain relationship channels feed the Client directory freshness.
    ///
    /// `refresh materialized view concurrently` is used (both carry a unique index) so readers are
    /// never blocked. This is the single place the read models are rebuilt.
    /// `warehouse_promote_apple_contacts` — the landing -> warehouse promotion for Apple Contacts.
    ///
    /// The transformation is a database function (`db/migrations/253_apple_contacts_promote.sql`): the
    /// rules about what is true of a person are set-based, and the rows never have to leave the
    /// database to be decided. This call is the shell around it — dry run by default, one jsonb tally
    /// back. Refreshing the client read models is the caller's job afterwards: a materialized view
    /// cannot be refreshed `concurrently` inside a function's transaction.
    pub async fn promote_apple_contacts(&self, apply: bool) -> DbResult<Value> {
        sqlx::query_scalar("select warehouse_promote_apple_contacts($1)")
            .bind(apply)
            .fetch_one(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("landing.promote.apple_contacts", &error))
    }

    /// `apple_contacts_load` — the landing intake for one Apple Contacts export.
    ///
    /// The transformation is a database function (`db/migrations/254_apple_contacts_load_project.sql`):
    /// the batch receipt, one inbox receipt per contact, the immutable staged revisions, the snapshot
    /// membership and the batch totals are all set-based in Neon. This call hands over the export as
    /// the caller read it from disk (raw contacts, untouched) plus the three facts only the caller
    /// knows — the export id/timestamp from the file and its sha256 — and prints the tally back.
    ///
    /// The payload travels as text and is cast to jsonb in the statement, so the parameter type is
    /// never left to inference.
    pub async fn load_apple_contacts(
        &self,
        payload: &Value,
        source_account: &str,
    ) -> DbResult<Value> {
        sqlx::query_scalar("select apple_contacts_load($1::text::jsonb, $2)")
            .bind(payload.to_string())
            .bind(source_account)
            .fetch_one(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("landing.load.apple_contacts", &error))
    }

    /// `apple_contacts_project` — rebuild `l_person` / `l_property` as the current snapshot projection
    /// of the Apple Contacts ODS.
    ///
    /// The function resolves the account and the latest LOADED batch itself when it is given neither,
    /// and does the whole rebuild in one transaction (the retired script had to open one by hand).
    pub async fn project_apple_contacts(&self, source_account: Option<&str>) -> DbResult<Value> {
        sqlx::query_scalar("select apple_contacts_project($1, null::uuid)")
            .bind(source_account)
            .fetch_one(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("landing.project.apple_contacts", &error))
    }

    pub async fn refresh_client_read_models(&self) -> DbResult<()> {
        for (operation, sql) in [
            (
                "landing.refresh.relationship_channels",
                "refresh materialized view concurrently mv_client_relationship_channels",
            ),
            (
                "landing.refresh.directory",
                "refresh materialized view concurrently mv_client_directory",
            ),
            (
                "landing.refresh.history",
                "refresh materialized view concurrently mv_client_contact_history",
            ),
        ] {
            sqlx::query(sql)
                .execute(self.db.pool())
                .await
                .map_err(|error| DbFailure::from_sqlx(operation, &error))?;
        }
        Ok(())
    }
}
