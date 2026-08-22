import { PortalWriteError } from '../lib/portal-write-error'
import { neonTx, type TxRunner } from './tx'
import {
  finishStoryRun,
  mapStory,
  setStoryboardStatus,
  startStoryRun,
  terminateStoryRun,
  updateStoryRunProgress,
  type StoryboardStory,
  type StoryRow,
  type StoryRun,
  type StoryRunProgressInput,
} from './storyboard'
import type { QueryExecutor, QueryRow } from './query-executor'

// ---------------------------------------------------------------------------
// Agent work queue repository (migration 025).
//
// The dispatch layer between the authoritative Story Board (control plane) and
// the coding agent. Reads execution authorization FROM the production
// storyboard_story table and WRITES queue/run/result evidence back to the
// production Story Board tables. agent_work_item stores no story
// specification — the authoritative spec stays on storyboard_story and is
// snapshotted into storyboard_story_run when execution begins.
//
// Single-worker semantics are enforced database-side by migration 025:
//   - at most one active work item per story (partial unique index)
//   - at most one Claimed/Running item system-wide (partial unique index on a
//     constant), with the claim command additionally serializing concurrent
//     claims through an advisory lock so a second worker is refused cleanly
//     instead of racing into a second active execution.
// ---------------------------------------------------------------------------

export type AgentWorkState =
  | 'Ready'
  | 'Claimed'
  | 'Running'
  | 'Paused'
  | 'Done'
  | 'Error'
  | 'Cancelled'

export type AgentWorkItem = {
  id: string
  storyId: string
  state: AgentWorkState
  priority: number
  queuedAt: string
  claimedAt: string | null
  claimedBy: string | null
  startedAt: string | null
  finishedAt: string | null
  storyRunId: string | null
  errorText: string | null
  /** Logical agent role (architect | builder | reviewer | verifier | ...). */
  role: string | null
  /** LOGICAL model profile (architect-pro, builder-flash, ...) — never vendor. */
  modelProfile: string | null
  /** Optional additive special instructions for this command. */
  specialInstructions: string | null
  /** Runtime adapter selected for this attempt. */
  runtimeAdapter: string | null
  /** Opaque external runtime/session id correlation (never canonical truth). */
  externalRunId: string | null
  /** Retry accounting. */
  attempts: number
  maxAttempts: number
  /** SDLC execution policy (unattended eligibility). */
  executionPolicy: string
  /** Intended execution target for this command (DEV|PROD|TEST|LOCAL). */
  executionEnvironment: string | null
  createdAt: string
  updatedAt: string
}

export type AgentWorkClaim = {
  workItem: AgentWorkItem
  story: StoryboardStory
}

type AgentWorkRow = QueryRow & {
  id: string
  story_id: string
  state: string
  priority: number
  queued_at: string
  claimed_at: string | null
  claimed_by: string | null
  started_at: string | null
  finished_at: string | null
  story_run_id: string | null
  error_text: string | null
  role: string | null
  model_profile: string | null
  special_instructions: string | null
  runtime_adapter: string | null
  external_run_id: string | null
  attempts: number
  max_attempts: number
  execution_policy: string
  execution_environment: string | null
  created_at: string
  updated_at: string
}

// NOTE: column lists are written literally (Neon driver parameterizes
// interpolated strings — a `select ${cols}` would become `select $1`).

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

function mapWorkItem(row: AgentWorkRow): AgentWorkItem {
  return {
    id: row.id,
    storyId: row.story_id,
    state: row.state as AgentWorkState,
    priority: row.priority,
    queuedAt: row.queued_at,
    claimedAt: row.claimed_at ?? null,
    claimedBy: row.claimed_by ?? null,
    startedAt: row.started_at ?? null,
    finishedAt: row.finished_at ?? null,
    storyRunId: row.story_run_id ?? null,
    errorText: row.error_text ?? null,
    role: row.role ?? null,
    modelProfile: row.model_profile ?? null,
    specialInstructions: row.special_instructions ?? null,
    runtimeAdapter: row.runtime_adapter ?? null,
    externalRunId: row.external_run_id ?? null,
    attempts: row.attempts ?? 0,
    maxAttempts: row.max_attempts ?? 3,
    executionPolicy: row.execution_policy ?? 'Unattended OK',
    executionEnvironment: row.execution_environment ?? null,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  }
}

