import { PortalWriteError } from '../lib/portal-write-error'
import type {
  StoryPriority,
  StoryStatus,
  Workstream,
} from '../lib/storyboard-data'
import type { OperatingSurface } from '../lib/storyboard-data'
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
  /** UI-01: operating surface (NEXUS | OPS | TECH | SUPPORT). Null = not yet
   *  deliberately classified; never interpreted as a fake surface. */
  operatingSurface: OperatingSurface | null
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
  testMode?: string | null
  assayCommands?: string | null
  packetSha?: string | null
  createdAt: string
  updatedAt: string
}

export type StoryboardStoryInput = {
  id: string
  workstream: string
  /** UI-01: optional operating surface; omitted/null stays unclassified. */
  operatingSurface?: string | null
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
  testMode?: string | null
  assayCommands?: string | null
  packetSha?: string | null
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
  operating_surface: string | null
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
  test_mode?: string | null
  assay_commands?: string | null
  packet_sha?: string | null
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
    operatingSurface: (row.operating_surface as OperatingSurface | null) ?? null,
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
    testMode: (row.test_mode as string | null) ?? null,
    assayCommands: (row.assay_commands as string | null) ?? null,
    packetSha: (row.packet_sha as string | null) ?? null,
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
    select id, workstream, operating_surface, title, priority, status, notes,
      batch, goal, scope,
      dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, architect_brief_updated_at,
      test_mode, assay_commands, packet_sha,
      completion, rollup, planned_start_at, actual_start_at, completed_at,
      created_at, updated_at
    from storyboard_story
    order by workstream, id
  `
  return rows.map((row) => mapStory(row as StoryRow))
}

export type StoryExecutionSummary = {
  storyId: string
  workItemState: string | null
  latestRunResult: string | null
  latestRunAt: string | null
}

/**
 * Forge execution projection for the Story Board: the latest agent_work_item
 * state and the latest storyboard_story_run result/timestamp per story. Runs
 * are ordered by started_at so the board always shows the newest attempt.
 */
export async function listStoryExecutionSummaries(
  execute?: QueryExecutor,
): Promise<StoryExecutionSummary[]> {
  const q = execute ?? (await executor())
  const [workRows, runRows] = await Promise.all([
    q`
      select distinct on (story_id) story_id, state
      from agent_work_item
      where story_id is not null
      order by story_id, updated_at desc
    `,
    q`
      select distinct on (story_id) story_id, result_status, started_at
      from storyboard_story_run
      order by story_id, started_at desc
    `,
  ])

  const workState = new Map<string, string | null>()
  for (const row of workRows) {
    workState.set(String(row.story_id), (row.state as string | null) ?? null)
  }

  const run = new Map<
    string,
    { result: string | null; at: string | null }
  >()
  for (const row of runRows) {
    run.set(String(row.story_id), {
      result: (row.result_status as string | null) ?? null,
      at: (row.started_at as string | null) ?? null,
    })
  }

  const ids = new Set([...workState.keys(), ...run.keys()])
  return [...ids].map((storyId) => ({
    storyId,
    workItemState: workState.get(storyId) ?? null,
    latestRunResult: run.get(storyId)?.result ?? null,
    latestRunAt: run.get(storyId)?.at ?? null,
  }))
}

export async function getStoryboardStory(
  storyId: string,
  execute?: QueryExecutor,
): Promise<StoryboardStory | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, workstream, operating_surface, title, priority, status, notes,
      batch, goal, scope,
      dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, architect_brief_updated_at,
      test_mode, assay_commands, packet_sha,
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
      test_mode, assay_commands, packet_sha,
      completion, rollup, planned_start_at, actual_start_at, completed_at,
      operating_surface
    ) values (
      ${input.id}, ${input.workstream}, ${input.title}, ${input.priority},
      ${input.status}, ${input.notes}, ${input.batch ?? null},
      ${input.goal ?? null}, ${input.scope ?? null},
      ${input.dependencies ?? null}, ${input.preconditions ?? null},
      ${input.architectBrief ?? null}, ${input.contextRefs ?? null},
      ${input.acceptanceCriteria ?? null}, ${input.postconditions ?? null},
      case when ${input.architectBrief ?? null}::text is null then null else now() end,
      ${input.testMode ?? null}, ${input.assayCommands ?? null}, ${input.packetSha ?? null},
      ${input.completion}, ${input.rollup},
      ${input.plannedStartAt ?? null}, ${input.actualStartAt ?? null},
      ${input.completedAt ?? null},
      ${input.operatingSurface ?? null}
    )
    on conflict (id) do nothing
    returning id, workstream, operating_surface, title, priority, status, notes,
      batch, goal,
      scope, dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, architect_brief_updated_at,
      test_mode, assay_commands, packet_sha,
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
        test_mode = coalesce(${input.testMode ?? null}, test_mode),
        assay_commands = coalesce(${input.assayCommands ?? null}, assay_commands),
        packet_sha = coalesce(${input.packetSha ?? null}, packet_sha),
        architect_brief_updated_at = case
          when architect_brief is distinct from ${input.architectBrief ?? null}::text
          then now() else architect_brief_updated_at end,
        completion = ${input.completion},
        rollup = ${input.rollup},
        planned_start_at = ${input.plannedStartAt ?? null},
        operating_surface = ${input.operatingSurface ?? null},
        updated_at = now()
    where id = ${id}
    returning id, workstream, operating_surface, title, priority, status, notes,
      batch, goal,
      scope, dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, architect_brief_updated_at,
      test_mode, assay_commands, packet_sha,
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
 * ENG-FORGE-V5-03R: Persist durable executable contract facts into Neon so
 * subsequent execution lanes do not depend on reading local git packets.
 */
export async function updateStoryboardExecutableContract(
  storyId: string,
  input: {
    testMode?: string | null
    assayCommands?: string | null
    packetSha?: string | null
  },
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  await q`
    update storyboard_story
    set test_mode = coalesce(${input.testMode ?? null}, test_mode),
        assay_commands = coalesce(${input.assayCommands ?? null}, assay_commands),
        packet_sha = coalesce(${input.packetSha ?? null}, packet_sha),
        updated_at = now()
    where id = ${storyId}
  `
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
    returning id, workstream, operating_surface, title, priority, status, notes,
      batch, goal,
      scope, dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, architect_brief_updated_at,
      test_mode, assay_commands, packet_sha,
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

/**
 * PORTAL-13A — Active Work selection seam. Selecting a story for Active Work is
 * an INTENT flag recorded in storyboard_active_work only: it never changes
 * story status, never touches Forge run state, never creates an agent work item,
 * and never launches work. Adding is idempotent (ON CONFLICT DO NOTHING).
 */
export async function setActiveWork(
  storyId: string,
  active: boolean,
  selectedByAppUserId?: string | null,
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  if (active) {
    const order = await q`
      select coalesce(max(work_order), 0) + 1 as next_order from storyboard_active_work
    `
    const nextOrder = (order[0]?.next_order as number) ?? 1
    await q`
      insert into storyboard_active_work (story_id, work_order, selected_at, selected_by_app_user_id)
      values (${storyId}, ${nextOrder}, now(), ${selectedByAppUserId ?? null})
      on conflict (story_id) do nothing
    `
  } else {
    await q`
      delete from storyboard_active_work where story_id = ${storyId}
    `
  }
}

/** PORTAL-13A — deterministic active-queue reorder by explicit id order. */
export async function reorderActiveWork(
  ids: string[],
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  for (let i = 0; i < ids.length; i++) {
    await q`
      update storyboard_active_work set work_order = ${i + 1} where story_id = ${ids[i]}
    `
  }
}

/**
 * PORTAL-13A — Active Queue read model. Membership comes from
 * storyboard_active_work; all canonical story fields come from storyboard_story.
 * Ordered deterministically by work_order then story_id.
 */
export async function listActiveWork(
  execute?: QueryExecutor,
): Promise<StoryboardStory[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select s.id, s.workstream, s.operating_surface, s.title, s.priority, s.status,
      s.notes, s.batch, s.goal, s.scope, s.dependencies, s.preconditions,
      s.architect_brief, s.context_refs, s.acceptance_criteria, s.postconditions,
      s.architect_brief_updated_at, s.completion, s.rollup, s.planned_start_at,
      s.actual_start_at, s.completed_at, s.created_at, s.updated_at
    from storyboard_active_work aw
    join storyboard_story s on s.id = aw.story_id
    order by aw.work_order, aw.story_id
  `
  return rows.map((row) => mapStory(row as StoryRow))
}

