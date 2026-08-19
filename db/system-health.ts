import { sql } from './client'

// Read-only Portal system-health projection (OPS-01). Exposes operational
// counts and data-quality signals derivable from the existing schema. This is
// not a generic DB admin tool: no table editing, no secrets, no writes.

export type SystemHealthSnapshot = {
  unresolvedIntakeCount: number
  openTaskCount: number
  overdueTaskCount: number
  activeDealCount: number
  underContractCount: number
  activePropertyCount: number
  recentInteractionAtLabel: string | null
  interactionsLast7Days: number
  // Data-quality signals
  personsWithoutEmailIdentity: number
  personsWithoutPhoneIdentity: number
  openTasksWithoutDueDate: number
  activePropertiesWithoutHeroMedia: number
}

type CountRow = { count: number }

export async function getSystemHealth(): Promise<SystemHealthSnapshot> {
  const [
    intake,
    openTasks,
    overdueTasks,
    deals,
    properties,
    recent,
    quality,
  ] = await Promise.all([
    sql`
      select count(*)::int as count
      from website_intake_submission
      where status in ('received', 'resolution_required')
    `,
    sql`
      select count(*)::int as count
      from task
      where status = 'open'
    `,
    sql`
      select count(*)::int as count
      from task
      where status = 'open'
        and due_at is not null
        and due_at < now()
    `,
    sql`
      select
        (select count(*)::int from deal where stage <> 'closed') as active_count,
        (select count(*)::int from deal where stage = 'under_contract') as under_contract_count
    `,
    sql`
      select count(*)::int as count
      from property
      where archived_at is null
        and status in ('active', 'coming_soon', 'under_contract')
    `,
    sql`
      select
        to_char(
          max(occurred_at) at time zone 'America/Puerto_Rico',
          'Mon FMDD, YYYY HH12:MI AM'
        ) as recent_label,
        (select count(*)::int
          from interaction
          where occurred_at >= now() - interval '7 days') as last_7_days
      from interaction
    `,
    sql`
      select
        (
          select count(*)::int
          from person p
          where p.archived_at is null
            and not exists (
              select 1 from person_identity pi
              where pi.person_id = p.id and pi.identity_type = 'email'
            )
        ) as persons_without_email,
        (
          select count(*)::int
          from person p
          where p.archived_at is null
            and not exists (
              select 1 from person_identity pi
              where pi.person_id = p.id and pi.identity_type = 'phone'
            )
        ) as persons_without_phone,
        (
          select count(*)::int
          from task
          where status = 'open' and due_at is null
        ) as open_tasks_without_due,
        (
          select count(*)::int
          from property p
          where p.archived_at is null
            and p.status in ('active', 'coming_soon', 'under_contract')
            and not exists (
              select 1 from property_media pm
              where pm.property_id = p.id and pm.role = 'hero'
            )
        ) as active_properties_without_hero
    `,
  ])

  const intakeCount = (intake[0] as CountRow)?.count ?? 0
  const openTaskCount = (openTasks[0] as CountRow)?.count ?? 0
  const overdueTaskCount = (overdueTasks[0] as CountRow)?.count ?? 0
  const dealRow = deals[0] as
    | { active_count: number; under_contract_count: number }
    | undefined
  const propertyCount = (properties[0] as CountRow)?.count ?? 0
  const recentRow = recent[0] as
    | { recent_label: string | null; last_7_days: number }
    | undefined
  const qualityRow = quality[0] as
    | {
        persons_without_email: number
        persons_without_phone: number
        open_tasks_without_due: number
        active_properties_without_hero: number
      }
    | undefined

  return {
    unresolvedIntakeCount: intakeCount,
    openTaskCount,
    overdueTaskCount,
    activeDealCount: dealRow?.active_count ?? 0,
    underContractCount: dealRow?.under_contract_count ?? 0,
    activePropertyCount: propertyCount,
    recentInteractionAtLabel: recentRow?.recent_label ?? null,
    interactionsLast7Days: recentRow?.last_7_days ?? 0,
    personsWithoutEmailIdentity: qualityRow?.persons_without_email ?? 0,
    personsWithoutPhoneIdentity: qualityRow?.persons_without_phone ?? 0,
    openTasksWithoutDueDate: qualityRow?.open_tasks_without_due ?? 0,
    activePropertiesWithoutHeroMedia:
      qualityRow?.active_properties_without_hero ?? 0,
  }
}
