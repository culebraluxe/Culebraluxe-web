import type { ForgeConvergenceExecution } from '../workflow_app/forge/forge-convergence'
import { projectStoryRunsToConvergence, type ForgeNodeRun } from '../workflow_app/forge/forge-convergence-projector'
import type { QueryExecutor, QueryRow } from './query-executor'

// ---------------------------------------------------------------------------
// Scope D — server read that assembles a Forge story's durable role-node runs
// and projects them through the convergence read model for the TECH UI.
// Read-only over existing durable evidence; no writes, no schema change.
// ---------------------------------------------------------------------------

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

type EngineRunRow = QueryRow & {
  node_id: string
  commit_hash: string | null
  result_status: string | null
  created_at: unknown
}

function asIso(value: unknown): string {
  if (value instanceof Date) return value.toISOString()
  if (typeof value === 'string') return value
  if (typeof value === 'number') return new Date(value).toISOString()
  return new Date().toISOString()
}

function mapRuns(rows: EngineRunRow[]): ForgeNodeRun[] {
  return rows.map((row) => ({
    nodeId: String(row.node_id),
    storyRunId: null,
    commitHash: row.commit_hash ? String(row.commit_hash) : null,
    createdAt: asIso(row.created_at),
    resultStatus: row.result_status ? String(row.result_status) : null,
  }))
}

export type StoryForgeConvergence = {
  storyId: string
  processInstanceId: string | null
  processStatus: string | null
  executions: ForgeConvergenceExecution[]
}

/**
 * Convergence for one Forge story. Falls back to the active instance first;
 * prior instances (replans that spawned a new process) are preserved in the
 * run-derived history via generation reconstruction.
 */
export async function readStoryForgeConvergence(
  storyId: string,
  execute?: QueryExecutor,
): Promise<StoryForgeConvergence> {
  const q = execute ?? (await executor())
  const instanceRows = await q`
    select pi.id, pi.status
    from process_instances pi
    join process_definitions pd on pd.id = pi.definition_id
    where pi.subject_type = 'story' and pi.subject_id = ${storyId} and pd.key = 'FORGE_SDLC'
    order by (pi.status = 'active') desc, pi.created_at desc
    limit 1
  `
  const instance = instanceRows[0] as { id: string; status: string } | undefined
  const processInstanceId = instance ? String(instance.id) : null
  const processStatus = instance ? String(instance.status) : null

  const ledgerRows = await q`
    select forge_repair_attempts, forge_replan_attempts
    from storyboard_story where id = ${storyId}
  `
  const ledger = ledgerRows[0] as { forge_repair_attempts: number | null; forge_replan_attempts: number | null } | undefined
  const repairAttempts = Number(ledger?.forge_repair_attempts ?? 0)
  const replanAttempts = Number(ledger?.forge_replan_attempts ?? 0)

  const runRows = processInstanceId
    ? await q`
        select f.node_id, r.commit_hash, r.result_status, f.created_at
        from forge_engine_task_execution f
        left join storyboard_story_run r on r.id = f.story_run_id
        where f.story_id = ${storyId} and f.process_instance_id = ${processInstanceId}
        order by f.created_at
      `
    : []
  const runs = mapRuns(runRows as EngineRunRow[])

  const executions = projectStoryRunsToConvergence({
    storyId,
    processInstanceId: processInstanceId ?? `story-${storyId}`,
    runs,
    repairAttempts,
    replanAttempts,
  })
  return { storyId, processInstanceId, processStatus, executions }
}

/** Convergence for every story that has run under the Forge engine. */
export async function listForgeConvergence(
  execute?: QueryExecutor,
): Promise<StoryForgeConvergence[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select distinct story_id from forge_engine_task_execution
  `
  const ids = (rows as Array<{ story_id: string }>).map((r) => String(r.story_id))
  const out: StoryForgeConvergence[] = []
  for (const storyId of ids) {
    out.push(await readStoryForgeConvergence(storyId, q))
  }
  out.sort((a, b) => a.storyId.localeCompare(b.storyId))
  return out
}
