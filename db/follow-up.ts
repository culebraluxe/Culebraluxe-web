// ---------------------------------------------------------------------------
// CORE-DAILY-01 — relationship follow-up lifecycle repository.
//
// Extends the existing `task` model (migration 075) into a deterministic,
// auditable, replay-safe relationship follow-up lifecycle. Every mutation goes
// through a canonical command receipt so a duplicate/replayed command_id never
// re-applies a side effect.
//
//   OPEN       task.status = 'open'  (due_at governs due/overdue)
//   SNOOZED    task.status = 'snoozed' + snoozed_until (reactivates to open when due)
//   COMPLETED  task.status = 'completed' + completed_at (+ optional next_touch task)
//   DISMISSED  task.status = 'dismissed'  (operator/soft dismissal)
//   CANCELLED  task.status = 'cancelled'
//
// Snooze only sets snoozed_until — it NEVER touches workflow/legal deadlines.
// Relationship completion never satisfies workflow human tasks (no mapping here).
// ---------------------------------------------------------------------------
import { sql } from './client'
import type { QueryExecutor } from './query-executor'
import { PortalWriteError } from '../lib/portal-write-error'

export type FollowUpStatus = 'open' | 'snoozed' | 'completed' | 'dismissed' | 'cancelled'
export type FollowUpCommandType = 'create' | 'snooze' | 'complete' | 'dismiss' | 'cancel'

export type FollowUp = {
  id: string
  title: string
  detail: string | null
  personId: string | null
  propertyId: string | null
  dealId: string | null
  dueAt: string | null
  status: FollowUpStatus
  snoozedUntil: string | null
  outcome: string | null
  nextTouchAt: string | null
  source: string | null
  reason: string | null
  recommendationKey: string | null
  completedAt: string | null
  createdAt: string
  updatedAt: string
}

export type FollowUpReceipt = {
  id: string
  commandId: string
  commandType: FollowUpCommandType
  followUpId: string | null
  personId: string | null
  applied: boolean
  duplicate: boolean
  occurredAt: string
  result: Record<string, unknown>
}

export type FollowUpCommandPayload = {
  followUpId?: string | null
  personId?: string | null
  propertyId?: string | null
  dealId?: string | null
  title?: string | null
  detail?: string | null
  dueAt?: string | null
  snoozeUntil?: string | null
  outcome?: string | null
  nextTouchAt?: string | null
  nextTouchTitle?: string | null
  source?: string | null
  reason?: string | null
  recommendationKey?: string | null
}

type TaskRow = {
  id: string
  title: string
  detail: string | null
  person_id: string | null
  property_id: string | null
  deal_id: string | null
  due_at: string | null
  status: string
  snoozed_until: string | null
  outcome: string | null
  next_touch_at: string | null
  source: string | null
  reason: string | null
  recommendation_key: string | null
  completed_at: string | null
  created_at: string
  updated_at: string
}

function mapFollowUp(row: TaskRow): FollowUp {
  return {
    id: row.id,
    title: row.title,
    detail: row.detail,
    personId: row.person_id,
    propertyId: row.property_id,
    dealId: row.deal_id,
    dueAt: row.due_at,
    status: row.status as FollowUpStatus,
    snoozedUntil: row.snoozed_until,
    outcome: row.outcome,
    nextTouchAt: row.next_touch_at,
    source: row.source,
    reason: row.reason,
    recommendationKey: row.recommendation_key,
    completedAt: row.completed_at,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  }
}

type ReceiptRow = {
  id: string
  command_id: string
  command_type: string
  follow_up_id: string | null
  person_id: string | null
  applied: boolean
  duplicate: boolean
  occurred_at: string
  result: unknown
}

function mapReceipt(row: ReceiptRow): FollowUpReceipt {
  return {
    id: row.id,
    commandId: row.command_id,
    commandType: row.command_type as FollowUpCommandType,
    followUpId: row.follow_up_id,
    personId: row.person_id,
    applied: row.applied,
    duplicate: row.duplicate,
    occurredAt: row.occurred_at,
    result: (row.result as Record<string, unknown>) ?? {},
  }
}

async function readTask(q: QueryExecutor, id: string): Promise<FollowUp | null> {
  const rows = (await q`
    select id, title, detail, person_id, property_id, deal_id, due_at, status,
      snoozed_until, outcome, next_touch_at, source, reason, recommendation_key,
      completed_at, created_at, updated_at
    from task where id = ${id} limit 1
  `) as TaskRow[]
  return rows[0] ? mapFollowUp(rows[0]) : null
}

async function insertFollowUp(
  q: QueryExecutor,
  p: FollowUpCommandPayload,
): Promise<FollowUp> {
  const rows = (await q`
    insert into task (
      title, detail, person_id, property_id, deal_id, due_at, status,
      outcome, next_touch_at, source, reason, recommendation_key
    ) values (
      ${p.title?.trim() || 'Follow up'},
      ${p.detail ?? null},
      ${p.personId ?? null},
      ${p.propertyId ?? null},
      ${p.dealId ?? null},
      ${p.dueAt ?? null},
      'open',
      ${p.outcome ?? null},
      ${p.nextTouchAt ?? null},
      ${p.source ?? null},
      ${p.reason ?? null},
      ${p.recommendationKey ?? null}
    )
    returning id, title, detail, person_id, property_id, deal_id, due_at, status,
      snoozed_until, outcome, next_touch_at, source, reason, recommendation_key,
      completed_at, created_at, updated_at
  `) as TaskRow[]
  return mapFollowUp(rows[0])
}

