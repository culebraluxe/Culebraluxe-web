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
