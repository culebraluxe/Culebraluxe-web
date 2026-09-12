import { sql } from './client'

import type {
  CreateInteractionInput,
  Interaction,
  InteractionChannel,
  InteractionDirection,
  JsonObject,
} from '../lib/crm-types'
import type { QueryExecutor } from './query-executor'

export type { QueryExecutor } from './query-executor'

type InteractionRow = {
  id: string
  person_id: string
  property_id: string | null
  deal_id: string | null
  channel: InteractionChannel
  event_type: string
  direction: InteractionDirection | null
  occurred_at: string
  title: string | null
  summary: string | null
  duration_seconds: number | null
  source_system: string | null
  source_external_id: string | null
  source_metadata: JsonObject | null
  created_at: string
}

function mapInteraction(row: InteractionRow): Interaction {
  return {
    id: row.id,
    personId: row.person_id,
    propertyId: row.property_id ?? undefined,
    dealId: row.deal_id ?? undefined,
    channel: row.channel,
    eventType: row.event_type,
    direction: row.direction ?? undefined,
    occurredAt: row.occurred_at,
    title: row.title ?? undefined,
    summary: row.summary ?? undefined,
    durationSeconds: row.duration_seconds ?? undefined,
    sourceSystem: row.source_system ?? undefined,
    sourceExternalId: row.source_external_id ?? undefined,
    sourceMetadata: row.source_metadata ?? {},
    createdAt: row.created_at,
  }
}

function validateSourceIdentity(input: CreateInteractionInput) {
  const hasSourceSystem = Boolean(input.sourceSystem?.trim())
  const hasSourceExternalId = Boolean(input.sourceExternalId?.trim())

  if (hasSourceSystem !== hasSourceExternalId) {
    throw new Error(
      'sourceSystem and sourceExternalId must be provided together.',
    )
  }
}

export async function getInteractionById(
  id: string,
  execute: QueryExecutor = sql,
): Promise<Interaction | null> {
  const rows = await execute`
    select
      id,
      person_id,
      property_id,
      deal_id,
      channel,
      event_type,
      direction,
      occurred_at,
      title,
      summary,
      duration_seconds,
      source_system,
      source_external_id,
      source_metadata,
      created_at
    from interaction
    where id = ${id}
    limit 1
  `

  const row = rows[0] as InteractionRow | undefined
  return row ? mapInteraction(row) : null
}

export async function getInteractionBySourceIdentity(
  sourceSystem: string,
  sourceExternalId: string,
  execute: QueryExecutor = sql,
): Promise<Interaction | null> {
  const normalizedSystem = sourceSystem.trim()
  const normalizedExternalId = sourceExternalId.trim()

  if (!normalizedSystem || !normalizedExternalId) {
    throw new Error(
      'sourceSystem and sourceExternalId must be provided together.',
    )
  }

  const rows = await execute`
    select
      id,
      person_id,
      property_id,
      deal_id,
      channel,
      event_type,
      direction,
      occurred_at,
      title,
      summary,
      duration_seconds,
      source_system,
      source_external_id,
      source_metadata,
      created_at
    from interaction
    where source_system = ${normalizedSystem}
      and source_external_id = ${normalizedExternalId}
    limit 1
  `

  const row = rows[0] as InteractionRow | undefined
  return row ? mapInteraction(row) : null
}

export async function createInteraction(
  input: CreateInteractionInput,
  execute: QueryExecutor = sql,
): Promise<{ interaction: Interaction; created: boolean }> {
  validateSourceIdentity(input)

  if (!input.personId || !input.eventType.trim()) {
    throw new Error('personId and eventType are required.')
  }

  if (
    input.durationSeconds !== undefined &&
    (!Number.isInteger(input.durationSeconds) || input.durationSeconds < 0)
  ) {
    throw new Error('durationSeconds must be a non-negative integer.')
  }

  const occurredAt =
    input.occurredAt instanceof Date
      ? input.occurredAt.toISOString()
      : input.occurredAt
  const sourceSystem = input.sourceSystem?.trim() || null
  const sourceExternalId = input.sourceExternalId?.trim() || null

  const insertedRows = await execute`
    insert into interaction (
      person_id,
      property_id,
      deal_id,
      channel,
      event_type,
      direction,
      occurred_at,
      title,
      summary,
      duration_seconds,
      source_system,
      source_external_id,
      source_metadata
    ) values (
      ${input.personId},
      ${input.propertyId ?? null},
      ${input.dealId ?? null},
      ${input.channel},
      ${input.eventType.trim()},
      ${input.direction ?? null},
      ${occurredAt},
      ${input.title ?? null},
      ${input.summary ?? null},
      ${input.durationSeconds ?? null},
      ${sourceSystem},
      ${sourceExternalId},
      ${JSON.stringify(input.sourceMetadata ?? {})}::jsonb
    )
    on conflict (source_system, source_external_id)
      where source_system is not null
        and source_external_id is not null
    do nothing
    returning
      id,
      person_id,
      property_id,
      deal_id,
      channel,
      event_type,
      direction,
      occurred_at,
      title,
      summary,
      duration_seconds,
      source_system,
      source_external_id,
      source_metadata,
      created_at
  `

  const inserted = insertedRows[0] as InteractionRow | undefined
  if (inserted) {
    return { interaction: mapInteraction(inserted), created: true }
  }

  if (!sourceSystem || !sourceExternalId) {
    throw new Error('Interaction insert did not return a row.')
  }

  const existingRows = await execute`
    select
      id,
      person_id,
      property_id,
      deal_id,
      channel,
      event_type,
      direction,
      occurred_at,
      title,
      summary,
      duration_seconds,
      source_system,
      source_external_id,
      source_metadata,
      created_at
    from interaction
    where source_system = ${sourceSystem}
      and source_external_id = ${sourceExternalId}
    limit 1
  `

  const existing = existingRows[0] as InteractionRow | undefined
  if (!existing) {
    throw new Error('Idempotent interaction could not be resolved.')
  }

  return { interaction: mapInteraction(existing), created: false }
}

