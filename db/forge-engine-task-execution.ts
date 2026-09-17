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

// How long a claim may go untouched before it is treated as abandoned. Deliberately the same window
// `forge:clean` uses to interrupt stale claims, so the screen and the control-plane cleaner agree on
// what "abandoned" means.
export const ENGINE_CLAIM_STALE_MS = 15 * 60 * 1000

/**
 * A POSTGRES TIMESTAMP, READ AS THE INSTANT IT NAMES.
 *
 * The driver emits `updated_at::text` with an offset already attached - `2026-09-17 07:16:52.653+00`
 * - and the previous read appended a literal `Z` to that, so `Date.parse` saw `...+00Z` and returned
 * NaN. Every live claim then read as stale, because the stale test treated an unparseable value as
 * abandoned. This normalizes each form the driver emits to one instant:
 *   `... 07:16:52.653+00`     (bare offset)   -> `+00:00`
 *   `... 07:16:52.653+00:00`  (colon offset)  -> as-is
 *   `... 07:16:52.653Z`       (Z)             -> as-is
 *   `... 07:16:52.653`        (no offset)     -> read as UTC
 * Returns NaN only for text that names no instant at all; the caller must NOT read that as stale.
 */
export function parsePostgresInstant(value: string | null | undefined): number {
  if (value == null) return NaN
  const text = String(value).trim().replace(' ', 'T')
  if (/[Zz]$/.test(text)) return Date.parse(text)
  if (/[+-]\d{2}$/.test(text)) return Date.parse(`${text}:00`)
  if (/[+-]\d{2}:?\d{2}$/.test(text)) return Date.parse(text)
  return Date.parse(`${text}Z`)
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
  /** `completed` | `failed` | `interrupted` | `claimed` | `running` (the ledger's own vocabulary). */
  status: string
  /** Attempts recorded for this story. */
  attempts: number
  at: string | null
  /**
   * TRUE when the row still says a worker holds it but nobody has touched it inside the stale-claim
   * window (the same 15 minutes `forge:clean` uses).
   *
   * Without this, an abandoned claim looks EXACTLY like live work: the captain saw
   * ENG-FORGE-TURN-VISIBILITY-01 sitting in ENGINE QUEUE as "running · 83" hours after the worker that
   * claimed it was killed. A screen that reports an abandoned claim as running is the same lie as a
   * silent refusal - the engine is idle and the board says it is busy.
   */
  stale: boolean
  /** ISO text of the row's last touch, for saying HOW stale. Normalized at this boundary. */
  updatedAt: string | null
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
        e.updated_at::text as updated_at,
        (select count(*)::int from forge_engine_task_execution x where x.story_id = e.story_id) as attempts
      from forge_engine_task_execution e
      left join storyboard_story s on s.id = e.story_id
      order by e.story_id, e.created_at desc
    ) latest
    order by latest.at desc
    limit ${limit}
  `
  const staleBefore = Date.now() - ENGINE_CLAIM_STALE_MS
  return rows.map((row) => {
    const status = String(row.status ?? '')
    const updatedAt = row.updated_at == null ? null : String(row.updated_at)
    const touched = parsePostgresInstant(updatedAt)
    const terminal = status === 'completed' || status === 'failed' || status === 'interrupted'
    return {
      storyId: String(row.story_id),
      title: String(row.title ?? row.story_id),
      instanceId: String(row.instance_id ?? ''),
      lastNode: row.node_id == null ? null : String(row.node_id),
      status,
      attempts: Number(row.attempts ?? 0),
      at: row.at == null ? null : String(row.at),
      // Only a NON-TERMINAL row can be stale: a completed row is simply finished. A timestamp that
      // names no instant is NOT stale either - the screen must not call a claim abandoned on text it
      // could not read; the cleaner, not this read, is the authority on abandonment.
      stale: !terminal && Number.isFinite(touched) && touched < staleBefore,
      updatedAt,
    }
  })
}

/**
 * THE LEDGER'S OWN NUMBERS.
 *
 * The queues board's stats strip used to render fixture figures while the copy above it said the
 * numbers were "LIVE from PROD" - a screen claiming provenance it did not have. These come from
 * `forge_engine_task_execution`, which is where the engine records every role turn it dispatches, so
 * the strip measures churn (attempts) rather than throughput, which is the honest reading of a
 * workflow engine that retries.
 *
 * Timestamps are normalized to text at this boundary, like every other reader here: a driver Date
 * escaping the repository is what took the cockpit down once already.
 */
export type EngineLedgerStats = {
  /** Role turns recorded - the engine's own count of work attempts. */
  totalAttempts: number
  /** Stories the ledger has ever touched. */
  stories: number
  /** Latest attempt's status per story: `completed` | `failed` | `interrupted`. */
  latest: { completed: number; failed: number; interrupted: number }
  /** The story with the most recorded attempts, or null when the ledger is empty. */
  worstOffender: { storyId: string; attempts: number } | null
  /** Newest attempt timestamp (text), for an honest "as of". */
  asOf: string | null
}

export async function listEngineLedgerStats(
  execute?: QueryExecutor,
): Promise<EngineLedgerStats> {
  const q = execute ?? (await executor())
  const totals = await q`
    select count(*)::int as attempts,
           count(distinct story_id)::int as stories,
           max(created_at)::text as as_of
    from forge_engine_task_execution
  `
  const latest = await q`
    select status, count(*)::int as n
    from (
      select distinct on (story_id) story_id, status
      from forge_engine_task_execution
      order by story_id, created_at desc
    ) per_story
    group by status
  `
  const worst = await q`
    select story_id, count(*)::int as attempts
    from forge_engine_task_execution
    group by story_id
    order by attempts desc, story_id asc
    limit 1
  `

  const byStatus = new Map<string, number>()
  for (const row of latest) byStatus.set(String(row.status), Number(row.n ?? 0))

  const totalsRow = (totals[0] ?? {}) as Record<string, unknown>
  const worstRow = (worst[0] ?? null) as Record<string, unknown> | null

  return {
    totalAttempts: Number(totalsRow.attempts ?? 0),
    stories: Number(totalsRow.stories ?? 0),
    latest: {
      completed: byStatus.get('completed') ?? 0,
      failed: byStatus.get('failed') ?? 0,
      interrupted: byStatus.get('interrupted') ?? 0,
    },
    worstOffender:
      worstRow && worstRow.story_id
        ? { storyId: String(worstRow.story_id), attempts: Number(worstRow.attempts ?? 0) }
        : null,
    asOf: totalsRow.as_of == null ? null : String(totalsRow.as_of),
  }
}

/**
 * THE ENGINE QUEUED BAND, from the work items that are genuinely still open.
 *
 * "Handed to Forge - queued, not started" has a durable meaning: an `agent_work_item` that is not in
 * a terminal state. There is no terminal-but-parked value here on purpose - Done, Error and Cancelled
 * are the terminal ones, so anything else is real waiting work. Empty is a legitimate and common
 * answer (the engine is often idle), and it is far better than a fixture card standing in for a queue.
 */
export type EngineQueuedCard = {
  storyId: string
  title: string
  state: string
  since: string | null
}

export async function listEngineQueuedCards(
  limit = 20,
  execute?: QueryExecutor,
): Promise<EngineQueuedCard[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select w.story_id,
           coalesce(s.title, w.story_id) as title,
           w.state,
           w.updated_at::text as since
    from agent_work_item w
    left join storyboard_story s on s.id = w.story_id
    where w.story_id is not null
      and w.state not in ('Done', 'Error', 'Cancelled')
    order by w.updated_at desc
    limit ${limit}
  `
  return rows.map((row) => ({
    storyId: String(row.story_id),
    title: String(row.title ?? row.story_id),
    state: String(row.state ?? ''),
    since: row.since == null ? null : String(row.since),
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
