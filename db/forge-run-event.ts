import type { QueryExecutor, QueryRow } from './query-executor'

export type ForgeRunEvent = {
  id: number
  storyRunId: string
  storyId: string
  eventType: string
  payload: Record<string, unknown>
  createdAt: string
}

type ForgeRunEventRow = QueryRow & {
  id: number | string
  story_run_id: string
  story_id: string
  event_type: string
  payload: Record<string, unknown> | null
  created_at: string | Date
}

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

function iso(value: unknown): string {
  if (value instanceof Date) return value.toISOString()
  return String(value ?? '')
}

function mapEvent(row: ForgeRunEventRow): ForgeRunEvent {
  return {
    id: Number(row.id),
    storyRunId: row.story_run_id,
    storyId: row.story_id,
    eventType: row.event_type,
    payload: row.payload ?? {},
    createdAt: iso(row.created_at),
  }
}

/** Insert one immutable execution fact. This module intentionally exposes no update/delete API. */
export async function appendForgeRunEvent(
  input: {
    storyRunId: string
    storyId: string
    eventType: string
    payload: Record<string, unknown>
  },
  execute?: QueryExecutor,
): Promise<ForgeRunEvent> {
  const q = execute ?? (await executor())
  const rows = await q`
    insert into forge_run_event (story_run_id, story_id, event_type, payload)
    values (
      ${input.storyRunId},
      ${input.storyId},
      ${input.eventType},
      ${JSON.stringify(input.payload)}::jsonb
    )
    returning id, story_run_id, story_id, event_type, payload, created_at
  `
  const row = rows[0] as ForgeRunEventRow | undefined
  if (!row) throw new Error('forge_run_event insert returned no row')
  return mapEvent(row)
}

export async function listForgeRunEventsForRun(
  storyRunId: string,
  execute?: QueryExecutor,
): Promise<ForgeRunEvent[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, story_run_id, story_id, event_type, payload, created_at
    from forge_run_event
    where story_run_id = ${storyRunId}
    order by created_at asc, id asc
  `
  return rows.map((row) => mapEvent(row as ForgeRunEventRow))
}

export async function latestForgeRunEvent(
  storyRunId: string,
  eventType: string,
  execute?: QueryExecutor,
): Promise<ForgeRunEvent | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, story_run_id, story_id, event_type, payload, created_at
    from forge_run_event
    where story_run_id = ${storyRunId}
      and event_type = ${eventType}
    order by created_at desc, id desc
    limit 1
  `
  const row = rows[0] as ForgeRunEventRow | undefined
  return row ? mapEvent(row) : null
}

export async function latestForgeEventForStory(
  storyId: string,
  eventType: string,
  execute?: QueryExecutor,
): Promise<ForgeRunEvent | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, story_run_id, story_id, event_type, payload, created_at
    from forge_run_event
    where story_id = ${storyId}
      and event_type = ${eventType}
    order by created_at desc, id desc
    limit 1
  `
  const row = rows[0] as ForgeRunEventRow | undefined
  return row ? mapEvent(row) : null
}
