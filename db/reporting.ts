import { sql } from './client'

// Read-only boutique reporting projection (CRM-17). Only metrics that the
// canonical schema supports honestly. No forecasting, no AI scoring, no fake
// conversion or revenue metrics.

export type ReportingSnapshot = {
  activeClientCount: number
  clientRoleDistribution: { role: string; count: number }[]
  activeDealCount: number
  totalDealCount: number
  dealStageDistribution: { stage: string; count: number }[]
  openTaskCount: number
  overdueTaskCount: number
  totalInteractions: number
  interactionsLast7Days: number
  interactionByChannel: { channel: string; count: number }[]
  unresolvedNeedsReviewCount: number
  activePropertyCount: number
}

type CountRow = { count: number }

export async function getReportingSnapshot(): Promise<ReportingSnapshot> {
  const [
    clientCount,
    clientRoles,
    dealStages,
    tasks,
    interactions,
    channels,
    needsReview,
    properties,
  ] = await Promise.all([
    sql`
      select count(*)::int as count
      from person
      where archived_at is null
    `,
    sql`
      select role, count(*)::int as count
      from person
      where archived_at is null
      group by role
      order by count desc
    `,
    sql`
      select stage, count(*)::int as count
      from deal
      group by stage
      order by count desc
    `,
    sql`
      select
        (select count(*)::int from task where status = 'open') as open_count,
        (select count(*)::int
          from task
          where status = 'open' and due_at is not null and due_at < now()) as overdue_count
    `,
    sql`
      select
        (select count(*)::int from interaction) as total_count,
        (select count(*)::int
          from interaction
          where occurred_at >= now() - interval '7 days') as last_7_days
    `,
    sql`
      select channel, count(*)::int as count
      from interaction
      group by channel
      order by count desc
    `,
    sql`
      select count(*)::int as count
      from website_intake_submission
      where status in ('received', 'resolution_required')
    `,
    sql`
      select count(*)::int as count
      from property
      where archived_at is null
        and status in ('active', 'coming_soon', 'under_contract')
    `,
  ])

  return {
    activeClientCount: (clientCount[0] as CountRow)?.count ?? 0,
    clientRoleDistribution: (clientRoles as { role: string; count: number }[]).map(
      (row) => ({ role: String(row.role), count: Number(row.count) }),
    ),
    activeDealCount: (dealStages as { stage: string; count: number }[])
      .filter((row) => row.stage !== 'closed')
      .reduce((sum, row) => sum + Number(row.count), 0),
    totalDealCount: (dealStages as { stage: string; count: number }[]).reduce(
      (sum, row) => sum + Number(row.count),
      0,
    ),
    dealStageDistribution: (dealStages as { stage: string; count: number }[]).map(
      (row) => ({ stage: String(row.stage), count: Number(row.count) }),
    ),
    openTaskCount: (tasks[0] as { open_count: number })?.open_count ?? 0,
    overdueTaskCount: (tasks[0] as { overdue_count: number })?.overdue_count ?? 0,
    totalInteractions: (interactions[0] as { total_count: number })?.total_count ?? 0,
    interactionsLast7Days: (interactions[0] as { last_7_days: number })?.last_7_days ?? 0,
    interactionByChannel: (channels as { channel: string; count: number }[]).map(
      (row) => ({ channel: String(row.channel), count: Number(row.count) }),
    ),
    unresolvedNeedsReviewCount: (needsReview[0] as CountRow)?.count ?? 0,
    activePropertyCount: (properties[0] as CountRow)?.count ?? 0,
  }
}
