import { randomUUID } from 'node:crypto'

import { sql } from './client'
import type { QueryExecutor } from './query-executor'

export const APPLE_GATEWAY_ROUTES = {
  CALENDAR_CREATE: 'apple.calendar.create.requested',
  REMINDER_UPSERT: 'apple.reminder.upsert.requested',
} as const

export type AppleCalendarCreateCommand = {
  title: string
  startAt: string
  endAt: string
  allDay?: boolean
  location?: string | null
  notes?: string | null
}

export type AppleReminderUpsertCommand = {
  wbsId: string
  title: string
  dueAt: string | null
  completed: boolean
  notes?: string | null
}

type EnqueueContext = {
  actorAppUserId: string | null
  correlationId: string
}

async function enqueue(
  route: string,
  aggregateType: string,
  aggregateId: string,
  payload: Record<string, unknown>,
  context: EnqueueContext,
  execute: QueryExecutor = sql,
): Promise<string> {
  const id = randomUUID()
  await execute`
    insert into outbox_message (
      id, event_type, aggregate_type, aggregate_id,
      correlation_id, actor_app_user_id, occurred_at, payload
    ) values (
      ${id}, ${route}, ${aggregateType}, ${aggregateId},
      ${context.correlationId}, ${context.actorAppUserId}, now(),
      ${JSON.stringify(payload)}::jsonb
    )
    on conflict (id) do nothing
  `
  return id
}

export async function enqueueAppleCalendarCreate(
  command: AppleCalendarCreateCommand,
  context: EnqueueContext,
  execute: QueryExecutor = sql,
): Promise<string> {
  return enqueue(
    APPLE_GATEWAY_ROUTES.CALENDAR_CREATE,
    'calendar',
    randomUUID(),
    command,
    context,
    execute,
  )
}

export async function enqueueAppleReminderUpsert(
  command: AppleReminderUpsertCommand,
  context: EnqueueContext,
  execute: QueryExecutor = sql,
): Promise<string> {
  return enqueue(
    APPLE_GATEWAY_ROUTES.REMINDER_UPSERT,
    'task',
    command.wbsId,
    command,
    context,
    execute,
  )
}
