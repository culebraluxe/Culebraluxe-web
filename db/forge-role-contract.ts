/**
 * The role contract, in FIELDS (migration 170).
 *
 * Replaces parsing a JSON line out of the model's reply. The model runs
 * scripts/forge-handoff.mjs; this reads what it wrote. A missing decision is a NULL
 * column — deterministic, not "we could not parse the text".
 */
import type { QueryExecutor } from './query-executor'

export type ForgeRoleContract = {
  decision: 'SOLO' | 'SMITH' | 'SPLIT' | 'HOLD' | null
  size: 'SMALL' | 'MEDIUM' | 'LARGE' | null
  sizeReason: string | null
  reason: string | null
  assignmentCount: number | null
  findingIds: string[]
  mergeChecks: string[]
  surfaceScope: string[]
  attempt: number
}

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

/** The contract recorded for this task/node/attempt, or null when none was written. */
export async function getForgeRoleContract(
  key: { taskId: string; nodeId: string; attempt: number },
  execute?: QueryExecutor,
): Promise<ForgeRoleContract | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select decision, size, size_reason, reason, assignment_count,
           finding_ids, merge_checks, surface_scope, attempt
    from forge_role_contract
    where task_id = ${key.taskId}
      and node_id = ${key.nodeId}
      and attempt = ${key.attempt}
    limit 1
  `
  const row = rows[0]
  if (!row) return null
  return {
    decision: (row.decision ?? null) as ForgeRoleContract['decision'],
    size: (row.size ?? null) as ForgeRoleContract['size'],
    sizeReason: (row.size_reason ?? null) as string | null,
    reason: (row.reason ?? null) as string | null,
    assignmentCount: row.assignment_count == null ? null : Number(row.assignment_count),
    findingIds: Array.isArray(row.finding_ids) ? (row.finding_ids as string[]) : [],
    mergeChecks: Array.isArray(row.merge_checks) ? (row.merge_checks as string[]) : [],
    surfaceScope: Array.isArray(row.surface_scope) ? (row.surface_scope as string[]) : [],
    attempt: Number(row.attempt ?? key.attempt),
  }
}
