use crate::{Database, DbFailure, DbResult};
use chrono::{DateTime, Utc};
use model::{CatchUpItem, CatchUpSnapshot};
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct CatchUpRow {
    person_id: String,
    display_name: String,
    role: String,
    status: String,
    reason_code: String,
    reason: String,
    priority: i32,
    signal_at: DateTime<Utc>,
    signal_at_label: String,
    last_contact_at: Option<DateTime<Utc>>,
    last_contact_label: Option<String>,
    last_contact_channel: Option<String>,
    last_contact_direction: Option<String>,
    last_contact_summary: Option<String>,
    primary_phone: Option<String>,
    primary_email: Option<String>,
    active_deal_id: Option<String>,
    active_property_name: Option<String>,
    task_id: Option<String>,
    due_at_label: Option<String>,
}

#[derive(Clone)]
pub struct CatchUpDao {
    db: Database,
}

impl CatchUpDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// A derived relationship queue. Person, interactions, showings, deals and tasks keep owning their facts;
    /// this read only decides which of those facts deserves attention now.
    pub async fn snapshot(&self) -> DbResult<CatchUpSnapshot> {
        let rows = crate::retrying_read!(async {
            sqlx::query_as::<_, CatchUpRow>(
                r#"
                with people as (
                  select p.id, p.display_name, p.role, p.status
                  from person p
                  where p.archived_at is null
                    and p.status in ('active', 'warm')
                ),
                stats as (
                  select
                    i.person_id,
                    max(i.occurred_at) filter (where lower(coalesce(i.direction, '')) = 'inbound') as last_inbound_at,
                    max(i.occurred_at) filter (where lower(coalesce(i.direction, '')) = 'outbound') as last_outbound_at
                  from interaction i
                  join people p on p.id = i.person_id
                  group by i.person_id
                ),
                last_interaction as (
                  select distinct on (i.person_id)
                    i.person_id,
                    i.occurred_at,
                    i.channel,
                    i.direction,
                    coalesce(nullif(i.summary, ''), nullif(i.title, '')) as summary
                  from interaction i
                  join people p on p.id = i.person_id
                  order by i.person_id, i.occurred_at desc, i.id desc
                ),
                latest_showing as (
                  select distinct on (s.person_id)
                    s.person_id,
                    s.id,
                    s.completed_at,
                    pr.name as property_name
                  from showing s
                  join people p on p.id = s.person_id
                  left join property pr on pr.id = s.property_id
                  where s.status = 'completed'
                    and s.completed_at is not null
                  order by s.person_id, s.completed_at desc, s.id desc
                ),
                next_task as (
                  select distinct on (t.person_id)
                    t.person_id,
                    t.id,
                    t.title,
                    t.due_at
                  from task t
                  join people p on p.id = t.person_id
                  where t.status = 'open'
                    and t.person_id is not null
                    and t.due_at is not null
                  order by t.person_id, t.due_at asc, t.created_at asc
                ),
                active_deal as (
                  select distinct on (dp.person_id)
                    dp.person_id,
                    d.id,
                    pr.name as property_name
                  from deal_participant dp
                  join deal d on d.id = dp.deal_id
                  join property pr on pr.id = d.property_id
                  where dp.active = true
                    and dp.person_id is not null
                    and d.stage <> 'closed'
                  order by dp.person_id, d.updated_at desc, d.id
                ),
                identity as (
                  select
                    p.id as person_id,
                    phone.identity_value as primary_phone,
                    email.identity_value as primary_email
                  from people p
                  left join lateral (
                    select pi.identity_value
                    from person_identity pi
                    where pi.person_id = p.id and pi.identity_type = 'phone'
                    order by pi.is_primary desc, pi.created_at asc
                    limit 1
                  ) phone on true
                  left join lateral (
                    select pi.identity_value
                    from person_identity pi
                    where pi.person_id = p.id and pi.identity_type = 'email'
                    order by pi.is_primary desc, pi.created_at asc
                    limit 1
                  ) email on true
                ),
                signals as (
                  select
                    p.id as person_id,
                    p.display_name,
                    p.role,
                    p.status,
                    'unanswered_inbound'::text as reason_code,
                    'An inbound message has no later outgoing reply.'::text as reason,
                    100::int as priority,
                    st.last_inbound_at as signal_at,
                    nt.id::text as task_id,
                    nt.due_at
                  from people p
                  join stats st on st.person_id = p.id
                  left join next_task nt on nt.person_id = p.id
                  where st.last_inbound_at is not null
                    and st.last_inbound_at > coalesce(st.last_outbound_at, 'epoch'::timestamptz)
                    and st.last_inbound_at >= now() - interval '30 days'

                  union all

                  select
                    p.id,
                    p.display_name,
                    p.role,
                    p.status,
                    'showing_follow_up',
                    case
                      when ls.property_name is not null
                        then 'A completed showing at ' || ls.property_name || ' has no later recorded contact.'
                      else 'A completed showing has no later recorded contact.'
                    end,
                    90,
                    ls.completed_at,
                    nt.id::text,
                    nt.due_at
                  from people p
                  join latest_showing ls on ls.person_id = p.id
                  left join last_interaction li on li.person_id = p.id
                  left join next_task nt on nt.person_id = p.id
                  where ls.completed_at >= now() - interval '21 days'
                    and (li.occurred_at is null or li.occurred_at <= ls.completed_at)

                  union all

                  select
                    p.id,
                    p.display_name,
                    p.role,
                    p.status,
                    'overdue_follow_up',
                    'An open follow-up task is overdue.',
                    85,
                    nt.due_at,
                    nt.id::text,
                    nt.due_at
                  from people p
                  join next_task nt on nt.person_id = p.id
                  where nt.due_at < now()

                  union all

                  select
                    p.id,
                    p.display_name,
                    p.role,
                    p.status,
                    'follow_up_due_soon',
                    'A follow-up task is due within seven days.',
                    70,
                    nt.due_at,
                    nt.id::text,
                    nt.due_at
                  from people p
                  join next_task nt on nt.person_id = p.id
                  where nt.due_at >= now()
                    and nt.due_at <= now() + interval '7 days'

                  union all

                  select
                    p.id,
                    p.display_name,
                    p.role,
                    p.status,
                    'relationship_quiet',
                    'This active relationship has had no recorded contact for at least fourteen days.',
                    50,
                    coalesce(li.occurred_at, now() - interval '365 days'),
                    nt.id::text,
                    nt.due_at
                  from people p
                  left join last_interaction li on li.person_id = p.id
                  left join next_task nt on nt.person_id = p.id
                  where li.occurred_at is null
                     or li.occurred_at < now() - interval '14 days'
                ),
                ranked as (
                  select distinct on (s.person_id)
                    s.*,
                    li.occurred_at as last_contact_at,
                    li.channel as last_contact_channel,
                    li.direction as last_contact_direction,
                    li.summary as last_contact_summary,
                    ident.primary_phone,
                    ident.primary_email,
                    ad.id::text as active_deal_id,
                    ad.property_name as active_property_name
                  from signals s
                  left join last_interaction li on li.person_id = s.person_id
                  left join identity ident on ident.person_id = s.person_id
                  left join active_deal ad on ad.person_id = s.person_id
                  left join catch_up_disposition disposition
                    on disposition.person_id = s.person_id
                   and disposition.reason_code = s.reason_code
                  where (disposition.snoozed_until is null or disposition.snoozed_until <= now())
                    and (disposition.handled_at is null or disposition.handled_at < s.signal_at)
                  order by s.person_id, s.priority desc, s.signal_at desc
                )
                select
                  person_id::text as person_id,
                  display_name,
                  role,
                  status,
                  reason_code,
                  reason,
                  priority,
                  signal_at,
                  to_char(signal_at at time zone 'America/Puerto_Rico', 'Mon FMDD, YYYY HH12:MI AM') as signal_at_label,
                  last_contact_at,
                  case when last_contact_at is not null
                    then to_char(last_contact_at at time zone 'America/Puerto_Rico', 'Mon FMDD, YYYY HH12:MI AM')
                    else null
                  end as last_contact_label,
                  last_contact_channel,
                  last_contact_direction,
                  last_contact_summary,
                  primary_phone,
                  primary_email,
                  active_deal_id,
                  active_property_name,
                  task_id,
                  case when due_at is not null
                    then to_char(due_at at time zone 'America/Puerto_Rico', 'Mon FMDD, YYYY HH12:MI AM')
                    else null
                  end as due_at_label
                from ranked
                order by priority desc, signal_at desc, display_name asc
                limit 100
                "#,
            )
            .fetch_all(self.db.pool())
            .await
            .map_err(|error| DbFailure::from_sqlx("catch_up.snapshot", &error))
        })?;

        let items: Vec<CatchUpItem> = rows
            .into_iter()
            .map(|row| CatchUpItem {
                person_id: row.person_id,
                display_name: row.display_name,
                role: row.role,
                status: row.status,
                reason_code: row.reason_code,
                reason: row.reason,
                priority: row.priority,
                signal_at: row.signal_at.to_rfc3339(),
                signal_at_label: row.signal_at_label,
                last_contact_at: row.last_contact_at.map(|value| value.to_rfc3339()),
                last_contact_label: row.last_contact_label,
                last_contact_channel: row.last_contact_channel,
                last_contact_direction: row.last_contact_direction,
                last_contact_summary: row.last_contact_summary,
                primary_phone: row.primary_phone,
                primary_email: row.primary_email,
                active_deal_id: row.active_deal_id,
                active_property_name: row.active_property_name,
                task_id: row.task_id,
                due_at_label: row.due_at_label,
            })
            .collect();
        let high_priority_count = items.iter().filter(|item| item.priority >= 85).count() as i64;
        Ok(CatchUpSnapshot {
            generated_at: Utc::now().to_rfc3339(),
            total: items.len() as i64,
            high_priority_count,
            items,
        })
    }

    pub async fn handle(&self, person_id: &str, reason_code: &str) -> DbResult<()> {
        sqlx::query(
            r#"
            insert into catch_up_disposition (person_id, reason_code, handled_at, snoozed_until)
            values ($1::uuid, $2, now(), null)
            on conflict (person_id, reason_code)
            do update set handled_at = now(), snoozed_until = null, updated_at = now()
            "#,
        )
        .bind(person_id)
        .bind(reason_code)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("catch_up.handle", &error))?;
        Ok(())
    }

    pub async fn snooze(&self, person_id: &str, reason_code: &str, days: i32) -> DbResult<()> {
        sqlx::query(
            r#"
            insert into catch_up_disposition (person_id, reason_code, handled_at, snoozed_until)
            values ($1::uuid, $2, null, now() + make_interval(days => $3::int))
            on conflict (person_id, reason_code)
            do update set handled_at = null,
                          snoozed_until = now() + make_interval(days => $3::int),
                          updated_at = now()
            "#,
        )
        .bind(person_id)
        .bind(reason_code)
        .bind(days)
        .execute(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("catch_up.snooze", &error))?;
        Ok(())
    }
}
