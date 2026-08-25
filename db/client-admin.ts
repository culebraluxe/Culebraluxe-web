import { sql } from './client'
import type { QueryExecutor } from './query-executor'

// Read-only client administration projection (OPS-02). One row per active
// person with operational admin fields derived from canonical data. No edits,
// no identity inference.

export type ClientAdminRow = {
  id: string
  displayName: string
  role: string
  status: string
  location: string | null
  assignedAgent: string | null
  primaryEmail: string | null
  primaryPhone: string | null
  lastInteractionLabel: string | null
  openTaskCount: number
  activeDealCount: number
  interestCount: number
}

type ClientAdminRowRaw = {
  id: string
  display_name: string
  role: string
  status: string
  location: string | null
  assigned_agent: string | null
  primary_email: string | null
  primary_phone: string | null
  last_interaction_label: string | null
  open_task_count: number
  active_deal_count: number
  interest_count: number
}

export async function getClientAdmin(): Promise<ClientAdminRow[]> {
  const rows = await sql`
    select
      p.id,
      p.display_name,
      p.role,
      p.status,
      p.location,
      u.display_name as assigned_agent,
      email.identity_value as primary_email,
      phone.identity_value as primary_phone,
      to_char(
        latest.occurred_at at time zone 'America/Puerto_Rico',
        'Mon FMDD, YYYY'
      ) as last_interaction_label,
      (
        select count(*)::int
        from task t
        where t.person_id = p.id and t.status = 'open'
      ) as open_task_count,
      (
        select count(*)::int
        from deal d
        where d.stage <> 'closed'
          and exists (
            select 1
            from deal_participant dp
            where dp.deal_id = d.id
              and dp.person_id = p.id
              and dp.role = 'client'
              and dp.active = true
          )
      ) as active_deal_count,
      (
        select count(*)::int
        from property_interest pi
        where pi.person_id = p.id
      ) as interest_count
    from person p
    left join app_user u
      on u.id = p.assigned_user_id
    left join lateral (
      select pi.identity_value
      from person_identity pi
      where pi.person_id = p.id
        and pi.identity_type = 'email'
      order by pi.is_primary desc, pi.created_at asc
      limit 1
    ) email on true
    left join lateral (
      select pi.identity_value
      from person_identity pi
      where pi.person_id = p.id
        and pi.identity_type = 'phone'
      order by pi.is_primary desc, pi.created_at asc
      limit 1
    ) phone on true
    left join lateral (
      select i.occurred_at
      from interaction i
      where i.person_id = p.id
      order by i.occurred_at desc
      limit 1
    ) latest on true
    where p.archived_at is null
    order by p.display_name asc
  `

  return (rows as ClientAdminRowRaw[]).map((row) => ({
    id: row.id,
    displayName: row.display_name,
    role: row.role,
    status: row.status,
    location: row.location ?? null,
    assignedAgent: row.assigned_agent ?? null,
    primaryEmail: row.primary_email ?? null,
    primaryPhone: row.primary_phone ?? null,
    lastInteractionLabel: row.last_interaction_label ?? null,
    openTaskCount: row.open_task_count,
    activeDealCount: row.active_deal_count,
    interestCount: row.interest_count,
  }))
}

export type ClientAdminPageResult = {
  rows: ClientAdminRow[]
  total: number
  page: number
  pageSize: number
}

/**
 * Server-side-paginated client administration projection over the canonical
 * `person` parent: separate COUNT(*) + SQL LIMIT/OFFSET, SQL search, no
 * full-table hydration. Per-row admin counts (tasks/deals/interests) are
 * page-bounded correlated subqueries (one statement, no N+1).
 */
export async function getClientAdminPage(
  opts: { search?: string; page?: number; pageSize?: number },
  execute: QueryExecutor = sql,
): Promise<ClientAdminPageResult> {
  const search = (opts.search ?? "").trim()
  const like = search === "" ? null : `%${search}%`
  const page = Math.max(1, opts.page ?? 1)
  const pageSize = Math.max(1, Math.min(50, opts.pageSize ?? 50))
  const offset = (page - 1) * pageSize

  const guard = sql`
    where p.archived_at is null
    and (${like}::text is null or (
      p.display_name ilike ${like}
      or exists (
        select 1 from person_identity pi
        where pi.person_id = p.id and pi.identity_value ilike ${like}
      )
    ))
  `

  const countRows = (await execute`
    select count(*)::int as total from person p ${guard}
  `) as { total: number }[]
  const total = Number(countRows[0]?.total ?? 0)

  const rows = (await execute`
    select
      p.id,
      p.display_name,
      p.role,
      p.status,
      p.location,
      u.display_name as assigned_agent,
      email.identity_value as primary_email,
      phone.identity_value as primary_phone,
      to_char(
        latest.occurred_at at time zone 'America/Puerto_Rico',
        'Mon FMDD, YYYY'
      ) as last_interaction_label,
      (
        select count(*)::int
        from task t
        where t.person_id = p.id and t.status = 'open'
      ) as open_task_count,
      (
        select count(*)::int
        from deal d
        where d.stage <> 'closed'
          and exists (
            select 1
            from deal_participant dp
            where dp.deal_id = d.id
              and dp.person_id = p.id
              and dp.role = 'client'
              and dp.active = true
          )
      ) as active_deal_count,
      (
        select count(*)::int
        from property_interest pi
        where pi.person_id = p.id
      ) as interest_count
    from person p
    left join app_user u
      on u.id = p.assigned_user_id
    left join lateral (
      select pi.identity_value
      from person_identity pi
      where pi.person_id = p.id
        and pi.identity_type = 'email'
      order by pi.is_primary desc, pi.created_at asc
      limit 1
    ) email on true
    left join lateral (
      select pi.identity_value
      from person_identity pi
      where pi.person_id = p.id
        and pi.identity_type = 'phone'
      order by pi.is_primary desc, pi.created_at asc
      limit 1
    ) phone on true
    left join lateral (
      select i.occurred_at
      from interaction i
      where i.person_id = p.id
      order by i.occurred_at desc
      limit 1
    ) latest on true
    ${guard}
    order by p.display_name asc, p.id asc
    limit ${pageSize} offset ${offset}
  `) as ClientAdminRowRaw[]

  return {
    rows: rows.map((row) => ({
      id: row.id,
      displayName: row.display_name,
      role: row.role,
      status: row.status,
      location: row.location ?? null,
      assignedAgent: row.assigned_agent ?? null,
      primaryEmail: row.primary_email ?? null,
      primaryPhone: row.primary_phone ?? null,
      lastInteractionLabel: row.last_interaction_label ?? null,
      openTaskCount: row.open_task_count,
      activeDealCount: row.active_deal_count,
      interestCount: row.interest_count,
    })),
    total,
    page,
    pageSize,
  }
}
