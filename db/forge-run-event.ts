import type { QueryExecutor, QueryRow } from './query-executor'

export type ForgeRunEvent = {
  id: number
  storyRunId: string
  eventType: string
  payload: Record<string, unknown>
  createdAt: string
}

type ForgeRunEventRow = QueryRow & {
  id: number | string
  story_run_id: string
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
    eventType: row.event_type,
    payload: row.payload ?? {},
    createdAt: iso(row.created_at),
  }
}

/**
 * Insert one immutable material fact beneath storyboard_story_run.
 *
 * The parent run is the mutable execution summary. This child is the durable
 * fact ledger for any Forge lane (Smith, Assay, recovery, transition, publish).
 * This module intentionally exposes no update/delete API.
 */
export async function appendForgeRunEvent(
  input: {
    storyRunId: string
    /** Compatibility only. The canonical relationship is inherited from the parent run and is not persisted twice. */
    storyId?: string
    eventType: string
    payload: Record<string, unknown>
  },
  execute?: QueryExecutor,
): Promise<ForgeRunEvent> {
  const q = execute ?? (await executor())
  const rows = await q`
    insert into storyboard_story_run_event (story_run_id, event_type, payload)
    values (
      ${input.storyRunId},
      ${input.eventType},
      ${JSON.stringify(input.payload)}::jsonb
    )
    returning id, story_run_id, event_type, payload, created_at
  `
  const row = rows[0] as ForgeRunEventRow | undefined
  if (!row) throw new Error('storyboard_story_run_event insert returned no row')
  return mapEvent(row)
}

export async function listForgeRunEventsForRun(
  storyRunId: string,
  execute?: QueryExecutor,
): Promise<ForgeRunEvent[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, story_run_id, event_type, payload, created_at
    from storyboard_story_run_event
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
    select id, story_run_id, event_type, payload, created_at
    from storyboard_story_run_event
    where story_run_id = ${storyRunId}
      and event_type = ${eventType}
    order by created_at desc, id desc
    limit 1
  `
  const row = rows[0] as ForgeRunEventRow | undefined
  return row ? mapEvent(row) : null
}

/** Resolve story-level history through the canonical run parent; no story_id duplication in the child. */
export async function latestForgeEventForStory(
  storyId: string,
  eventType: string,
  execute?: QueryExecutor,
): Promise<ForgeRunEvent | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select e.id, e.story_run_id, e.event_type, e.payload, e.created_at
    from storyboard_story_run_event e
    join storyboard_story_run r on r.id = e.story_run_id
    where r.story_id = ${storyId}
      and e.event_type = ${eventType}
    order by e.created_at desc, e.id desc
    limit 1
  `
  const row = rows[0] as ForgeRunEventRow | undefined
  return row ? mapEvent(row) : null
}
