import type { QueryExecutor } from './query-executor'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

/**
 * HOW MANY TURNS THIS GENERATION HAS ALREADY DISPATCHED.
 *
 * The engine's own ledger is the count: every role turn this process instance started is a
 * row here, so the number is a fact rather than an estimate. The Assay counts too — it is a
 * turn of the loop even when no model runs in it, and a cap that ignored the checkpoint
 * would bound the wrong thing.
 *
 * Measured on 2026-09-13: a healthy FEATURE generation costs 5 turns (architect, lead_pre,
 * smith, post, qa), a FAST one with a repair costs 4, a generation that HOLDs at the Lead
 * costs 2. See `workflow_app/forge/model-turn-budget.ts` for why the cap is 10.
 */
export async function countForgeGenerationTurns(
  processInstanceId: string,
  execute?: QueryExecutor,
): Promise<number> {
  const q = execute ?? (await executor())
  const rows = await q`
    select count(*)::int as turns
    from forge_engine_task_execution
    where process_instance_id = ${processInstanceId}
  `
  return Number((rows[0] as { turns?: unknown } | undefined)?.turns ?? 0)
}

export async function linkForgeEngineTaskExecution(
  input: {
    taskId: string
    processInstanceId: string
    tokenId: string
    storyId: string
    nodeId: string
    workItemId: string
    workerId: string
  },
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  await q`
    with relinked as (
      update forge_engine_task_execution
      set task_id = ${input.taskId},
          process_instance_id = ${input.processInstanceId},
          token_id = ${input.tokenId},
          story_id = ${input.storyId},
          node_id = ${input.nodeId},
          worker_id = ${input.workerId},
          story_run_id = null,
          status = 'claimed',
          last_error = null,
          heartbeat_at = now(),
          completed_at = null,
          updated_at = now()
      where work_item_id = ${input.workItemId}
      returning work_item_id
    )
    insert into forge_engine_task_execution (
      task_id, process_instance_id, token_id, story_id, node_id,
      work_item_id, worker_id, status
    )
    select
      ${input.taskId}, ${input.processInstanceId}, ${input.tokenId},
      ${input.storyId}, ${input.nodeId}, ${input.workItemId},
      ${input.workerId}, 'claimed'
    where not exists (select 1 from relinked)
    on conflict (task_id) do update set
      heartbeat_at = now(),
      updated_at = now()
    where forge_engine_task_execution.work_item_id = excluded.work_item_id
  `
}

export async function finishForgeEngineTaskExecution(
  taskId: string,
  input: {
    storyRunId?: string | null
    status: 'completed' | 'failed' | 'interrupted'
    error?: string | null
  },
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  await q`
    update forge_engine_task_execution
    set story_run_id = coalesce(${input.storyRunId ?? null}, story_run_id),
        status = ${input.status},
        last_error = ${input.error ?? null},
        heartbeat_at = now(),
        completed_at = case when ${input.status} = 'completed' then now() else completed_at end,
        updated_at = now()
    where task_id = ${taskId}
  `
}
