import { PortalWriteError } from '@/lib/portal-write-error'
import type { QueryExecutor, QueryRow } from '@/legacy/db/query-executor'

// ---------------------------------------------------------------------------
// Sprint repository (migration 187).
//
// A SPRINT IS THE PARENT OF THE STORIES WHOSE BATCH EQUALS ITS NUMBER. The batch integer stays the
// engine-facing axis — the engine, the release scheduler and the storyboard all read
// `storyboard_story.batch` — and `storyboard_sprint` is that same integer with a goal, a status, an
// owner and an outcome attached, so a sprint can be judged by something other than a story count.
//
// The child link (`storyboard_story.sprint_id`) is DERIVED IN THE DATABASE by trigger and never
// written from here: this module writes the parent and reads the rollup. A disagreement between a
// story's sprint and its batch is refused by the trigger by name, so there is no code path here that
// could quietly reconcile one.
//
// The close rule exists TWICE, deliberately: as a CHECK constraint (so no writer can bypass it) and
// as `sprintCloseRefusal` (so the refusal is a sentence naming what is missing rather than a bare
// constraint code — measured 2026-09-17: the operator-facing client surfaces a raised message as
// `CONSTRAINT`, which tells a human nothing).
//
// The default executor is resolved lazily (mirroring db/storyboard.ts) so importing this module never
// requires a DATABASE_URL; tests inject an in-memory fake.
// ---------------------------------------------------------------------------

export type SprintStatus = 'Planned' | 'Active' | 'Closing' | 'Closed' | 'Cancelled'

export const SPRINT_STATUSES: SprintStatus[] = [
  'Planned',
  'Active',
  'Closing',
  'Closed',
  'Cancelled',
]

export type Sprint = {
  id: string
  number: number
  title: string
  theme: string | null
  goal: string | null
  status: SprintStatus
  owner: string | null
  startedAt: string | null
  targetEndAt: string | null
  closedAt: string | null
  outcome: string | null
  notes: string | null
}

export type SprintRollup = Sprint & {
  stories: number
  storiesComplete: number
  storiesOpen: number
  storiesHeld: number
  /** Null for a sprint with no stories: 0% and 0/0 are different facts. */
  percentComplete: number | null
  firstCompletedAt: string | null
  lastCompletedAt: string | null
}

/**
 * The one spelling of the batch axis. `batch 92` and sprint `S92` are the same sprint, and this is the
 * only place the pairing is expressed in TypeScript.
 */
export function sprintIdForBatch(batch: number | null | undefined): string | null {
  if (batch === null || batch === undefined) return null
  if (!Number.isFinite(batch)) return null
  return `S${batch}`
}

/** The number a sprint id carries, or null when the id is not `S<digits>`. */
export function sprintNumberFromId(id: string | null | undefined): number | null {
  const match = /^S(\d+)$/.exec((id ?? '').trim())
  if (!match) return null
  return Number(match[1])
}

/**
 * WHY A SPRINT MAY NOT CLOSE YET — or null when it may.
 *
 * Mirrors the `storyboard_sprint_closed_says_what_happened` constraint exactly, including its shape:
 * every branch is true or false and never null, because a CHECK treats NULL as satisfied and the
 * first version of that rule therefore let an empty close through.
 */
