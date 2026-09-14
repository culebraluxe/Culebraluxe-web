import { ENGINE_DISPATCH_STATUS } from '../lib/story-moves'
import { setStoryboardStatus } from './storyboard'
import type { QueryExecutor } from './query-executor'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

// ---------------------------------------------------------------------------
// FORGE BATCH — the ENGINE BATCH column as a durable thing (migration 178).
//
// WHAT IT IS FOR, in the captain's words: "engine batch is just trying to say these are ready to go
// but i dont want to run them yet - maybe save for a night run when its cheaper to run". So a batch is
// a GROUP you build now and fire later, and `scheduled_for` is what makes "later" real: the unattended
// worker fires any batch whose time has passed, with nobody awake to press anything.
//
// FIRING IS THE SAME WRITE AS THE BUTTON: status `Ready` per member, which is the engine's own
// dispatch trigger (`agent_work_item_dispatch()`), so a batch creates one work item per story - never
// more, never fewer. This module does not invent a parallel dispatch path.
//
// ONE STORY AT A TIME (the system-wide single-active lock): a batch of five queues five items and the
// engine runs them one after another. That is the design, not a limitation.
// ---------------------------------------------------------------------------

export type ForgeBatchStatus = 'Staged' | 'Scheduled' | 'Fired' | 'Cancelled'
export type ForgeBatchItemState = 'Staged' | 'Queued' | 'Skipped'

export type ForgeBatch = {
  id: string
  label: string | null
  status: ForgeBatchStatus
  scheduledFor: string | null
  firedAt: string | null
  createdAt: string
  createdBy: string | null
  note: string | null
  storyCount: number
  queuedCount: number
  skippedCount: number
}

export type CreateForgeBatchInput = {
  storyIds: string[]
  label?: string | null
  /** ISO timestamp; when set the batch is `Scheduled` and fires by itself once the clock passes it. */
  scheduledFor?: string | null
  actorId?: string | null
  note?: string | null
}

/** A Date or ISO string from the driver becomes an ISO string or null — normalized at this boundary. */
export function isoOrNull(value: unknown): string | null {
  if (value == null) return null
  if (value instanceof Date) return value.toISOString()
  const text = String(value)
  const parsed = Date.parse(text)
  return Number.isNaN(parsed) ? null : new Date(parsed).toISOString()
}

export function mapForgeBatch(row: Record<string, unknown>): ForgeBatch {
  return {
    id: String(row.id),
    label: row.label == null ? null : String(row.label),
    status: String(row.status) as ForgeBatchStatus,
    scheduledFor: isoOrNull(row.scheduled_for),
    firedAt: isoOrNull(row.fired_at),
    createdAt: isoOrNull(row.created_at) ?? '',
    createdBy: row.created_by == null ? null : String(row.created_by),
    note: row.note == null ? null : String(row.note),
    storyCount: Number(row.story_count ?? 0),
    queuedCount: Number(row.queued_count ?? 0),
    skippedCount: Number(row.skipped_count ?? 0),
  }
}

// NOTE ON SHARED SQL: `QueryExecutor` is a tagged template with no fragment support — every `${}`
// becomes a bound parameter — so the batch SELECT is written out where it is used rather than kept in
// a constant that would silently become `$1`.

/**
 * Build a batch from stories the board has staged. Durable immediately: the record exists from the
 * moment it is built, so "what did I load up last night?" is answerable even if nothing ever fires.
 */
