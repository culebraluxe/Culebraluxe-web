import { sql } from './client'

import type {
  CreateTaskInput,
  Task,
  TaskKind,
  TaskStatus,
} from '../lib/crm-types'
import { PortalWriteError } from '../lib/portal-write-error'
import type { QueryExecutor } from './query-executor'

type TaskRow = {
  id: string
  title: string
  detail: string | null
  person_id: string | null
  property_id: string | null
  deal_id: string | null
  source_interaction_id: string | null
  assigned_user_id: string | null
  due_at: string | null
  task_kind: TaskKind
  priority: number
  status: TaskStatus
  completed_at: string | null
  created_at: string
  updated_at: string
}

function mapTask(row: TaskRow): Task {
  return {
    id: row.id,
    title: row.title,
    detail: row.detail ?? undefined,
    personId: row.person_id ?? undefined,
    propertyId: row.property_id ?? undefined,
    dealId: row.deal_id ?? undefined,
    sourceInteractionId: row.source_interaction_id ?? undefined,
    assignedUserId: row.assigned_user_id ?? undefined,
    dueAt: row.due_at ?? undefined,
    taskKind: row.task_kind,
    priority: row.priority,
    status: row.status,
    completedAt: row.completed_at ?? undefined,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  }
}

export async function getTaskById(
  id: string,
  execute: QueryExecutor = sql,
): Promise<Task | null> {
  const rows = await execute`
    select
      id,
      title,
      detail,
      person_id,
      property_id,
      deal_id,
      source_interaction_id,
      assigned_user_id,
      due_at,
      task_kind,
      priority,
      status,
      completed_at,
      created_at,
      updated_at
    from task
    where id = ${id}
    limit 1
  `

  const row = rows[0] as TaskRow | undefined
  return row ? mapTask(row) : null
}

export type PropertyOpenTask = {
  id: string
  title: string
  detail: string | null
  dueAt: string | null
  dueAtLabel: string | null
  isOverdue: boolean
  personId: string | null
  personName: string | null
  dealId: string | null
  dealName: string | null
}

// Read-only property-scoped open tasks (LOPS-05). Returns open tasks whose
// property_id matches the listing, with best-available person/deal context
// joined only through canonical FKs. Reuses the existing task table; no new
// task semantics or workflow behavior.
export async function getPropertyOpenTasks(
  propertyId: string,
  execute: QueryExecutor = sql,
): Promise<PropertyOpenTask[]> {
  const rows = await execute`
    select
      t.id,
      t.title,
      t.detail,
      t.due_at::text as due_at,
      t.person_id,
      t.deal_id,
      to_char(
        t.due_at at time zone 'America/Puerto_Rico',
        'Mon FMDD, YYYY HH12:MI AM'
      ) as due_at_label,
      person.display_name as person_name,
      deal_property.name as deal_name
    from task t
    left join person
      on person.id = t.person_id
    left join deal
      on deal.id = t.deal_id
    left join property deal_property
      on deal_property.id = deal.property_id
    where t.property_id = ${propertyId}
      and t.status = 'open'
    order by t.due_at asc nulls last, t.created_at asc
  `

  const nowIso = new Date().toISOString()

  return (rows as Array<{
    id: string
    title: string
    detail: string | null
    due_at: string | null
    due_at_label: string | null
    person_id: string | null
    person_name: string | null
    deal_id: string | null
    deal_name: string | null
  }>).map((row) => ({
    id: row.id,
    title: row.title,
    detail: row.detail ?? null,
    dueAt: row.due_at ?? null,
    dueAtLabel: row.due_at_label ?? null,
    isOverdue:
      row.due_at !== null && row.due_at < nowIso,
    personId: row.person_id ?? null,
    personName: row.person_name ?? null,
    dealId: row.deal_id ?? null,
    dealName: row.deal_name ?? null,
  }))
}

export async function createTask(
  input: CreateTaskInput,
  execute: QueryExecutor = sql,
): Promise<Task> {
  if (!input.title.trim()) {
    throw new PortalWriteError('validation', 'Task title is required.')
  }

  if (!input.personId && !input.propertyId && !input.dealId) {
    throw new PortalWriteError(
      'validation',
      'A task requires person, property, or deal context.',
    )
  }

  const priority = input.priority ?? 0
  if (!Number.isInteger(priority) || priority < 0 || priority > 32767) {
    throw new PortalWriteError(
      'validation',
      'Task priority must be an integer between 0 and 32767.',
    )
  }

  const taskKind = input.taskKind ?? 'human'
  if (taskKind !== 'human' && taskKind !== 'system') {
    throw new PortalWriteError('validation', 'Task kind must be human or system.')
  }

  const dueAt =
    input.dueAt instanceof Date ? input.dueAt.toISOString() : input.dueAt

  const rows = await execute`
    insert into task (
      title,
      detail,
      person_id,
      property_id,
      deal_id,
      source_interaction_id,
      assigned_user_id,
      due_at,
      task_kind,
      priority
    ) values (
      ${input.title.trim()},
      ${input.detail ?? null},
      ${input.personId ?? null},
      ${input.propertyId ?? null},
      ${input.dealId ?? null},
      ${input.sourceInteractionId ?? null},
      ${input.assignedUserId ?? null},
      ${dueAt ?? null},
      ${taskKind},
      ${priority}
    )
    returning
      id,
      title,
      detail,
      person_id,
      property_id,
      deal_id,
      source_interaction_id,
      assigned_user_id,
      due_at,
      task_kind,
      priority,
      status,
      completed_at,
      created_at,
      updated_at
  `

  const task = rows[0] as TaskRow | undefined
  if (!task) throw new Error('Task insert did not return a row.')

  return mapTask(task)
}
