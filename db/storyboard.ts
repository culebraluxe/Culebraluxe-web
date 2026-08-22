import { PortalWriteError } from '../lib/portal-write-error'
import type {
  StoryPriority,
  StoryStatus,
  Workstream,
} from '../lib/storyboard-data'
import type { QueryExecutor, QueryRow } from './query-executor'

// ---------------------------------------------------------------------------
// Storyboard story repository (migration 021).
//
// /portal/storyboard reads and edits these rows. Story IDs are human-assigned
// and are the primary key; nothing here auto-generates them. The default
// executor is resolved lazily (mirroring db/tx.ts) so importing this module
// never requires a DATABASE_URL; tests inject an in-memory fake.
// ---------------------------------------------------------------------------

export type StoryboardStory = {
  id: string
  workstream: Workstream
  title: string
  priority: StoryPriority
  status: StoryStatus
  notes: string
  batch: number | null
  goal: string | null
  scope: string | null
  dependencies: string | null
  preconditions: string | null
  architectBrief: string | null
  contextRefs: string | null
  acceptanceCriteria: string | null
  postconditions: string | null
  architectBriefUpdatedAt: string | null
  completion: number
  rollup: boolean
  plannedStartAt: string | null
  actualStartAt: string | null
  completedAt: string | null
  createdAt: string
  updatedAt: string
}

export type StoryboardStoryInput = {
  id: string
  workstream: string
  title: string
  priority: string
  status: string
  notes: string
  batch: number | null
  goal: string | null
  scope: string | null
  dependencies: string | null
  preconditions: string | null
  architectBrief: string | null
  contextRefs: string | null
  acceptanceCriteria: string | null
  postconditions: string | null
  completion: number
  rollup: boolean
  plannedStartAt: string | null
  actualStartAt: string | null
  completedAt: string | null
}

export type StoryboardStoryUpdate = Omit<StoryboardStoryInput, 'id'>

export type StoryRow = QueryRow & {
  id: string
  workstream: string
  title: string
  priority: string
  status: string
  notes: string
  batch: number | null
  goal: string | null
  scope: string | null
  dependencies: string | null
  preconditions: string | null
  architect_brief: string | null
  context_refs: string | null
  acceptance_criteria: string | null
  postconditions: string | null
  architect_brief_updated_at: string | null
  completion: number
  rollup: boolean
  planned_start_at: string | null
  actual_start_at: string | null
  completed_at: string | null
  created_at: string
  updated_at: string
}

// NOTE: column lists are written literally in every query. The Neon driver
// parameterizes interpolated string values (a `select ${cols}` would become
// `select $1` and return a `?column?` row), so a shared string constant can
// never be interpolated into these templates.

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

function dateOrNull(value: unknown): string | null {
  if (value === null || value === undefined) return null
  if (value instanceof Date) return value.toISOString()
  return String(value)
}

export function mapStory(row: StoryRow): StoryboardStory {
  return {
    id: row.id,
    workstream: row.workstream as Workstream,
    title: row.title,
    priority: row.priority as StoryPriority,
    status: row.status as StoryStatus,
    notes: row.notes,
    batch: row.batch,
    goal: row.goal,
    scope: row.scope,
    dependencies: row.dependencies,
    preconditions: row.preconditions,
    architectBrief: row.architect_brief,
    contextRefs: row.context_refs,
    acceptanceCriteria: row.acceptance_criteria,
    postconditions: row.postconditions,
    architectBriefUpdatedAt: dateOrNull(row.architect_brief_updated_at),
    completion: row.completion,
    rollup: row.rollup,
    plannedStartAt: dateOrNull(row.planned_start_at),
    actualStartAt: dateOrNull(row.actual_start_at),
    completedAt: dateOrNull(row.completed_at),
    createdAt: dateOrNull(row.created_at) ?? '',
    updatedAt: dateOrNull(row.updated_at) ?? '',
  }
}

export async function isStoryboardTableReady(
  execute?: QueryExecutor,
): Promise<boolean> {
  const q = execute ?? (await executor())
  const rows = await q`
    select to_regclass('storyboard_story') is not null as ready
  `
  return rows[0]?.ready === true
}

export async function listStoryboardStories(
  execute?: QueryExecutor,
): Promise<StoryboardStory[] | null> {
  const q = execute ?? (await executor())
  const ready = await isStoryboardTableReady(q)
  if (!ready) return null

  const rows = await q`
    select id, workstream, title, priority, status, notes, batch, goal, scope,
      dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, architect_brief_updated_at,
      completion, rollup, planned_start_at, actual_start_at, completed_at,
      created_at, updated_at
    from storyboard_story
    order by workstream, id
  `
  return rows.map((row) => mapStory(row as StoryRow))
}

