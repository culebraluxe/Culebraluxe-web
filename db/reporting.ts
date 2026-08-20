import { sql } from './client'
import { getOpsCounts } from './ops-counts'

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
    counts,
    clientCount,
    clientRoles,
    dealStages,
    interactions,
    channels,
  ] = await Promise.all([
    getOpsCounts(),
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
  ])

  const dealStageDistribution = (dealStages as { stage: string; count: number }[]).map(
    (row) => ({ stage: String(row.stage), count: Number(row.count) }),
  )

  return {
    activeClientCount: (clientCount[0] as CountRow)?.count ?? 0,
    clientRoleDistribution: (clientRoles as { role: string; count: number }[]).map(
      (row) => ({ role: String(row.role), count: Number(row.count) }),
    ),
    activeDealCount: counts.activeDealCount,
    totalDealCount: dealStageDistribution.reduce((sum, row) => sum + row.count, 0),
    dealStageDistribution,
    openTaskCount: counts.openTaskCount,
    overdueTaskCount: counts.overdueTaskCount,
    totalInteractions: (interactions[0] as { total_count: number })?.total_count ?? 0,
    interactionsLast7Days: (interactions[0] as { last_7_days: number })?.last_7_days ?? 0,
    interactionByChannel: (channels as { channel: string; count: number }[]).map(
      (row) => ({ channel: String(row.channel), count: Number(row.count) }),
    ),
    unresolvedNeedsReviewCount: counts.unresolvedIntakeCount,
    activePropertyCount: counts.activePropertyCount,
  }
}
