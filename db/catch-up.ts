import { sql } from './client'
import type { QueryExecutor } from './query-executor'

// ---------------------------------------------------------------------------
// CATCH-UP — application read model (ENG-34 bounded read).
//
// Canonical tables -> purpose-built Catch-Up read model -> indexed bounded
// query -> Catch-Up UI. Two bounded queries total (page + facts for that page
// via `any(...)`): never materializes every person, never N+1. Deterministic
// urgency ordering in SQL; the pure rules (lib/catchup/rules.ts) own the
// explainable per-row reason. Canonical data remains source of truth.
// ---------------------------------------------------------------------------

export type CatchUpEligibleRow = {
  personId: string
  displayName: string
  role: string
  status: string
  email: string | null
  phone: string | null
  createdAt: string
  lastMeaningfulContactAt: string | null
  lastInboundAt: string | null
  lastOutboundAt: string | null
  activeDealStage: string | null
  activeDealProperty: string | null
  nextEventAt: string | null
  nextEventLabel: string | null
}

export type CatchUpPageResult = {
  rows: CatchUpEligibleRow[]
  total: number
  page: number
  pageSize: number
}

type PersonPageRow = {
  person_id: string
  display_name: string
  role: string
  status: string
  created_at: string
}

type FactsRow = {
  person_id: string
  email: string | null
  phone: string | null
  last_meaningful_contact_at: string | null
  last_inbound_at: string | null
  last_outbound_at: string | null
  active_deal_stage: string | null
  active_deal_property: string | null
  next_event_at: string | null
  next_event_label: string | null
}

const ELIGIBLE_ROLES = ['buyer', 'seller', 'both']

/**
 * Page of eligible persons (Lead / Client / active deal-chain participant)
 * with their derived relationship facts. Bounded: one page query + one
 * `any(ids)` facts query. Rules are applied by the caller (lib/catchup/queue).
 */
export async function getCatchUpEligiblePage(
  opts: { search?: string; page?: number; pageSize?: number },
  execute: QueryExecutor = sql,
): Promise<CatchUpPageResult> {
  const search = (opts.search ?? '').trim()
  const page = Math.max(1, opts.page ?? 1)
  const pageSize = Math.max(1, opts.pageSize ?? 50)
  const offset = (page - 1) * pageSize
  const like = `%${search}%`

  const rows = (await execute`
    select
      p.id as person_id,
      p.display_name,
      p.role,
      p.status,
      p.created_at
    from person p
    where p.archived_at is null
      and (
        p.role = any(${ELIGIBLE_ROLES})
        or exists (
          select 1 from deal d
          where d.client_person_id = p.id and d.stage <> 'closed'
        )
      )
      and (${search} = '' or p.display_name ilike ${like})
    order by
      case
        when p.created_at >= now() - interval '5 days'
             and not exists (
               select 1 from interaction i
               where i.person_id = p.id and i.direction = 'outbound'
             ) then 0
        when exists (
               select 1 from interaction i
               where i.person_id = p.id and i.direction = 'inbound'
                 and i.occurred_at >= now() - interval '2 days'
             ) and not exists (
               select 1 from interaction i
               where i.person_id = p.id and i.direction = 'outbound'
                 and i.occurred_at >= (
                   select max(occurred_at) from interaction
                   where person_id = p.id and direction = 'inbound'
                 )
             ) then 1
        when exists (
               select 1 from showing s
               where s.person_id = p.id and s.status = 'scheduled'
                 and s.scheduled_at >= now()
                 and s.scheduled_at <= now() + interval '7 days'
             ) then 1
        when exists (
               select 1 from deal d
               where d.client_person_id = p.id and d.stage <> 'closed'
             ) and (
               select max(occurred_at) from interaction
               where person_id = p.id
             ) < now() - interval '5 days' then 2
        when (
               select max(occurred_at) from interaction
               where person_id = p.id
             ) < now() - interval '10 days' then 3
        else 9
      end asc,
      p.created_at desc,
      p.display_name asc,
      p.id asc
    limit ${pageSize}
    offset ${offset}
  `) as PersonPageRow[]

  const countRows = (await execute`
    select count(*)::int as total
    from person p
    where p.archived_at is null
      and (
        p.role = any(${ELIGIBLE_ROLES})
        or exists (
          select 1 from deal d
          where d.client_person_id = p.id and d.stage <> 'closed'
        )
      )
      and (${search} = '' or p.display_name ilike ${like})
  `) as { total: number }[]

  const total = countRows[0]?.total ?? 0
  const ids = rows.map((r) => r.person_id)


  let factsRows: FactsRow[] = []
  if (ids.length > 0) {
    factsRows = (await execute`
      select
        p.id as person_id,
        (
          select pi.identity_value from person_identity pi
          where pi.person_id = p.id and pi.identity_type = 'email'
          order by (pi.is_primary) desc, pi.created_at asc limit 1
        ) as email,
        (
          select pi.identity_value from person_identity pi
          where pi.person_id = p.id and pi.identity_type = 'phone'
          order by (pi.is_primary) desc, pi.created_at asc limit 1
        ) as phone,
        (select max(i.occurred_at) from interaction i where i.person_id = p.id) as last_meaningful_contact_at,
        (select max(i.occurred_at) from interaction i where i.person_id = p.id and i.direction = 'inbound') as last_inbound_at,
        (select max(i.occurred_at) from interaction i where i.person_id = p.id and i.direction = 'outbound') as last_outbound_at,
        (
          select d.stage from deal d
          where d.client_person_id = p.id and d.stage <> 'closed'
          order by d.created_at desc limit 1
        ) as active_deal_stage,
        (
          select prop.name
          from deal d
          join property prop on prop.id = d.property_id
          where d.client_person_id = p.id and d.stage <> 'closed'
          order by d.created_at desc limit 1
        ) as active_deal_property,
        (
          select s.scheduled_at from showing s
          where s.person_id = p.id and s.status = 'scheduled'
            and s.scheduled_at >= now()
          order by s.scheduled_at asc limit 1
        ) as next_event_at,
        (
          select to_char(
            s2.scheduled_at at time zone 'America/Puerto_Rico',
            'Mon FMDD at HH12:MI AM'
          )
          from showing s2
          where s2.person_id = p.id and s2.status = 'scheduled'
            and s2.scheduled_at >= now()
          order by s2.scheduled_at asc limit 1
        ) as next_event_label
      from person p
      where p.id = any (${ids})
    `) as FactsRow[]
  }

  const factsById = new Map(factsRows.map((f) => [f.person_id, f]))

  const mapped = rows.map((r): CatchUpEligibleRow => {
    const f = factsById.get(r.person_id)
    return {
      personId: r.person_id,
      displayName: r.display_name,
      role: r.role,
      status: r.status,
      email: f?.email ?? null,
      phone: f?.phone ?? null,
      createdAt: r.created_at,
      lastMeaningfulContactAt: f?.last_meaningful_contact_at ?? null,
      lastInboundAt: f?.last_inbound_at ?? null,
      lastOutboundAt: f?.last_outbound_at ?? null,
      activeDealStage: f?.active_deal_stage ?? null,
      activeDealProperty: f?.active_deal_property ?? null,
      nextEventAt: f?.next_event_at ?? null,
      nextEventLabel: f?.next_event_label ?? null,
    }
  })

  return { rows: mapped, total, page, pageSize }
}

