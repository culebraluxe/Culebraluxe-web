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
 * THE ENGINE INSTANCE THAT RAN THIS STORY — the Flight Recorder's key.
 *
 * The recorder reads the engine's own transaction model (timeline, causality, swimlane, raw
 * events) keyed by process instance, so a story screen can only link to it when the engine
 * has actually executed the story. Null means no run yet: the honest answer, and the reason
 * the link stays hidden rather than opening an empty shell.
 */
export async function latestForgeInstanceForStory(
  storyId: string,
  execute?: QueryExecutor,
): Promise<string | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select process_instance_id
    from forge_engine_task_execution
    where story_id = ${storyId}
    order by created_at desc
    limit 1
  `
  const id = (rows[0] as { process_instance_id?: unknown } | undefined)?.process_instance_id
  return id == null ? null : String(id)
}

/**
 * THE ENGINE'S OWN LANES: one card per STORY, from the engine ledger.
 *
 * The queues board's RUNNING and RESULTS lanes are the engine's lanes — "the batch the machine is
 * executing" and "finished ATTEMPTS" — and they were being filled by static fixture data, so a card
 * in "ENGINE DONE" looked exactly like real engine output while no run existed behind it. Opening
 * one produced an honest "no instance recorded" and an operator rightly asks why the engine's own
 * lane is lying. This reads the real ledger instead.
 *
 * Grain: the board's rule is that results rows are ATTEMPTS, not stories, so `attempts` carries the
 * count and `node` says where the latest attempt ended. `instanceId` is the real UUID for THIS
 * story's latest attempt, which is what the recorder needs — no resolution guesswork.
 *
 * Timestamps are normalized here (`created_at::text`), because a driver Date escaping the
 * repository is exactly what took the cockpit down once (see db/storyboard.ts).
 */
export type EngineRunCard = {
  storyId: string
  title: string
  instanceId: string
  /** Where the latest attempt ended, e.g. `qa_verify`, `repair_smith`. */
  lastNode: string | null
  /** `completed` | `failed` | `interrupted` (the ledger's own vocabulary). */
  status: string
  /** Attempts recorded for this story. */
  attempts: number
  at: string | null
}

export async function listEngineRunCards(
  limit = 30,
  execute?: QueryExecutor,
): Promise<EngineRunCard[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select * from (
      select distinct on (e.story_id)
        e.story_id,
        coalesce(s.title, e.story_id) as title,
        e.process_instance_id::text as instance_id,
        e.node_id,
        e.status,
        e.created_at::text as at,
        (select count(*)::int from forge_engine_task_execution x where x.story_id = e.story_id) as attempts
      from forge_engine_task_execution e
      left join storyboard_story s on s.id = e.story_id
      order by e.story_id, e.created_at desc
    ) latest
    order by latest.at desc
    limit ${limit}
  `
  return rows.map((row) => ({
    storyId: String(row.story_id),
    title: String(row.title ?? row.story_id),
    instanceId: String(row.instance_id ?? ''),
    lastNode: row.node_id == null ? null : String(row.node_id),
    status: String(row.status ?? ''),
    attempts: Number(row.attempts ?? 0),
    at: row.at == null ? null : String(row.at),
  }))
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
