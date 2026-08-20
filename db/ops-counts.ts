import { sql } from './client'

// Read-only shared projection (ENG-02A) for scalar operational counts whose
// semantics are identical across Portal screens (System Health, Reporting).
// Deliberately narrow: no generic repository abstraction, no per-screen shape.
// Dashboard/Attention task-list projections intentionally remain separate.

export type OpsCounts = {
  activeDealCount: number
  openTaskCount: number
  overdueTaskCount: number
  unresolvedIntakeCount: number
  activePropertyCount: number
}

type OpsCountsRow = {
  active_deal_count: number
  open_task_count: number
  overdue_task_count: number
  unresolved_intake_count: number
  active_property_count: number
}

export async function getOpsCounts(): Promise<OpsCounts> {
  const rows = await sql`
    select
      (
        select count(*)::int
        from deal
        where stage <> 'closed'
      ) as active_deal_count,
      (
        select count(*)::int
        from task
        where status = 'open'
      ) as open_task_count,
      (
        select count(*)::int
        from task
        where status = 'open'
          and due_at is not null
          and due_at < now()
      ) as overdue_task_count,
      (
        select count(*)::int
        from website_intake_submission
        where status in ('received', 'resolution_required')
      ) as unresolved_intake_count,
      (
        select count(*)::int
        from property
        where archived_at is null
          and status in ('active', 'coming_soon', 'under_contract')
      ) as active_property_count
  `

  const row = (rows[0] as OpsCountsRow) ?? {
    active_deal_count: 0,
    open_task_count: 0,
    overdue_task_count: 0,
    unresolved_intake_count: 0,
    active_property_count: 0,
  }

  return {
    activeDealCount: row.active_deal_count,
    openTaskCount: row.open_task_count,
    overdueTaskCount: row.overdue_task_count,
    unresolvedIntakeCount: row.unresolved_intake_count,
    activePropertyCount: row.active_property_count,
  }
}
