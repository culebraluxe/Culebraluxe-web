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