/**
 * Apply one follow-up command atomically with a replay-safe receipt.
 * Duplicate command_id returns the existing receipt without re-applying any
 * side effect (one business effect per command).
 */
export async function applyFollowUpCommand(
  input: {
    commandId: string
    commandType: FollowUpCommandType
    payload: FollowUpCommandPayload
    actorUserId?: string | null
  },
  execute: QueryExecutor = sql,
): Promise<{
  duplicate: boolean
  receipt: FollowUpReceipt
  followUp: FollowUp | null
  nextFollowUp: FollowUp | null
}> {
  const q = execute
  {
    const existing = (await q`
      select id, command_id, command_type, follow_up_id, person_id, applied, duplicate, occurred_at, result
      from relationship_follow_up_receipt where command_id = ${input.commandId} limit 1
    `) as ReceiptRow[]
    if (existing[0]) {
      return {
        duplicate: true,
        receipt: mapReceipt(existing[0]),
        followUp: existing[0].follow_up_id ? await readTask(q, existing[0].follow_up_id) : null,
        nextFollowUp: null,
      }
    }

    await q`
      insert into relationship_follow_up_receipt (
        command_id, command_type, follow_up_id, person_id, actor_user_id, applied, duplicate
      ) values (
        ${input.commandId}, ${input.commandType}, ${input.payload.followUpId ?? null},
        ${input.payload.personId ?? null}, ${input.actorUserId ?? null}, false, false
      )
    `

    let followUp: FollowUp | null = null
    let nextFollowUp: FollowUp | null = null
    let result: Record<string, unknown> = { commandType: input.commandType }

    switch (input.commandType) {
      case 'create': {
        followUp = await insertFollowUp(q, input.payload)
        result.followUpId = followUp.id
        break
      }
      case 'snooze': {
        if (!input.payload.followUpId) throw new PortalWriteError('validation', 'followUpId is required')
        const snoozeUntil = input.payload.snoozeUntil ?? null
        await q`
          update task set status = 'snoozed', snoozed_until = ${snoozeUntil}, updated_at = now()
          where id = ${input.payload.followUpId}
        `
        followUp = await readTask(q, input.payload.followUpId)
        result.snoozedUntil = snoozeUntil
        break
      }
      case 'complete': {
        if (!input.payload.followUpId) throw new PortalWriteError('validation', 'followUpId is required')
        await q`
          update task set
            status = 'completed',
            completed_at = now(),
            outcome = coalesce(${input.payload.outcome ?? null}, outcome),
            next_touch_at = ${input.payload.nextTouchAt ?? null},
            updated_at = now()
          where id = ${input.payload.followUpId}
        `
        followUp = await readTask(q, input.payload.followUpId)
        result.followUpId = followUp?.id ?? null
        // Done + next touch creates exactly one next obligation (new task row).
        if (followUp && input.payload.nextTouchAt) {
          nextFollowUp = await insertFollowUp(q, {
            title: input.payload.nextTouchTitle ?? followUp.title,
            detail: followUp.detail,
            personId: followUp.personId,
            propertyId: followUp.propertyId,
            dealId: followUp.dealId,
            dueAt: input.payload.nextTouchAt,
            source: input.payload.source ?? followUp.source,
            reason: 'next_touch_after_completion',
          })
          result.nextFollowUpId = nextFollowUp.id
        }
        break
      }
      case 'dismiss': {
        if (!input.payload.followUpId) throw new PortalWriteError('validation', 'followUpId is required')
        await q`
          update task set status = 'dismissed', updated_at = now()
          where id = ${input.payload.followUpId}
        `
        followUp = await readTask(q, input.payload.followUpId)
        result.followUpId = followUp?.id ?? null
        break
      }
      case 'cancel': {
        if (!input.payload.followUpId) throw new PortalWriteError('validation', 'followUpId is required')
        await q`
          update task set status = 'cancelled', updated_at = now()
          where id = ${input.payload.followUpId}
        `
        followUp = await readTask(q, input.payload.followUpId)
        result.followUpId = followUp?.id ?? null
        break
      }
    }

    await q`
      update relationship_follow_up_receipt
        set applied = true, follow_up_id = ${followUp?.id ?? null},
            result = ${JSON.stringify(result)}::jsonb
      where command_id = ${input.commandId}
    `
    const receiptRow = (await q`
      select id, command_id, command_type, follow_up_id, person_id, applied, duplicate, occurred_at, result
      from relationship_follow_up_receipt where command_id = ${input.commandId} limit 1
    `) as ReceiptRow[]

    return {
      duplicate: false,
      receipt: mapReceipt(receiptRow[0]),
      followUp,
      nextFollowUp,
    }
  }
}

/** Read a single follow-up by id. */
export async function getFollowUpById(
  id: string,
  execute: QueryExecutor = sql,
): Promise<FollowUp | null> {
  return readTask(execute, id)
}

/** Open + due/snooze-due follow-ups for a person (bounded daily surface). */
export async function listActiveFollowUpsForPerson(
  personId: string,
  execute: QueryExecutor = sql,
): Promise<FollowUp[]> {
  const now = new Date().toISOString()
  const rows = (await execute`
    select id, title, detail, person_id, property_id, deal_id, due_at, status,
      snoozed_until, outcome, next_touch_at, source, reason, recommendation_key,
      completed_at, created_at, updated_at
    from task
    where person_id = ${personId}
      and status in ('open', 'snoozed')
      and (
        (status = 'open')
        or (status = 'snoozed' and snoozed_until <= ${now})
      )
    order by coalesce(due_at, snoozed_until) asc nulls last
  `) as TaskRow[]
  return rows.map(mapFollowUp)
}