export function sprintCloseRefusal(sprint: {
  status: string
  closedAt: string | null
  outcome: string | null
}): string | null {
  const outcome = (sprint.outcome ?? '').trim()
  if (sprint.status !== 'Closed') return null
  if (!sprint.closedAt) {
    return 'a closed sprint needs closed_at: record when it closed, not only that it did'
  }
  if (outcome.length === 0) {
    return 'a closed sprint needs an outcome: say what actually happened, or it cannot be judged'
  }
  return null
}

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('@/legacy/db/client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

function dateOrNull(value: unknown): string | null {
  if (value === null || value === undefined) return null
  if (value instanceof Date) return value.toISOString()
  return String(value)
}

function numberOrNull(value: unknown): number | null {
  if (value === null || value === undefined) return null
  const n = Number(value)
  return Number.isFinite(n) ? n : null
}

function textOrNull(value: unknown): string | null {
  if (value === null || value === undefined) return null
  return String(value)
}

function mapSprint(row: QueryRow): Sprint {
  return {
    id: String(row.id),
    number: Number(row.number),
    title: String(row.title),
    theme: textOrNull(row.theme),
    goal: textOrNull(row.goal),
    status: String(row.status) as SprintStatus,
    owner: textOrNull(row.owner),
    startedAt: dateOrNull(row.started_at),
    targetEndAt: dateOrNull(row.target_end_at),
    closedAt: dateOrNull(row.closed_at),
    outcome: textOrNull(row.outcome),
    notes: textOrNull(row.notes),
  }
}

function mapRollup(row: QueryRow): SprintRollup {
  return {
    ...mapSprint(row),
    stories: Number(row.stories ?? 0),
    storiesComplete: Number(row.stories_complete ?? 0),
    storiesOpen: Number(row.stories_open ?? 0),
    storiesHeld: Number(row.stories_held ?? 0),
    percentComplete: numberOrNull(row.percent_complete),
    firstCompletedAt: dateOrNull(row.first_completed_at),
    lastCompletedAt: dateOrNull(row.last_completed_at),
  }
}

export async function isSprintTableReady(execute?: QueryExecutor): Promise<boolean> {
  const q = execute ?? (await executor())
  const rows = await q`select to_regclass('storyboard_sprint') is not null as ready`
  return rows[0]?.ready === true
}

/** Every sprint with its children counted, newest number first. Null when the table is not there yet. */
export async function listSprintRollups(execute?: QueryExecutor): Promise<SprintRollup[] | null> {
  const q = execute ?? (await executor())
  if (!(await isSprintTableReady(q))) return null
  const rows = await q`select * from storyboard_sprint_rollup order by number desc`
  return rows.map(mapRollup)
}

export async function getSprintByNumber(
  number: number,
  execute?: QueryExecutor,
): Promise<SprintRollup | null> {
  const q = execute ?? (await executor())
  const rows = await q`select * from storyboard_sprint_rollup where number = ${number}`
  return rows[0] ? mapRollup(rows[0]) : null
}

export type OpenSprintInput = {
  number: number
  title: string
  theme?: string | null
  goal?: string | null
  owner?: string | null
}

/**
 * Create the sprint a batch belongs to. REFUSES when the sprint already exists rather than
 * overwriting a goal somebody may have written, because a sprint whose goal is silently replaced is
 * a sprint nobody can be held to.
 */
export async function openSprint(
  input: OpenSprintInput,
  execute?: QueryExecutor,
): Promise<SprintRollup> {
  const q = execute ?? (await executor())
  const id = sprintIdForBatch(input.number)
  if (!id) {
    throw new PortalWriteError(
      'validation',
      `a sprint number is required (got ${String(input.number)})`,
    )
  }
  const title = input.title.trim()
  if (title.length === 0) {
    throw new PortalWriteError('validation', `sprint ${id} needs a title: name what this sprint is`)
  }
  const existing = await q`select id from storyboard_sprint where number = ${input.number}`
  if (existing.length > 0) {
    throw new PortalWriteError(
      'conflict',
      `sprint ${id} already exists; open the next number, or change its goal with the goal command`,
    )
  }
  await q`
    insert into storyboard_sprint (id, number, title, theme, goal, owner, status, started_at)
    values (${id}, ${input.number}, ${title}, ${input.theme ?? null}, ${input.goal ?? null},
            ${input.owner ?? null}, 'Active', now())
  `
  const created = await getSprintByNumber(input.number, q)
  if (!created) throw new PortalWriteError('not-found', `sprint ${id} was not created`)
  return created
}

/**
 * Set (or correct) the goal. A sprint with no goal cannot be judged at close, so writing one is a
 * real step rather than a note.
 */
export async function setSprintGoal(
  number: number,
  goal: string,
  execute?: QueryExecutor,
): Promise<SprintRollup> {
  const q = execute ?? (await executor())
  const id = sprintIdForBatch(number)
  const text = goal.trim()
  if (!id || text.length === 0) {
    throw new PortalWriteError('validation', 'a sprint goal must say something')
  }
  const rows = await q`
    update storyboard_sprint set goal = ${text}, updated_at = now()
    where number = ${number} returning id
  `
  if (rows.length === 0) throw new PortalWriteError('not-found', `sprint ${id} does not exist`)
  const updated = await getSprintByNumber(number, q)
  if (!updated) throw new PortalWriteError('not-found', `sprint ${id} could not be read back`)
  return updated
}

/**
 * Close the sprint and record what actually happened. The rule is checked HERE first so the refusal
 * is a sentence a human can act on, and again by the CHECK constraint so no other writer can skip it.
 */
export async function closeSprint(
  number: number,
  outcome: string,
  execute?: QueryExecutor,
): Promise<SprintRollup> {
  const q = execute ?? (await executor())
  const id = sprintIdForBatch(number)
  const text = outcome.trim()
  const current = await getSprintByNumber(number, q)
  if (!current) throw new PortalWriteError('not-found', `sprint ${id} does not exist`)
  const refusal = sprintCloseRefusal({
    status: 'Closed',
    closedAt: current.closedAt ?? new Date().toISOString(),
    outcome: text,
  })
  if (refusal) throw new PortalWriteError('validation', `cannot close ${id}: ${refusal}`)
  await q`
    update storyboard_sprint
    set status = 'Closed', closed_at = coalesce(closed_at, now()), outcome = ${text}, updated_at = now()
    where number = ${number}
  `
  const closed = await getSprintByNumber(number, q)
  if (!closed) throw new PortalWriteError('not-found', `sprint ${id} could not be read back`)
  return closed
}

// ---------------------------------------------------------------------------
// ACCOUNTING (migration 188) — what a sprint cost and how long its lanes ran.
//
// Every fact here already exists on `storyboard_story_run` (started_at/ended_at, tokens, cost_usd,
// cost_widgets, cost_source); the views sum them and these functions READ them. Two rules from the
// database are restated here so a human meets them at the point of reading, not only in a comment:
//
//   * USD AND WIDGETS ARE SEPARATE UNITS. Never added, never averaged, never shown as one number.
//     `cost_usd` is what the vendor invoiced; `cost_widgets` is model_weight x elapsed minutes.
//   * EVERY NUMBER NAMES WHAT IT RESTS ON. Measured 2026-09-17: 435 of 1075 runs carried vendor USD
//     and 278 carried widgets, so "$11.14" is the cost of the two thirds we have, not of the sprint.
//     `accountingCaveats` is how that reaches the reader.
// ---------------------------------------------------------------------------

export type SprintAccounting = {
  runs: number
  runsWithCost: number
  runsWithUsd: number
  runsWithWidgets: number
  costUsd: number | null
  costWidgets: number | null
  tokensInput: number | null
  tokensOutput: number | null
  runSeconds: number | null
  wallSeconds: number | null
  storiesWithCycle: number
  meanCycleSeconds: number | null
  firstRunAt: string | null
  lastRunEndAt: string | null
}

export type SprintBoard = SprintRollup & SprintAccounting

function mapAccounting(row: QueryRow): SprintAccounting {
  return {
    runs: Number(row.runs ?? 0),
    runsWithCost: Number(row.runs_with_cost ?? 0),
    runsWithUsd: Number(row.runs_with_usd ?? 0),
    runsWithWidgets: Number(row.runs_with_widgets ?? 0),
    costUsd: numberOrNull(row.cost_usd),
    costWidgets: numberOrNull(row.cost_widgets),
    tokensInput: numberOrNull(row.tokens_input),
    tokensOutput: numberOrNull(row.tokens_output),
    runSeconds: numberOrNull(row.run_seconds),
    wallSeconds: numberOrNull(row.wall_seconds),
    storiesWithCycle: Number(row.stories_with_cycle ?? 0),
    meanCycleSeconds: numberOrNull(row.mean_cycle_seconds),
    firstRunAt: dateOrNull(row.first_run_at),
    lastRunEndAt: dateOrNull(row.last_run_end_at),
  }
}

/** The sprint parent as the board reads it: story counts plus cost and time, newest first. */
export async function listSprintBoard(execute?: QueryExecutor): Promise<SprintBoard[] | null> {
  const q = execute ?? (await executor())
  if (!(await isSprintTableReady(q))) return null
  const rows = await q`select * from storyboard_sprint_board order by number desc`
  return rows.map((row) => ({ ...mapRollup(row), ...mapAccounting(row) }))
}

export async function getSprintBoardByNumber(
  number: number,
  execute?: QueryExecutor,
): Promise<SprintBoard | null> {
  const q = execute ?? (await executor())
  const rows = await q`select * from storyboard_sprint_board where number = ${number}`
  return rows[0] ? { ...mapRollup(rows[0]), ...mapAccounting(rows[0]) } : null
}

/**
 * The spending that belongs to NO sprint: stories with no batch, and the runs they consumed. A sprint
 * total that leaves this out is not wrong, it is INCOMPLETE — and this exists so the omission can be
 * stated instead of swallowed (measured 2026-09-17 on prod: 493 of 1075 runs, $3.85, 525 lane hours).
 */
export async function unassignedAccounting(
  execute?: QueryExecutor,
): Promise<SprintAccounting & { stories: number }> {
  const q = execute ?? (await executor())
  const rows = await q`
    select
      count(*) as stories,
      coalesce(sum(runs), 0) as runs,
      coalesce(sum(runs_with_cost), 0) as runs_with_cost,
      coalesce(sum(runs_with_usd), 0) as runs_with_usd,
      coalesce(sum(runs_with_widgets), 0) as runs_with_widgets,
      sum(cost_usd) as cost_usd,
      sum(cost_widgets) as cost_widgets,
      sum(tokens_input) as tokens_input,
      sum(tokens_output) as tokens_output,
      sum(run_seconds) as run_seconds,
      null::numeric as wall_seconds,
      0 as stories_with_cycle,
      null::numeric as mean_cycle_seconds,
      min(first_run_at) as first_run_at,
      max(last_run_end_at) as last_run_end_at
    from storyboard_story_accounting
    where sprint_id is null
  `
  const row = rows[0] ?? {}
  return { ...mapAccounting(row), stories: Number(row.stories ?? 0) }
}

/** Freeze the live accounting onto the sprint row (the close trigger does this; this is the deliberate re-take). */
export async function snapshotSprint(
  number: number,
  execute?: QueryExecutor,
): Promise<SprintBoard | null> {
  const q = execute ?? (await executor())
  const id = sprintIdForBatch(number)
  const existing = await q`select id from storyboard_sprint where number = ${number}`
  if (existing.length === 0) throw new PortalWriteError('not-found', `sprint ${id} does not exist`)
  await q`select storyboard_sprint_snapshot(${id ?? ''})`
  return getSprintBoardByNumber(number, q)
}

// --- Carry-over: a story that a sprint did not finish -----------------------

export type CarryOverResult = {
  storyId: string
  fromSprintId: string
  toSprintId: string
}

/**
 * MOVE A STORY TO ANOTHER SPRINT AND RECORD WHERE IT CAME FROM.
 *
 * `batch` is the engine-facing axis and the trigger derives `sprint_id` from it, so this writes BOTH:
 * changing only the batch would carry the story silently, and changing only the sprint would be
 * refused by the trigger ("change both, or neither"). The origin is recorded in
 * `carried_over_from_sprint_id`, so a story that a sprint did not finish leaves a fact behind instead
 * of a re-batching somebody has to reconstruct later.
 *
 * A story that belongs to no sprint cannot be carried, because there is nothing to carry it from: it
 * is ASSIGNED to its first sprint by an ordinary batch change instead.
 */
export async function carryOverStory(
  storyId: string,
  toSprintNumber: number,
  execute?: QueryExecutor,
): Promise<CarryOverResult> {
  const q = execute ?? (await executor())
  const toSprintId = sprintIdForBatch(toSprintNumber)
  if (!toSprintId) {
    throw new PortalWriteError('validation', `a target sprint number is required (got ${String(toSprintNumber)})`)
  }
  const rows = await q`
    select id, batch, sprint_id, carried_over_from_sprint_id
    from storyboard_story where id = ${storyId}
  `
  const story = rows[0]
  if (!story) throw new PortalWriteError('not-found', `story ${storyId} does not exist`)
  const fromSprintId = textOrNull(story.sprint_id)
  if (!fromSprintId) {
    throw new PortalWriteError(
      'validation',
      `story ${storyId} belongs to no sprint, so there is nothing to carry it from; give it a batch instead`,
    )
  }
  if (fromSprintId === toSprintId) {
    throw new PortalWriteError('conflict', `story ${storyId} is already in sprint ${toSprintId}`)
  }
  const target = await q`select id from storyboard_sprint where number = ${toSprintNumber}`
  if (target.length === 0) {
    throw new PortalWriteError(
      'not-found',
      `sprint ${toSprintId} does not exist; create it before carrying work into it`,
    )
  }
  // Both sides of the same integer move together, and the origin is recorded in the same write.
  await q`
    update storyboard_story
    set batch = ${toSprintNumber},
        sprint_id = ${toSprintId},
        carried_over_from_sprint_id = ${fromSprintId},
        updated_at = now()
    where id = ${storyId}
  `
  return { storyId, fromSprintId, toSprintId }
}

// --- Reading the numbers without lying about them ---------------------------

/** Seconds as a duration a human reads. Null means UNMEASURED, which is not the same as zero. */
export function formatDuration(seconds: number | null | undefined): string {
  if (seconds === null || seconds === undefined || !Number.isFinite(seconds)) return 'unmeasured'
  const total = Math.max(0, Math.round(seconds))
  if (total === 0) return '0m'
  const days = Math.floor(total / 86400)
  const hours = Math.floor((total % 86400) / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  const parts: string[] = []
  if (days > 0) parts.push(`${days}d`)
  if (hours > 0) parts.push(`${hours}h`)
  if (minutes > 0 && days === 0) parts.push(`${minutes}m`)
  if (parts.length === 0) return `${total}s`
  return parts.join(' ')
}

/** "59 of 83 runs (71%)" — the denominator is the point, so it is never omitted. */
export function formatCoverage(measured: number, total: number): string {
  if (total <= 0) return 'no runs'
  const percent = Math.round((100 * measured) / total)
  return `${measured} of ${total} runs (${percent}%)`
}

/** Money, or the honest absence of it. Never merged with widgets. */
export function formatUsd(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return 'unmeasured'
  return `$${value.toFixed(2)}`
}

export function formatWidgets(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return 'unmeasured'
  return `${value.toFixed(2)} widgets`
}

/**
 * A SPRINT WITH NO RUNS HAS NO COST — NOT $0.00. Zero says "this was free"; 'n/a' says "nothing was
 * recorded". The difference matters when the reader is deciding whether a number is a measurement or
 * an absence, which is the whole reason this module formats instead of the pages.
 */
export function formatSprintCost(input: { runs: number; costUsd: number | null }): string {
  return input.runs === 0 ? 'n/a' : formatUsd(input.costUsd)
}

/** The same rule for lane time: no runs means no measurement, not zero minutes. */
export function formatSprintLaneTime(input: { runs: number; runSeconds: number | null }): string {
  return input.runs === 0 ? 'n/a' : formatDuration(input.runSeconds)
}

/**
 * WHAT EACH NUMBER RESTS ON. A cost total whose coverage is unstated reads as THE cost; these are the
 * sentences that stop that, and they come back empty only when everything really was measured.
 */
export function accountingCaveats(input: {
  runs: number
  runsWithCost: number
  runsWithUsd: number
  runsWithWidgets: number
  storiesWithCycle: number
  stories: number
}): string[] {
  const caveats: string[] = []
  if (input.runs === 0) {
    caveats.push('no runs recorded for this sprint, so it has no cost or lane time to report')
    return caveats
  }
  if (input.runsWithUsd === 0) {
    caveats.push(`no vendor USD on any of the ${input.runs} runs: the dollar figure is unknown, not zero`)
  } else if (input.runsWithUsd < input.runs) {
    caveats.push(
      `USD rests on ${formatCoverage(input.runsWithUsd, input.runs)}; the vendor invoices late, so this is a floor`,
    )
  }
  if (input.runsWithWidgets === 0) {
    caveats.push('no widget-weighted consumption recorded; widgets and USD are separate units')
  }
  if (input.storiesWithCycle < input.stories) {
    caveats.push(
      `mean cycle time rests on ${input.storiesWithCycle} of ${input.stories} stories: the rest never recorded both a start and a completion`,
    )
  }
  return caveats
}
