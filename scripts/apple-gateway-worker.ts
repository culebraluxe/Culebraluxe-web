import { spawnSync } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { writeFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { sql } from '../db/client'
import { APPLE_GATEWAY_ROUTES } from '../db/apple-gateway-outbox'
import type { QueryExecutor } from '../db/query-executor'

const REPO_ROOT = process.env.CULEBRALUXE_REPO || process.cwd()
const WRITER = join(REPO_ROOT, 'scripts', 'macbridge', 'AppleGatewayWrite.swift')
const WORKER_ID = `apple-gateway:${process.pid}:${randomUUID()}`
const MAX_ATTEMPTS = 5
const RETRY_SECONDS = 30
const LIMIT = 20

type RouteSpec = {
  subscriptionId: string
  routingKey: string
  kind: 'calendar_create' | 'reminder_upsert'
}

const ROUTES: readonly RouteSpec[] = [
  {
    subscriptionId: 'apple-gateway-calendar-v1',
    routingKey: APPLE_GATEWAY_ROUTES.CALENDAR_CREATE,
    kind: 'calendar_create',
  },
  {
    subscriptionId: 'apple-gateway-reminder-v1',
    routingKey: APPLE_GATEWAY_ROUTES.REMINDER_UPSERT,
    kind: 'reminder_upsert',
  },
]

type Claimed = {
  id: string
  message_id: string
  attempt_count: number
}

type MessageRow = {
  id: string
  payload: Record<string, unknown>
}

function stringOrNull(value: unknown): string | null {
  return typeof value === 'string' ? value : null
}

function boolOr(value: unknown, fallback = false): boolean {
  return typeof value === 'boolean' ? value : fallback
}

function commandPayload(spec: RouteSpec, message: MessageRow): Record<string, unknown> {
  const payload = message.payload ?? {}
  const title = stringOrNull(payload.title)?.trim()
  if (!title) throw new Error(`${spec.kind}: title required`)

  if (spec.kind === 'calendar_create') {
    const startAt = stringOrNull(payload.startAt)
    const endAt = stringOrNull(payload.endAt)
    if (!startAt || !endAt) throw new Error('calendar_create: startAt/endAt required')
    return {
      kind: spec.kind,
      commandId: message.id,
      title,
      startAt,
      endAt,
      allDay: boolOr(payload.allDay),
      location: stringOrNull(payload.location),
      notes: stringOrNull(payload.notes),
      alert: boolOr(payload.alert),
    }
  }

  const wbsId = stringOrNull(payload.wbsId)
  if (!wbsId) throw new Error('reminder_upsert: wbsId required')
  return {
    kind: spec.kind,
    commandId: message.id,
    wbsId,
    title,
    dueAt: stringOrNull(payload.dueAt),
    completed: boolOr(payload.completed),
    notes: stringOrNull(payload.notes),
    alert: boolOr(payload.alert),
  }
}

async function registerAndMaterialize(spec: RouteSpec, q: QueryExecutor): Promise<void> {
  await q`
    insert into mq_subscription (
      id, routing_key, description, max_attempts, retry_backoff_seconds, enabled
    ) values (
      ${spec.subscriptionId}, ${spec.routingKey},
      ${'Trusted Mac EventKit gateway'}, ${MAX_ATTEMPTS}, ${RETRY_SECONDS}, true
    )
    on conflict (id) do update set
      routing_key = excluded.routing_key,
      max_attempts = excluded.max_attempts,
      retry_backoff_seconds = excluded.retry_backoff_seconds,
      enabled = true
  `
  await q`
    insert into mq_delivery (message_id, subscription_id)
    select id, ${spec.subscriptionId}
    from outbox_message
    where event_type = ${spec.routingKey}
    on conflict (message_id, subscription_id) do nothing
  `
}

async function claim(spec: RouteSpec, q: QueryExecutor): Promise<Claimed[]> {
  return (await q`
    update mq_delivery d
    set state = 'claimed',
        claimed_at = now(),
        claimed_by = ${WORKER_ID},
        lease_until = now() + interval '5 minutes',
        attempt_count = attempt_count + 1,
        updated_at = now()
    where d.id in (
      select id
      from mq_delivery
      where subscription_id = ${spec.subscriptionId}
        and (
          (state in ('pending', 'failed') and available_at <= now())
          or (state = 'claimed' and lease_until <= now())
        )
      order by available_at asc, id asc
      limit ${LIMIT}
      for update skip locked
    )
    returning id, message_id, attempt_count
  `) as unknown as Claimed[]
}

async function loadMessage(messageId: string, q: QueryExecutor): Promise<MessageRow | null> {
  const rows = (await q`
    select id, payload
    from outbox_message
    where id = ${messageId}
    limit 1
  `) as unknown as MessageRow[]
  return rows[0] ?? null
}

async function deliver(spec: RouteSpec, delivery: Claimed, q: QueryExecutor): Promise<boolean> {
  const message = await loadMessage(delivery.message_id, q)
  if (!message) throw new Error('outbox message missing')

  const path = join(tmpdir(), `culebraluxe-apple-command-${message.id}.json`)
  try {
    await writeFile(path, JSON.stringify(commandPayload(spec, message)), { mode: 0o600 })
    const result = spawnSync('/usr/bin/swift', [WRITER, '--input', path], {
      cwd: REPO_ROOT,
      encoding: 'utf8',
      timeout: 60_000,
    })
    if (result.error) throw result.error
    if (result.status !== 0) {
      throw new Error((result.stderr || result.stdout || `swift exit ${result.status}`).trim())
    }
    await q`
      update mq_delivery
      set state = 'delivered', acknowledged_at = now(), lease_until = null,
          claimed_at = null, claimed_by = null, last_error = null, updated_at = now()
      where id = ${delivery.id} and claimed_by = ${WORKER_ID}
    `
    return true
  } finally {
    await rm(path, { force: true }).catch(() => undefined)
  }
}

async function failDelivery(delivery: Claimed, error: unknown, q: QueryExecutor): Promise<void> {
  const message = String(error instanceof Error ? error.message : error).slice(0, 2000)
  const dead = delivery.attempt_count >= MAX_ATTEMPTS
  await q`
    update mq_delivery
    set state = ${dead ? 'dead' : 'failed'},
        last_error = ${message},
        lease_until = null,
        claimed_at = null,
        claimed_by = null,
        available_at = case when ${dead} then available_at else now() + interval '30 seconds' end,
        updated_at = now()
    where id = ${delivery.id} and claimed_by = ${WORKER_ID}
  `
}

async function main(): Promise<void> {
  const q = sql as unknown as QueryExecutor
  let claimed = 0
  let delivered = 0
  let failed = 0

  for (const spec of ROUTES) {
    await registerAndMaterialize(spec, q)
    const deliveries = await claim(spec, q)
    claimed += deliveries.length
    for (const delivery of deliveries) {
      try {
        if (await deliver(spec, delivery, q)) delivered += 1
      } catch (error) {
        failed += 1
        await failDelivery(delivery, error, q)
      }
    }
  }

  // Aggregate-only output: never log task/event titles, notes, or locations.
  console.log(JSON.stringify({ source: 'apple_gateway_outbound', claimed, delivered, failed }))
  if (failed > 0) process.exitCode = 1
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exitCode = 1
})