export async function getStoryboardStory(
  storyId: string,
  execute?: QueryExecutor,
): Promise<StoryboardStory | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, workstream, title, priority, status, notes, batch, goal, scope,
      dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, architect_brief_updated_at,
      completion, rollup, planned_start_at, actual_start_at, completed_at,
      created_at, updated_at
    from storyboard_story
    where id = ${storyId}
  `
  const row = rows[0] as StoryRow | undefined
  return row ? mapStory(row) : null
}

export async function createStoryboardStory(
  input: StoryboardStoryInput,
  execute?: QueryExecutor,
): Promise<StoryboardStory> {
  const q = execute ?? (await executor())
  const rows = await q`
    insert into storyboard_story (
      id, workstream, title, priority, status, notes, batch, goal, scope,
      dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, architect_brief_updated_at,
      completion, rollup, planned_start_at, actual_start_at, completed_at
    ) values (
      ${input.id}, ${input.workstream}, ${input.title}, ${input.priority},
      ${input.status}, ${input.notes}, ${input.batch ?? null},
      ${input.goal ?? null}, ${input.scope ?? null},
      ${input.dependencies ?? null}, ${input.preconditions ?? null},
      ${input.architectBrief ?? null}, ${input.contextRefs ?? null},
      ${input.acceptanceCriteria ?? null}, ${input.postconditions ?? null},
      case when ${input.architectBrief ?? null}::text is null then null else now() end,
      ${input.completion}, ${input.rollup},
      ${input.plannedStartAt ?? null}, ${input.actualStartAt ?? null},
      ${input.completedAt ?? null}
    )
    on conflict (id) do nothing
    returning id, workstream, title, priority, status, notes, batch, goal,
      scope, dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, architect_brief_updated_at,
      completion, rollup, planned_start_at, actual_start_at, completed_at,
      created_at, updated_at
  `
  const row = rows[0] as StoryRow | undefined
  if (!row) {
    throw new PortalWriteError(
      'conflict',
      `Story "${input.id}" already exists. Choose a different ID.`,
    )
  }
  return mapStory(row)
}

export async function updateStoryboardStory(
  id: string,
  input: StoryboardStoryUpdate,
  execute?: QueryExecutor,
): Promise<StoryboardStory> {
  const q = execute ?? (await executor())
  const rows = await q`
    update storyboard_story
    set workstream = ${input.workstream},
        title = ${input.title},
        priority = ${input.priority},
        status = ${input.status},
        notes = ${input.notes},
        batch = ${input.batch ?? null},
        goal = ${input.goal ?? null},
        scope = ${input.scope ?? null},
        dependencies = ${input.dependencies ?? null},
        preconditions = ${input.preconditions ?? null},
        architect_brief = ${input.architectBrief ?? null},
        context_refs = ${input.contextRefs ?? null},
        acceptance_criteria = ${input.acceptanceCriteria ?? null},
        postconditions = ${input.postconditions ?? null},
        architect_brief_updated_at = case
          when architect_brief is distinct from ${input.architectBrief ?? null}::text
          then now() else architect_brief_updated_at end,
        completion = ${input.completion},
        rollup = ${input.rollup},
        planned_start_at = ${input.plannedStartAt ?? null},
        updated_at = now()
    where id = ${id}
    returning id, workstream, title, priority, status, notes, batch, goal,
      scope, dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, architect_brief_updated_at,
      completion, rollup, planned_start_at, actual_start_at, completed_at,
      created_at, updated_at
  `
  const row = rows[0] as StoryRow | undefined
  if (!row) {
    throw new PortalWriteError('not-found', `Story "${id}" was not found.`)
  }
  return mapStory(row)
}

/**
 * System-owned dates:
 *  - actual_start_at is set by startStoryRun (COALESCE — the first start is
 *    preserved); the normal human edit path never touches it.
 *  - completed_at is set by finishStoryRun on a Complete result.
 * Human editing updates planned_start_at only (see the Story Execution
 * Contract / Story Board spec). Never wipe execution dates with a stale form.
 */

export async function setStoryboardStatus(
  id: string,
  status: string,
  execute?: QueryExecutor,
): Promise<StoryboardStory> {
  const q = execute ?? (await executor())
  const rows = await q`
    update storyboard_story
    set status = ${status},
        completion = case when ${status} = 'Complete' then 100 else completion end,
        updated_at = now()
    where id = ${id}
    returning id, workstream, title, priority, status, notes, batch, goal,
      scope, dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, architect_brief_updated_at,
      completion, rollup, planned_start_at, actual_start_at, completed_at,
      created_at, updated_at
  `
  const row = rows[0] as StoryRow | undefined
  if (!row) {
    throw new PortalWriteError('not-found', `Story "${id}" was not found.`)
  }
  return mapStory(row)
}

// ---------------------------------------------------------------------------
// Execution runs (storyboard_story_run)
// ---------------------------------------------------------------------------

export type StoryRun = {
  id: string
  storyId: string
  startedAt: string
  endedAt: string | null
  resultStatus: string | null
  completion: number | null
  notes: string | null
  commitHash: string | null
  testsSummary: string | null
  /** Execution target this run actually executed against (DEV|PROD|TEST|LOCAL). */
  executionEnvironment: string | null
  goalSnapshot: string | null
  preconditionsSnapshot: string | null
  architectBriefSnapshot: string | null
  contextRefsSnapshot: string | null
  acceptanceCriteriaSnapshot: string | null
  postconditionsSnapshot: string | null
  createdAt: string
  /** Last activity on the run (progress updates, terminal writes). */
  updatedAt: string
}

export type FinishRunInput = {
  resultStatus: string
  completion: number
  notes: string
  commitHash: string | null
  testsSummary: string | null
  executionEnvironment?: string | null
}

type RunRow = QueryRow & {
  id: string
  story_id: string
  started_at: string
  ended_at: string | null
  result_status: string | null
  completion: number | null
  notes: string | null
  commit_hash: string | null
  tests_summary: string | null
  execution_environment: string | null
  goal_snapshot: string | null
  preconditions_snapshot: string | null
  architect_brief_snapshot: string | null
  context_refs_snapshot: string | null
  acceptance_criteria_snapshot: string | null
  postconditions_snapshot: string | null
  created_at: string
  updated_at: string
}

function mapRun(row: RunRow): StoryRun {
  return {
    id: row.id,
    storyId: row.story_id,
    startedAt: dateOrNull(row.started_at) ?? '',
    endedAt: dateOrNull(row.ended_at),
    resultStatus: row.result_status,
    completion: row.completion,
    notes: row.notes,
    commitHash: row.commit_hash,
    testsSummary: row.tests_summary,
    executionEnvironment: row.execution_environment ?? null,
    goalSnapshot: row.goal_snapshot,
    preconditionsSnapshot: row.preconditions_snapshot,
    architectBriefSnapshot: row.architect_brief_snapshot,
    contextRefsSnapshot: row.context_refs_snapshot,
    acceptanceCriteriaSnapshot: row.acceptance_criteria_snapshot,
    postconditionsSnapshot: row.postconditions_snapshot,
    createdAt: dateOrNull(row.created_at) ?? '',
    updatedAt: dateOrNull(row.updated_at) ?? '',
  }
}


export async function isRunTableReady(
  execute?: QueryExecutor,
): Promise<boolean> {
  const q = execute ?? (await executor())
  const rows = await q`
    select to_regclass('storyboard_story_run') is not null as ready
  `
  return rows[0]?.ready === true
}

export async function listStoryboardRuns(
  execute?: QueryExecutor,
): Promise<StoryRun[] | null> {
  const q = execute ?? (await executor())
  const ready = await isRunTableReady(q)
  if (!ready) return null

  const rows = await q`
    select id, story_id, started_at, ended_at, result_status, completion,
      notes, commit_hash, tests_summary, execution_environment,
      goal_snapshot, preconditions_snapshot, architect_brief_snapshot,
      context_refs_snapshot, acceptance_criteria_snapshot,
      postconditions_snapshot, created_at,
      updated_at
    from storyboard_story_run
    order by started_at desc, id
  `
  return rows.map((row) => mapRun(row as RunRow))
}

export async function listStoryRuns(
  storyId: string,
  execute?: QueryExecutor,
): Promise<StoryRun[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, story_id, started_at, ended_at, result_status, completion,
      notes, commit_hash, tests_summary, execution_environment,
      goal_snapshot, preconditions_snapshot, architect_brief_snapshot,
      context_refs_snapshot, acceptance_criteria_snapshot,
      postconditions_snapshot, created_at,
      updated_at
    from storyboard_story_run
    where story_id = ${storyId}
    order by started_at desc, id
  `
  return rows.map((row) => mapRun(row as RunRow))
}

