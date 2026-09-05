import type { QueryExecutor } from './query-executor'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
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
    insert into forge_engine_task_execution (
      task_id, process_instance_id, token_id, story_id, node_id,
      work_item_id, worker_id, status
    ) values (
      ${input.taskId}, ${input.processInstanceId}, ${input.tokenId},
      ${input.storyId}, ${input.nodeId}, ${input.workItemId},
      ${input.workerId}, 'claimed'
    )
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
