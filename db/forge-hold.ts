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
// ENG-FORGE-V10 S4 — durable HOLD audit (forge_hold_record).
// ---------------------------------------------------------------------------

export type ForgeHoldResolutionInput = {
  processInstanceId: string
  taskId: string | null
  storyId: string
  reason: string
  originatingNode: string | null
  failureClass: string | null
  resumeTarget: string | null
  resolver: string
  resolution: 'resolve' | 'cancel' | 'fail'
  resolutionNote: string | null
}

/** Append one durable HOLD audit row (open or resolved). */
export async function appendForgeHoldRecord(
  input: ForgeHoldResolutionInput,
  execute?: QueryExecutor,
): Promise<number> {
  const q = execute ?? (await executor())
  const rows = await q`
    insert into forge_hold_record (
      process_instance_id, task_id, story_id, reason, originating_node,
      failure_class, resume_target, resolver, resolution, resolution_note, resolved_at
    ) values (
      ${input.processInstanceId}, ${input.taskId}, ${input.storyId}, ${input.reason},
      ${input.originatingNode}, ${input.failureClass}, ${input.resumeTarget},
      ${input.resolver}, ${input.resolution}, ${input.resolutionNote}, now()
    )
    returning id
  `
  return Number(rows[0]?.id)
}

/**
 * Record an OPEN hold - the durable row for "the machine deliberately stopped, here is why".
 *
 * `appendForgeHoldRecord` sets `resolved_at = now()`, which is right for a resolution and wrong for a
 * park: it made every row look already-resolved, so an unresolved HOLD had no durable row at all and
 * `forge_hold_record` stayed EMPTY (measured 2026-09-14: zero rows, ever) while the engine parked
 * repeatedly. The table is append-only, so an open hold is an insert with a null `resolved_at` and its
 * resolution is a later row - history is never rewritten.
 */
/**
 * An OPEN hold has no resolver and no resolution yet - that is the point of recording it.
 */
export type ForgeOpenHoldInput = {
  processInstanceId: string
  taskId: string | null
  storyId: string
  reason: string
  originatingNode: string | null
  failureClass: string | null
  resumeTarget: string | null
}

export async function openForgeHoldRecord(
  input: ForgeOpenHoldInput,
  execute?: QueryExecutor,
): Promise<number> {
  const q = execute ?? (await executor())
  const rows = await q`
    insert into forge_hold_record (
      process_instance_id, task_id, story_id, reason, originating_node,
      failure_class, resume_target, resolver, resolution, resolution_note, resolved_at
    ) values (
      ${input.processInstanceId}, ${input.taskId}, ${input.storyId}, ${input.reason},
      ${input.originatingNode}, ${input.failureClass}, ${input.resumeTarget},
      null, null, null, null
    )
    returning id
  `
  return Number(rows[0]?.id)
}

/**
 * THE ENGINE'S CURRENT STOP FOR A STORY, if it is still parked.
 *
 * A HOLD is a deliberate machine stop with a reason, and until now the board did not show it at all:
 * a story the engine had abandoned sat on screen reading "In Progress / 100%" while `forge_hold_record`
 * held the truth. `resolved_at is null` is what makes a hold CURRENT - the table is append-only, so
 * every resolution is a later row rather than an edit, and history is never rewritten.
 */
export type ForgeStoryHold = {
  reason: string | null
  originatingNode: string | null
  failureClass: string | null
  resumeTarget: string | null
  since: string | null
  processInstanceId: string | null
}

export async function latestOpenForgeHold(
  storyId: string,
  execute?: QueryExecutor,
): Promise<ForgeStoryHold | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select reason, originating_node, failure_class, resume_target,
           created_at::text as since, process_instance_id::text as instance_id
    from forge_hold_record
    where story_id = ${storyId} and resolved_at is null
    order by created_at desc
    limit 1
  `
  const row = (rows[0] ?? null) as Record<string, unknown> | null
  if (!row) return null
  return {
    reason: row.reason == null ? null : String(row.reason),
    originatingNode: row.originating_node == null ? null : String(row.originating_node),
    failureClass: row.failure_class == null ? null : String(row.failure_class),
    resumeTarget: row.resume_target == null ? null : String(row.resume_target),
    since: row.since == null ? null : String(row.since),
    processInstanceId: row.instance_id == null ? null : String(row.instance_id),
  }
}

export async function listForgeHolds(
  storyId: string,
  execute?: QueryExecutor,
): Promise<Record<string, unknown>[]> {
  const q = execute ?? (await executor())
  return q`
    select id, process_instance_id, task_id, reason, originating_node,
           failure_class, resume_target, resolver, resolution, resolution_note,
           created_at, resolved_at
    from forge_hold_record
    where story_id = ${storyId}
    order by created_at desc
  `
}