export type StoryRun = {
  id: string
  storyId: string
  startedAt: string
  endedAt: string | null
  resultStatus: string | null
  /** Run classification (ARCHITECTURE | IMPLEMENTATION | VERIFICATION). */
  runType: string | null
  /** Agent / runtime identity (e.g. DeepSeek | Cline | Forge profile). */
  agentRuntime: string | null
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
  // ENG-FORGE-V6-VIS — V6 frozen contract + machine facts (migration 106).
  // Nullable: historical rows predate the columns; readers must tolerate null.
  scopeSnapshot?: string | null
  dependenciesSnapshot?: string | null
  operatingSurfaceSnapshot?: string | null
  testModeSnapshot?: string | null
  assayCommandsSnapshot?: string | null
  packetShaSnapshot?: string | null
  baseCommitHash?: string | null
  commandsTotal?: number | null
  commandsPassed?: number | null
  commandsFailed?: number | null
  testsTotal?: number | null
  testsPassed?: number | null
  testsFailed?: number | null
  policyViolationCount?: number | null
  failureCode?: string | null
  evidenceDetail?: string | null
  runPhase?: string | null
  leadDecision?: string | null
  leadSplitCount?: number | null
  // Spend vision (migration 107). Null = unmeasured/unpriced.
  modelUsed?: string | null
  tokensInput?: number | null
  tokensOutput?: number | null
  costUsd?: number | null
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
  run_type: string | null
  agent_runtime: string | null
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
  // ENG-FORGE-V6-VIS — migration 106 columns; absent on old rows / old DBs.
  scope_snapshot?: string | null
  dependencies_snapshot?: string | null
  operating_surface_snapshot?: string | null
  test_mode_snapshot?: string | null
  assay_commands_snapshot?: string | null
  packet_sha_snapshot?: string | null
  base_commit_hash?: string | null
  commands_total?: number | null
  commands_passed?: number | null
  commands_failed?: number | null
  tests_total?: number | null
  tests_passed?: number | null
  tests_failed?: number | null
  policy_violation_count?: number | null
  failure_code?: string | null
  evidence_detail?: string | null
  run_phase?: string | null
  lead_decision?: string | null
  lead_split_count?: number | null
  // Spend vision (migration 107). Absent on pre-107 rows → null.
  model_used?: string | null
  tokens_input?: number | null
  tokens_output?: number | null
  cost_usd?: number | string | null
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
    runType: row.run_type ?? null,
    agentRuntime: row.agent_runtime ?? null,
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
    scopeSnapshot: row.scope_snapshot ?? null,
    dependenciesSnapshot: row.dependencies_snapshot ?? null,
    operatingSurfaceSnapshot: row.operating_surface_snapshot ?? null,
    testModeSnapshot: row.test_mode_snapshot ?? null,
    assayCommandsSnapshot: row.assay_commands_snapshot ?? null,
    packetShaSnapshot: row.packet_sha_snapshot ?? null,
    baseCommitHash: row.base_commit_hash ?? null,
    commandsTotal: row.commands_total ?? null,
    commandsPassed: row.commands_passed ?? null,
    commandsFailed: row.commands_failed ?? null,
    testsTotal: row.tests_total ?? null,
    testsPassed: row.tests_passed ?? null,
    testsFailed: row.tests_failed ?? null,
    policyViolationCount: row.policy_violation_count ?? null,
    failureCode: row.failure_code ?? null,
    evidenceDetail: row.evidence_detail ?? null,
    runPhase: row.run_phase ?? null,
    leadDecision: row.lead_decision ?? null,
    leadSplitCount: row.lead_split_count ?? null,
    modelUsed: row.model_used ?? null,
    tokensInput: row.tokens_input ?? null,
    tokensOutput: row.tokens_output ?? null,
    costUsd: row.cost_usd === null || row.cost_usd === undefined ? null : Number(row.cost_usd),
    createdAt: dateOrNull(row.created_at) ?? '',
    updatedAt: dateOrNull(row.updated_at) ?? '',
  }
}