/**
 * Start an execution run for a story:
 *   - story status → In Progress
 *   - actual_start_at = COALESCE(actual_start_at, now()) — the first start is
 *     preserved across retries unless a human edits it
 *   - creates a storyboard_story_run row with started_at and an immutable
 *     snapshot of the story execution specification
 * Human story fields (title, workstream, priority, notes) are never touched.
 */
export async function startStoryRun(
  storyId: string,
  execute?: QueryExecutor,
  opts?: { executionEnvironment?: string | null },
): Promise<{ run: StoryRun; story: StoryboardStory }> {
  const q = execute ?? (await executor())
  const storyRows = await q`
    update storyboard_story
    set status = 'In Progress',
        actual_start_at = coalesce(actual_start_at, now()),
        updated_at = now()
    where id = ${storyId}
    returning id, workstream, title, priority, status, notes, batch, goal,
      scope, dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, architect_brief_updated_at,
      completion, rollup, planned_start_at, actual_start_at, completed_at,
      created_at, updated_at
  `
  const storyRow = storyRows[0] as StoryRow | undefined
  if (!storyRow) {
    throw new PortalWriteError('not-found', `Story "${storyId}" was not found.`)
  }

  const runRows = await q`
    insert into storyboard_story_run (
      story_id, started_at, execution_environment,
      goal_snapshot, preconditions_snapshot, architect_brief_snapshot,
      context_refs_snapshot, acceptance_criteria_snapshot,
      postconditions_snapshot
    ) values (
      ${storyId}, now(), ${opts?.executionEnvironment ?? null},
      ${storyRow.goal ?? null}, ${storyRow.preconditions ?? null},
      ${storyRow.architect_brief ?? null}, ${storyRow.context_refs ?? null},
      ${storyRow.acceptance_criteria ?? null}, ${storyRow.postconditions ?? null}
    )
    returning id, story_id, started_at, ended_at, result_status, completion,
      notes, commit_hash, tests_summary, execution_environment,
      goal_snapshot, preconditions_snapshot, architect_brief_snapshot,
      context_refs_snapshot, acceptance_criteria_snapshot,
      postconditions_snapshot, created_at,
      updated_at
  `
  const runRow = runRows[0] as RunRow
  return { run: mapRun(runRow), story: mapStory(storyRow) }
}