export async function createForgeBatch(
  input: CreateForgeBatchInput,
  execute?: QueryExecutor,
  options?: { allowEmpty?: boolean },
): Promise<ForgeBatch> {
  const q = execute ?? (await executor())
  const storyIds = [...new Set(input.storyIds.map((s) => String(s).trim()).filter(Boolean))]
  if (storyIds.length === 0 && !options?.allowEmpty) {
    throw new Error('a batch needs at least one story')
  }

  const status: ForgeBatchStatus = input.scheduledFor ? 'Scheduled' : 'Staged'
  const rows = await q`
    insert into forge_batch (label, status, scheduled_for, created_by, note)
    values (
      ${input.label ?? null}, ${status}, ${input.scheduledFor ? new Date(input.scheduledFor) : null},
      ${input.actorId ?? null}, ${input.note ?? null}
    )
    returning id
  `
  const batchId = String((rows[0] as { id: string }).id)
  for (const storyId of storyIds) {
    await q`
      insert into forge_batch_item (batch_id, story_id, state)
      values (${batchId}, ${storyId}, 'Staged')
      on conflict (batch_id, story_id) do nothing
    `
  }
  const created = await getForgeBatch(batchId, q)
  if (!created) throw new Error(`batch ${batchId} vanished after insert`)
  return created
}

export async function getForgeBatch(
  batchId: string,
  execute?: QueryExecutor,
): Promise<ForgeBatch | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select b.id, b.label, b.status, b.scheduled_for, b.fired_at, b.created_at, b.created_by, b.note,
      (select count(*)::int from forge_batch_item i where i.batch_id = b.id) as story_count,
      (select count(*)::int from forge_batch_item i where i.batch_id = b.id and i.state = 'Queued') as queued_count,
      (select count(*)::int from forge_batch_item i where i.batch_id = b.id and i.state = 'Skipped') as skipped_count
    from forge_batch b
    where b.id = ${batchId}
    limit 1
  `
  const row = rows[0] as Record<string, unknown> | undefined
  return row ? mapForgeBatch(row) : null
}

/** Recent batches, newest first — the Cockpit's history line. */
export async function listForgeBatches(
  limit = 10,
  execute?: QueryExecutor,
): Promise<ForgeBatch[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select b.id, b.label, b.status, b.scheduled_for, b.fired_at, b.created_at, b.created_by, b.note,
      (select count(*)::int from forge_batch_item i where i.batch_id = b.id) as story_count,
      (select count(*)::int from forge_batch_item i where i.batch_id = b.id and i.state = 'Queued') as queued_count,
      (select count(*)::int from forge_batch_item i where i.batch_id = b.id and i.state = 'Skipped') as skipped_count
    from forge_batch b
    order by b.created_at desc
    limit ${Math.max(1, limit)}
  `
  return (rows as Record<string, unknown>[]).map(mapForgeBatch)
}

export type FireForgeBatchResult = {
  batchId: string
  queued: number
  failed: Array<{ storyId: string; error: string }>
}

/**
 * FIRE A BATCH — dispatch every staged member, one work item each, and record it.
 *
 * Partial success is reported honestly (the same discipline as the Send button): a member whose write
 * throws becomes `Skipped` with its reason, the rest still fire, and the caller gets the counts.
 */
export async function fireForgeBatch(
  batchId: string,
  execute?: QueryExecutor,
): Promise<FireForgeBatchResult> {
  const q = execute ?? (await executor())
  const members = (await q`
    select story_id from forge_batch_item
    where batch_id = ${batchId} and state = 'Staged'
    order by story_id
  `) as Array<{ story_id: string }>

  const failed: Array<{ storyId: string; error: string }> = []
  let queued = 0
  for (const member of members) {
    try {
      // THE DISPATCH ITSELF: status Ready -> the engine's own trigger creates the work item.
      await setStoryboardStatus(member.story_id, ENGINE_DISPATCH_STATUS, q)
      await q`
        update forge_batch_item set state = 'Queued', queued_at = now(), error_text = null
        where batch_id = ${batchId} and story_id = ${member.story_id}
      `
      queued += 1
    } catch (error) {
      const message = String((error as Error)?.message ?? error)
      failed.push({ storyId: member.story_id, error: message })
      await q`
        update forge_batch_item set state = 'Skipped', error_text = ${message}
        where batch_id = ${batchId} and story_id = ${member.story_id}
      `
    }
  }

  await q`
    update forge_batch set status = 'Fired', fired_at = now()
    where id = ${batchId} and status <> 'Fired'
  `
  return { batchId, queued, failed }
}

