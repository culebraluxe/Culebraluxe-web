import { sql } from './client'

import type { InteractionChannel } from '@/lib/crm-types'

// Read-only projection over the existing CRM substrate for the Portal
// Dashboard V2. This is a situational-awareness read model: it never
// writes and never mutates the CRM domain model.

export type DashboardTask = {
  id: string
  personId: string | null
  title: string
  detail: string | null
  dueAt: string | null
  dueAtLabel: string | null
  contextName: string | null
}

export type DashboardRecentInteraction = {
  id: string
  personName: string
  channel: InteractionChannel
  occurredAtLabel: string
  summary: string | null
  title: string | null
}

export type DashboardSnapshot = {
  openTaskCount: number
  overdueTasks: DashboardTask[]
  tasksDueSoon: DashboardTask[]
  recentInteractions: DashboardRecentInteraction[]
}

const DUE_SOON_WINDOW_DAYS = 7

type TaskRow = {
  id: string
  person_id: string | null
  title: string
  detail: string | null
  due_at: string | null
  due_at_label: string | null
  person_name: string | null
  property_name: string | null
  deal_name: string | null
}

type RecentInteractionRow = {
  id: string
  person_name: string
  channel: InteractionChannel
  occurred_at_label: string
  summary: string | null
  title: string | null
}

function mapTask(row: TaskRow): DashboardTask {
  return {
    id: row.id,
    personId: row.person_id ?? null,
    title: row.title,
    detail: row.detail ?? null,
    dueAt: row.due_at ?? null,
    dueAtLabel: row.due_at_label ?? null,
    contextName:
      row.person_name ??
      row.deal_name ??
      row.property_name ??
      null,
  }
}

export async function getDashboardSnapshot(): Promise<DashboardSnapshot> {
  const [taskRows, interactionRows] = await Promise.all([
    sql`
      select
        t.id,
        person.id as person_id,
        t.title,
        t.detail,
        t.due_at,
        to_char(
          t.due_at at time zone 'America/Puerto_Rico',
          'Mon FMDD, YYYY HH12:MI AM'
        ) as due_at_label,
        person.display_name as person_name,
        property.name as property_name,
        deal_property.name as deal_name
      from task t
      left join person
        on person.id = t.person_id
      left join property
        on property.id = t.property_id
      left join deal
        on deal.id = t.deal_id
      left join property deal_property
        on deal_property.id = deal.property_id
      where t.status = 'open'
      order by
        t.due_at asc nulls last,
        t.created_at asc
    `,
    sql`
      select
        i.id,
        p.display_name as person_name,
        i.channel,
        to_char(
          i.occurred_at at time zone 'America/Puerto_Rico',
          'Mon FMDD, YYYY HH12:MI AM'
        ) as occurred_at_label,
        i.summary,
        i.title
      from interaction i
      join person p
        on p.id = i.person_id
      order by i.occurred_at desc
      limit 6
    `,
  ])

  const allTasks = (taskRows as TaskRow[]).map(mapTask)
  const now = Date.now()
  const dueSoonCutoff =
    now + DUE_SOON_WINDOW_DAYS * 24 * 60 * 60 * 1000

  const overdueTasks: DashboardTask[] = []
  const tasksDueSoon: DashboardTask[] = []

  for (const task of allTasks) {
    if (!task.dueAt) continue

    const dueMs = new Date(task.dueAt).getTime()

    if (dueMs < now) {
      overdueTasks.push(task)
    } else if (dueMs <= dueSoonCutoff) {
      tasksDueSoon.push(task)
    }
  }

  return {
    openTaskCount: allTasks.length,
    overdueTasks,
    tasksDueSoon,
    recentInteractions: (interactionRows as RecentInteractionRow[]).map(
      (row) => ({
        id: row.id,
        personName: row.person_name,
        channel: row.channel,
        occurredAtLabel: row.occurred_at_label,
        summary: row.summary ?? null,
        title: row.title ?? null,
      }),
    ),
  }
}