/**
 * Finish an execution run:
 *   - sets the run's ended_at, result_status, completion, notes, and optional
 *     commit_hash / tests_summary; run notes are APPENDED to any live progress
 *     narrative already persisted during the run (never destroys it)
 *   - updates the parent story: status = result_status (a 'Cancelled' run maps
 *     to the story status 'Hold'), completion = run completion; a Complete
 *     result forces completion = 100 and sets completed_at = now(); anything
 *     else leaves completed_at null
 * Human story notes are never overwritten.
 */
export async function finishStoryRun(
  runId: string,
  input: FinishRunInput,
  execute?: QueryExecutor,
): Promise<{ run: StoryRun; story: StoryboardStory }> {
  const q = execute ?? (await executor())
  const runRows = await q`
    update storyboard_story_run
    set ended_at = now(),
        result_status = ${input.resultStatus},
        completion = ${input.completion},
        notes = case
          when ${input.notes}::text is null or ${input.notes}::text = '' then notes
          when notes is null or notes = '' then ${input.notes}
          else notes || E'\\n' || ${input.notes}
        end,
        commit_hash = ${input.commitHash ?? null},
        tests_summary = ${input.testsSummary ?? null},
        execution_environment = coalesce(${input.executionEnvironment ?? null}, execution_environment),
        updated_at = now()
    where id = ${runId}
    returning id, story_id, started_at, ended_at, result_status, completion,
      notes, commit_hash, tests_summary, execution_environment,
      goal_snapshot, preconditions_snapshot, architect_brief_snapshot,
      context_refs_snapshot, acceptance_criteria_snapshot,
      postconditions_snapshot, created_at,
      updated_at
  `
  const runRow = runRows[0] as RunRow | undefined
  if (!runRow) {
    throw new PortalWriteError('not-found', `Run "${runId}" was not found.`)
  }
  const run = mapRun(runRow)

  const completion =
    input.resultStatus === 'Complete' ? 100 : input.completion
  const storyStatus =
    input.resultStatus === 'Cancelled' ? 'Hold' : input.resultStatus
  const storyRows = await q`
    update storyboard_story
    set status = ${storyStatus},
        completion = ${completion},
        completed_at = case when ${input.resultStatus} = 'Complete'
          then now() else null end,
        updated_at = now()
    where id = ${run.storyId}
    returning id, workstream, title, priority, status, notes, batch, goal,
      scope, dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, architect_brief_updated_at,
      completion, rollup, planned_start_at, actual_start_at, completed_at,
      created_at, updated_at
  `
  const storyRow = storyRows[0] as StoryRow | undefined
  if (!storyRow) {
    throw new PortalWriteError(
      'not-found',
      `Story "${run.storyId}" was not found.`,
    )
  }
  return { run, story: mapStory(storyRow) }
}