export async function isAgentWorkTableReady(
  execute?: QueryExecutor,
): Promise<boolean> {
  const q = execute ?? (await executor())
  const rows = await q`
    select to_regclass('agent_work_item') is not null as ready
  `
  return rows[0]?.ready === true
}

export async function listAgentWorkItems(
  execute?: QueryExecutor,
): Promise<AgentWorkItem[] | null> {
  const q = execute ?? (await executor())
  const ready = await isAgentWorkTableReady(q)
  if (!ready) return null

  const rows = await q`
    select id, story_id, state, priority, queued_at, claimed_at, claimed_by,
      started_at, finished_at, story_run_id, error_text, role, model_profile, special_instructions, runtime_adapter, external_run_id, attempts, max_attempts, execution_policy, execution_environment, created_at, updated_at
    from agent_work_item
    order by queued_at desc, id
  `
  return rows.map((row) => mapWorkItem(row as AgentWorkRow))
}

export async function listAgentWorkForStory(
  storyId: string,
  execute?: QueryExecutor,
): Promise<AgentWorkItem[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, story_id, state, priority, queued_at, claimed_at, claimed_by,
      started_at, finished_at, story_run_id, error_text, role, model_profile, special_instructions, runtime_adapter, external_run_id, attempts, max_attempts, execution_policy, execution_environment, created_at, updated_at
    from agent_work_item
    where story_id = ${storyId}
    order by queued_at desc, id
  `
  return rows.map((row) => mapWorkItem(row as AgentWorkRow))
}

/** Active work for a story (Ready / Claimed / Running), newest first. */
export async function listActiveAgentWorkForStory(
  storyId: string,
  execute?: QueryExecutor,
): Promise<AgentWorkItem[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, story_id, state, priority, queued_at, claimed_at, claimed_by,
      started_at, finished_at, story_run_id, error_text, role, model_profile, special_instructions, runtime_adapter, external_run_id, attempts, max_attempts, execution_policy, execution_environment, created_at, updated_at
    from agent_work_item
    where story_id = ${storyId}
      and state in ('Ready', 'Claimed', 'Running')
    order by queued_at desc, id
  `
  return rows.map((row) => mapWorkItem(row as AgentWorkRow))
}

export async function getAgentWorkItem(
  workItemId: string,
  execute?: QueryExecutor,
): Promise<AgentWorkItem | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, story_id, state, priority, queued_at, claimed_at, claimed_by,
      started_at, finished_at, story_run_id, error_text, role, model_profile, special_instructions, runtime_adapter, external_run_id, attempts, max_attempts, execution_policy, execution_environment, created_at, updated_at
    from agent_work_item
    where id = ${workItemId}
  `
  const row = rows[0] as AgentWorkRow | undefined
  return row ? mapWorkItem(row) : null
}

/** The single system-wide active work item (Claimed or Running), if any. */
export async function getActiveAgentWorkItem(
  execute?: QueryExecutor,
): Promise<AgentWorkItem | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, story_id, state, priority, queued_at, claimed_at, claimed_by,
      started_at, finished_at, story_run_id, error_text, role, model_profile, special_instructions, runtime_adapter, external_run_id, attempts, max_attempts, execution_policy, execution_environment, created_at, updated_at
    from agent_work_item
    where state in ('Claimed', 'Running')
    limit 1
  `
  const row = rows[0] as AgentWorkRow | undefined
  return row ? mapWorkItem(row) : null
}

/**
 * Stale Claimed/Running work items — active items whose heartbeat
 * (updated_at) has been silent for at least `staleAfterMinutes`. A stale item
 * means the worker is presumed dead; it still BLOCKS the queue until
 * `recoverStaleAgentWork` marks it terminal.
 */
