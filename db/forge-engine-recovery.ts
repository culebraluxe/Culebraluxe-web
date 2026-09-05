import { withTransaction } from '../lib/neon-interactive'

// ---------------------------------------------------------------------------
// ENG-FORGE-V10 S3 — recover stale claimed Forge engine tasks.
//
// An engine task is claimed (work item + forge_engine_task_execution row) and
// its owner heartbeat must stay fresh. If the owner dies, recovery must be safe
// and idempotent:
//   - detect staleness from heartbeat_at/status (never steal a fresh claim);
//   - lock the process instance FIRST, then the execution row (engine lock
//     order) so termination/recovery serialize;
//   - mark the execution 'interrupted' with a compare-and-set (status +
//     stale heartbeat) — never reuse completed evidence;
//   - release the agent_work_item back to 'Ready' so a later worker creates a
//     new attempt (idempotent; a completed task is left untouched).
// ---------------------------------------------------------------------------

export const DEFAULT_FORGE_STALE_MS = 10 * 60 * 1000

export function isStaleHeartbeat(
  heartbeatAt: Date | string | null | undefined,
  now: Date,
  staleMs: number = DEFAULT_FORGE_STALE_MS,
): boolean {
  if (!heartbeatAt) return true
  const hb = heartbeatAt instanceof Date ? heartbeatAt.getTime() : new Date(heartbeatAt).getTime()
  if (Number.isNaN(hb)) return true
  return now.getTime() - hb >= staleMs
}

export type ForgeRecoveryOutcome =
  | { taskId: string; recovered: true }
  | { taskId: string; recovered: false; reason: string }

export type RecoverStaleClaimsOptions = {
  now?: Date
  staleMs?: number
  limit?: number
}

/** Recover stale claimed/running Forge engine tasks. Idempotent + observable. */
export async function recoverStaleForgeEngineClaims(
  opts: RecoverStaleClaimsOptions = {},
): Promise<ForgeRecoveryOutcome[]> {
  const now = opts.now ?? new Date()
  const staleMs = opts.staleMs ?? DEFAULT_FORGE_STALE_MS
  const limit = opts.limit ?? 20
  const cutoff = new Date(now.getTime() - staleMs)

  const stale = await withTransaction(async (tx) =>
    tx`
      select task_id, process_instance_id, work_item_id, heartbeat_at
      from forge_engine_task_execution
      where status in ('claimed', 'running') and heartbeat_at <= ${cutoff}
      order by heartbeat_at asc
      limit ${limit}
    `,
  )

  const results: ForgeRecoveryOutcome[] = []
  for (const row of stale as Array<{
    task_id: string
    process_instance_id: string
    work_item_id: string
    heartbeat_at: Date | string | null
  }>) {
    // Each recovery runs in its own transaction so a failure in one never
    // rolls back another (and never steals a fresh claim).
    const outcome = await withTransaction(async (tx) => {
      // 1. Lock the owning process instance first (engine lock order).
      await tx`select id from process_instances where id = ${row.process_instance_id} for update`
      // 2. Re-read the execution row under lock.
      const exec = await tx`
        select task_id, status, heartbeat_at from forge_engine_task_execution
        where task_id = ${row.task_id} for update
      `
      const e = (exec[0] ?? {}) as {
        status?: unknown
        heartbeat_at?: Date | string | null
      }
      if (!exec[0]) return { reason: 'missing' }
      const status = String(e.status)
      if (status !== 'claimed' && status !== 'running') return { reason: 'not-active' }
      const hb = e.heartbeat_at
      if (hb && new Date(hb).getTime() > cutoff.getTime()) return { reason: 'fresh-claim' }
      // 3. CAS: mark interrupted only if still stale.
      const updated = await tx`
        update forge_engine_task_execution
        set status = 'interrupted', last_error = 'stale claim recovered', updated_at = now()
        where task_id = ${row.task_id}
          and status in ('claimed', 'running')
          and heartbeat_at <= ${cutoff}
      `
      if (!updated.length) return { reason: 'cas-miss' }
      // 4. Release the agent work item back to Ready for a fresh attempt.
      await tx`
        update agent_work_item
        set state = 'Ready', claimed_by = null,
            error_text = 'stale claim recovered; awaiting fresh attempt', updated_at = now()
        where id = ${row.work_item_id} and state in ('Claimed', 'Running')
      `
      return null
    })

    if (!outcome) results.push({ taskId: row.task_id, recovered: true })
    else results.push({ taskId: row.task_id, recovered: false, reason: outcome.reason })
  }
  return results
}