/**
 * Set-based interaction write for a bounded batch.
 *
 * createInteraction pays one round trip for the insert and, when the row already
 * exists, a SECOND round trip to read it back. At 68.7ms per round trip (measured
 * over the pooled Neon driver) that is what turns a 93,000-message store into
 * hours. This form answers the only question a bulk materializer asks - how many
 * were genuinely new - in ONE statement, and a row it cannot accept is counted
 * instead of thrown, matching the single-row contract.
 *
 * Replay-safe on the same unique partial index (source_system, source_external_id).
 */
export async function createInteractionsBatch(
  inputs: readonly CreateInteractionInput[],
  execute: QueryExecutor = sql,
): Promise<{ inserted: number; rejected: number }> {
  const accepted: CreateInteractionInput[] = []
  let rejected = 0

  for (const input of inputs) {
    try {
      validateSourceIdentity(input)
      if (!input.personId || !input.eventType.trim()) {
        throw new Error('personId and eventType are required.')
      }
      if (
        input.durationSeconds !== undefined &&
        (!Number.isInteger(input.durationSeconds) || input.durationSeconds < 0)
      ) {
        throw new Error('durationSeconds must be a non-negative integer.')
      }
      accepted.push(input)
    } catch {
      rejected += 1
    }
  }

  if (accepted.length === 0) return { inserted: 0, rejected }

  const occurredAt = accepted.map((input) =>
    input.occurredAt instanceof Date ? input.occurredAt.toISOString() : input.occurredAt,
  )

  const rows = (await execute`
    insert into interaction (
      person_id, property_id, deal_id, channel, event_type, direction, occurred_at,
      title, summary, duration_seconds, source_system, source_external_id, source_metadata
    )
    select
      t.person_id, t.property_id, t.deal_id, t.channel, t.event_type, t.direction,
      t.occurred_at, t.title, t.summary, t.duration_seconds, t.source_system,
      t.source_external_id, (t.source_metadata)::jsonb
    from unnest(
      ${accepted.map((input) => input.personId)}::uuid[],
      ${accepted.map((input) => input.propertyId ?? null)}::uuid[],
      ${accepted.map((input) => input.dealId ?? null)}::uuid[],
      ${accepted.map((input) => input.channel)}::text[],
      ${accepted.map((input) => input.eventType.trim())}::text[],
      ${accepted.map((input) => input.direction ?? null)}::text[],
      ${occurredAt}::timestamptz[],
      ${accepted.map((input) => input.title ?? null)}::text[],
      ${accepted.map((input) => input.summary ?? null)}::text[],
      ${accepted.map((input) => input.durationSeconds ?? null)}::int[],
      ${accepted.map((input) => input.sourceSystem?.trim() || null)}::text[],
      ${accepted.map((input) => input.sourceExternalId?.trim() || null)}::text[],
      ${accepted.map((input) => JSON.stringify(input.sourceMetadata ?? {}))}::text[]
    ) as t(
      person_id, property_id, deal_id, channel, event_type, direction, occurred_at,
      title, summary, duration_seconds, source_system, source_external_id, source_metadata
    )
    on conflict (source_system, source_external_id)
      where source_system is not null
        and source_external_id is not null
    do nothing
    returning id
  `) as unknown as unknown[]

  return { inserted: Array.isArray(rows) ? rows.length : 0, rejected }
}