/**
 * Live progress on an ACTIVE run (migration 026 telemetry):
 *   - completion: sets the run's 0-100 completion when provided
 *   - note: APPENDS a timestamped milestone to the run's execution narrative
 *     (never destroys previously accumulated notes)
 *   - testsSummary: replaces the run's current tests summary when provided
 *   - updated_at = now() (last activity)
 * Does NOT touch the parent story — the story status/completion remain the
 * terminal lifecycle's responsibility.
 */
export type StoryRunProgressInput = {
  completion?: number
  note?: string
  testsSummary?: string | null
}

export async function updateStoryRunProgress(
  runId: string,
  input: StoryRunProgressInput,
  execute?: QueryExecutor,
): Promise<StoryRun> {
  const q = execute ?? (await executor())
  const rows = await q`
    update storyboard_story_run
    set completion = case when ${input.completion ?? null}::int is null
          then completion else ${input.completion ?? null} end,
        notes = case
          when ${input.note ?? null}::text is null or ${input.note ?? null}::text = '' then notes
          when notes is null or notes = '' then to_char(now(), 'YYYY-MM-DD HH24:MI') || ' — ' || ${input.note ?? null}
          -- HEARTBEAT IS STATE, NOT HISTORY (ENG-20A): an unchanged heartbeat note
          -- (identical content regardless of the minute-level timestamp prefix) is
          -- NOT appended again. Only meaningful state changes create history.
          when regexp_replace(split_part(notes, E'\\n', -1), '^[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2} — ', '') = ${input.note ?? null}
            then notes
          else notes || E'\\n' || to_char(now(), 'YYYY-MM-DD HH24:MI') || ' — ' || ${input.note ?? null}
        end,
        tests_summary = case when ${input.testsSummary ?? null}::text is null
          then tests_summary else ${input.testsSummary ?? null} end,
        updated_at = now()
    where id = ${runId}
    returning id, story_id, started_at, ended_at, result_status, completion,
      notes, commit_hash, tests_summary, execution_environment,
      goal_snapshot, preconditions_snapshot, architect_brief_snapshot,
      context_refs_snapshot, acceptance_criteria_snapshot,
      postconditions_snapshot, created_at, updated_at
  `
  const row = rows[0] as RunRow | undefined
  if (!row) {
    throw new PortalWriteError('not-found', `Run "${runId}" was not found.`)
  }
  return mapRun(row)
}

/**
 * Terminate a run with a non-standard outcome (failure / cancellation / stale
 * recovery): ended_at + result_status + completion (preserved when omitted) +
 * a timestamped explanatory note appended to the narrative + tests summary
 * (preserved when omitted). The parent story is updated by the caller.
 */
export type TerminateRunInput = {
  resultStatus: 'Failed' | 'Cancelled'
  completion?: number
  note: string
  testsSummary?: string | null
}

export async function terminateStoryRun(
  runId: string,
  input: TerminateRunInput,
  execute?: QueryExecutor,
): Promise<StoryRun> {
  const q = execute ?? (await executor())
  const rows = await q`
    update storyboard_story_run
    set ended_at = now(),
        result_status = ${input.resultStatus},
        completion = case when ${input.completion ?? null}::int is null
          then completion else ${input.completion ?? null} end,
        notes = case
          when notes is null or notes = '' then to_char(now(), 'YYYY-MM-DD HH24:MI') || ' — ' || ${input.note}
          else notes || E'\\n' || to_char(now(), 'YYYY-MM-DD HH24:MI') || ' — ' || ${input.note}
        end,
        tests_summary = case when ${input.testsSummary ?? null}::text is null
          then tests_summary else ${input.testsSummary ?? null} end,
        updated_at = now()
    where id = ${runId}
    returning id, story_id, started_at, ended_at, result_status, completion,
      notes, commit_hash, tests_summary, execution_environment,
      goal_snapshot, preconditions_snapshot, architect_brief_snapshot,
      context_refs_snapshot, acceptance_criteria_snapshot,
      postconditions_snapshot, created_at, updated_at
  `
  const row = rows[0] as RunRow | undefined
  if (!row) {
    throw new PortalWriteError('not-found', `Run "${runId}" was not found.`)
  }
  return mapRun(row)
}