export async function listStaleAgentWork(
  staleAfterMinutes = 60,
  execute?: QueryExecutor,
): Promise<AgentWorkItem[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, story_id, state, priority, queued_at, claimed_at, claimed_by,
      started_at, finished_at, story_run_id, error_text, role, model_profile, special_instructions, runtime_adapter, external_run_id, attempts, max_attempts, execution_policy, execution_environment, created_at, updated_at
    from agent_work_item
    where state in ('Claimed', 'Running')
      and updated_at < now() - (${staleAfterMinutes} || ' minutes')::interval
    order by updated_at asc
  `
  return rows.map((row) => mapWorkItem(row as AgentWorkRow))
}

/**
 * Atomically claim a SPECIFIC Ready work item (single-worker semantics kept:
 * refuses when any Claimed/Running/Paused item exists system-wide). Used by
 * the invoker's deterministic claim of a chosen command and by contract
 * fixtures; the normal poller path uses claimNextAgentWork instead.
 */
export async function claimSpecificAgentWork(
  workItemId: string,
  workerId: string,
  runner: TxRunner = neonTx,
): Promise<AgentWorkItem | null> {
  return runner(async (tx) => {
    await tx`select pg_advisory_xact_lock(cast(9000212 as bigint))`

    const activeRows = await tx`
      select id from agent_work_item
      where state in ('Claimed', 'Running', 'Paused')
      limit 1
    `
    if (activeRows.length > 0) return null

    const claimedRows = await tx`
      update agent_work_item
      set state = 'Claimed',
          claimed_at = now(),
          claimed_by = ${workerId},
          updated_at = now()
      where id = ${workItemId}
        and state = 'Ready'
      returning id, story_id, state, priority, queued_at, claimed_at,
        claimed_by, started_at, finished_at, story_run_id, error_text,
        role, model_profile, special_instructions, runtime_adapter,
        external_run_id, attempts, max_attempts, created_at, updated_at
    `
    const row = claimedRows[0] as AgentWorkRow | undefined
    return row ? mapWorkItem(row) : null
  })
}

/**
 * Pause a Running work item (preserves assignment/context; state -> Paused).
 * The runtime adapter may also record a runtime-level pause; the durable row
 * reflects the pause so a second worker cannot claim while paused (the
 * single-active unique index includes Paused). Paused items are still subject
 * to stale-heartbeat recovery.
 */
export async function pauseAgentWork(
  workItemId: string,
  execute?: QueryExecutor,
): Promise<AgentWorkItem> {
  const q = execute ?? (await executor())
  const rows = await q`
    update agent_work_item
    set state = 'Paused',
        updated_at = now()
    where id = ${workItemId}
      and state = 'Running'
    returning id, story_id, state, priority, queued_at, claimed_at,
      claimed_by, started_at, finished_at, story_run_id, error_text,
      role, model_profile, special_instructions, runtime_adapter,
      external_run_id, attempts, max_attempts, execution_policy, execution_environment, created_at, updated_at
  `
  const row = rows[0] as AgentWorkRow | undefined
  if (!row) {
    throw new PortalWriteError(
      'conflict',
      `Work item "${workItemId}" is not Running and cannot be paused.`,
    )
  }
  return mapWorkItem(row)
}

/**
 * Resume a Paused work item back to Running (same logical attempt).
 */
export async function resumeAgentWork(
  workItemId: string,
  execute?: QueryExecutor,
): Promise<AgentWorkItem> {
  const q = execute ?? (await executor())
  const rows = await q`
    update agent_work_item
    set state = 'Running',
        updated_at = now()
    where id = ${workItemId}
      and state = 'Paused'
    returning id, story_id, state, priority, queued_at, claimed_at,
      claimed_by, started_at, finished_at, story_run_id, error_text,
      role, model_profile, special_instructions, runtime_adapter,
      external_run_id, attempts, max_attempts, execution_policy, execution_environment, created_at, updated_at
  `
  const row = rows[0] as AgentWorkRow | undefined
  if (!row) {
    throw new PortalWriteError(
      'conflict',
      `Work item "${workItemId}" is not Paused and cannot be resumed.`,
    )
  }
  return mapWorkItem(row)
}

/**
 * Set the runtime adapter identity + external run correlation on a work item
 * (migration 028 command envelope). Used by the invoker/adapter at execution
 * time so the durable row answers "which adapter/provider/model executed this
 * command and which external session correlates". External ids are opaque.
 */
export async function setAgentWorkRuntime(
  workItemId: string,
  input: { runtimeAdapter: string; externalRunId?: string | null },
  execute?: QueryExecutor,
): Promise<AgentWorkItem> {
  const q = execute ?? (await executor())
  const rows = await q`
    update agent_work_item
    set runtime_adapter = ${input.runtimeAdapter},
        external_run_id = coalesce(${input.externalRunId ?? null}, external_run_id),
        updated_at = now()
    where id = ${workItemId}
    returning id, story_id, state, priority, queued_at, claimed_at,
      claimed_by, started_at, finished_at, story_run_id, error_text,
      role, model_profile, special_instructions, runtime_adapter,
      external_run_id, attempts, max_attempts, execution_policy, execution_environment, created_at, updated_at
  `
  const row = rows[0] as AgentWorkRow | undefined
  if (!row) {
    throw new PortalWriteError('not-found', `Work item "${workItemId}" was not found.`)
  }
  return mapWorkItem(row)
}

/**
 * Enqueue a durable Agent Work Command (migration 028 envelope) for a story.
 * Creates a Ready work item with the logical role/model profile + optional
 * special instructions. The authoritative story spec is NOT copied here — it
 * is resolved from storyboard_story at execution time. The story must already
 * be Ready (explicit authorization) or the caller enqueues directly for
 * test/dogfood fixtures; duplicate protection (one active item per story) is
 * enforced by the database.
 */
export async function enqueueAgentWorkCommand(
  input: {
    storyId: string
    role?: string | null
    modelProfile?: string | null
    specialInstructions?: string | null
    priority?: number
    maxAttempts?: number
    executionPolicy?: string
    executionEnvironment?: string | null
  },
  execute?: QueryExecutor,
): Promise<AgentWorkItem> {
  const q = execute ?? (await executor())
  // One story = one durable command. A Ready story already has a Ready work
  // item created by the DB dispatch trigger (migration 025); the console
  // "Queue command" action UPSERTS that row with the command envelope rather
  // than creating a second queue row. Duplicate protection (one active per
  // story) is preserved.
  const rows = await q`
    insert into agent_work_item (
      story_id, state, priority, role, model_profile, special_instructions,
      max_attempts, execution_policy, execution_environment
    ) values (
      ${input.storyId}, 'Ready', ${input.priority ?? 0},
      ${input.role ?? null}, ${input.modelProfile ?? null},
      ${input.specialInstructions ?? null}, ${input.maxAttempts ?? 3},
      ${input.executionPolicy ?? 'Unattended OK'}, ${input.executionEnvironment ?? null}
    )
    on conflict (story_id) where state in ('Ready', 'Claimed', 'Running')
    do update set
      role = coalesce(excluded.role, agent_work_item.role),
      model_profile = coalesce(excluded.model_profile, agent_work_item.model_profile),
      special_instructions = coalesce(excluded.special_instructions, agent_work_item.special_instructions),
      max_attempts = excluded.max_attempts,
      execution_policy = excluded.execution_policy,
      execution_environment = coalesce(excluded.execution_environment, agent_work_item.execution_environment),
      updated_at = now()
    returning id, story_id, state, priority, queued_at, claimed_at,
      claimed_by, started_at, finished_at, story_run_id, error_text,
      role, model_profile, special_instructions, runtime_adapter,
      external_run_id, attempts, max_attempts, execution_policy, execution_environment, created_at, updated_at
  `
  const row = rows[0] as AgentWorkRow | undefined
  if (!row) {
    throw new PortalWriteError('conflict', `Unable to enqueue work for story "${input.storyId}".`)
  }
  return mapWorkItem(row)
}

/**
 * Persist live progress on a Running work item (migration 026 telemetry):
 *   - the work item's updated_at is refreshed (heartbeat — a Running job must
 *     not appear indistinguishable from a dead worker)
 *   - the linked storyboard_story_run gets completion / an appended
 *     timestamped milestone note / an updated tests summary (all optional)
 * Requires the work item to be Running with a begun story run.
 */
export async function updateAgentWorkProgress(
  workItemId: string,
  input: StoryRunProgressInput,
  execute?: QueryExecutor,
): Promise<{ workItem: AgentWorkItem; run: StoryRun }> {
  if (
    input.completion !== undefined &&
    (!Number.isInteger(input.completion) ||
      input.completion < 0 ||
      input.completion > 100)
  ) {
    throw new PortalWriteError(
      'validation',
      'completion must be an integer between 0 and 100.',
    )
  }

  const q = execute ?? (await executor())
  const item = await getAgentWorkItem(workItemId, q)
  if (!item) {
    throw new PortalWriteError('not-found', `Work item "${workItemId}" was not found.`)
  }
  if (!item.storyRunId) {
    throw new PortalWriteError(
      'conflict',
      `Work item "${workItemId}" has no story run; begin execution first.`,
    )
  }

  // Heartbeat: refresh updated_at only while the item is actually Running.
  const heartbeats = await q`
    update agent_work_item
    set updated_at = now()
    where id = ${workItemId}
      and state = 'Running'
    returning id, story_id, state, priority, queued_at, claimed_at,
      claimed_by, started_at, finished_at, story_run_id, error_text,
      role, model_profile, special_instructions, runtime_adapter,
      external_run_id, attempts, max_attempts, execution_policy, execution_environment, created_at, updated_at
  `
  const heartbeatRow = heartbeats[0] as AgentWorkRow | undefined
  if (!heartbeatRow) {
    throw new PortalWriteError(
      'conflict',
      `Work item "${workItemId}" is not Running; progress requires an active run.`,
    )
  }

  const run = await updateStoryRunProgress(item.storyRunId, input, q)
  return { workItem: mapWorkItem(heartbeatRow), run }
}

/**
 * Atomically claim the next Ready work item for a single worker.
 *
 * Transactional behavior (via the injected runner; defaults to the Neon
 * interactive transaction):
 *   1. acquire a system-wide advisory lock so concurrent claims serialize
 *   2. refuse cleanly if any item is already Claimed or Running
 *   3. select one Ready item — priority DESC, then queued_at ASC
 *   4. claim exactly one row: state -> Claimed, claimed_at -> now(),
 *      claimed_by -> workerId
 *   5. return the claimed work item plus the authoritative story specification
 *
 * Returns null when there is no Ready work, or when another item is already
 * active (so a second worker is never handed work concurrently).
 */
export async function claimNextAgentWork(
  workerId: string,
  runner: TxRunner = neonTx,
): Promise<AgentWorkClaim | null> {
  return runner(async (tx) => {
    // Serialize concurrent claims system-wide. Literal key: 'culebra-agent-claim'.
    await tx`select pg_advisory_xact_lock(cast(9000212 as bigint))`

    const activeRows = await tx`
      select id from agent_work_item
      where state in ('Claimed', 'Running')
      limit 1
    `
    if (activeRows.length > 0) return null

    const claimedRows = await tx`
      update agent_work_item
      set state = 'Claimed',
          claimed_at = now(),
          claimed_by = ${workerId},
          updated_at = now()
      where id = (
        select id from agent_work_item
        where state = 'Ready'
        order by priority desc, queued_at asc, id
        limit 1
      )
      returning id, story_id, state, priority, queued_at, claimed_at,
        claimed_by, started_at, finished_at, story_run_id, error_text,
        created_at, updated_at
    `
    const claimedRow = claimedRows[0] as AgentWorkRow | undefined
    if (!claimedRow) return null

    const workItem = mapWorkItem(claimedRow)
    const storyRows = await tx`
      select id, workstream, title, priority, status, notes, batch, goal, scope,
        dependencies, preconditions, architect_brief, context_refs,
        acceptance_criteria, postconditions, architect_brief_updated_at,
        completion, rollup, planned_start_at, actual_start_at, completed_at,
        created_at, updated_at
      from storyboard_story
      where id = ${workItem.storyId}
    `
    const storyRow = storyRows[0] as StoryRow | undefined
    if (!storyRow) {
      throw new PortalWriteError(
        'not-found',
        `Story "${workItem.storyId}" for work item ${workItem.id} was not found.`,
      )
    }
    return { workItem, story: mapStory(storyRow) }
  })
}

/**
 * Begin execution of a claimed work item:
 *   - work item state -> Running, started_at -> now()
 *   - runs the existing Story Board start lifecycle against the authoritative
 *     story: story -> In Progress, first actual_start_at preserved, a
 *     storyboard_story_run is created with an immutable snapshot of the
 *     execution specification, and the run id is recorded on the work item.
 */
export async function beginAgentWorkRun(
  workItemId: string,
  execute?: QueryExecutor,
): Promise<{ workItem: AgentWorkItem; story: StoryboardStory }> {
  const q = execute ?? (await executor())
  const item = await getAgentWorkItem(workItemId, q)
  if (!item) {
    throw new PortalWriteError('not-found', `Work item "${workItemId}" was not found.`)
  }
  if (item.state !== 'Claimed') {
    throw new PortalWriteError(
      'conflict',
      `Work item "${workItemId}" is ${item.state}; only a Claimed item can begin.`,
    )
  }

  const { run, story } = await startStoryRun(item.storyId, q, {
    executionEnvironment: item.executionEnvironment,
  })

  const rows = await q`
    update agent_work_item
    set state = 'Running',
        started_at = now(),
        story_run_id = ${run.id},
        updated_at = now()
    where id = ${workItemId}
    returning id, story_id, state, priority, queued_at, claimed_at,
      claimed_by, started_at, finished_at, story_run_id, error_text,
      role, model_profile, special_instructions, runtime_adapter,
      external_run_id, attempts, max_attempts, execution_policy, execution_environment, created_at, updated_at
  `
  const row = rows[0] as AgentWorkRow | undefined
  if (!row) {
    throw new PortalWriteError('not-found', `Work item "${workItemId}" was not found.`)
  }
  return { workItem: mapWorkItem(row), story }
}

export type FinishAgentWorkInput = {
  resultStatus: string
  completion: number
  notes: string
  commitHash: string | null
  testsSummary: string | null
}

/**
 * Finish execution of a Running work item:
 *   - records the storyboard_story_run result (ended_at, result_status,
 *     completion, notes, commit_hash, tests_summary) and updates the
 *     authoritative story status/completion via the existing finish lifecycle
 *   - work item -> Done, finished_at -> now()
 *
 * A work item is Done whenever the coding attempt finished normally and its
 * result was recorded — the story result may be Partial/Blocked/etc. Work item
 * Error is reserved for execution-infrastructure failure (failAgentWork).
 */
export async function finishAgentWork(
  workItemId: string,
  input: FinishAgentWorkInput,
  execute?: QueryExecutor,
): Promise<{ workItem: AgentWorkItem; run: unknown; story: StoryboardStory }> {
  const q = execute ?? (await executor())
  const item = await getAgentWorkItem(workItemId, q)
  if (!item) {
    throw new PortalWriteError('not-found', `Work item "${workItemId}" was not found.`)
  }
  if (!item.storyRunId) {
    throw new PortalWriteError(
      'conflict',
      `Work item "${workItemId}" has no story run; begin execution first.`,
    )
  }

  const { run, story } = await finishStoryRun(item.storyRunId, input, q)

  const rows = await q`
    update agent_work_item
    set state = 'Done',
        finished_at = now(),
        updated_at = now()
    where id = ${workItemId}
    returning id, story_id, state, priority, queued_at, claimed_at,
      claimed_by, started_at, finished_at, story_run_id, error_text,
      role, model_profile, special_instructions, runtime_adapter,
      external_run_id, attempts, max_attempts, execution_policy, execution_environment, created_at, updated_at
  `
  const row = rows[0] as AgentWorkRow | undefined
  if (!row) {
    throw new PortalWriteError('not-found', `Work item "${workItemId}" was not found.`)
  }
  return { workItem: mapWorkItem(row), run, story }
}

export type AgentWorkFailInput = {
  completion?: number
  note?: string
  testsSummary?: string | null
}

/**
 * Mark a work item Error (execution-infrastructure failure) and TERMINATE the
 * linked run as Failed so nothing stays Running forever:
 *   - when the work item has a begun story run: run -> ended_at now(),
 *     result_status Failed, completion preserved/given, an explanatory note
 *     appended to the narrative, tests summary preserved/given; the story ->
 *     Failed (the run did not finish cleanly; deliberate retry re-Readies it)
 *   - the work item -> Error with error_text + finished_at + updated_at
 * When the claim never began (no run), only the work item is marked Error.
 */
export async function failAgentWork(
  workItemId: string,
  errorText: string,
  input: AgentWorkFailInput = {},
  execute?: QueryExecutor,
): Promise<AgentWorkItem> {
  const q = execute ?? (await executor())
  const item = await getAgentWorkItem(workItemId, q)
  if (!item) {
    throw new PortalWriteError('not-found', `Work item "${workItemId}" was not found.`)
  }

  if (item.storyRunId) {
    await terminateStoryRun(
      item.storyRunId,
      {
        resultStatus: 'Failed',
        completion: input.completion,
        note: input.note ?? `execution failed: ${errorText}`,
        testsSummary: input.testsSummary,
      },
      q,
    )
    await setStoryboardStatus(item.storyId, 'Failed', q)
  }

  const rows = await q`
    update agent_work_item
    set state = 'Error',
        error_text = ${errorText},
        finished_at = now(),
        updated_at = now()
    where id = ${workItemId}
    returning id, story_id, state, priority, queued_at, claimed_at,
      claimed_by, started_at, finished_at, story_run_id, error_text,
      role, model_profile, special_instructions, runtime_adapter,
      external_run_id, attempts, max_attempts, execution_policy, execution_environment, created_at, updated_at
  `
  const row = rows[0] as AgentWorkRow | undefined
  if (!row) {
    throw new PortalWriteError('not-found', `Work item "${workItemId}" was not found.`)
  }
  return mapWorkItem(row)
}

export type AgentWorkCancelInput = {
  note?: string
}

/**
 * Cancel an active coding run (human/architect/operator decision):
 *   - when the work item has a begun story run: run -> ended_at now(),
 *     result_status Cancelled (migration 026), completion preserved, a
 *     cancellation note appended to the narrative; the story -> Hold
 *     (existing canonical "paused, can resume" status)
 *   - the work item -> Cancelled with finished_at + updated_at
 * User cancellation is a distinct terminal outcome — never classified as a
 * failure. When the claim never began (no run), only the work item is marked
 * Cancelled.
 */
export async function cancelAgentWork(
  workItemId: string,
  input: AgentWorkCancelInput = {},
  execute?: QueryExecutor,
): Promise<AgentWorkItem> {
  const q = execute ?? (await executor())
  const item = await getAgentWorkItem(workItemId, q)
  if (!item) {
    throw new PortalWriteError('not-found', `Work item "${workItemId}" was not found.`)
  }

  if (item.storyRunId) {
    await terminateStoryRun(
      item.storyRunId,
      {
        resultStatus: 'Cancelled',
        note: input.note ?? 'run cancelled by operator.',
      },
      q,
    )
    await setStoryboardStatus(item.storyId, 'Hold', q)
  }

  const rows = await q`
    update agent_work_item
    set state = 'Cancelled',
        finished_at = now(),
        updated_at = now()
    where id = ${workItemId}
    returning id, story_id, state, priority, queued_at, claimed_at,
      claimed_by, started_at, finished_at, story_run_id, error_text,
      role, model_profile, special_instructions, runtime_adapter,
      external_run_id, attempts, max_attempts, execution_policy, execution_environment, created_at, updated_at
  `
  const row = rows[0] as AgentWorkRow | undefined
  if (!row) {
    throw new PortalWriteError('not-found', `Work item "${workItemId}" was not found.`)
  }
  return mapWorkItem(row)
}

/**
 * Recover stale Claimed/Running work — items whose heartbeat has been silent
 * for at least `staleAfterMinutes` (worker presumed dead / host restarted).
 * Each stale item is marked EXPLICITLY terminal:
 *   - run (when begun) -> Failed with a timestamped stale-recovery note
 *   - story -> Failed (deliberate retry re-Readies the story)
 *   - work item -> Error with error_text naming the last heartbeat
 * Recovery NEVER reruns work automatically; it only unblocks the queue so a
 * fresh claim (and human deliberation) can proceed.
 */
export async function recoverStaleAgentWork(
  staleAfterMinutes = 60,
  execute?: QueryExecutor,
): Promise<AgentWorkItem[]> {
  const q = execute ?? (await executor())
  const stale = await listStaleAgentWork(staleAfterMinutes, q)

  const recovered: AgentWorkItem[] = []
  for (const item of stale) {
    if (item.storyRunId) {
      await terminateStoryRun(
        item.storyRunId,
        {
          resultStatus: 'Failed',
          note: `marked stale — no heartbeat since ${item.updatedAt}; worker presumed terminated. Deliberate retry: set the story back to Ready.`,
        },
        q,
      )
      await setStoryboardStatus(item.storyId, 'Failed', q)
    }
    const rows = await q`
      update agent_work_item
      set state = 'Error',
          error_text = ${`stale: no heartbeat since ${item.updatedAt}`},
          finished_at = now(),
          updated_at = now()
      where id = ${item.id}
        and state in ('Claimed', 'Running')
      returning id, story_id, state, priority, queued_at, claimed_at,
        claimed_by, started_at, finished_at, story_run_id, error_text,
        created_at, updated_at
    `
    const row = rows[0] as AgentWorkRow | undefined
    if (row) recovered.push(mapWorkItem(row))
  }
  return recovered
}