/** Mark a batch cancelled before it fires (nothing has been dispatched). */
export async function cancelForgeBatch(batchId: string, execute?: QueryExecutor): Promise<number> {
  const q = execute ?? (await executor())
  const rows = await q`
    update forge_batch set status = 'Cancelled'
    where id = ${batchId} and status in ('Staged', 'Scheduled')
    returning id
  `
  return rows.length
}

/**
 * FIRE EVERY BATCH THAT IS DUE — the night run.
 *
 * Called at the top of every unattended worker pass, so a batch scheduled for 02:00 fires on the first
 * pass after 02:00 with nobody awake. The 3-minute scheduler tick is therefore also the batch tick;
 * there is no second daemon and no cron entry to forget.
 */
export async function fireDueForgeBatches(
  execute?: QueryExecutor,
): Promise<FireForgeBatchResult[]> {
  const q = execute ?? (await executor())
  const due = (await q`
    select id from forge_batch
    where status = 'Scheduled' and scheduled_for is not null and scheduled_for <= now()
    order by scheduled_for asc
  `) as Array<{ id: string }>

  const results: FireForgeBatchResult[] = []
  for (const row of due) {
    results.push(await fireForgeBatch(String(row.id), q))
  }
  return results
}

// ---------------------------------------------------------------------------
// THE STAGING BATCH — the board's ENGINE BATCH column and `forge_batch` kept in step.
//
// THE AUTOSYS MODEL, which is what the captain ran overnight cycles with for 15 years: a TABLE is the
// job stream and the tool is its view. "if its in the table it goes" (2026-09-14). So membership is not
// re-derived from a status at fire time any more - the board WRITES the row when a story is staged and
// DELETES it when the story leaves, and firing reads the rows.
//
// ONE OPEN STAGING BATCH at a time: the newest `Staged` batch is the one being built. Staging a story
// ensures it exists, then adds the member; unstaging removes the member (and the empty batch can be
// left behind harmlessly as history of an abandoned staging).
//
// BACKFILL, because the two directions have to agree during the changeover: `fireStagingBatch` also
// sweeps up any story whose status is still `Batched` but which has no row yet (staged before this
// table existed), so nothing that looks staged on the board is silently dropped from the run.
// ---------------------------------------------------------------------------

