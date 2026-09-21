/**
 * The role contract, in FIELDS (migration 170).
 *
 * Replaces parsing a JSON line out of the model's reply. The model runs
 * scripts/forge-handoff.mjs; this reads what it wrote. A missing decision is a NULL
 * column — deterministic, not "we could not parse the text".
 */
import type { QueryExecutor } from '@/legacy/db/query-executor'
import { normalizeAcceptanceAssertions } from '@/legacy/workflow_app/forge/forge-architect-contract'

export type ForgeRoleContract = {
  decision: 'SOLO' | 'SMITH' | 'SPLIT' | 'HOLD' | 'ASSAY' | null
  size: 'SMALL' | 'MEDIUM' | 'LARGE' | null
  sizeReason: string | null
  reason: string | null
  assignmentCount: number | null
  findingIds: string[]
  mergeChecks: string[]
  surfaceScope: string[]
  /**
   * ASSAY only (migration 198, work package A): the 40-hex commit the decision verifies.
   *
   * This is a DECISION-BEARING field, not a note. The direct-to-QA route is "judge the work that already
   * exists", so the row has to say WHICH work: without this column the decision was recordable and the
   * proposal builder had nothing to name, which is why the route could not fire however well the validator
   * was written. Null for every other decision and refused by the database beside them.
   */
  verifyCandidate: string | null
  /**
   * ENG-FORGE-ACCEPTANCE-SUPPLIER-01: the HANDOFF declaration of the clause -> assertion mapping,
   * written through `scripts/forge-handoff.mjs --acceptance-assertion`. Null = not declared.
   */
  acceptanceAssertions: Record<string, string[]> | null
  attempt: number
}

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('@/legacy/db/client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

/** Map one contract row. The ONE place the row shape becomes the application contract. */
function mapContractRow(row: Record<string, unknown>, fallbackAttempt: number): ForgeRoleContract {
  return {
    decision: (row.decision ?? null) as ForgeRoleContract['decision'],
    size: (row.size ?? null) as ForgeRoleContract['size'],
    sizeReason: (row.size_reason ?? null) as string | null,
    reason: (row.reason ?? null) as string | null,
    assignmentCount: row.assignment_count == null ? null : Number(row.assignment_count),
    findingIds: Array.isArray(row.finding_ids) ? (row.finding_ids as string[]) : [],
    mergeChecks: Array.isArray(row.merge_checks) ? (row.merge_checks as string[]) : [],
    surfaceScope: Array.isArray(row.surface_scope) ? (row.surface_scope as string[]) : [],
    verifyCandidate: typeof row.verify_candidate === 'string' ? row.verify_candidate : null,
    acceptanceAssertions: normalizeAcceptanceAssertions(row.acceptance_assertions),
    attempt: Number(row.attempt ?? fallbackAttempt),
  }
}

/** The contract recorded for this task/node/attempt, or null when none was written. */
export async function getForgeRoleContract(
  key: { taskId: string; nodeId: string; attempt: number },
  execute?: QueryExecutor,
): Promise<ForgeRoleContract | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select decision, size, size_reason, reason, assignment_count,
           finding_ids, merge_checks, surface_scope, verify_candidate, acceptance_assertions, attempt
    from forge_role_contract
    where task_id = ${key.taskId}
      and node_id = ${key.nodeId}
      and attempt = ${key.attempt}
    limit 1
  `
  const row = rows[0]
  if (!row) return null
  return mapContractRow(row as Record<string, unknown>, key.attempt)
}

/**
 * The NEWEST contract for a story/process/node, across tasks.
 *
 * The consumer that needs the HANDOFF acceptance mapping (QA) runs under a DIFFERENT task than the
 * `lead_pre` that wrote the contract, so the task-scoped read above returns nothing there. This
 * story-scoped read is what lets a Lead-declared mapping actually reach the lane that consumes it —
 * without it the handoff place was declarable but unreadable by QA.
 */
export async function getLatestForgeRoleContractForStory(
  key: { storyId: string; processInstanceId: string; nodeId: string },
  execute?: QueryExecutor,
): Promise<ForgeRoleContract | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select decision, size, size_reason, reason, assignment_count,
           finding_ids, merge_checks, surface_scope, verify_candidate, acceptance_assertions, attempt
    from forge_role_contract
    where story_id = ${key.storyId}
      and process_instance_id = ${key.processInstanceId}
      and node_id = ${key.nodeId}
    order by attempt desc
    limit 1
  `
  const row = rows[0]
  if (!row) return null
  return mapContractRow(row as Record<string, unknown>, 1)
}