// ENG-FORGE-V6-VIS — V6 run columns (migration 106). Column lists stay
// literal at every query site per repo convention (the `sql` tag binds
// ${} as parameters, never SQL). Readers probe `hasRunV6Columns` then run
// one of the two literal paths; mapRun tolerates absence via `?? null`.

/**
 * Whether the connected database has the V6 run-contract columns (migration
 * 106). Probed once per process; false on older DBs so legacy columns alone
 * are selected and no query ever fails on a missing column.
 *
 * NOTE: the probe never runs speculatively inside unit-test fakes. Callers
 * use runWithV6Fallback below, which tries the literal V6 path and retries
 * the legacy path only on an undefined-column error — so fakes that never
 * implement information_schema keep working unchanged.
 */
let v6RunColumnsAvailable: boolean | null = null

export function resetRunV6ColumnCache(): void {
  v6RunColumnsAvailable = null
}

async function hasRunV6Columns(q: QueryExecutor): Promise<boolean> {
  if (v6RunColumnsAvailable !== null) return v6RunColumnsAvailable
  try {
    const rows = await q`
      select count(*)::int as count
      from information_schema.columns
      where table_name = 'storyboard_story_run'
        and column_name = 'packet_sha_snapshot'
    `
    v6RunColumnsAvailable = Number((rows[0] as { count: number })?.count ?? 0) > 0
  } catch {
    v6RunColumnsAvailable = false
  }
  return v6RunColumnsAvailable
}