/** The newest batch still being built, or null when nothing is staged. */
export async function getStagingBatch(execute?: QueryExecutor): Promise<ForgeBatch | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select b.id, b.label, b.status, b.scheduled_for, b.fired_at, b.created_at, b.created_by, b.note,
      (select count(*)::int from forge_batch_item i where i.batch_id = b.id) as story_count,
      (select count(*)::int from forge_batch_item i where i.batch_id = b.id and i.state = 'Queued') as queued_count,
      (select count(*)::int from forge_batch_item i where i.batch_id = b.id and i.state = 'Skipped') as skipped_count
    from forge_batch b
    where b.status = 'Staged'
    order by b.created_at desc
    limit 1
  `
  const row = rows[0] as Record<string, unknown> | undefined
  return row ? mapForgeBatch(row) : null
}

/** The staging batch, created on first use. */
export async function ensureStagingBatch(
  actorId?: string | null,
  execute?: QueryExecutor,
): Promise<ForgeBatch> {
  const existing = await getStagingBatch(execute)
  if (existing) return existing
  return createForgeBatch(
    {
      storyIds: [],
      label: 'staging',
      actorId: actorId ?? null,
      note: 'built by the Cockpit ENGINE BATCH column',
    },
    execute,
    { allowEmpty: true },
  )
}

/** STAGE: the story is now a row in the table. Idempotent. */
export async function stageStoryForBatch(
  storyId: string,
  actorId?: string | null,
  execute?: QueryExecutor,
): Promise<{ batchId: string; members: number }> {
  const q = execute ?? (await executor())
  const batch = await ensureStagingBatch(actorId, q)
  await q`
    insert into forge_batch_item (batch_id, story_id, state)
    values (${batch.id}, ${storyId}, 'Staged')
    on conflict (batch_id, story_id) do nothing
  `
  const rows = (await q`
    select count(*)::int as c from forge_batch_item where batch_id = ${batch.id} and state = 'Staged'
  `) as Array<{ c: number }>
  return { batchId: batch.id, members: Number(rows[0]?.c ?? 0) }
}

/** UNSTAGE: the story leaves the table with the card. Only un-fired membership is removed. */
export async function unstageStoryForBatch(
  storyId: string,
  execute?: QueryExecutor,
): Promise<number> {
  const q = execute ?? (await executor())
  const rows = await q`
    delete from forge_batch_item
    where story_id = ${storyId} and state = 'Staged'
    returning batch_id
  `
  return rows.length
}

/** Stories whose status says staged but which have no row yet (staged before the table existed). */
export async function backfillStagingBatch(
  batchId: string,
  execute?: QueryExecutor,
): Promise<number> {
  const q = execute ?? (await executor())
  const rows = await q`
    insert into forge_batch_item (batch_id, story_id, state)
    select ${batchId}, s.id, 'Staged'
    from storyboard_story s
    where s.status = 'Batched'
      and not exists (
        select 1 from forge_batch_item i where i.batch_id = ${batchId} and i.story_id = s.id
      )
    returning story_id
  `
  return rows.length
}

/**
 * RUN THE STAGING BATCH — "if it's in the table it goes".
 *
 * Reads the rows (after sweeping up anything staged before the table existed) and fires exactly those.
 * Returns null when there is nothing staged, so the caller can say so instead of reporting "0 queued"
 * as though a run had happened.
 */
export async function fireStagingBatch(
  execute?: QueryExecutor,
): Promise<FireForgeBatchResult | null> {
  const q = execute ?? (await executor())
  const stagedOnBoard = (await q`
    select id from storyboard_story where status = 'Batched' order by id
  `) as Array<{ id: string }>
  if (stagedOnBoard.length === 0) return null

  const batch = await ensureStagingBatch(null, q)
  await backfillStagingBatch(batch.id, q)
  return fireForgeBatch(batch.id, q)
}

/**
 * SCHEDULE THE STAGING BATCH — put the time on the table and let the worker fire it.
 * Same row, same membership; scheduling only says WHEN.
 */
export async function scheduleStagingBatch(
  scheduledForIso: string,
  actorId?: string | null,
  label?: string | null,
  execute?: QueryExecutor,
): Promise<ForgeBatch> {
  const q = execute ?? (await executor())
  const batch = await ensureStagingBatch(actorId, q)
  await backfillStagingBatch(batch.id, q)
  const rows = await q`
    update forge_batch
    set status = 'Scheduled', scheduled_for = ${new Date(scheduledForIso)}, label = ${label ?? null}
    where id = ${batch.id}
    returning id
  `
  if (rows.length === 0) throw new Error(`could not schedule batch ${batch.id}`)
  const updated = await getForgeBatch(batch.id, q)
  if (!updated) throw new Error(`batch ${batch.id} vanished while scheduling`)
  return updated
}

/** Which batches a story has been in — the story's side of the history. */
export async function listForgeBatchesForStory(
  storyId: string,
  execute?: QueryExecutor,
): Promise<Array<{ batchId: string; state: ForgeBatchItemState; queuedAt: string | null }>> {
  const q = execute ?? (await executor())
  const rows = (await q`
    select batch_id, state, queued_at from forge_batch_item
    where story_id = ${storyId}
    order by queued_at desc nulls last
  `) as Array<{ batch_id: string; state: string; queued_at: unknown }>
  return rows.map((r) => ({
    batchId: String(r.batch_id),
    state: String(r.state) as ForgeBatchItemState,
    queuedAt: isoOrNull(r.queued_at),
  }))
}

