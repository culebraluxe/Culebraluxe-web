use crate::{Database, DbFailure, DbResult};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::FromRow;

#[derive(Debug, Clone)]
pub struct WhatsAppCanonicalInput {
    pub source_account: String,
    pub external_event_id: String,
    pub event_type: String,
    pub occurred_at: String,
    pub observed_at: String,
    pub direction: String,
    pub external_phone_e164: String,
    pub external_display_name: Option<String>,
    pub thread_id: Option<String>,
    pub summary: Option<String>,
    pub message_type: String,
    pub attachment_metadata: Value,
}

#[derive(Debug, Clone)]
pub struct WhatsAppLandingInput {
    pub source_account: Option<String>,
    pub source_message_id: String,
    pub conversation_id: Option<String>,
    pub context_id: Option<String>,
    pub from_address: Option<String>,
    pub to_address: Option<String>,
    pub direction: String,
    pub message_type: Option<String>,
    pub text: Option<String>,
    pub media_id: Option<String>,
    pub sent_at: Option<String>,
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhatsAppProcessOutcome {
    Completed { person_id: String, interaction_id: String, created: bool },
    Duplicate { interaction_id: Option<String> },
    ResolutionRequired,
    Rejected,
    InFlight,
    FailedRetryable { attempts: i32 },
    Poisoned { attempts: i32 },
}

#[derive(Debug, FromRow)]
struct InboxState {
    id: String,
    status: String,
    attempt_count: i32,
    max_attempts: i32,
    processing_started_at: Option<DateTime<Utc>>,
    resolved_person_id: Option<String>,
    interaction_id: Option<String>,
}

#[derive(Clone)]
pub struct WhatsAppDao {
    db: Database,
}

impl WhatsAppDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn land(&self, input: &WhatsAppLandingInput) -> DbResult<bool> {
        let inserted = sqlx::query_scalar::<_, String>(
            r#"
            insert into l_whatsapp (
              source_account, source_message_id, conversation_id, context_id,
              from_address, to_address, direction, message_type, text_content,
              media_id, sent_at, raw
            ) values (
              $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::timestamptz, $12
            )
            on conflict (coalesce(source_account, ''), source_message_id) do nothing
            returning id::text
            "#,
        )
        .bind(&input.source_account)
        .bind(&input.source_message_id)
        .bind(&input.conversation_id)
        .bind(&input.context_id)
        .bind(&input.from_address)
        .bind(&input.to_address)
        .bind(&input.direction)
        .bind(&input.message_type)
        .bind(&input.text)
        .bind(&input.media_id)
        .bind(&input.sent_at)
        .bind(&input.raw)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("whatsapp.land", &error))?;
        Ok(inserted.is_some())
    }

    pub async fn process_event(
        &self,
        input: &WhatsAppCanonicalInput,
    ) -> DbResult<WhatsAppProcessOutcome> {
        let mut tx = self.db.begin("whatsapp.process").await?;
        let participants = json!([
            {
                "kind": "phone",
                "value": input.external_phone_e164,
                "displayName": input.external_display_name,
                "role": if input.direction == "inbound" { "sender" } else { "recipient" }
            }
        ]);
        let candidates = json!([{
            "kind": "phone",
            "value": input.external_phone_e164,
            "displayName": input.external_display_name
        }]);

        let created = sqlx::query_as::<_, InboxState>(
            r#"
            insert into integration_inbox (
              source, source_account, external_event_id, event_type,
              occurred_at, observed_at, direction, correlation_id, thread_id,
              subject, summary, content_reference, provenance_reference,
              participant_identities, contact_candidates, attachment_metadata,
              max_attempts
            ) values (
              'whatsapp', $1, $2, $3, $4::timestamptz, $5::timestamptz, $6,
              null, $7, null, $8, null, null, $9, $10, $11, 3
            )
            on conflict (source, source_account, external_event_id) do nothing
            returning id::text as id, status, attempt_count, max_attempts,
              processing_started_at, resolved_person_id::text as resolved_person_id,
              interaction_id::text as interaction_id
            "#,
        )
        .bind(&input.source_account)
        .bind(&input.external_event_id)
        .bind(&input.event_type)
        .bind(&input.occurred_at)
        .bind(&input.observed_at)
        .bind(&input.direction)
        .bind(&input.thread_id)
        .bind(&input.summary)
        .bind(&participants)
        .bind(&candidates)
        .bind(&input.attachment_metadata)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("whatsapp.inbox.insert", &error))?;

        let state = match created {
            Some(row) => row,
            None => sqlx::query_as::<_, InboxState>(
                r#"
                select id::text as id, status, attempt_count, max_attempts,
                  processing_started_at, resolved_person_id::text as resolved_person_id,
                  interaction_id::text as interaction_id
                from integration_inbox
                where source = 'whatsapp' and source_account = $1 and external_event_id = $2
                limit 1
                "#,
            )
            .bind(&input.source_account)
            .bind(&input.external_event_id)
            .fetch_one(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("whatsapp.inbox.read", &error))?,
        };

        if created.is_none() {
            if let Some(outcome) = replay_outcome(&state) {
                tx.commit().await?;
                return Ok(outcome);
            }
        }

        let claimed = sqlx::query_as::<_, InboxState>(
            r#"
            update integration_inbox
            set status = 'processing',
                processing_started_at = date_trunc('milliseconds', now()),
                updated_at = now()
            where id = $1::uuid
              and (
                status = 'received'
                or (status = 'processing'
                    and processing_started_at <= now() - interval '15 minutes')
              )
            returning id::text as id, status, attempt_count, max_attempts,
              processing_started_at, resolved_person_id::text as resolved_person_id,
              interaction_id::text as interaction_id
            "#,
        )
        .bind(&state.id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("whatsapp.inbox.claim", &error))?;

        let Some(claimed) = claimed else {
            tx.commit().await?;
            return Ok(WhatsAppProcessOutcome::InFlight);
        };

        let people = sqlx::query_scalar::<_, String>(
            r#"
            select p.id::text
            from person_identity pi
            join person p on p.id = pi.person_id
            where pi.identity_type = 'phone'
              and p.archived_at is null
              and (case
                when length(regexp_replace(pi.identity_value, '[^0-9]', '', 'g')) = 11
                  and left(regexp_replace(pi.identity_value, '[^0-9]', '', 'g'), 1) = '1'
                then substring(regexp_replace(pi.identity_value, '[^0-9]', '', 'g') from 2)
                else regexp_replace(pi.identity_value, '[^0-9]', '', 'g')
              end) = $1
            order by p.id
            limit 2
            "#,
        )
        .bind(semantic_phone(&input.external_phone_e164))
        .fetch_all(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("whatsapp.person.resolve", &error))?;

        if people.is_empty() {
            transition_without_target(tx.connection(), &claimed, "resolution_required").await?;
            tx.commit().await?;
            return Ok(WhatsAppProcessOutcome::ResolutionRequired);
        }
        if people.len() > 1 {
            let outcome = fail_claim(tx.connection(), &claimed, "ambiguous canonical phone ownership").await?;
            tx.commit().await?;
            return Ok(outcome);
        }
        let person_id = people[0].clone();

        let source_system = format!(
            "communications:meta:{}",
            source_token(&input.source_account)
        );
        let source_external_id = format!("whatsapp:{}", input.external_event_id);
        let command_id = format!(
            "integration-inbox:{}:{}",
            source_system, source_external_id
        );

        let command_claimed = sqlx::query_scalar::<_, String>(
            r#"
            insert into workflow_command_receipt (
              command_id, outcome, aggregate_id, message, actor_app_user_id
            ) values ($1, 'pending', null, null, null)
            on conflict (command_id) do nothing
            returning command_id
            "#,
        )
        .bind(&command_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("whatsapp.command.claim", &error))?
        .is_some();

        let (interaction_id, interaction_created) = if command_claimed {
            let metadata = json!({
                "transport": "whatsapp",
                "contentClass": "free_form",
                "messageType": input.message_type
            });
            let inserted = sqlx::query_scalar::<_, String>(
                r#"
                insert into interaction (
                  person_id, property_id, deal_id, channel, event_type, direction,
                  occurred_at, title, summary, duration_seconds,
                  source_system, source_external_id, source_metadata
                ) values (
                  $1::uuid, null, null, 'whatsapp', $2, $3, $4::timestamptz,
                  null, $5, null, $6, $7, $8
                )
                on conflict (source_system, source_external_id)
                  where source_system is not null and source_external_id is not null
                do nothing
                returning id::text
                "#,
            )
            .bind(&person_id)
            .bind(if input.direction == "inbound" { "whatsapp_received" } else { "whatsapp_sent" })
            .bind(&input.direction)
            .bind(&input.occurred_at)
            .bind(&input.summary)
            .bind(&source_system)
            .bind(&source_external_id)
            .bind(&metadata)
            .fetch_optional(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("whatsapp.interaction.insert", &error))?;

            let (id, created) = match inserted {
                Some(id) => (id, true),
                None => {
                    let row = sqlx::query_as::<_, (String, String)>(
                        r#"
                        select id::text, person_id::text
                        from interaction
                        where source_system = $1 and source_external_id = $2
                        limit 1
                        "#,
                    )
                    .bind(&source_system)
                    .bind(&source_external_id)
                    .fetch_one(tx.connection())
                    .await
                    .map_err(|error| DbFailure::from_sqlx("whatsapp.interaction.replay", &error))?;
                    if row.1 != person_id {
                        return Err(DbFailure::schema_mismatch(
                            "whatsapp.interaction.replay",
                            "source identity belongs to another Person",
                        ));
                    }
                    (row.0, false)
                }
            };

            sqlx::query(
                r#"
                update workflow_command_receipt
                set outcome = 'success', aggregate_id = $2::uuid, message = null
                where command_id = $1
                "#,
            )
            .bind(&command_id)
            .bind(&id)
            .execute(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("whatsapp.command.finalize", &error))?;
            (id, created)
        } else {
            let receipt = sqlx::query_as::<_, (String, Option<String>)>(
                r#"
                select outcome, aggregate_id::text
                from workflow_command_receipt
                where command_id = $1
                limit 1
                "#,
            )
            .bind(&command_id)
            .fetch_one(tx.connection())
            .await
            .map_err(|error| DbFailure::from_sqlx("whatsapp.command.replay", &error))?;
            if receipt.0 != "success" {
                return Err(DbFailure::configuration(
                    "whatsapp.command.replay",
                    "interaction command receipt is not final",
                ));
            }
            let id = receipt.1.ok_or_else(|| {
                DbFailure::schema_mismatch(
                    "whatsapp.command.replay",
                    "successful interaction receipt has no aggregate id",
                )
            })?;
            (id, false)
        };

        let transitioned = sqlx::query_scalar::<_, String>(
            r#"
            update integration_inbox
            set status = 'completed',
                processing_started_at = null,
                processing_completed_at = now(),
                interaction_id = $3::uuid,
                resolved_person_id = $4::uuid,
                updated_at = now()
            where id = $1::uuid
              and status = 'processing'
              and processing_started_at = $2
            returning id::text
            "#,
        )
        .bind(&claimed.id)
        .bind(claimed.processing_started_at)
        .bind(&interaction_id)
        .bind(&person_id)
        .fetch_optional(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("whatsapp.inbox.complete", &error))?;
        if transitioned.is_none() {
            tx.rollback().await?;
            return Ok(WhatsAppProcessOutcome::InFlight);
        }

        project_relationship(
            tx.connection(),
            input,
            &person_id,
        )
        .await?;

        tx.commit().await?;
        Ok(WhatsAppProcessOutcome::Completed {
            person_id,
            interaction_id,
            created: interaction_created,
        })
    }

    pub async fn refresh_client_read_models(&self) -> DbResult<()> {
        for (operation, sql) in [
            (
                "whatsapp.refresh.relationship_channels",
                "refresh materialized view concurrently mv_client_relationship_channels",
            ),
            (
                "whatsapp.refresh.directory",
                "refresh materialized view concurrently mv_client_directory",
            ),
            (
                "whatsapp.refresh.history",
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

fn replay_outcome(state: &InboxState) -> Option<WhatsAppProcessOutcome> {
    match state.status.as_str() {
        "completed" => match (&state.resolved_person_id, &state.interaction_id) {
            (Some(person_id), Some(interaction_id)) => Some(WhatsAppProcessOutcome::Completed {
                person_id: person_id.clone(),
                interaction_id: interaction_id.clone(),
                created: false,
            }),
            _ => Some(WhatsAppProcessOutcome::InFlight),
        },
        "duplicate" => Some(WhatsAppProcessOutcome::Duplicate {
            interaction_id: state.interaction_id.clone(),
        }),
        "resolution_required" => Some(WhatsAppProcessOutcome::ResolutionRequired),
        "rejected" => Some(WhatsAppProcessOutcome::Rejected),
        "poisoned" => Some(WhatsAppProcessOutcome::Poisoned {
            attempts: state.attempt_count,
        }),
        _ => None,
    }
}

async fn transition_without_target(
    connection: &mut sqlx::PgConnection,
    state: &InboxState,
    status: &str,
) -> DbResult<()> {
    sqlx::query(
        r#"
        update integration_inbox
        set status = $3,
            processing_started_at = null,
            processing_completed_at = now(),
            updated_at = now()
        where id = $1::uuid and status = 'processing' and processing_started_at = $2
        "#,
    )
    .bind(&state.id)
    .bind(state.processing_started_at)
    .bind(status)
    .execute(connection)
    .await
    .map_err(|error| DbFailure::from_sqlx("whatsapp.inbox.transition", &error))?;
    Ok(())
}

async fn fail_claim(
    connection: &mut sqlx::PgConnection,
    state: &InboxState,
    message: &str,
) -> DbResult<WhatsAppProcessOutcome> {
    let attempts = state.attempt_count + 1;
    let poisoned = attempts >= state.max_attempts;
    sqlx::query(
        r#"
        update integration_inbox
        set status = case when $3 then 'poisoned' else 'received' end,
            attempt_count = $4,
            last_error = $5,
            processing_started_at = null,
            processing_completed_at = case when $3 then now() else null end,
            updated_at = now()
        where id = $1::uuid and status = 'processing' and processing_started_at = $2
        "#,
    )
    .bind(&state.id)
    .bind(state.processing_started_at)
    .bind(poisoned)
    .bind(attempts)
    .bind(message)
    .execute(connection)
    .await
    .map_err(|error| DbFailure::from_sqlx("whatsapp.inbox.fail", &error))?;

    Ok(if poisoned {
        WhatsAppProcessOutcome::Poisoned { attempts }
    } else {
        WhatsAppProcessOutcome::FailedRetryable { attempts }
    })
}

async fn project_relationship(
    connection: &mut sqlx::PgConnection,
    input: &WhatsAppCanonicalInput,
    person_id: &str,
) -> DbResult<()> {
    sqlx::query(
        r#"
        insert into integration_source_person_link (
          source, source_account, source_identity_key,
          canonical_person_id, link_method, link_reason
        ) values (
          'whatsapp', $1, $2, $3::uuid, 'exact_phone', 'resolved_whatsapp_event'
        )
        on conflict (source, source_account, source_identity_key) do nothing
        "#,
    )
    .bind(&input.source_account)
    .bind(&input.external_phone_e164)
    .bind(person_id)
    .execute(&mut *connection)
    .await
    .map_err(|error| DbFailure::from_sqlx("whatsapp.relationship.link", &error))?;

    let owner = sqlx::query_scalar::<_, String>(
        r#"
        select canonical_person_id::text
        from integration_source_person_link
        where source = 'whatsapp' and source_account = $1 and source_identity_key = $2
        limit 1
        "#,
    )
    .bind(&input.source_account)
    .bind(&input.external_phone_e164)
    .fetch_one(&mut *connection)
    .await
    .map_err(|error| DbFailure::from_sqlx("whatsapp.relationship.owner", &error))?;
    if owner != person_id {
        return Err(DbFailure::schema_mismatch(
            "whatsapp.relationship.owner",
            "WhatsApp source-person ownership conflict",
        ));
    }

    let aggregate = sqlx::query_as::<_, (
        Option<DateTime<Utc>>, Option<DateTime<Utc>>, Option<DateTime<Utc>>,
        Option<DateTime<Utc>>, i64, i64
    )>(
        r#"
        select min(occurred_at), max(occurred_at),
          max(occurred_at) filter (where direction = 'inbound'),
          max(occurred_at) filter (where direction = 'outbound'),
          count(*) filter (where direction = 'inbound')::bigint,
          count(*) filter (where direction = 'outbound')::bigint
        from integration_inbox
        where source = 'whatsapp' and source_account = $1
          and resolved_person_id = $2::uuid and status = 'completed'
        "#,
    )
    .bind(&input.source_account)
    .bind(person_id)
    .fetch_one(&mut *connection)
    .await
    .map_err(|error| DbFailure::from_sqlx("whatsapp.relationship.aggregate", &error))?;

    let phones = sqlx::query_scalar::<_, String>(
        r#"
        select source_identity_key
        from integration_source_person_link
        where source = 'whatsapp' and source_account = $1 and canonical_person_id = $2::uuid
        order by source_identity_key
        "#,
    )
    .bind(&input.source_account)
    .bind(person_id)
    .fetch_all(&mut *connection)
    .await
    .map_err(|error| DbFailure::from_sqlx("whatsapp.relationship.phones", &error))?;

    let phone_json: Vec<Value> = phones
        .iter()
        .map(|phone| json!({"value": phone, "normalized": phone, "label": null}))
        .collect();
    let first = aggregate.0.map(|v| v.to_rfc3339());
    let last = aggregate.1.map(|v| v.to_rfc3339());
    let last_in = aggregate.2.map(|v| v.to_rfc3339());
    let last_out = aggregate.3.map(|v| v.to_rfc3339());
    let source_identity_key = format!("person:{person_id}");
    let fingerprint_input = format!(
        "whatsapp|{}|{}|{}|{}|{}|{}|{}|{}",
        input.source_account,
        source_identity_key,
        phones.join(","),
        first.as_deref().unwrap_or(""),
        last.as_deref().unwrap_or(""),
        last_in.as_deref().unwrap_or(""),
        last_out.as_deref().unwrap_or(""),
        aggregate.4 + aggregate.5
    );
    let evidence_fingerprint = fingerprint(&fingerprint_input);

    sqlx::query(
        r#"
        insert into integration_relationship_evidence (
          source, source_account, source_identity_key, source_label,
          display_name, organization, emails, phones,
          first_observed_at, last_observed_at, last_inbound_at, last_outbound_at,
          inbound_count, outbound_count, is_two_way, is_owner_initiated,
          is_automated_or_bulk, is_organization_or_service, known_apple_contact,
          has_email, has_phone, coverage_note, evidence_fingerprint,
          canonical_person_id, match_method, match_confidence, review_state,
          match_reason, rule_version
        ) values (
          'whatsapp', $1, $2, 'Meta WhatsApp',
          null, null, '[]'::jsonb, $3,
          $4::timestamptz, $5::timestamptz, $6::timestamptz, $7::timestamptz,
          $8::int, $9::int, $10, $11,
          false, false, null, false, $12,
          'Meta WhatsApp Cloud API realtime webhook coverage.', $13,
          $14::uuid, 'source_link', 'exact', 'exact_linked',
          'durable_whatsapp_source_person_link', 'rel-intel.v1'
        )
        on conflict (source, source_account, source_identity_key) do update set
          phones = excluded.phones,
          first_observed_at = excluded.first_observed_at,
          last_observed_at = excluded.last_observed_at,
          last_inbound_at = excluded.last_inbound_at,
          last_outbound_at = excluded.last_outbound_at,
          inbound_count = excluded.inbound_count,
          outbound_count = excluded.outbound_count,
          is_two_way = excluded.is_two_way,
          is_owner_initiated = excluded.is_owner_initiated,
          has_phone = excluded.has_phone,
          coverage_note = excluded.coverage_note,
          evidence_fingerprint = excluded.evidence_fingerprint,
          canonical_person_id = excluded.canonical_person_id,
          match_method = excluded.match_method,
          match_confidence = excluded.match_confidence,
          review_state = excluded.review_state,
          match_reason = excluded.match_reason,
          rule_version = excluded.rule_version,
          updated_at = now()
        "#,
    )
    .bind(&input.source_account)
    .bind(&source_identity_key)
    .bind(Value::Array(phone_json))
    .bind(first)
    .bind(last)
    .bind(last_in)
    .bind(last_out)
    .bind(aggregate.4 as i32)
    .bind(aggregate.5 as i32)
    .bind(aggregate.4 > 0 && aggregate.5 > 0)
    .bind(aggregate.5 > 0)
    .bind(!phones.is_empty())
    .bind(evidence_fingerprint)
    .bind(person_id)
    .execute(&mut *connection)
    .await
    .map_err(|error| DbFailure::from_sqlx("whatsapp.relationship.evidence", &error))?;

    Ok(())
}

fn semantic_phone(value: &str) -> String {
    let digits: String = value.chars().filter(char::is_ascii_digit).collect();
    if digits.len() == 11 && digits.starts_with('1') {
        digits[1..].to_owned()
    } else {
        digits
    }
}

fn source_token(value: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for ch in value.trim().to_ascii_lowercase().chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-' {
            out.push(ch);
            dash = false;
        } else if !dash {
            out.push('-');
            dash = true;
        }
        if out.len() >= 63 {
            break;
        }
    }
    out.trim_matches('-').to_owned()
}

fn fingerprint(input: &str) -> String {
    let mut h1: u32 = 0xdeadbeef;
    let mut h2: u32 = 0x41c6ce57;
    for unit in input.encode_utf16() {
        h1 = (h1 ^ unit as u32).wrapping_mul(2_654_435_761);
        h2 = (h2 ^ unit as u32).wrapping_mul(1_597_334_677);
    }
    h1 = (h1 ^ (h1 >> 16)).wrapping_mul(2_246_822_507)
        ^ (h2 ^ (h2 >> 13)).wrapping_mul(3_266_489_909);
    h2 = (h2 ^ (h2 >> 16)).wrapping_mul(2_246_822_507)
        ^ (h1 ^ (h1 >> 13)).wrapping_mul(3_266_489_909);
    format!("{h2:08x}{h1:08x}")
}