function isUndefinedColumnError(error: unknown): boolean {
  const message = String((error as { message?: unknown })?.message ?? error)
  return /undefined_column|column .* does not exist|no such column/i.test(message)
}

/**
 * Run a V6 literal query, falling back to the legacy literal query when the
 * database predates migration 106. Callers MUST probe hasRunV6Columns first;
 * this helper exists for write paths that already probed and want a single
 * retry point. The fallback triggers ONLY on an undefined-column error — any
 * other failure propagates.
 */
async function runWithV6Fallback<T>(
  probedV6: boolean,
  v6Query: () => Promise<T>,
  legacyQuery: () => Promise<T>,
): Promise<T> {
  if (!probedV6) return legacyQuery()
  try {
    return await v6Query()
  } catch (error) {
    if (isUndefinedColumnError(error)) {
      v6RunColumnsAvailable = false
      return legacyQuery()
    }
    throw error
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

  // ENG-FORGE-V6-VIS: literal V6 path when migration 106 is present, legacy
  // path otherwise. No dynamic SQL — the `sql` tag binds ${} as parameters.
  const probedV6 = await hasRunV6Columns(q)
  const rows = await runWithV6Fallback(
    probedV6,
    () => q`
      select id, story_id, started_at, ended_at, result_status, run_type,
        agent_runtime, completion,
        notes, commit_hash, tests_summary, execution_environment,
        goal_snapshot, preconditions_snapshot, architect_brief_snapshot,
        context_refs_snapshot, acceptance_criteria_snapshot,
        postconditions_snapshot,
        scope_snapshot, dependencies_snapshot, operating_surface_snapshot,
        test_mode_snapshot, assay_commands_snapshot, packet_sha_snapshot,
        base_commit_hash, commands_total, commands_passed, commands_failed,
        tests_total, tests_passed, tests_failed, policy_violation_count,
        failure_code, evidence_detail, run_phase, lead_decision, lead_split_count,
        model_used, tokens_input, tokens_output, cost_usd,
        created_at,
        updated_at
      from storyboard_story_run
      order by started_at desc, id
    `,
    () => q`
    select id, story_id, started_at, ended_at, result_status, run_type,
      agent_runtime, completion,
      notes, commit_hash, tests_summary, execution_environment,
      goal_snapshot, preconditions_snapshot, architect_brief_snapshot,
      context_refs_snapshot, acceptance_criteria_snapshot,
      postconditions_snapshot, created_at,
      updated_at
    from storyboard_story_run
    order by started_at desc, id
  `,
  )
  return rows.map((row) => mapRun(row as RunRow))
}

export async function listStoryRuns(
  storyId: string,
  execute?: QueryExecutor,
): Promise<StoryRun[]> {
  const q = execute ?? (await executor())
  const probedV6 = await hasRunV6Columns(q)
  const rows = await runWithV6Fallback(
    probedV6,
    () => q`
      select id, story_id, started_at, ended_at, result_status, run_type,
        agent_runtime, completion,
        notes, commit_hash, tests_summary, execution_environment,
        goal_snapshot, preconditions_snapshot, architect_brief_snapshot,
        context_refs_snapshot, acceptance_criteria_snapshot,
        postconditions_snapshot,
        scope_snapshot, dependencies_snapshot, operating_surface_snapshot,
        test_mode_snapshot, assay_commands_snapshot, packet_sha_snapshot,
        base_commit_hash, commands_total, commands_passed, commands_failed,
        tests_total, tests_passed, tests_failed, policy_violation_count,
        failure_code, evidence_detail, run_phase, lead_decision, lead_split_count,
        model_used, tokens_input, tokens_output, cost_usd,
        created_at,
        updated_at
      from storyboard_story_run
      where story_id = ${storyId}
      order by started_at desc, id
    `,
    () => q`
    select id, story_id, started_at, ended_at, result_status, run_type,
      agent_runtime, completion,
      notes, commit_hash, tests_summary, execution_environment,
      goal_snapshot, preconditions_snapshot, architect_brief_snapshot,
      context_refs_snapshot, acceptance_criteria_snapshot,
      postconditions_snapshot, created_at,
      updated_at
    from storyboard_story_run
    where story_id = ${storyId}
    order by started_at desc, id
  `,
  )
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
  // ENG-FORGE-V6-VIS: freeze the V6 executable packet onto the new Run when
  // migration 106 is present; legacy insert otherwise. Snapshot values come
  // from the already-returned parent Story row — no extra query.
  const probedV6Start = await hasRunV6Columns(q)
  const storyRows = await q`
    update storyboard_story
    set status = 'In Progress',
        actual_start_at = coalesce(actual_start_at, now()),
        updated_at = now()
    where id = ${storyId}
    returning id, workstream, title, priority, status, notes, batch, goal,
      scope, dependencies, preconditions, architect_brief, context_refs,
      acceptance_criteria, postconditions, architect_brief_updated_at,
      test_mode, assay_commands, packet_sha,
      completion, rollup, planned_start_at, actual_start_at, completed_at,
      created_at, updated_at
  `
  const storyRow = storyRows[0] as StoryRow | undefined
  if (!storyRow) {
    throw new PortalWriteError('not-found', `Story "${storyId}" was not found.`)
  }

  const runRows = await runWithV6Fallback(
    probedV6Start,
    () => q`
    insert into storyboard_story_run (
      story_id, started_at, execution_environment,
      goal_snapshot, preconditions_snapshot, architect_brief_snapshot,
      context_refs_snapshot, acceptance_criteria_snapshot,
      postconditions_snapshot,
      scope_snapshot, dependencies_snapshot, operating_surface_snapshot,
      test_mode_snapshot, assay_commands_snapshot, packet_sha_snapshot
    ) values (
      ${storyId}, now(), ${opts?.executionEnvironment ?? null},
      ${storyRow.goal ?? null}, ${storyRow.preconditions ?? null},
      ${storyRow.architect_brief ?? null}, ${storyRow.context_refs ?? null},
      ${storyRow.acceptance_criteria ?? null}, ${storyRow.postconditions ?? null},
      ${storyRow.scope ?? null}, ${storyRow.dependencies ?? null},
      ${storyRow.operating_surface ?? null}, ${storyRow.test_mode ?? null},
      ${storyRow.assay_commands ?? null}, ${storyRow.packet_sha ?? null}
    )
    returning id, story_id, started_at, ended_at, result_status, completion,
      notes, commit_hash, tests_summary, execution_environment,
      goal_snapshot, preconditions_snapshot, architect_brief_snapshot,
      context_refs_snapshot, acceptance_criteria_snapshot,
      postconditions_snapshot,
      scope_snapshot, dependencies_snapshot, operating_surface_snapshot,
      test_mode_snapshot, assay_commands_snapshot, packet_sha_snapshot,
      base_commit_hash, commands_total, commands_passed, commands_failed,
      tests_total, tests_passed, tests_failed, policy_violation_count,
      failure_code, evidence_detail, run_phase, lead_decision, lead_split_count,
      model_used, tokens_input, tokens_output, cost_usd,
      created_at,
      updated_at
  `,
    () => q`
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
  `,
  )
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
  const probedV6Finish = await hasRunV6Columns(q)
  const runRows = await runWithV6Fallback(
    probedV6Finish,
    () => q`
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
      postconditions_snapshot,
      scope_snapshot, dependencies_snapshot, operating_surface_snapshot,
      test_mode_snapshot, assay_commands_snapshot, packet_sha_snapshot,
      base_commit_hash, commands_total, commands_passed, commands_failed,
      tests_total, tests_passed, tests_failed, policy_violation_count,
      failure_code, evidence_detail, run_phase, lead_decision, lead_split_count,
      model_used, tokens_input, tokens_output, cost_usd,
      created_at,
      updated_at
  `,
    () => q`
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
  `,
  )
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
  const probedV6Progress = await hasRunV6Columns(q)
  const rows = await runWithV6Fallback(
    probedV6Progress,
    () => q`
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
      postconditions_snapshot,
      scope_snapshot, dependencies_snapshot, operating_surface_snapshot,
      test_mode_snapshot, assay_commands_snapshot, packet_sha_snapshot,
      base_commit_hash, commands_total, commands_passed, commands_failed,
      tests_total, tests_passed, tests_failed, policy_violation_count,
      failure_code, evidence_detail, run_phase, lead_decision, lead_split_count,
      model_used, tokens_input, tokens_output, cost_usd,
      created_at, updated_at
  `,
    () => q`
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
  `,
  )
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
  const probedV6Terminate = await hasRunV6Columns(q)
  const rows = await runWithV6Fallback(
    probedV6Terminate,
    () => q`
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
      postconditions_snapshot,
      scope_snapshot, dependencies_snapshot, operating_surface_snapshot,
      test_mode_snapshot, assay_commands_snapshot, packet_sha_snapshot,
      base_commit_hash, commands_total, commands_passed, commands_failed,
      tests_total, tests_passed, tests_failed, policy_violation_count,
      failure_code, evidence_detail, run_phase, lead_decision, lead_split_count,
      model_used, tokens_input, tokens_output, cost_usd,
      created_at, updated_at
  `,
    () => q`
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
  `,
  )
  const row = rows[0] as RunRow | undefined
  if (!row) {
    throw new PortalWriteError('not-found', `Run "${runId}" was not found.`)
  }
  return mapRun(row)
}
